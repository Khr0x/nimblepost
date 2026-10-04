use nimblepost_core::FieldEdit;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryTab {
    pub id: u64,
    pub root: Option<PathBuf>,
    pub path: String,
    pub environment: String,
    pub value: Value,
    pub edits: Vec<FieldEdit>,
    pub source: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverySnapshot {
    pub version: u8,
    pub workspace_id: u32,
    pub active_tab: Option<u64>,
    pub home: bool,
    pub tabs: Vec<RecoveryTab>,
}

impl RecoverySnapshot {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.workspace_id == 0 || self.tabs.len() > 128 {
            return Err("Unsupported recovery file or more than 128 tabs".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut requests = std::collections::BTreeSet::new();
        for tab in &self.tabs {
            if tab.id == 0
                || tab.id > 9_007_199_254_740_991
                || !ids.insert(tab.id)
                || tab.environment.len() > 512
                || tab.edits.len() > 1024
                || tab.value["info"]["type"] != "http"
                || !tab.value["http"].is_object()
                || tab.root.as_ref().is_some_and(|root| !root.is_absolute())
            {
                return Err("Invalid recovered request tab".into());
            }
            if !tab.path.is_empty() {
                let path = Path::new(&tab.path);
                if tab.root.is_none()
                    || tab.path.len() > 4096
                    || path.is_absolute()
                    || path
                        .components()
                        .any(|part| !matches!(part, Component::Normal(_)))
                    || !matches!(
                        path.extension().and_then(|s| s.to_str()),
                        Some("yml" | "yaml")
                    )
                    || !requests.insert((&tab.root, &tab.path))
                    || (!tab.edits.is_empty()
                        && tab
                            .source
                            .as_ref()
                            .is_none_or(|source| source["info"]["type"] != "http"))
                {
                    return Err("Invalid recovered request path or source".into());
                }
            }
        }
        if self.active_tab.is_some_and(|id| !ids.contains(&id))
            || (!ids.is_empty() && self.active_tab.is_none())
        {
            return Err("Recovered active tab is missing".into());
        }
        Ok(())
    }
}

#[derive(Clone, Default, Serialize)]
pub struct RecoveryView {
    pub snapshot: Option<RecoverySnapshot>,
    pub warning: Option<String>,
}

#[derive(Default)]
pub struct RecoveryStore {
    path: Option<PathBuf>,
    original: Option<Vec<u8>>,
    pub view: RecoveryView,
}

impl RecoveryStore {
    pub fn open(path: PathBuf) -> Self {
        let mut store = Self {
            path: Some(path),
            ..Self::default()
        };
        match store.read() {
            Ok(bytes) => {
                let result = bytes
                    .as_ref()
                    .map(|bytes| {
                        let snapshot: RecoverySnapshot = serde_json::from_slice(bytes)
                            .map_err(|_| "Recovery file is corrupt or incompatible")?;
                        snapshot.validate()?;
                        Ok::<_, String>(snapshot)
                    })
                    .transpose();
                match result {
                    Ok(snapshot) => {
                        store.original = bytes;
                        store.view.snapshot = snapshot;
                    }
                    Err(error) => store.view.warning = Some(error),
                }
            }
            Err(error) => store.view.warning = Some(error),
        }
        store
    }

    fn read(&self) -> Result<Option<Vec<u8>>, String> {
        let Some(path) = &self.path else {
            return Ok(None);
        };
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Ok(metadata) if metadata.is_file() && !metadata.permissions().readonly() => {}
            _ => {
                return Err(
                    "Recovery file is not writable or is a symlink; it will not be overwritten"
                        .into(),
                );
            }
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .and_then(|file| file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes))
            .map_err(|error| error.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("Recovery data exceeds 16 MiB".into());
        }
        Ok(Some(bytes))
    }

    fn check_revision(&self) -> Result<(), String> {
        if self.read()? != self.original {
            return Err(
                "Recovery file changed externally. Restart the app before saving recovery data"
                    .into(),
            );
        }
        Ok(())
    }

    pub fn save(&mut self, snapshot: RecoverySnapshot) -> Result<(), String> {
        if let Some(warning) = &self.view.warning {
            return Err(warning.clone());
        }
        snapshot.validate()?;
        let bytes = serde_json::to_vec(&snapshot).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("Recovery data exceeds 16 MiB".into());
        }
        if let Some(path) = &self.path {
            self.check_revision()?;
            if self.original.as_ref() != Some(&bytes) {
                let parent = path.parent().ok_or("Invalid recovery storage location")?;
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                let mut temporary =
                    tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    temporary
                        .as_file()
                        .set_permissions(fs::Permissions::from_mode(0o600))
                        .map_err(|error| error.to_string())?;
                }
                temporary
                    .write_all(&bytes)
                    .and_then(|_| temporary.as_file().sync_all())
                    .map_err(|error| error.to_string())?;
                self.check_revision()?;
                // ponytail: serialized app writes and byte checks, like workspaces; OS locking if multiple app instances must cooperate.
                temporary
                    .persist(path)
                    .map_err(|error| error.error.to_string())?;
                self.original = Some(bytes);
            }
        }
        self.view.snapshot = Some(snapshot);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
