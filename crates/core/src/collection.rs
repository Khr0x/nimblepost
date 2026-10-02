use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    sync::LazyLock,
};

use serde_json::{Map, Value};
use tokio::io::AsyncReadExt;
use yaml_rust2::{
    Yaml, YamlLoader,
    scanner::{Scanner, Token, TokenType},
};

use crate::{Error, Result};

pub const MAX_YAML_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DocumentKind {
    Collection,
    HttpRequest,
    Folder,
    Environment,
}

/// Source document; edits preserve untouched syntax instead of serializing the projection.
#[derive(Clone)]
pub struct Document {
    pub(crate) path: PathBuf,
    source: String,
    pub(crate) value: Value,
}

static VALIDATORS: LazyLock<BTreeMap<DocumentKind, jsonschema::Validator>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/opencollection/1.0.0/opencollection.schema.json"
    ))
    .expect("checked-in schema must be valid JSON");
    [
        (DocumentKind::Collection, None),
        (DocumentKind::HttpRequest, Some("HttpRequest")),
        (DocumentKind::Folder, Some("Folder")),
        (DocumentKind::Environment, Some("Environment")),
    ]
    .into_iter()
    .map(|(kind, definition)| {
        let mut selected = schema.clone();
        if let Some(name) = definition {
            // Keep $defs for local references; validate the selected file type.
            selected.as_object_mut().unwrap().remove("properties");
            selected["$ref"] = Value::String(format!("#/$defs/{name}"));
        }
        (
            kind,
            jsonschema::validator_for(&selected).expect("checked-in schema must compile"),
        )
    })
    .collect()
});

