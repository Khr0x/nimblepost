use crate::{Document, DocumentKind, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    str::FromStr,
    sync::Mutex,
};
use yaml_edit::AsYaml;
use yaml_edit::path::YamlPath;

// ponytail: one writer lock for the initial single-request app; per-file locks if contention matters.
pub(crate) static WRITER: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldEdit {
    pub path: Vec<String>,
    /// None (JSON null over IPC) removes the addressed node.
    pub value: Option<Value>,
}

impl Document {
    pub fn edited(&self, kind: DocumentKind, edits: &[FieldEdit]) -> Result<Self> {
        if edits.len() > 1024 {
            return Err(self.error("edits", "Máximo 1,024 cambios por guardado"));
        }
        let mut syntax = yaml_edit::YamlFile::from_str(self.original())
            .map_err(|_| self.error("yaml", "Esta representación YAML no admite edición segura"))?;
        if syntax.to_string() != self.original() {
            return Err(self.error("yaml", "El editor no conserva este documento byte a byte"));
        }
        let mut expected = self.value.clone();
        for edit in edits {
            let tree = syntax
                .document()
                .ok_or_else(|| self.error("yaml", "Documento ausente"))?;
            if !allowed(kind, &edit.path) {
                return Err(self.error("edits.path", "Campo no editable en esta versión"));
            }
            let mut path = String::new();
            for segment in &edit.path {
                if segment.bytes().all(|b| b.is_ascii_digit()) {
                    path.push_str(&format!("[{segment}]"));
                } else {
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(segment);
                }
            }
            let before = expected.clone();
            change_value(&mut expected, &edit.path, edit.value.as_ref())
                .map_err(|_| self.error("edits.path", "Ruta o índice inválido"))?;
            if expected == before {
                continue;
            }
            if let Some(value) = &edit.value {
                // Existing values are replaced only at their exact CST span. Preserve
                // trailing trivia: block scalars/collections own the next line separator.
                let existing = tree.try_get_path(&path).ok();
                let is_new = existing.is_none();
                let node = if let Some(node) = existing {
                    node
                } else {
                    // yaml-edit 0.3.2 misindents inserted collections. Create a scalar
                    // slot, then insert a JSON flow value (valid YAML 1.2) at its span.
                    tree.try_set_path(&path, yaml_edit::YamlValue::parse_raw("null"))
                        .map_err(|_| self.error("edits.path", "No se pudo crear el nodo"))?;
                    tree.try_get_path(&path)
                        .map_err(|_| self.error("edits.path", "Nodo insertado ausente"))?
                };
                let range = node
                    .as_node()
                    .ok_or_else(|| self.error("edits.path", "Nodo sin posición"))?
                    .text_range();
                let mut rendered = syntax.to_string();
                let start = usize::from(range.start());
                let end = usize::from(range.end());
                let old = &rendered[start..end];
                let trivia = &old[old.trim_end().len()..];
                let mut replacement = format!("{}{trivia}", value);
                // The library omits a separator after an appended block-sequence item.
                if is_new
                    && edit
                        .path
                        .last()
                        .is_some_and(|part| part.parse::<usize>().is_ok())
                    && rendered[end..].starts_with(' ')
                {
                    replacement.push('\n');
                }
                rendered.replace_range(start..end, &replacement);
                syntax = yaml_edit::YamlFile::from_str(&rendered)
                    .map_err(|_| self.error("yaml", "Edición no conservable"))?;
            } else {
                if let Some(index) = edit.path.last().and_then(|part| part.parse::<usize>().ok()) {
                    let parent = path.rsplit_once('[').unwrap().0;
                    tree.try_get_path(parent)
                        .ok()
                        .and_then(|node| node.as_sequence().cloned())
                        .and_then(|sequence| sequence.remove(index))
                        .ok_or_else(|| self.error("edits.path", "No se pudo eliminar la fila"))?;
                } else {
                    tree.try_remove_path(&path)
                        .map_err(|_| self.error("edits.path", "No se pudo eliminar el nodo"))?;
                }
            }
        }
        if expected == self.value {
            return Ok(self.clone());
        }
        let edited = Self::from_yaml(self.path.clone(), syntax.to_string())?;
        if edited.value != expected {
            return Err(self.error(
                "yaml",
                "La edición cambiaría otros valores; guardado bloqueado",
            ));
        }
        let old = self.validation_signatures(kind);
        if !edited.validation_signatures(kind).is_subset(&old) {
            return Err(self.error("edits", "El cambio introduce errores de esquema"));
        }
        Ok(edited)
    }
}

