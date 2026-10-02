use crate::{
    Document, DocumentKind, Error, Result,
    collection::{read_collection, read_within},
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub struct CollectionIndex {
    pub root: PathBuf,
    pub name: String,
    pub requests: Vec<String>,
    pub folders: Vec<String>,
    pub environments: Vec<String>,
}

pub struct RequestInfo {
    pub name: String,
    pub method: String,
    pub url: String,
}

impl Document {
    /// Returns the saved template, never resolved credentials or URLs.
    pub fn request_info(&self) -> Result<RequestInfo> {
        self.validate(DocumentKind::HttpRequest)?;
        if self.value["info"]["type"] != "http" {
            return Err(self.error("info.type", "Se requiere una petición HTTP"));
        }
        let required = |key: &str| {
            self.value["http"][key]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| self.error(key, "Campo HTTP obligatorio"))
        };
        Ok(RequestInfo {
            name: self.value["info"]["name"]
                .as_str()
                .unwrap_or("Request")
                .into(),
            method: required("method")?,
            url: required("url")?,
        })
    }
}

/// Index filenames lazily; request YAML is parsed only when selected.
pub async fn collection_index(root: impl AsRef<Path>) -> Result<CollectionIndex> {
    let root = tokio::fs::canonicalize(root.as_ref())
        .await
        .map_err(|e| Error::io(root.as_ref(), e))?;
    let collection = read_collection(&root).await?;
    let mut requests = Vec::new();
    let mut directories = Vec::new();
    let mut environments = BTreeSet::new();
    let mut folders = vec![(root.clone(), 0)];
    while let Some((folder, depth)) = folders.pop() {
        if depth > 64 {
            return Err(Error::config(folder, "tree", "La profundidad máxima es 64"));
        }
        let mut entries = tokio::fs::read_dir(&folder)
            .await
            .map_err(|e| Error::io(&folder, e))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::io(&folder, e))?
        {
            let path = entry.path();
            let file_type = entry.file_type().await.map_err(|e| Error::io(&path, e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            // Symlink roots can be opened explicitly; discovery never follows links.
            if file_type.is_symlink()
                || name.starts_with('.')
                || ["node_modules", "scripts", "fixtures", "auth"].contains(&name.as_str())
            {
                continue;
            }
            if file_type.is_dir() {
                if folder == root && name == "environments" {
                    let mut env_files = tokio::fs::read_dir(&path)
                        .await
                        .map_err(|e| Error::io(&path, e))?;
                    while let Some(env_file) = env_files
                        .next_entry()
                        .await
                        .map_err(|e| Error::io(&path, e))?
                    {
                        let env_path = env_file.path();
                        if !env_file
                            .file_type()
                            .await
                            .map_err(|e| Error::io(&env_path, e))?
                            .is_file()
                            || !is_yaml(&env_path)
                        {
                            continue;
                        }
                        let doc = read_within(&root, env_path.strip_prefix(&root).unwrap()).await?;
                        doc.validate(DocumentKind::Environment)?;
                        let name = doc.value["name"].as_str().unwrap();
                        if name.is_empty() || !environments.insert(name.to_owned()) {
                            return Err(
                                doc.error("name", "Nombre de environment vacío o duplicado")
                            );
                        }
                    }
                } else if name != "environments" {
                    directories.push(relative_path(&root, &path)?);
                    if directories.len() > 10_000 {
                        return Err(Error::config(
                            &root,
                            "tree",
                            "Esta versión indexa hasta 10,000 carpetas",
                        ));
                    }
                    folders.push((path, depth + 1));
                }
            } else if file_type.is_file()
                && is_yaml(&path)
                && !["opencollection.yml", "folder.yml"].contains(&name.as_str())
            {
                requests.push(relative_path(&root, &path)?);
                if requests.len() > 10_000 {
                    return Err(Error::config(
                        &root,
                        "tree",
                        "Esta versión indexa hasta 10,000 archivos YAML",
                    ));
                }
            }
        }
    }
    requests.sort();
    directories.sort();
    Ok(CollectionIndex {
        name: collection.value["info"]["name"].as_str().unwrap().into(),
        root,
        requests,
        folders: directories,
        environments: environments.into_iter().collect(),
    })
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)
        .unwrap()
        .to_str()
        .ok_or_else(|| Error::config(path, "path", "Se requiere una ruta UTF-8"))?
        .replace('\\', "/"))
}

fn is_yaml(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("yml" | "yaml")
    )
}
