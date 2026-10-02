use nimblepost_core::{
    Document, DocumentKind, Error, FieldEdit, History, HistoryEntry, collection_index,
    create_collection, create_environment, create_folder, create_request, duplicate_request,
    load_request, rename_folder, rename_request, save_document,
};
use serde_json::json;
use std::fs;

fn edit(path: &[&str], value: Option<serde_json::Value>) -> FieldEdit {
    FieldEdit {
        path: path.iter().map(|s| s.to_string()).collect(),
        value,
    }
}
const SOURCE: &str = "# comentario ajeno\ninfo: {name: Test, type: http}\nhttp:\n  method: GET # conservar inline\n  url: '{{baseUrl}}/users'\n  headers:\n    - name: X-Demo # conservar fila\n      value: 'yes'\n      description: untouched\nforeign: {note: 'keep unknown'}\ndocs: |\n  multilínea ñ\n";

#[tokio::test]
async fn new_collections_and_requests_are_valid_editable_and_never_overwrite() {
    let parent = tempfile::tempdir().unwrap();
    let root = create_collection(
        parent.path().into(),
        "Mi API: \"ñ\"".into(),
        "my-api".into(),
    )
    .await
    .unwrap();
    let index = collection_index(&root).await.unwrap();
    assert_eq!(index.name, "Mi API: \"ñ\"");
    assert!(index.requests.is_empty());
    let manifest = fs::read(root.join("opencollection.yml")).unwrap();
    assert!(
        create_collection(parent.path().into(), "Other".into(), "my-api".into())
            .await
            .is_err()
    );
    assert_eq!(fs::read(root.join("opencollection.yml")).unwrap(), manifest);
    let doc = create_request(
        root.clone(),
        String::new(),
        "List: \"users\"".into(),
        "list-users".into(),
        "GET".into(),
        "{{baseUrl}}/users".into(),
    )
    .await
    .unwrap();
    assert!(doc.diagnostics(DocumentKind::HttpRequest).is_empty());
    assert_eq!(doc.request_info().unwrap().name, "List: \"users\"");
    assert_eq!(
        collection_index(&root).await.unwrap().requests,
        ["list-users.yml"]
    );
    load_request(&root, "list-users.yml", None).await.unwrap();
    assert!(
        create_request(
            root.clone(),
            String::new(),
            "Duplicate".into(),
            "list-users".into(),
            "POST".into(),
            "https://example.com".into()
        )
        .await
        .is_err()
    );
    assert_eq!(fs::read_to_string(doc.path()).unwrap(), doc.original());
    let edited = doc
        .edited(
            DocumentKind::HttpRequest,
            &[edit(&["http", "method"], Some(json!("POST")))],
        )
        .unwrap();
    let saved = save_document(doc, edited).await.unwrap();
    assert_eq!(saved.request_info().unwrap().method, "POST");
    let unicode_name = "ñ".repeat(128);
    let unicode = create_request(
        root.clone(),
        String::new(),
        unicode_name.clone(),
        "unicode".into(),
        "GET".into(),
        "https://example.com".into(),
    )
    .await
    .unwrap();
    assert_eq!(unicode.request_info().unwrap().name, unicode_name);
    assert!(
        create_request(
            root,
            String::new(),
            "ñ".repeat(129),
            "too-long".into(),
            "GET".into(),
            "https://example.com".into()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn requests_can_start_without_a_url_or_explicit_filename() {
    let parent = tempfile::tempdir().unwrap();
    let root = create_collection(parent.path().into(), "Test".into(), "test".into())
        .await
        .unwrap();
    for (name, file) in [
        ("List users", "list-users.yml"),
        ("Folder", "request-folder.yml"),
        ("CON", "request-con.yml"),
        ("東京", "request.yml"),
    ] {
        let doc = create_request(
            root.clone(),
            String::new(),
            name.into(),
            String::new(),
            "GET".into(),
            String::new(),
        )
        .await
        .unwrap();
        assert_eq!(doc.path(), root.join(file));
        assert_eq!(doc.value()["info"]["name"], name);
        assert_eq!(doc.value()["http"]["url"], "");
        assert!(doc.diagnostics(DocumentKind::HttpRequest).is_empty());
        assert!(doc.request_info().is_err());
        load_request(&root, file, None).await.unwrap();
        assert!(
            create_request(
                root.clone(),
                String::new(),
                name.into(),
                String::new(),
                "GET".into(),
                String::new(),
            )
            .await
            .is_err()
        );
        assert_eq!(fs::read_to_string(doc.path()).unwrap(), doc.original());
        let edited = doc
            .edited(
                DocumentKind::HttpRequest,
                &[edit(&["http", "url"], Some(json!("https://example.com")))],
            )
            .unwrap();
        save_document(doc, edited).await.unwrap();
        load_request(&root, file, None).await.unwrap();
    }
}

#[tokio::test]
async fn creation_rejects_escapes_reserved_paths_and_invalid_fields_before_writing() {
    let parent = tempfile::tempdir().unwrap();
    let root = create_collection(parent.path().into(), "Test".into(), "test".into())
        .await
        .unwrap();
    for file in [
        "../escape",
        "/absolute",
        "a/b",
        "a\\b",
        ".git",
        "CON",
        "lpt1",
    ] {
        assert!(
            create_request(
                root.clone(),
                String::new(),
                "Name".into(),
                file.into(),
                "GET".into(),
                "https://example.com".into()
            )
            .await
            .is_err(),
            "{file}"
        );
        assert!(
            create_collection(parent.path().into(), "Name".into(), file.into())
                .await
                .is_err(),
            "{file}"
        );
    }
    assert!(
        create_collection(parent.path().into(), "Name".into(), String::new())
            .await
            .is_err()
    );
    for file in ["opencollection", "folder"] {
        assert!(
            create_request(
                root.clone(),
                String::new(),
                "Name".into(),
                file.into(),
                "GET".into(),
                "https://example.com".into()
            )
            .await
            .is_err()
        );
    }
    for (name, method, url) in [
        (" ", "GET", "https://example.com"),
        ("Name", "GET\nPOST", "https://example.com"),
        ("Name", "", "https://example.com"),
        ("Name", "GET", "https://example.com\nheaders: bad"),
    ] {
        assert!(
            create_request(
                root.clone(),
                String::new(),
                name.into(),
                "invalid".into(),
                method.into(),
                url.into()
            )
            .await
            .is_err()
        );
    }
    assert!(!root.join("invalid.yml").exists());
    #[cfg(unix)]
    {
        let outside = parent.path().join("outside.yml");
        fs::write(&outside, "preserve").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link.yml")).unwrap();
        assert!(
            create_request(
                root.clone(),
                String::new(),
                "Link".into(),
                "link".into(),
                "GET".into(),
                "https://example.com".into()
            )
            .await
            .is_err()
        );
        assert_eq!(fs::read_to_string(outside).unwrap(), "preserve");
    }
    assert!(collection_index(root).await.unwrap().requests.is_empty());
}

#[tokio::test]
async fn no_op_preserves_bytes_mtime_and_permissions_and_scalar_edit_preserves_everything_else() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("request.yml");
    let source = SOURCE.replace('\n', "\r\n");
    fs::write(&file, &source).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
    }
    let original = Document::read(&file).await.unwrap();
    let modified = fs::metadata(&file).unwrap().modified().unwrap();
    let noop = original
        .edited(
            DocumentKind::HttpRequest,
            &[edit(&["http", "method"], Some(json!("GET")))],
        )
        .unwrap();
    save_document(original.clone(), noop).await.unwrap();
    assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), modified);
    assert_eq!(fs::read_to_string(&file).unwrap(), source);
    let changed = original
        .edited(
            DocumentKind::HttpRequest,
            &[edit(&["http", "method"], Some(json!("POST")))],
        )
        .unwrap();
    assert_eq!(
        changed.original(),
        source.replace("method: GET", "method: \"POST\"")
    );
    assert_eq!(
        changed.diagnostics(DocumentKind::HttpRequest),
        original.diagnostics(DocumentKind::HttpRequest)
    );
    save_document(original, changed).await.unwrap();
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        source.replace("method: GET", "method: \"POST\"")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}