fn allowed(kind: DocumentKind, path: &[String]) -> bool {
    let fields: Vec<&str> = path.iter().map(String::as_str).collect();
    match fields.as_slice() {
        ["info", "name"] | ["http", "method" | "url" | "auth" | "body"]
            if kind == DocumentKind::HttpRequest =>
        {
            true
        }
        [
            "http",
            "auth",
            "type" | "username" | "password" | "token" | "key" | "value" | "placement",
        ] if kind == DocumentKind::HttpRequest => true,
        ["http", "body", "type" | "data"] if kind == DocumentKind::HttpRequest => true,
        ["http", "headers" | "params", rest @ ..] if kind == DocumentKind::HttpRequest => {
            row_path(rest, &["name", "value", "disabled", "type"])
        }
        ["runtime", "variables", rest @ ..] if kind == DocumentKind::HttpRequest => {
            variable_path(rest)
        }
        ["name"] if kind == DocumentKind::Environment => true,
        ["variables", rest @ ..] if kind == DocumentKind::Environment => {
            variable_path(rest)
                || matches!(rest, [index, "description"] | [index, "description", "content"] if index.parse::<usize>().is_ok())
        }
        _ => false,
    }
}
fn row_path(path: &[&str], fields: &[&str]) -> bool {
    match path {
        [] => true,
        [index] => index.parse::<usize>().is_ok(),
        [index, field] => index.parse::<usize>().is_ok() && fields.contains(field),
        _ => false,
    }
}
fn variable_path(path: &[&str]) -> bool {
    row_path(path, &["name", "value", "disabled", "secret", "type"])
        || matches!(path, [index, "value", "type" | "data"] if index.parse::<usize>().is_ok())
}

fn change_value(
    node: &mut Value,
    path: &[String],
    value: Option<&Value>,
) -> std::result::Result<(), ()> {
    let (first, tail) = path.split_first().ok_or(())?;
    if let Some(object) = node.as_object_mut() {
        if tail.is_empty() {
            if let Some(value) = value {
                object.insert(first.clone(), value.clone());
            } else {
                object.remove(first);
            }
        } else {
            let child = object.entry(first).or_insert_with(|| {
                if tail[0].parse::<usize>().is_ok() {
                    Value::Array(vec![])
                } else {
                    serde_json::json!({})
                }
            });
            change_value(child, tail, value)?;
        }
    } else if let Some(array) = node.as_array_mut() {
        let index: usize = first.parse().map_err(|_| ())?;
        if tail.is_empty() {
            if let Some(value) = value {
                if index == array.len() {
                    array.push(value.clone());
                } else {
                    *array.get_mut(index).ok_or(())? = value.clone();
                }
            } else {
                if index >= array.len() {
                    return Err(());
                }
                array.remove(index);
            }
        } else {
            change_value(array.get_mut(index).ok_or(())?, tail, value)?;
        }
    } else {
        return Err(());
    }
    Ok(())
}

fn conflict(path: &Path, temporary: Option<PathBuf>) -> Error {
    Error::Conflict {
        path: path.into(),
        temporary,
    }
}
fn check_revision(original: &Document) -> Result<fs::Metadata> {
    let path = original.path();
    let metadata = fs::symlink_metadata(path).map_err(|_| conflict(path, None))?;
    if !metadata.is_file()
        || fs::canonicalize(path).ok().as_deref() != Some(path)
        || fs::read(path).ok().as_deref() != Some(original.original().as_bytes())
    {
        return Err(conflict(path, None));
    }
    Ok(metadata)
}

