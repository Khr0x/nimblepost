use super::*;

#[test]
fn renaming_validates_names_and_removing_keeps_a_usable_default() {
    let mut registry = Registry::default();
    registry.create("Team".into()).unwrap();
    let team = registry.active_workspace;
    assert!(registry.rename(team, "my workspace".into()).is_err());
    assert!(registry.rename(team, "\n".into()).is_err());
    assert!(registry.rename(team, "x".repeat(129)).is_err());
    assert!(registry.rename(999, "Missing".into()).is_err());
    registry.rename(team, " Services ".into()).unwrap();
    assert_eq!(registry.active().name, "Services");
    assert!(registry.remove(999).is_err());
    registry.remove(team).unwrap();
    assert_eq!(registry.active().name, "My Workspace");
    registry.create("Other".into()).unwrap();
    let other = registry.active_workspace;
    registry.remove(1).unwrap();
    assert_eq!(registry.active_workspace, other);
    registry.remove(other).unwrap();
    assert_eq!(registry.active().name, "My Workspace");
    assert!(registry.active().collections.is_empty());
    registry.remove(1).unwrap();
    assert_eq!(registry.workspaces.len(), 1);
    registry.validate().unwrap();
    // A renamed default keeps its user-chosen label when used as fallback.
    registry.rename(1, "Home".into()).unwrap();
    registry.create("Team".into()).unwrap();
    registry.remove(registry.active_workspace).unwrap();
    assert_eq!(registry.active().name, "Home");
}

#[test]
fn default_label_is_updated_without_changing_existing_references() {
    assert_eq!(Registry::default().active().name, "My Workspace");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspaces.json");
    let mut registry = Registry {
        version: 1,
        ..Registry::default()
    };
    registry.active_mut().name = "Local workspace".into();
    let root = directory.path().join("api");
    registry.attach(CollectionRef {
        root: root.clone(),
        name: "API".into(),
        read_only: false,
    });
    fs::write(&path, serde_json::to_vec(&registry).unwrap()).unwrap();
    let mut store = Store::open(path.clone()).unwrap();
    assert_eq!(store.registry.active().name, "My Workspace");
    assert_eq!(
        store.registry.active().active_collection.as_ref(),
        Some(&root)
    );
    store.update(|_| Ok(())).unwrap();
    assert_eq!(
        Store::open(path.clone()).unwrap().registry.active().name,
        "My Workspace"
    );
    store
        .update(|registry| {
            registry.active_mut().name = "Custom".into();
            Ok(())
        })
        .unwrap();
    assert_eq!(
        Store::open(path.clone()).unwrap().registry.active().name,
        "Custom"
    );
    store
        .update(|registry| registry.rename(1, "Local workspace".into()))
        .unwrap();
    assert_eq!(
        Store::open(path).unwrap().registry.active().name,
        "Local workspace"
    );
}

#[test]
fn roundtrip_multiple_workspaces_and_deduplicated_references() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspaces.json");
    let mut store = Store::open(path.clone()).unwrap();
    let root = directory.path().join("api");
    for _ in 0..2 {
        store
            .update(|registry| {
                registry.attach(CollectionRef {
                    root: root.clone(),
                    name: "API".into(),
                    read_only: true,
                });
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(store.registry.active().collections.len(), 1);
    store
        .update(|registry| registry.create(" Team ".into()))
        .unwrap();
    assert!(
        store
            .update(|registry| registry.create("team".into()))
            .is_err()
    );
    assert!(
        store
            .update(|registry| registry.create("\n".into()))
            .is_err()
    );
    let reopened = Store::open(path.clone()).unwrap();
    assert_eq!(reopened.registry.active().name, "Team");
    assert!(reopened.registry.active().collections.is_empty());
    assert_eq!(reopened.registry.workspaces[0].collections[0].root, root);
    assert!(reopened.registry.workspaces[0].collections[0].read_only);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn failed_commit_and_corruption_preserve_disk_and_memory() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspaces.json");
    let mut store = Store::open(path.clone()).unwrap();
    store
        .update(|registry| registry.create("Team".into()))
        .unwrap();
    fs::write(&path, b"externally changed").unwrap();
    assert!(
        store
            .update(|registry| registry.create("Other".into()))
            .err()
            .unwrap()
            .contains("externally")
    );
    assert_eq!(store.registry.active().name, "Team");
    assert_eq!(fs::read(&path).unwrap(), b"externally changed");
    assert!(Store::open(path.clone()).is_err());
    fs::remove_file(&path).unwrap();
    assert!(
        store
            .update(|registry| registry.create("Other".into()))
            .is_err()
    );
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn symlink_cannot_redirect_workspace_writes() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.json");
    let path = directory.path().join("workspaces.json");
    fs::write(&target, serde_json::to_vec(&Registry::default()).unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let bytes = fs::read(&target).unwrap();
    let mut store = Store::open(path).unwrap();
    assert!(
        store
            .update(|registry| registry.create("Team".into()))
            .is_err()
    );
    assert_eq!(fs::read(target).unwrap(), bytes);
    assert_eq!(store.registry.workspaces.len(), 1);
}