#[test]
fn edits_rows_body_auth_and_variables_without_losing_siblings_or_unknowns() {
    let doc = Document::from_yaml("request.yml", SOURCE).unwrap();
    let changed = doc
        .edited(
            DocumentKind::HttpRequest,
            &[
                edit(&["http", "headers", "0", "value"], Some(json!("true"))),
                edit(
                    &["http", "params"],
                    Some(json!([{"name":"q","value":"a & b","type":"query"}])),
                ),
                edit(
                    &["http", "body"],
                    Some(json!({"type":"json","data":"{\"name\":\"ñ\"}\n"})),
                ),
                edit(
                    &["http", "auth"],
                    Some(json!({"type":"bearer","token":"{{token}}"})),
                ),
                edit(
                    &["runtime", "variables"],
                    Some(json!([{"name":"local","value":"001"}])),
                ),
            ],
        )
        .unwrap();
    assert!(changed.original().contains("# conservar fila"));
    assert!(changed.original().contains("description: untouched"));
    assert!(
        changed
            .original()
            .contains("foreign: {note: 'keep unknown'}")
    );
    assert!(changed.original().contains("docs: |\n  multilínea ñ\n"));
    assert_eq!(changed.value()["http"]["headers"][0]["value"], "true");
    let changed = changed
        .edited(
            DocumentKind::HttpRequest,
            &[edit(
                &["http", "headers", "1"],
                Some(json!({"name":"X-New","value":"v"})),
            )],
        )
        .unwrap();
    assert_eq!(
        changed.value()["http"]["headers"].as_array().unwrap().len(),
        2
    );
    let changed = changed
        .edited(
            DocumentKind::HttpRequest,
            &[
                edit(&["http", "headers", "0"], None),
                edit(&["http", "body"], None),
            ],
        )
        .unwrap();
    assert_eq!(changed.value()["http"]["headers"][0]["name"], "X-New");
    assert!(changed.value()["http"].get("body").is_none());
    assert!(
        doc.edited(
            DocumentKind::HttpRequest,
            &[edit(&["foreign"], Some(json!("delete")))]
        )
        .is_err()
    );
    assert!(
        doc.edited(
            DocumentKind::HttpRequest,
            &[edit(&["http", "method"], Some(json!(7)))]
        )
        .is_err()
    );
}