/// Atomically replace one opened document, preserving permissions and checking its original bytes twice.
pub async fn save_document(original: Document, edited: Document) -> Result<Document> {
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        if original.path() != edited.path() {
            return Err(original.error("path", "El destino del documento cambió"));
        }
        let metadata = check_revision(&original)?;
        if original.original() == edited.original() {
            return Ok(original);
        }
        if metadata.permissions().readonly() {
            return Err(Error::io(
                original.path(),
                std::io::ErrorKind::PermissionDenied.into(),
            ));
        }
        let path = original.path();
        let mut temporary = tempfile::Builder::new()
            .prefix(".nimblepost-")
            .tempfile_in(path.parent().unwrap())
            .map_err(|e| Error::io(path, e))?;
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .map_err(|e| Error::io(path, e))?;
        temporary
            .write_all(edited.original().as_bytes())
            .map_err(|e| Error::io(path, e))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|e| Error::io(path, e))?;
        if check_revision(&original).is_err() {
            let (_, recovery) = temporary.keep().map_err(|e| Error::io(path, e.error))?;
            return Err(conflict(path, Some(recovery)));
        }
        temporary
            .persist(path)
            .map_err(|e| Error::io(path, e.error))?;
        #[cfg(unix)]
        fs::File::open(path.parent().unwrap())
            .and_then(|file| file.sync_all())
            .map_err(|_| {
                original.error(
                    "save",
                    "Archivo reemplazado, pero no se pudo sincronizar el directorio",
                )
            })?;
        Ok(edited)
    })
    .await
    .map_err(|_| Error::config("", "save", "No se pudo completar el guardado"))?
}

pub async fn create_environment(root: PathBuf, name: String) -> Result<Document> {
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        let root = fs::canonicalize(&root).map_err(|e| Error::io(&root, e))?;
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(Error::config(
                &root,
                "name",
                "Usa 1–64 letras ASCII, números, _ o -",
            ));
        }
        let directory = root.join("environments");
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(Error::io(&directory, e)),
        }
        let directory = fs::canonicalize(&directory).map_err(|e| Error::io(&directory, e))?;
        if !directory.starts_with(&root) {
            return Err(Error::config(
                directory,
                "path",
                "La ruta sale de la raíz abierta",
            ));
        }
        let path = directory.join(format!("{name}.yml"));
        let doc = Document::from_yaml(
            &path,
            format!(
                "name: {}\nvariables: []\n",
                serde_json::to_string(&name).unwrap()
            ),
        )?;
        doc.validate(DocumentKind::Environment)?;
        write_new_document(&doc)?;
        Ok(doc)
    })
    .await
    .map_err(|_| Error::config("", "environment", "No se pudo crear el environment"))?
}

fn write_new_document(doc: &Document) -> Result<()> {
    write_new_with_permissions(doc, None)
}

fn write_new_with_permissions(doc: &Document, permissions: Option<fs::Permissions>) -> Result<()> {
    let path = doc.path();
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(|e| Error::io(path, e))?;
    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .map_err(|e| Error::io(path, e))?;
    }
    temporary
        .write_all(doc.original().as_bytes())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|e| Error::io(path, e))?;
    temporary.persist_noclobber(path).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::config(path, "fileName", "El archivo ya existe; usa otro nombre")
        } else {
            Error::io(path, e.error)
        }
    })?;
    Ok(())
}

fn validate_new_name(root: &Path, name: &str, file_name: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
        return Err(Error::config(
            root,
            "name",
            "Usa un nombre de 1–128 caracteres sin controles",
        ));
    }
    if file_name.is_empty()
        || file_name.len() > 64
        || !file_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || ["con", "prn", "aux", "nul"].contains(&file_name.to_ascii_lowercase().as_str())
        || (file_name.len() == 4
            && ["com", "lpt"]
                .iter()
                .any(|prefix| file_name.to_ascii_lowercase().starts_with(prefix))
            && matches!(file_name.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::config(
            root,
            "fileName",
            "Usa 1–64 letras ASCII, números, _ o -; sin nombres reservados",
        ));
    }
    Ok(())
}