impl Document {
    pub fn from_yaml(path: impl Into<PathBuf>, source: impl Into<String>) -> Result<Self> {
        let path = path.into();
        let source = source.into();
        if source.len() > MAX_YAML_BYTES {
            return Err(Error::config(path, "yaml", "El archivo supera 8 MiB"));
        }
        // Scan before allocating the tree: reject alias expansion and deep nesting.
        let mut scanner = Scanner::new(source.chars());
        let mut depth = 0usize;
        for Token(_, token) in scanner.by_ref() {
            match token {
                TokenType::Anchor(_)
                | TokenType::Alias(_)
                | TokenType::Tag(..)
                | TokenType::TagDirective(..) => {
                    return Err(Error::config(
                        path,
                        "yaml",
                        "Anchors, aliases y tags no están soportados",
                    ));
                }
                TokenType::VersionDirective(major, minor) if (major, minor) != (1, 2) => {
                    return Err(Error::config(path, "yaml", "Se requiere YAML 1.2"));
                }
                TokenType::BlockMappingStart
                | TokenType::BlockSequenceStart
                | TokenType::FlowMappingStart
                | TokenType::FlowSequenceStart => {
                    depth += 1;
                    if depth > 64 {
                        return Err(Error::config(path, "yaml", "La profundidad máxima es 64"));
                    }
                }
                TokenType::BlockEnd | TokenType::FlowMappingEnd | TokenType::FlowSequenceEnd => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
        }
        if let Some(error) = scanner.get_error() {
            return Err(yaml_error(&path, &error));
        }
        let mut docs =
            YamlLoader::load_from_str(&source).map_err(|error| yaml_error(&path, &error))?;
        if docs.len() != 1 {
            return Err(Error::config(
                path,
                "yaml",
                "Se requiere exactamente un documento YAML",
            ));
        }
        let value = json_value(docs.remove(0), &path)?;
        if !value.is_object() {
            return Err(Error::config(path, "yaml", "La raíz debe ser un objeto"));
        }
        Ok(Self {
            path,
            source,
            value,
        })
    }

    pub async fn read(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let resolved = tokio::fs::canonicalize(path)
            .await
            .map_err(|error| Error::io(path, error))?;
        let path = resolved.as_path();
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|error| Error::io(path, error))?;
        let mut bytes = Vec::new();
        file.take((MAX_YAML_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| Error::io(path, error))?;
        if bytes.len() > MAX_YAML_BYTES {
            return Err(Error::config(path, "yaml", "El archivo supera 8 MiB"));
        }
        let source = String::from_utf8(bytes)
            .map_err(|_| Error::config(path, "yaml", "Se requiere UTF-8"))?;
        Self::from_yaml(path, source)
    }

    pub fn original(&self) -> &str {
        &self.source
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn validate(&self, kind: DocumentKind) -> Result<()> {
        VALIDATORS[&kind].validate(&self.value).map_err(|error| {
            Error::config(
                &self.path,
                error.instance_path().to_string(),
                "Campo incompatible con el esquema OpenCollection 1.0.0",
            )
        })
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn diagnostics(&self, kind: DocumentKind) -> Vec<String> {
        VALIDATORS[&kind]
            .iter_errors(&self.value)
            .map(|error| {
                format!(
                    "{}: campo incompatible con OpenCollection",
                    error.instance_path()
                )
            })
            .collect()
    }

    pub(crate) fn validation_signatures(
        &self,
        kind: DocumentKind,
    ) -> std::collections::BTreeSet<String> {
        VALIDATORS[&kind]
            .iter_errors(&self.value)
            .map(|error| format!("{}|{:?}", error.instance_path(), error.kind()))
            .collect()
    }

    pub(crate) fn error(&self, field: &str, message: &'static str) -> Error {
        Error::config(&self.path, field, message)
    }
}

fn yaml_error(path: &Path, error: &yaml_rust2::scanner::ScanError) -> Error {
    Error::Yaml {
        path: path.into(),
        line: error.marker().line(),
        column: error.marker().col(),
    }
}

fn json_value(node: Yaml, path: &Path) -> Result<Value> {
    Ok(match node {
        Yaml::String(value) => Value::String(value),
        Yaml::Integer(value) => Value::from(value),
        Yaml::Boolean(value) => Value::Bool(value),
        Yaml::Null => Value::Null,
        Yaml::Real(value) => Value::Number(
            value
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .ok_or_else(|| Error::config(path, "yaml", "Número no representable como JSON"))?,
        ),
        Yaml::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|v| json_value(v, path))
                .collect::<Result<_>>()?,
        ),
        Yaml::Hash(values) => {
            let mut map = Map::new();
            for (key, value) in values {
                let Yaml::String(key) = key else {
                    return Err(Error::config(path, "yaml", "Las keys deben ser strings"));
                };
                if key == "<<" {
                    return Err(Error::config(
                        path,
                        "yaml",
                        "Merge keys no están soportadas",
                    ));
                }
                map.insert(key, json_value(value, path)?);
            }
            Value::Object(map)
        }
        _ => return Err(Error::config(path, "yaml", "Nodo YAML no soportado")),
    })
}

/// Collection and folder defaults are ordered from least to most specific.
pub struct LoadedRequest {
    pub request: Document,
    pub(crate) defaults: Vec<Document>,
    pub(crate) environment: Option<Document>,
}

impl LoadedRequest {
    /// Execute an unsaved request with the selected collection's defaults and environment.
    pub async fn in_collection(
        root: impl AsRef<Path>,
        request: Document,
        environment: Option<&str>,
    ) -> Result<Self> {
        request.validate(DocumentKind::HttpRequest)?;
        let root = root.as_ref();
        Ok(Self {
            request,
            defaults: vec![read_collection(root).await?],
            environment: match environment {
                Some(name) => Some(load_environment(root, name).await?),
                None => None,
            },
        })
    }

    /// A standalone request has no collection defaults or implicit environment.
    pub fn standalone(request: Document) -> Result<Self> {
        request.validate(DocumentKind::HttpRequest)?;
        Ok(Self {
            request,
            defaults: Vec::new(),
            environment: None,
        })
    }