#[tokio::test]
async fn external_edit_deletion_and_unwritable_file_never_overwrite_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("request.yml");
    fs::write(&file, SOURCE).unwrap();
    let original = Document::read(&file).await.unwrap();
    let changed = original
        .edited(
            DocumentKind::HttpRequest,
            &[edit(&["http", "method"], Some(json!("POST")))],
        )
        .unwrap();
    fs::write(&file, "external edit").unwrap();
    assert!(matches!(
        save_document(original.clone(), changed.clone()).await,
        Err(Error::Conflict { .. })
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "external edit");
    fs::remove_file(&file).unwrap();
    assert!(matches!(
        save_document(original.clone(), changed.clone()).await,
        Err(Error::Conflict { .. })
    ));
    assert!(!file.exists());
    fs::write(&file, SOURCE).unwrap();
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&file, permissions).unwrap();
    assert!(save_document(original, changed).await.is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), SOURCE);
}

#[tokio::test]
async fn environments_preserve_secret_declarations_and_refuse_overwrite_or_escape() {
    let dir = tempfile::tempdir().unwrap();
    let doc = create_environment(dir.path().into(), "local".into())
        .await
        .unwrap();
    let changed = doc.edited(DocumentKind::Environment, &[edit(&["variables"], Some(json!([
        {"name":"baseUrl","value":"http://127.0.0.1:3000"}, {"name":"token","secret":true,"type":"string"}
    ])))]).unwrap();
    let saved = save_document(doc, changed).await.unwrap();
    assert!(saved.value()["variables"][1].get("value").is_none());
    let edited = saved
        .edited(
            DocumentKind::Environment,
            &[
                edit(
                    &["variables", "2"],
                    Some(json!({"name":"other","value":"001"})),
                ),
                edit(&["variables", "1"], None),
                edit(&["variables", "0"], None),
                edit(&["variables", "0"], None),
            ],
        )
        .unwrap();
    assert_eq!(edited.value()["variables"], json!([]));
    let edited = edited
        .edited(
            DocumentKind::Environment,
            &[edit(
                &["variables"],
                Some(json!([{"name":"again","value":"value"}])),
            )],
        )
        .unwrap();
    save_document(saved, edited).await.unwrap();
    assert!(
        create_environment(dir.path().into(), "local".into())
            .await
            .is_err()
    );
    assert!(
        create_environment(dir.path().into(), "../escape".into())
            .await
            .is_err()
    );
}