/// Create a filesystem collection in a new folder; never reuse an existing destination.
pub async fn create_collection(parent: PathBuf, name: String, folder: String) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        validate_new_name(&parent, &name, &folder)?;
        let parent = fs::canonicalize(&parent).map_err(|e| Error::io(&parent, e))?;
        let root = parent.join(folder);
        let doc = Document::from_yaml(
            root.join("opencollection.yml"),
            format!(
                "opencollection: \"1.0.0\"\ninfo:\n  name: {}\nbundled: false\n",
                serde_json::to_string(name.trim()).unwrap()
            ),
        )?;
        doc.validate(DocumentKind::Collection)?;
        fs::create_dir(&root).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::config(&root, "folder", "La carpeta ya existe; usa otro nombre")
            } else {
                Error::io(&root, e)
            }
        })?;
        if let Err(error) = write_new_document(&doc) {
            // Only remove our still-empty folder; preserve anything another writer added.
            let _ = fs::remove_dir(&root);
            return Err(error);
        }
        Ok(root)
    })
    .await
    .map_err(|_| Error::config("", "collection", "No se pudo crear la colección"))?
}

fn request_name(root: &Path, name: &str, file_name: &str) -> Result<()> {
    validate_new_name(root, name, file_name)?;
    if ["opencollection", "folder"].contains(&file_name.to_ascii_lowercase().as_str()) {
        return Err(Error::config(
            root,
            "fileName",
            "Este nombre está reservado para metadatos de la colección",
        ));
    }
    Ok(())
}

fn directory_within(root: &Path, folder: &str) -> Result<PathBuf> {
    let relative = Path::new(folder);
    if relative.is_absolute()
        || relative.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || component.as_os_str().to_str().is_none_or(|name| {
                    name.starts_with('.')
                        || [
                            "environments",
                            "node_modules",
                            "scripts",
                            "fixtures",
                            "auth",
                        ]
                        .contains(&name)
                })
        })
    {
        return Err(Error::config(
            relative,
            "folder",
            "Selecciona una carpeta de la colección",
        ));
    }
    let path = root.join(relative);
    let resolved = fs::canonicalize(&path).map_err(|e| Error::io(&path, e))?;
    if !resolved.starts_with(root) || resolved != path || !resolved.is_dir() {
        return Err(Error::config(
            path,
            "folder",
            "La carpeta debe estar dentro de la colección y no ser un enlace",
        ));
    }
    Ok(resolved)
}

fn validate_folder_name(root: &Path, name: &str, folder: &str) -> Result<()> {
    validate_new_name(root, name, folder)?;
    if [
        "environments",
        "node_modules",
        "scripts",
        "fixtures",
        "auth",
    ]
    .contains(&folder.to_ascii_lowercase().as_str())
    {
        return Err(Error::config(root, "folder", "Nombre de carpeta reservado"));
    }
    Ok(())
}

/// Rename a directory in place, preserving all its files and metadata.
pub async fn rename_folder(root: PathBuf, folder: String, name: String) -> Result<PathBuf> {
    let root = tokio::fs::canonicalize(&root)
        .await
        .map_err(|e| Error::io(&root, e))?;
    crate::collection::read_collection(&root).await?;
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        validate_folder_name(&root, &name, &name)?;
        let source = directory_within(&root, &folder)?;
        if source == root {
            return Err(Error::config(source, "folder", "Selecciona una subcarpeta"));
        }
        let destination = source.parent().unwrap().join(name);
        if source == destination {
            return Ok(source);
        }
        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                return Err(Error::config(
                    destination,
                    "folder",
                    "La carpeta ya existe; usa otro nombre",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::io(destination, error)),
        }
        // ponytail: path checks serialize app writes; handle-relative no-replace APIs if external writers require stronger race protection.
        fs::rename(&source, &destination).map_err(|e| Error::io(&source, e))?;
        Ok(destination)
    })
    .await
    .map_err(|_| Error::config("", "folder", "No se pudo renombrar la carpeta"))?
}

