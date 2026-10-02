use crate::{Error, Result, storage::WRITER};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryEntry {
    pub timestamp: u64,
    pub request_path: String,
    pub method: String,
    pub status: Option<u16>,
    pub outcome: String,
    pub elapsed_ms: u64,
    pub body_bytes: usize,
}

impl HistoryEntry {
    pub fn new(
        path: &str,
        method: &str,
        status: Option<u16>,
        outcome: &str,
        elapsed_ms: u64,
        body_bytes: usize,
    ) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            request_path: if !path.is_empty()
                && path.len() <= 1024
                && !path.contains("://")
                && Path::new(path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
            {
                path
            } else {
                "[invalid path]"
            }
            .into(),
            method: if [
                "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE",
            ]
            .contains(&method)
            {
                method
            } else {
                "CUSTOM"
            }
            .into(),
            status,
            outcome: if [
                "response",
                "cancelled",
                "timeout",
                "configuration",
                "transport",
            ]
            .contains(&outcome)
            {
                outcome
            } else {
                "configuration"
            }
            .into(),
            elapsed_ms,
            body_bytes,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredHistory {
    version: u8,
    entries: Vec<HistoryEntry>,
}

/// Only metadata is representable in this store. No URL, environment values, credentials, headers or bodies.
pub struct History {
    path: PathBuf,
    entries: Vec<HistoryEntry>,
}
impl History {
    pub fn open(path: PathBuf) -> Result<Self> {
        let mut bytes = Vec::new();
        match fs::File::open(&path) {
            Ok(file) => {
                file.take(512 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| Error::io(&path, e))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path,
                    entries: vec![],
                });
            }
            Err(e) => return Err(Error::io(&path, e)),
        }
        if bytes.len() > 512 * 1024 {
            return Err(Error::config(
                &path,
                "history",
                "Historial mayor de 512 KiB; no se sobrescribirá",
            ));
        }
        let stored: StoredHistory = serde_json::from_slice(&bytes).map_err(|_| {
            Error::config(&path, "history", "Historial inválido; no se sobrescribirá")
        })?;
        if stored.version != 1 || stored.entries.len() > 200 {
            return Err(Error::config(
                &path,
                "history",
                "Versión o límite de historial incompatible",
            ));
        }
        Ok(Self {
            path,
            entries: stored.entries,
        })
    }
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }
    pub fn record(&mut self, entry: HistoryEntry) -> Result<()> {
        // ponytail: bounded JSON (200 entries); SQLite when searchable history outgrows this limit.
        let mut entries = self.entries.clone();
        entries.insert(0, entry);
        entries.truncate(200);
        self.write(entries)
    }
    pub fn clear(&mut self) -> Result<()> {
        self.write(vec![])
    }
    fn write(&mut self, entries: Vec<HistoryEntry>) -> Result<()> {
        let _writer = WRITER.lock().unwrap();
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        let bytes = serde_json::to_vec(&StoredHistory {
            version: 1,
            entries: entries.clone(),
        })
        .expect("metadata serializes");
        if bytes.len() > 512 * 1024 {
            return Err(Error::config(
                &self.path,
                "history",
                "El historial supera 512 KiB",
            ));
        }
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|e| Error::io(&self.path, e))?;
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|e| Error::io(&self.path, e))?;
        temporary
            .persist(&self.path)
            .map_err(|e| Error::io(&self.path, e.error))?;
        self.entries = entries;
        Ok(())
    }
}