#[test]
fn history_is_bounded_private_metadata_only_and_corruption_is_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");
    let mut history = History::open(path.clone()).unwrap();
    for invalid in [
        "",
        "/private/collection/request.yml",
        "../request.yml",
        "https://token@example.com",
    ] {
        assert_eq!(
            HistoryEntry::new(invalid, "GET", None, "configuration", 0, 0).request_path,
            "[invalid path]"
        );
    }
    for _ in 0..205 {
        history
            .record(HistoryEntry::new(
                "users/list.yml",
                "GET",
                Some(200),
                "response",
                5,
                117,
            ))
            .unwrap();
    }
    assert_eq!(history.entries().len(), 200);
    assert_eq!(History::open(path.clone()).unwrap().entries().len(), 200);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let entry = value["entries"][0].as_object().unwrap();
    assert_eq!(entry.len(), 7);
    for name in [
        "url",
        "headers",
        "body",
        "secrets",
        "variables",
        "auth",
        "environment",
    ] {
        assert!(!entry.contains_key(name));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    history.clear().unwrap();
    assert!(History::open(path.clone()).unwrap().entries().is_empty());
    fs::write(&path, b"corrupt").unwrap();
    assert!(History::open(path.clone()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"corrupt");
}

#[test]
fn real_requests_keep_all_other_lines_and_multiline_body_edits_preserve_following_nodes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for relative in [
        "tests/fixtures/opencollection/official/httprequest.yml",
        "tests/fixtures/opencollection/bruno/get-users.yml",
        "tests/fixtures/opencollection/bruno/users/create-user.yml",
    ] {
        let source = fs::read_to_string(root.join(relative)).unwrap();
        let doc = Document::from_yaml(relative, source).unwrap();
        let changed = doc
            .edited(
                DocumentKind::HttpRequest,
                &[edit(&["http", "method"], Some(json!("PATCH")))],
            )
            .unwrap();
        let untouched = |source: &str| {
            source
                .lines()
                .filter(|line| !line.trim_start().starts_with("method:"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(untouched(doc.original()), untouched(changed.original()));
    }
    let source = fs::read_to_string(root.join("examples/basic-http/users/create.yml")).unwrap()
        + "\ndocs: |\n  following node\n";
    let doc = Document::from_yaml("request.yml", source).unwrap();
    let changed = doc
        .edited(
            DocumentKind::HttpRequest,
            &[edit(
                &["http", "body", "data"],
                Some(json!("{\"new\":true}\n")),
            )],
        )
        .unwrap();
    assert!(changed.original().ends_with("docs: |\n  following node\n"));
    assert_eq!(changed.value()["http"]["body"]["data"], "{\"new\":true}\n");
}

#[tokio::test]
async fn nested_folders_and_requests_are_indexed_and_reject_unsafe_destinations() {
    let parent = tempfile::tempdir().unwrap();
    let root = create_collection(parent.path().into(), "API".into(), "api".into())
        .await
        .unwrap();
    let users = create_folder(root.clone(), "".into(), "Users".into(), "users".into())
        .await
        .unwrap();
    create_folder(root.clone(), "users".into(), "Admin".into(), "admin".into())
        .await
        .unwrap();
    assert_eq!(
        collection_index(&root).await.unwrap().folders,
        ["users", "users/admin"]
    );
    Document::read(users.join("folder.yml"))
        .await
        .unwrap()
        .validate(DocumentKind::Folder)
        .unwrap();
    let doc = create_request(
        root.clone(),
        "users/admin".into(),
        "List".into(),
        "list".into(),
        "GET".into(),
        "https://example.com".into(),
    )
    .await
    .unwrap();
    assert_eq!(doc.path(), root.join("users/admin/list.yml"));
    load_request(&root, "users/admin/list.yml", None)
        .await
        .unwrap();
    let metadata = fs::read(users.join("folder.yml")).unwrap();
    assert!(
        create_folder(root.clone(), "".into(), "Overwrite".into(), "users".into())
            .await
            .is_err()
    );
    assert_eq!(fs::read(users.join("folder.yml")).unwrap(), metadata);
    for folder in [
        "../",
        "/",
        "users/../users",
        "missing",
        "environments",
        ".git",
    ] {
        assert!(
            create_request(
                root.clone(),
                folder.into(),
                "Escape".into(),
                "escape".into(),
                "GET".into(),
                "https://example.com".into()
            )
            .await
            .is_err(),
            "{folder}"
        );
        assert!(
            create_folder(
                root.clone(),
                folder.into(),
                "Escape".into(),
                "escape".into()
            )
            .await
            .is_err(),
            "{folder}"
        );
    }
    assert!(
        create_folder(
            root.clone(),
            "".into(),
            "Reserved".into(),
            "environments".into()
        )
        .await
        .is_err()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(parent.path(), root.join("link")).unwrap();
        assert!(
            create_folder(
                root.clone(),
                "link".into(),
                "Escape".into(),
                "escape".into()
            )
            .await
            .is_err()
        );
        assert!(!parent.path().join("escape").exists());
    }
    assert_eq!(
        collection_index(root).await.unwrap().requests,
        ["users/admin/list.yml"]
    );
}

#[tokio::test]
async fn folder_rename_preserves_nested_files_and_rejects_collisions_or_unsafe_paths() {
    let parent = tempfile::tempdir().unwrap();
    let root = create_collection(parent.path().into(), "API".into(), "api".into())
        .await
        .unwrap();
    let source = create_folder(root.clone(), "".into(), "Users".into(), "users".into())
        .await
        .unwrap();
    create_folder(root.clone(), "users".into(), "Admin".into(), "admin".into())
        .await
        .unwrap();
    fs::write(source.join("admin/list.yml"), SOURCE).unwrap();
    fs::write(source.join(".notes"), "private notes").unwrap();
    let metadata = fs::read(source.join("folder.yml")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            source.join("admin/list.yml"),
            fs::Permissions::from_mode(0o640),
        )
        .unwrap();
    }
    fs::create_dir(root.join("occupied")).unwrap();
    for name in [
        "occupied",
        "../escape",
        "",
        ".hidden",
        "environments",
        "CON",
        "a/b",
    ] {
        assert!(
            rename_folder(root.clone(), "users".into(), name.into())
                .await
                .is_err(),
            "{name}"
        );
        assert_eq!(
            fs::read_to_string(source.join("admin/list.yml")).unwrap(),
            SOURCE
        );
    }
    for folder in ["", "../", "users/../users", "missing"] {
        assert!(
            rename_folder(root.clone(), folder.into(), "members".into())
                .await
                .is_err(),
            "{folder}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(parent.path(), root.join("link")).unwrap();
        assert!(
            rename_folder(root.clone(), "link".into(), "members".into())
                .await
                .is_err()
        );
    }
    let renamed = rename_folder(root.clone(), "users".into(), "members".into())
        .await
        .unwrap();
    assert_eq!(renamed, root.join("members"));
    assert!(!source.exists());
    assert!(root.join("occupied").is_dir());
    assert_eq!(fs::read(renamed.join("folder.yml")).unwrap(), metadata);
    assert_eq!(
        fs::read_to_string(renamed.join("admin/list.yml")).unwrap(),
        SOURCE
    );
    assert_eq!(
        fs::read_to_string(renamed.join(".notes")).unwrap(),
        "private notes"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(renamed.join("admin/list.yml"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }
    assert_eq!(
        collection_index(&root).await.unwrap().requests,
        ["members/admin/list.yml"]
    );
    assert_eq!(
        rename_folder(root.clone(), "members".into(), "members".into())
            .await
            .unwrap(),
        renamed
    );
    rename_folder(root.clone(), "members/admin".into(), "staff".into())
        .await
        .unwrap();
    assert_eq!(
        collection_index(&root).await.unwrap().requests,
        ["members/staff/list.yml"]
    );
}

#[tokio::test]
async fn rename_and_duplicate_preserve_yaml_permissions_and_protect_existing_or_changed_files() {
    let parent = tempfile::tempdir().unwrap();
    let root = create_collection(parent.path().into(), "API".into(), "api".into())
        .await
        .unwrap();
    create_folder(root.clone(), "".into(), "Users".into(), "users".into())
        .await
        .unwrap();
    let source = root.join("users/list.yaml");
    fs::write(&source, SOURCE).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    }
    let original = Document::read(&source).await.unwrap();
    let copy = duplicate_request(
        root.clone(),
        original.clone(),
        "Test copy".into(),
        "list-copy".into(),
    )
    .await
    .unwrap();
    assert_eq!(fs::read_to_string(&source).unwrap(), SOURCE);
    assert_eq!(
        copy.original(),
        SOURCE.replace("name: Test", "name: \"Test copy\"")
    );
    assert_eq!(copy.path(), root.join("users/list-copy.yaml"));
    assert_eq!(
        copy.diagnostics(DocumentKind::HttpRequest),
        original.diagnostics(DocumentKind::HttpRequest)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(copy.path()).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
    assert!(
        rename_request(
            root.clone(),
            original.clone(),
            "Collision".into(),
            "list-copy".into()
        )
        .await
        .is_err()
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), SOURCE);
    let renamed = rename_request(
        root.clone(),
        copy.clone(),
        "Renamed".into(),
        "renamed".into(),
    )
    .await
    .unwrap();
    assert!(!copy.path().exists());
    assert_eq!(
        renamed.original(),
        SOURCE.replace("name: Test", "name: \"Renamed\"")
    );
    let same_path = rename_request(
        root.clone(),
        renamed.clone(),
        "Name only".into(),
        "renamed".into(),
    )
    .await
    .unwrap();
    assert_eq!(same_path.path(), renamed.path());
    assert_eq!(same_path.value()["info"]["name"], "Name only");
    fs::write(&source, format!("{SOURCE}# external change\n")).unwrap();
    assert!(
        duplicate_request(
            root.clone(),
            original.clone(),
            "Stale".into(),
            "stale".into()
        )
        .await
        .is_err()
    );
    assert!(
        rename_request(root.clone(), original, "Stale".into(), "stale".into())
            .await
            .is_err()
    );
    assert!(!root.join("users/stale.yaml").exists());
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        format!("{SOURCE}# external change\n")
    );
    assert_eq!(
        collection_index(root).await.unwrap().requests,
        ["users/list.yaml", "users/renamed.yaml"]
    );
}