pub async fn create_folder(
    root: PathBuf,
    parent: String,
    name: String,
    folder: String,
) -> Result<PathBuf> {
    let root = tokio::fs::canonicalize(&root)
        .await
        .map_err(|e| Error::io(&root, e))?;
    crate::collection::read_collection(&root).await?;
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        validate_folder_name(&root, &name, &folder)?;
        if Path::new(&parent).components().count() >= 64 {
            return Err(Error::config(
                &root,
                "folder",
                "Nombre reservado o profundidad máxima de carpetas alcanzada",
            ));
        }
        let path = directory_within(&root, &parent)?.join(folder);
        let doc = Document::from_yaml(
            path.join("folder.yml"),
            format!(
                "info:\n  name: {}\n  type: folder\n",
                serde_json::to_string(name.trim()).unwrap()
            ),
        )?;
        doc.validate(DocumentKind::Folder)?;
        fs::create_dir(&path).map_err(|e| Error::io(&path, e))?;
        if let Err(error) = write_new_document(&doc) {
            let _ = fs::remove_dir(&path);
            return Err(error);
        }
        Ok(path)
    })
    .await
    .map_err(|_| Error::config("", "folder", "No se pudo crear la carpeta"))?
}

/// Create a request in an existing collection folder with an exclusive, validated YAML write.
pub async fn create_request(
    root: PathBuf,
    folder: String,
    name: String,
    file_name: String,
    method: String,
    url: String,
) -> Result<Document> {
    create_request_with_edits(root, folder, name, file_name, method, url, Vec::new()).await
}

/// Validate the entire draft before the exclusive first write.
pub async fn create_request_with_edits(
    root: PathBuf,
    folder: String,
    name: String,
    file_name: String,
    method: String,
    url: String,
    edits: Vec<FieldEdit>,
) -> Result<Document> {
    if reqwest::Method::from_bytes(method.as_bytes()).is_err() {
        return Err(Error::config(&root, "method", "Método HTTP inválido"));
    }
    if url.len() > 8192 || url.chars().any(char::is_control) {
        return Err(Error::config(
            &root,
            "url",
            "La URL no debe superar 8192 bytes ni contener controles",
        ));
    }
    let source = format!(
        "info:\n  name: {}\n  type: http\nhttp:\n  method: {}\n  url: {}\n",
        serde_json::to_string(name.trim()).unwrap(),
        serde_json::to_string(&method).unwrap(),
        serde_json::to_string(url.trim()).unwrap()
    );
    create_request_from_source(root, folder, name, file_name, source, edits, true).await
}

/// Save the complete configuration of an untitled or recovered request.
pub async fn create_request_from_draft(
    root: PathBuf,
    folder: String,
    name: String,
    value: Value,
) -> Result<Document> {
    let source = serde_json::to_string(&value)
        .map_err(|_| Error::config(&root, "request", "Borrador inválido"))?;
    create_request_from_source(root, folder, name, String::new(), source, vec![], false).await
}

async fn create_request_from_source(
    root: PathBuf,
    folder: String,
    name: String,
    file_name: String,
    source: String,
    mut edits: Vec<FieldEdit>,
    validate_new: bool,
) -> Result<Document> {
    let root = tokio::fs::canonicalize(&root)
        .await
        .map_err(|e| Error::io(&root, e))?;
    crate::collection::read_collection(&root).await?;
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        let file_name = if file_name.is_empty() {
            let stem = name
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-")
                .to_ascii_lowercase();
            let stem = stem.chars().take(56).collect::<String>();
            if stem.is_empty() {
                "request".into()
            } else if request_name(&root, &name, &stem).is_err() {
                format!("request-{stem}")
            } else {
                stem
            }
        } else {
            file_name
        };
        request_name(&root, &name, &file_name)?;
        let doc = Document::from_yaml(
            directory_within(&root, &folder)?.join(format!("{file_name}.yml")),
            source,
        )?;
        let method = doc.value()["http"]["method"]
            .as_str()
            .ok_or_else(|| doc.error("method", "Método HTTP inválido"))?;
        let url = doc.value()["http"]["url"]
            .as_str()
            .ok_or_else(|| doc.error("url", "URL inválida"))?;
        if doc.value()["info"]["type"] != "http" {
            return Err(doc.error("info.type", "Se requiere una petición HTTP"));
        }
        if reqwest::Method::from_bytes(method.as_bytes()).is_err() {
            return Err(Error::config(&root, "method", "Método HTTP inválido"));
        }
        if url.len() > 8192 || url.chars().any(char::is_control) {
            return Err(Error::config(
                &root,
                "url",
                "La URL no debe superar 8192 bytes ni contener controles",
            ));
        }
        edits.push(FieldEdit {
            path: vec!["info".into(), "name".into()],
            value: Some(serde_json::json!(name.trim())),
        });
        let doc = doc.edited(DocumentKind::HttpRequest, &edits)?;
        // Recovered copies retain pre-existing unsupported fields, just like opened requests.
        if validate_new {
            doc.validate(DocumentKind::HttpRequest)?;
        }
        write_new_document(&doc)?;
        Ok(doc)
    })
    .await
    .map_err(|_| Error::config("", "request", "No se pudo crear la petición"))?
}

