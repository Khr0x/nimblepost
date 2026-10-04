use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize)]
pub struct CollectionChange {
    pub roots: Vec<PathBuf>,
    pub warning: Option<String>,
}

#[derive(Default)]
pub struct CollectionWatcher {
    watcher: Option<RecommendedWatcher>,
    roots: Arc<Mutex<BTreeSet<PathBuf>>>,
    subscriptions: BTreeMap<PathBuf, (RecursiveMode, String)>,
    pub warning: Option<String>,
}

impl CollectionWatcher {
    pub fn start(&mut self, callback: impl Fn(CollectionChange) + Send + 'static) {
        let roots = self.roots.clone();
        match RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                let roots = roots.lock().unwrap();
                let change = match result {
                    Ok(event) if !matches!(event.kind, EventKind::Access(_)) => CollectionChange {
                        roots: if event.need_rescan() {
                            roots.iter().cloned().collect()
                        } else {
                            affected_roots(&event, &roots)
                        },
                        warning: None,
                    },
                    Ok(_) => return,
                    Err(error) => CollectionChange {
                        roots: roots.iter().cloned().collect(),
                        warning: Some(error.to_string()),
                    },
                };
                drop(roots);
                if !change.roots.is_empty() {
                    callback(change);
                }
            },
            Config::default().with_follow_symlinks(false),
        ) {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(error) => self.warning = Some(error.to_string()),
        }
    }

    pub fn update(&mut self, roots: BTreeSet<PathBuf>) {
        *self.roots.lock().unwrap() = roots.clone();
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        let mut desired = BTreeMap::new();
        for root in roots {
            // Parent notifications also catch removal/recreation of the collection itself.
            if let Some(parent) = root.parent().filter(|parent| parent.is_dir()) {
                desired
                    .entry(parent.to_owned())
                    .or_insert(RecursiveMode::NonRecursive);
            }
            if root.is_dir() && std::fs::canonicalize(&root).ok().as_ref() == Some(&root) {
                desired.insert(root, RecursiveMode::Recursive);
            }
        }
        let desired: BTreeMap<_, _> = desired
            .into_iter()
            .map(|(path, mode)| {
                let identity = std::fs::metadata(&path)
                    .ok()
                    .map(|metadata| {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            format!("{}:{}", metadata.dev(), metadata.ino())
                        }
                        #[cfg(not(unix))]
                        {
                            format!("{:?}", metadata.created().ok())
                        }
                    })
                    .unwrap_or_default();
                (path, (mode, identity))
            })
            .collect();
        for path in self
            .subscriptions
            .keys()
            .filter(|path| desired.get(*path) != self.subscriptions.get(*path))
        {
            let _ = watcher.unwatch(path);
        }
        self.subscriptions
            .retain(|path, target| desired.get(path) == Some(target));
        let mut errors = vec![];
        for (path, target) in desired {
            if self.subscriptions.contains_key(&path) {
                continue;
            }
            match watcher.watch(&path, target.0) {
                Ok(()) => {
                    self.subscriptions.insert(path, target);
                }
                Err(error) => errors.push(error.to_string()),
            }
        }
        self.warning = (!errors.is_empty()).then(|| errors.join("; "));
    }
}

fn affected_roots(event: &Event, roots: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    roots
        .iter()
        .filter(|root| {
            event.paths.iter().any(|path| {
                let Ok(relative) = path.strip_prefix(root) else {
                    return false;
                };
                !relative.components().any(|part| match part {
                    Component::Normal(name) => {
                        let name = name.to_string_lossy();
                        name.starts_with('.')
                            || ["node_modules", "scripts", "fixtures", "auth"]
                                .contains(&name.as_ref())
                    }
                    _ => true,
                })
            })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
