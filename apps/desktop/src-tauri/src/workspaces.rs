use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollectionRef {
    pub root: PathBuf,
    pub name: String,
    pub read_only: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workspace {
    pub id: u32,
    pub name: String,
    pub collections: Vec<CollectionRef>,
    pub active_collection: Option<PathBuf>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registry {
    pub version: u8,
    pub active_workspace: u32,
    pub workspaces: Vec<Workspace>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            version: 2,
            active_workspace: 1,
            workspaces: vec![Workspace {
                id: 1,
                name: "My Workspace".into(),
                collections: vec![],
                active_collection: None,
            }],
        }
    }
}
impl Registry {
    pub fn active(&self) -> &Workspace {
        self.workspaces
            .iter()
            .find(|workspace| workspace.id == self.active_workspace)
            .unwrap()
    }
    pub fn active_mut(&mut self) -> &mut Workspace {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.id == self.active_workspace)
            .unwrap()
    }
    pub fn attach(&mut self, collection: CollectionRef) {
        let workspace = self.active_mut();
        workspace.active_collection = Some(collection.root.clone());
        if let Some(existing) = workspace
            .collections
            .iter_mut()
            .find(|existing| existing.root == collection.root)
        {
            *existing = collection;
        } else {
            workspace.collections.push(collection);
        }
    }
    pub fn create(&mut self, name: String) -> Result<(), String> {
        validate_name(&name)?;
        if self
            .workspaces
            .iter()
            .any(|workspace| workspace.name.eq_ignore_ascii_case(name.trim()))
        {
            return Err("A workspace with this name already exists".into());
        }
        let id = self
            .workspaces
            .iter()
            .map(|workspace| workspace.id)
            .max()
            .unwrap()
            .checked_add(1)
            .ok_or("Workspace IDs exhausted")?;
        self.workspaces.push(Workspace {
            id,
            name: name.trim().into(),
            collections: vec![],
            active_collection: None,
        });
        self.active_workspace = id;
        Ok(())
    }
    pub fn rename(&mut self, id: u32, name: String) -> Result<(), String> {
        validate_name(&name)?;
        if self
            .workspaces
            .iter()
            .any(|workspace| workspace.id != id && workspace.name.eq_ignore_ascii_case(name.trim()))
        {
            return Err("A workspace with this name already exists".into());
        }
        let workspace = self
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .ok_or("Workspace not found")?;
        workspace.name = name.trim().into();
        Ok(())
    }
    pub fn remove(&mut self, id: u32) -> Result<(), String> {
        if !self.workspaces.iter().any(|workspace| workspace.id == id) {
            return Err("Workspace not found".into());
        }
        self.workspaces.retain(|workspace| workspace.id != id);
        if self.active_workspace == id {
            let fallback = self
                .workspaces
                .iter()
                .find(|workspace| workspace.name.eq_ignore_ascii_case("My Workspace"))
                .or_else(|| self.workspaces.iter().find(|workspace| workspace.id == 1));
            if let Some(workspace) = fallback {
                self.active_workspace = workspace.id;
            } else {
                self.workspaces
                    .insert(0, Self::default().workspaces.remove(0));
                self.active_workspace = 1;
            }
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), String> {
        if !matches!(self.version, 1 | 2) || self.workspaces.is_empty() {
            return Err("Unsupported workspace file".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for workspace in &self.workspaces {
            validate_name(&workspace.name)?;
            if !ids.insert(workspace.id) {
                return Err("Duplicate workspace ID".into());
            }
            let mut roots = std::collections::BTreeSet::new();
            for collection in &workspace.collections {
                if !collection.root.is_absolute() || !roots.insert(&collection.root) {
                    return Err("Invalid or duplicate collection path".into());
                }
            }
            if workspace
                .active_collection
                .as_ref()
                .is_some_and(|root| !roots.contains(root))
            {
                return Err("Active collection is not in its workspace".into());
            }
        }
        if !ids.contains(&self.active_workspace) {
            return Err("Active workspace is missing".into());
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
        return Err("Use a name with 1–128 characters and no control characters".into());
    }
    Ok(())
}

#[derive(Default)]
pub struct Store {
    path: Option<PathBuf>,
    original: Option<Vec<u8>>,
    pub registry: Registry,
}
impl Store {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let mut bytes = vec![];
        match fs::File::open(&path) {
            Ok(file) => {
                file.take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path: Some(path),
                    ..Self::default()
                });
            }
            Err(error) => return Err(error.to_string()),
        }
        if bytes.len() > 1024 * 1024 {
            return Err("Workspace file exceeds 1 MiB; it will not be overwritten".into());
        }
        let mut registry: Registry = serde_json::from_slice(&bytes)
            .map_err(|_| "Invalid workspace file; it will not be overwritten")?;
        registry.validate()?;
        let legacy = registry.version == 1;
        // Keep existing collection references while updating the original default label.
        if let Some(workspace) = registry
            .workspaces
            .iter_mut()
            .find(|workspace| legacy && workspace.id == 1 && workspace.name == "Local workspace")
        {
            workspace.name = "My Workspace".into();
        }
        registry.version = 2;
        Ok(Self {
            path: Some(path),
            original: Some(bytes),
            registry,
        })
    }
    fn check_revision(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let current =
            match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.is_file() && !metadata.permissions().readonly() => {
                    Some(fs::read(path).map_err(|error| error.to_string())?)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                _ => return Err(
                    "Workspace file is not writable or is a symlink; it will not be overwritten"
                        .into(),
                ),
            };
        if current != self.original {
            return Err("Workspace file changed externally. Restart the app to reload it".into());
        }
        Ok(())
    }
    pub fn update(
        &mut self,
        edit: impl FnOnce(&mut Registry) -> Result<(), String>,
    ) -> Result<Registry, String> {
        let mut registry = self.registry.clone();
        edit(&mut registry)?;
        registry.validate()?;
        if let Some(path) = &self.path {
            self.check_revision()?;
            let parent = path.parent().ok_or("Invalid workspace storage location")?;
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            let bytes = serde_json::to_vec_pretty(&registry).map_err(|error| error.to_string())?;
            if bytes.len() > 1024 * 1024 {
                return Err("Workspace references exceed 1 MiB".into());
            }
            let mut temporary =
                tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
            temporary
                .write_all(&bytes)
                .and_then(|_| temporary.as_file().sync_all())
                .map_err(|error| error.to_string())?;
            self.check_revision()?;
            // ponytail: one app writer and path-based conflict checks, like history; use OS locking if multiple instances must edit concurrently.
            temporary
                .persist(path)
                .map_err(|error| error.error.to_string())?;
            self.original = Some(bytes);
        }
        self.registry = registry.clone();
        Ok(registry)
    }
}

#[cfg(test)]
mod tests;
