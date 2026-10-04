use super::*;
use serde_json::json;

fn snapshot() -> RecoverySnapshot {
    serde_json::from_value(json!({
        "version":1,"workspaceId":1,"activeTab":2,"home":false,
        "tabs":[{"id":2,"root":null,"path":"","environment":"",
            "value":{"info":{"name":"Untitled","type":"http"},"http":{"method":"POST","url":"https://example.com","body":{"type":"json","data":"{\"draft\":true}"}}},
            "edits":[{"path":["http","method"],"value":"POST"}],"source":null}]
    })).unwrap()
}

#[test]
fn roundtrip_is_private_atomic_and_preserves_last_good_backup_on_failure() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recovery.json");
    let mut store = RecoveryStore::open(path.clone());
    assert!(store.view.snapshot.is_none());
    store.save(snapshot()).unwrap();
    let bytes = fs::read(&path).unwrap();
    let reopened = RecoveryStore::open(path.clone());
    assert!(reopened.view.warning.is_none());
    assert_eq!(
        reopened.view.snapshot.unwrap().tabs[0].value["http"]["body"]["data"],
        "{\"draft\":true}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let mut invalid = snapshot();
    invalid.active_tab = Some(99);
    assert!(store.save(invalid).is_err());
    let mut large = snapshot();
    large.tabs[0].value["http"]["url"] = Value::String("x".repeat(MAX_BYTES));
    assert!(store.save(large).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(store.view.snapshot.unwrap().active_tab, Some(2));
    let mut store = RecoveryStore::open(path.clone());
    fs::write(&path, b"external change").unwrap();
    assert!(store.save(snapshot()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"external change");
    let mut corrupt = RecoveryStore::open(path.clone());
    assert!(corrupt.view.warning.is_some());
    assert!(corrupt.save(snapshot()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"external change");
}

#[test]
fn incompatible_data_and_symlinks_are_never_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recovery.json");
    for version in [0, 2] {
        let mut data = snapshot();
        data.version = version;
        let bytes = serde_json::to_vec(&data).unwrap();
        fs::write(&path, &bytes).unwrap();
        let mut store = RecoveryStore::open(path.clone());
        assert!(store.view.warning.is_some());
        assert!(store.save(snapshot()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    let mut data = serde_json::to_value(snapshot()).unwrap();
    data["tabs"][0]["secrets"] = json!({"token":"execution-secret"});
    assert!(serde_json::from_value::<RecoverySnapshot>(data).is_err());
    #[cfg(unix)]
    {
        let outside = directory.path().join("outside.json");
        fs::write(&outside, b"keep").unwrap();
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        let mut store = RecoveryStore::open(path);
        assert!(store.view.warning.is_some());
        assert!(store.save(snapshot()).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"keep");
    }
}
