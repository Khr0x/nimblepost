use super::*;
use notify::event::{CreateKind, ModifyKind};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn filters_hidden_git_and_unrelated_paths_but_keeps_rename_and_directory_changes() {
    let root = PathBuf::from("/tmp/api");
    let roots = BTreeSet::from([root.clone(), PathBuf::from("/tmp/other")]);
    for path in [
        "/tmp/api/.git/index",
        "/tmp/api/node_modules/x.yml",
        "/tmp/api/.draft.yml",
        "/tmp/api-other/a.yml",
        "/tmp/api/scripts/a.yml",
    ] {
        assert!(
            affected_roots(
                &Event::new(EventKind::Create(CreateKind::Any)).add_path(path.into()),
                &roots
            )
            .is_empty()
        );
    }
    for path in [
        "/tmp/api",
        "/tmp/api/folder",
        "/tmp/api/folder/request.yaml",
        "/tmp/api/environments/local.yml",
    ] {
        assert_eq!(
            affected_roots(
                &Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.into()),
                &roots
            ),
            std::slice::from_ref(&root)
        );
    }
    let rename = Event::new(EventKind::Modify(ModifyKind::Any))
        .add_path("/tmp/api/a.yml".into())
        .add_path("/tmp/other/b.yml".into());
    assert_eq!(affected_roots(&rename, &roots).len(), 2);
}

#[test]
fn native_notifications_resume_after_a_collection_is_replaced_and_stop_after_detaching() {
    let directory = tempfile::tempdir().unwrap();
    let parent = std::fs::canonicalize(directory.path()).unwrap();
    let root = parent.join("api");
    std::fs::create_dir(&root).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut watcher = CollectionWatcher::default();
    watcher.start(move |change| {
        let _ = tx.send(change);
    });
    watcher.update(BTreeSet::from([root.clone()]));
    assert!(watcher.warning.is_none(), "{:?}", watcher.warning);
    let notified = || {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let change = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("native notification");
            assert!(change.warning.is_none(), "{:?}", change.warning);
            if change.roots.contains(&root) {
                break;
            }
        }
    };
    std::fs::write(root.join("request.yml"), "draft").unwrap();
    notified();
    std::fs::rename(&root, parent.join("previous")).unwrap();
    std::fs::create_dir(&root).unwrap();
    notified();
    watcher.update(BTreeSet::from([root.clone()]));
    assert!(watcher.warning.is_none());
    while rx.try_recv().is_ok() {}
    std::fs::create_dir(root.join("nested")).unwrap();
    std::fs::write(root.join("nested/request.yml"), "after checkout").unwrap();
    notified();
    watcher.update(BTreeSet::new());
    assert!(watcher.subscriptions.is_empty());
    assert!(watcher.roots.lock().unwrap().is_empty());
}