    /// Edit only the in-memory execution target. The original YAML is unchanged.
    pub fn set_http_target(&mut self, method: String, url: String) {
        self.request.value["http"]["method"] = Value::String(method);
        self.request.value["http"]["url"] = Value::String(url);
    }
}

/// Load a request relative to an explicitly opened filesystem collection.
pub async fn load_request(
    root: impl AsRef<Path>,
    request: impl AsRef<Path>,
    environment: Option<&str>,
) -> Result<LoadedRequest> {
    let root = tokio::fs::canonicalize(root.as_ref())
        .await
        .map_err(|e| Error::io(root.as_ref(), e))?;
    let relative = request.as_ref();
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Error::config(
            relative,
            "path",
            "La petición debe ser una ruta relativa dentro de la colección",
        ));
    }
    if relative.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some(".git" | "node_modules" | "environments")
        )
    }) || matches!(
        relative.file_name().and_then(|s| s.to_str()),
        Some("opencollection.yml" | "folder.yml")
    ) {
        return Err(Error::config(
            relative,
            "path",
            "Esta ruta no es un archivo de petición",
        ));
    }
    let collection = read_collection(&root).await?;
    let mut defaults = vec![collection];
    let mut folder_path = PathBuf::new();
    if let Some(parent) = relative.parent() {
        for component in parent.components() {
            folder_path.push(component);
            let metadata = folder_path.join("folder.yml");
            match read_within(&root, &metadata).await {
                Ok(folder) => {
                    folder.validate(DocumentKind::Folder)?;
                    defaults.push(folder);
                }
                Err(Error::Io {
                    kind: std::io::ErrorKind::NotFound,
                    ..
                }) => {}
                Err(error) => return Err(error),
            }
        }
    }
    let request = read_within(&root, relative).await?;
    request.validate(DocumentKind::HttpRequest)?;
    let environment = match environment {
        Some(name) => Some(load_environment(&root, name).await?),
        None => None,
    };
    Ok(LoadedRequest {
        request,
        defaults,
        environment,
    })
}

pub(crate) async fn read_collection(root: &Path) -> Result<Document> {
    let collection = read_within(root, Path::new("opencollection.yml")).await?;
    collection.validate(DocumentKind::Collection)?;
    if collection.value["opencollection"] != "1.0.0" {
        return Err(collection.error("opencollection", "Se requiere la versión string 1.0.0"));
    }
    if collection.value["info"]["name"]
        .as_str()
        .is_none_or(str::is_empty)
    {
        return Err(collection.error("info.name", "El nombre de colección es obligatorio"));
    }
    for key in collection.value.as_object().unwrap().keys() {
        if ![
            "opencollection",
            "info",
            "config",
            "items",
            "request",
            "docs",
            "bundled",
            "extensions",
        ]
        .contains(&key.as_str())
        {
            return Err(collection.error(key, "Campo raíz no soportado para ejecución"));
        }
    }
    if collection.value["bundled"] == true
        || nonempty(&collection.value["items"])
        || nonempty(&collection.value["config"]["environments"])
    {
        return Err(collection.error(
            "bundled/items",
            "Esta API ejecuta colecciones filesystem; bundled y layouts mixtos están pendientes",
        ));
    }
    reject_active(
        &collection,
        &collection.value["config"],
        &["proxy", "clientCertificates", "protobuf"],
    )?;
    Ok(collection)
}

pub async fn read_within(root: &Path, relative: &Path) -> Result<Document> {
    let path = root.join(relative);
    let resolved = tokio::fs::canonicalize(&path)
        .await
        .map_err(|e| Error::io(&path, e))?;
    if !resolved.starts_with(root) {
        return Err(Error::config(
            path,
            "path",
            "La ruta sale de la raíz abierta",
        ));
    }
    Document::read(resolved).await
}

pub async fn load_environment(root: &Path, name: &str) -> Result<Document> {
    let dir = root.join("environments");
    let mut entries = tokio::fs::read_dir(&dir)
        .await
        .map_err(|e| Error::io(&dir, e))?;
    let mut selected = None;
    let mut names = std::collections::BTreeSet::new();
    while let Some(entry) = entries.next_entry().await.map_err(|e| Error::io(&dir, e))? {
        let path = entry.path();
        if !matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let doc = read_within(root, path.strip_prefix(root).unwrap()).await?;
        doc.validate(DocumentKind::Environment)?;
        let doc_name = doc.value["name"].as_str().unwrap();
        if doc_name.is_empty() || !names.insert(doc_name.to_owned()) {
            return Err(doc.error("name", "Nombre de environment vacío o duplicado"));
        }
        if doc_name == name {
            selected = Some(doc);
        }
    }
    selected.ok_or_else(|| Error::config(dir, "environment", "El environment solicitado no existe"))
}

pub(crate) fn nonempty(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

pub(crate) fn reject_active(doc: &Document, value: &Value, keys: &[&str]) -> Result<()> {
    for key in keys {
        let field = &value[*key];
        let active = if let Some(items) = field.as_array() {
            items.iter().any(|item| item["disabled"] != true)
        } else {
            nonempty(field)
        };
        if active {
            return Err(doc.error(key, "Configuración activa aún no soportada"));
        }
    }
    Ok(())
}