pub async fn duplicate_request(
    root: PathBuf,
    original: Document,
    name: String,
    file_name: String,
) -> Result<Document> {
    copy_request(root, original, name, file_name, false).await
}

pub async fn rename_request(
    root: PathBuf,
    original: Document,
    name: String,
    file_name: String,
) -> Result<Document> {
    copy_request(root, original, name, file_name, true).await
}

fn request_parent(root: &Path, original: &Document) -> Result<PathBuf> {
    let relative = original
        .path()
        .strip_prefix(root)
        .map_err(|_| original.error("path", "La petición pertenece a otra colección"))?;
    if original.value()["info"]["type"] != "http"
        || !matches!(
            relative.extension().and_then(|s| s.to_str()),
            Some("yml" | "yaml")
        )
        || matches!(
            relative.file_name().and_then(|s| s.to_str()),
            Some("opencollection.yml" | "folder.yml")
        )
        || relative
            .file_name()
            .is_some_and(|s| s.to_string_lossy().starts_with('.'))
    {
        return Err(original.error("path", "Selecciona un archivo de petición HTTP"));
    }
    directory_within(
        root,
        relative.parent().and_then(|p| p.to_str()).unwrap_or(""),
    )
}

/// Delete one opened HTTP request only if its original bytes still match.
pub async fn delete_request(root: PathBuf, original: Document) -> Result<()> {
    let root = tokio::fs::canonicalize(&root)
        .await
        .map_err(|e| Error::io(&root, e))?;
    crate::collection::read_collection(&root).await?;
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        request_parent(&root, &original)?;
        check_revision(&original)?;
        // ponytail: the final unlink can race external writers; use handle-relative OS APIs if stronger protection is required.
        fs::remove_file(original.path()).map_err(|e| Error::io(original.path(), e))
    })
    .await
    .map_err(|_| Error::config("", "request", "No se pudo eliminar la petición"))?
}

async fn copy_request(
    root: PathBuf,
    original: Document,
    name: String,
    file_name: String,
    rename: bool,
) -> Result<Document> {
    let root = tokio::fs::canonicalize(&root)
        .await
        .map_err(|e| Error::io(&root, e))?;
    crate::collection::read_collection(&root).await?;
    request_name(&root, &name, &file_name)?;
    let parent = request_parent(&root, &original)?;
    let destination = parent.join(format!(
        "{file_name}.{}",
        original.path().extension().unwrap().to_str().unwrap()
    ));
    let edited = original.edited(
        DocumentKind::HttpRequest,
        &[FieldEdit {
            path: vec!["info".into(), "name".into()],
            value: Some(serde_json::json!(name.trim())),
        }],
    )?;
    if rename && destination == original.path() {
        return save_document(original, edited).await;
    }
    let copy = Document::from_yaml(destination, edited.original())?;
    tokio::task::spawn_blocking(move || {
        let _writer = WRITER.lock().unwrap();
        let metadata = check_revision(&original)?;
        write_new_with_permissions(&copy, Some(metadata.permissions()))?;
        if rename {
            if check_revision(&original).is_err() {
                return Err(conflict(original.path(), Some(copy.path().into())));
            }
            // ponytail: path-based checks have a final unlink race with non-cooperating writers, like save's final replace; use handle-relative OS APIs if required.
            fs::remove_file(original.path())
                .map_err(|_| conflict(original.path(), Some(copy.path().into())))?;
        }
        Ok(copy)
    })
    .await
    .map_err(|_| {
        Error::config(
            "",
            "request",
            "No se pudo completar la operación de petición",
        )
    })?
}
