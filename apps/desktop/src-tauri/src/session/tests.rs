use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../examples/basic-http")
}

#[tokio::test]
async fn renames_and_removes_workspaces_without_touching_collection_files() {
    let directory = tempfile::tempdir().unwrap();
    let storage = directory.path().join("workspaces.json");
    let session = Session::default();
    session.init_workspaces(storage.clone());
    let one = session
        .create_collection(directory.path().into(), "One".into(), "one".into())
        .await
        .unwrap();
    session
        .create_request(CreateRequestInput {
            edits: Vec::new(),
            folder: String::new(),
            name: "One".into(),
            file_name: "list".into(),
            method: "GET".into(),
            url: "https://example.com/one".into(),
        })
        .await
        .unwrap();
    let team = session
        .create_workspace("Team".into())
        .await
        .unwrap()
        .active_workspace_id;
    let two = session
        .create_collection(directory.path().into(), "Two".into(), "two".into())
        .await
        .unwrap();
    let request = session
        .create_request(CreateRequestInput {
            edits: Vec::new(),
            folder: String::new(),
            name: "Two".into(),
            file_name: "list".into(),
            method: "GET".into(),
            url: "https://example.com/two".into(),
        })
        .await
        .unwrap()
        .request;
    let bytes = std::fs::read(Path::new(&two.root).join("list.yml")).unwrap();
    let renamed = session
        .rename_workspace(team, "Services".into())
        .await
        .unwrap();
    assert_eq!(
        renamed
            .workspaces
            .iter()
            .find(|workspace| workspace.id == team)
            .unwrap()
            .name,
        "Services"
    );
    assert_eq!(
        renamed.active_collection.as_deref(),
        Some(two.root.as_str())
    );
    session
        .save(SaveInput {
            revision: request.revision,
            edits: vec![],
        })
        .await
        .unwrap();
    assert!(
        session
            .rename_workspace(team, "My Workspace".into())
            .await
            .is_err()
    );
    assert_eq!(
        Store::open(storage.clone()).unwrap().registry.active().name,
        "Services"
    );
    let fallback = session.remove_workspace(team).await.unwrap();
    assert_eq!(fallback.active_workspace_id, 1);
    assert_eq!(
        fallback.active_collection.as_deref(),
        Some(one.root.as_str())
    );
    assert_eq!(session.read_request("list.yml").await.unwrap().name, "One");
    assert!(
        session
            .save(SaveInput {
                revision: request.revision,
                edits: vec![]
            })
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(Path::new(&two.root).join("list.yml")).unwrap(),
        bytes
    );
    let restored = Session::default();
    restored.init_workspaces(storage.clone());
    assert_eq!(restored.read_workspace().await.unwrap().workspaces.len(), 1);
    let team = restored
        .create_workspace("Team".into())
        .await
        .unwrap()
        .active_workspace_id;
    restored.open(two.root.clone().into()).await.unwrap();
    let request = restored.read_request("list.yml").await.unwrap();
    restored.remove_workspace(1).await.unwrap();
    restored
        .save(SaveInput {
            revision: request.revision,
            edits: vec![],
        })
        .await
        .unwrap();
    let empty = restored.remove_workspace(team).await.unwrap();
    assert_eq!(empty.active_workspace_id, 1);
    assert_eq!(empty.workspaces[0].name, "My Workspace");
    assert!(empty.collections.is_empty());
    assert!(empty.active_collection.is_none());
    assert!(Path::new(&one.root).join("list.yml").is_file());
    assert_eq!(
        std::fs::read(Path::new(&two.root).join("list.yml")).unwrap(),
        bytes
    );
    let reopened = Session::default();
    reopened.init_workspaces(storage);
    assert!(
        reopened
            .read_workspace()
            .await
            .unwrap()
            .collections
            .is_empty()
    );
}

#[tokio::test]
async fn workspaces_restore_multiple_collections_and_fence_revisions() {
    let directory = tempfile::tempdir().unwrap();
    let storage = directory.path().join("state/workspaces.json");
    let session = Session::default();
    session.init_workspaces(storage.clone());
    let mut roots = vec![];
    let mut revisions = vec![];
    for name in ["one", "two", "three"] {
        let collection = session
            .create_collection(directory.path().into(), name.into(), name.into())
            .await
            .unwrap();
        roots.push(PathBuf::from(collection.root));
        let created = session
            .create_request(CreateRequestInput {
                edits: Vec::new(),
                name: name.into(),
                file_name: "list".into(),
                folder: String::new(),
                method: "GET".into(),
                url: format!("https://example.com/{name}"),
            })
            .await
            .unwrap();
        revisions.push(created.request.revision);
    }
    session.select_collection(roots[0].clone()).await.unwrap();
    assert_eq!(session.read_request("list.yml").await.unwrap().name, "one");
    assert!(
        session
            .save(SaveInput {
                revision: revisions[2],
                edits: vec![]
            })
            .await
            .is_err()
    );
    session.open(roots[0].clone()).await.unwrap();
    assert_eq!(session.read_workspace().await.unwrap().collections.len(), 3);
    let team = session.create_workspace("Team".into()).await.unwrap();
    assert!(team.collections.is_empty());
    session.open(roots[1].clone()).await.unwrap();
    let restored = Session::default();
    restored.init_workspaces(storage.clone());
    let view = restored.read_workspace().await.unwrap();
    assert_eq!(view.active_workspace_id, team.active_workspace_id);
    assert_eq!(view.collections.len(), 1);
    assert_eq!(restored.read_request("list.yml").await.unwrap().name, "two");
    let view = restored.select_workspace(1).await.unwrap();
    assert_eq!(view.collections.len(), 3);
    assert_eq!(restored.read_request("list.yml").await.unwrap().name, "one");
    assert!(
        restored
            .select_collection(directory.path().into())
            .await
            .is_err()
    );
    restored.remove_collection(roots[0].clone()).await.unwrap();
    assert!(roots[0].join("list.yml").is_file());
    assert_eq!(restored.read_request("list.yml").await.unwrap().name, "two");
    std::fs::rename(&roots[2], directory.path().join("moved")).unwrap();
    let view = restored.read_workspace().await.unwrap();
    assert_eq!(view.collections.len(), 2);
    assert!(
        view.collections
            .iter()
            .find(|collection| Path::new(&collection.root) == roots[2])
            .unwrap()
            .warning
            .is_some()
    );
    restored.remove_collection(roots[2].clone()).await.unwrap();
    assert_eq!(
        restored.read_workspace().await.unwrap().collections.len(),
        1
    );
    // References are kept outside collection folders.
    assert!(!roots[1].join("workspaces.json").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn relocated_symlink_cannot_silently_change_registered_root() {
    let directory = tempfile::tempdir().unwrap();
    let session = Session::default();
    let one = session
        .create_collection(directory.path().into(), "One".into(), "one".into())
        .await
        .unwrap();
    let two = session
        .create_collection(directory.path().into(), "Two".into(), "two".into())
        .await
        .unwrap();
    std::fs::rename(&one.root, directory.path().join("moved")).unwrap();
    std::os::unix::fs::symlink(&two.root, &one.root).unwrap();
    let view = session.read_workspace().await.unwrap();
    let unavailable = view
        .collections
        .iter()
        .find(|collection| collection.root == one.root)
        .unwrap();
    assert!(
        unavailable
            .warning
            .as_ref()
            .unwrap()
            .contains("different location")
    );
    assert!(session.select_collection(one.root.into()).await.is_err());
    assert_eq!(
        session
            .read_workspace()
            .await
            .unwrap()
            .active_collection
            .as_deref(),
        Some(two.root.as_str())
    );
}

#[tokio::test]
async fn workspace_failures_do_not_replace_active_collection_or_corrupt_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspaces.json");
    let session = Session::default();
    session.init_workspaces(path.clone());
    session.open_example(example()).await.unwrap();
    let restored = Session::default();
    restored.init_workspaces(path.clone());
    assert!(restored.read_workspace().await.unwrap().collections[0].read_only);
    assert!(
        restored
            .create_folder("".into(), "No".into(), "no".into())
            .await
            .is_err()
    );
    std::fs::write(&path, b"external").unwrap();
    assert!(session.create_workspace("Team".into()).await.is_err());
    assert!(session.rename_workspace(1, "Changed".into()).await.is_err());
    assert!(session.remove_workspace(1).await.is_err());
    assert!(session.read_request("users/list.yml").await.is_ok());
    let error = session
        .create_collection(directory.path().into(), "New".into(), "new".into())
        .await
        .err()
        .unwrap();
    assert!(error.contains("created at"));
    assert!(directory.path().join("new/opencollection.yml").is_file());
    let broken = Session::default();
    broken.init_workspaces(path.clone());
    assert!(broken.read_workspace().await.unwrap().warning.is_some());
    assert!(broken.create_workspace("Team".into()).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"external");
}

#[tokio::test]
async fn sidebar_summaries_read_names_and_methods_without_opening_documents() {
    let directory = tempfile::tempdir().unwrap();
    let session = Session::default();
    let collection = session
        .create_collection(directory.path().into(), "My API".into(), "my-api".into())
        .await
        .unwrap();
    let root = PathBuf::from(&collection.root);
    for (file, name, method) in [
        ("create", "Create User", "POST"),
        ("find", "Find All", "GET"),
        ("update", "Update", "PATCH"),
        ("delete", "Delete", "DELETE"),
    ] {
        session
            .create_request(CreateRequestInput {
                edits: Vec::new(),
                name: name.into(),
                file_name: file.into(),
                method: method.into(),
                url: String::new(),
                folder: String::new(),
            })
            .await
            .unwrap();
    }
    std::fs::write(root.join("broken.yml"), "http: [broken").unwrap();
    std::fs::write(
        directory.path().join("outside.yml"),
        "info: {name: Private, type: http}\nhttp: {method: GET}\n",
    )
    .unwrap();
    let snapshots = session.inner.lock().unwrap().snapshots.len();
    let paths = [
        "create.yml",
        "find.yml",
        "update.yml",
        "delete.yml",
        "broken.yml",
        "../outside.yml",
        "missing.yml",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let summaries = session
        .request_summaries(root.clone(), paths)
        .await
        .unwrap();
    assert_eq!(summaries.len(), 4);
    assert_eq!(summaries["create.yml"].name, "Create User");
    assert_eq!(summaries["create.yml"].method, "POST");
    assert_eq!(summaries["find.yml"].name, "Find All");
    assert_eq!(summaries["update.yml"].method, "PATCH");
    assert_eq!(summaries["delete.yml"].method, "DELETE");
    assert_eq!(session.inner.lock().unwrap().snapshots.len(), snapshots);
    assert!(
        session
            .request_summaries(root.clone(), vec!["find.yml".into(); 129])
            .await
            .is_err()
    );
    assert!(
        session
            .request_summaries(directory.path().into(), vec![])
            .await
            .is_err()
    );
    session.create_workspace("Other".into()).await.unwrap();
    assert!(
        session
            .request_summaries(root, vec!["find.yml".into()])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn creates_and_reopens_request_with_only_a_name() {
    let directory = tempfile::tempdir().unwrap();
    let session = Session::default();
    session
        .create_collection(directory.path().into(), "My API".into(), "my-api".into())
        .await
        .unwrap();
    let input: CreateRequestInput =
        serde_json::from_value(serde_json::json!({"name": "List users"})).unwrap();
    let created = session.create_request(input).await.unwrap();
    assert_eq!(created.path, "list-users.yml");
    assert_eq!(created.request.method, "GET");
    assert_eq!(created.request.url, "");
    assert!(created.request.diagnostics.is_empty());
    session.open(directory.path().join("my-api")).await.unwrap();
    let reopened = session.read_request(&created.path).await.unwrap();
    assert_eq!(reopened.name, "List users");
    assert_eq!(reopened.url, "");
    assert!(
        session
            .send(SendInput {
                path: created.path,
                revision: Some(reopened.revision),
                method: "GET".into(),
                ..Default::default()
            })
            .await
            .is_err()
    );
    let saved = session
        .save(SaveInput {
            revision: reopened.revision,
            edits: vec![FieldEdit {
                path: vec!["http".into(), "url".into()],
                value: Some(serde_json::json!("https://example.com")),
            }],
        })
        .await
        .unwrap();
    assert_eq!(saved.url, "https://example.com");
    assert!(serde_json::from_value::<CreateRequestInput>(serde_json::json!({})).is_err());
}

#[tokio::test]
async fn first_save_keeps_the_entire_untitled_draft_and_failed_creation_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let session = Session::default();
    let collection = session
        .create_collection(directory.path().into(), "API".into(), "api".into())
        .await
        .unwrap();
    session
        .create_folder("".into(), "Events".into(), "events".into())
        .await
        .unwrap();
    let input = serde_json::json!({
        "name": "Create Event", "folder": "events", "method": "POST", "url": "https://example.com/events",
        "edits": [
            {"path":["http","params"],"value":[{"name":"source","value":"dashboard","type":"query"}]},
            {"path":["http","headers"],"value":[{"name":"Accept","value":"application/json"}]},
            {"path":["http","auth"],"value":{"type":"bearer","token":"{{token}}"}},
            {"path":["http","body"],"value":{"type":"json","data":"{\"name\":\"Launch\"}"}},
            {"path":["runtime","variables"],"value":[{"name":"token","value":"placeholder"}]}
        ]
    });
    let saved = session
        .create_request(serde_json::from_value(input.clone()).unwrap())
        .await
        .unwrap();
    assert_eq!(saved.path, "events/create-event.yml");
    let reopened = session.read_request(&saved.path).await.unwrap();
    assert_eq!(reopened.name, "Create Event");
    for edit in input["edits"].as_array().unwrap() {
        let path = edit["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| part.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("/");
        assert_eq!(
            reopened.document.pointer(&format!("/{path}")).unwrap(),
            &edit["value"]
        );
    }
    let disk = tokio::fs::read(Path::new(&collection.root).join(&saved.path))
        .await
        .unwrap();
    assert!(
        session
            .create_request(serde_json::from_value(input.clone()).unwrap())
            .await
            .is_err()
    );
    assert_eq!(
        tokio::fs::read(Path::new(&collection.root).join(&saved.path))
            .await
            .unwrap(),
        disk
    );
    for (name, folder, edits) in [
        (
            "Invalid",
            "events",
            serde_json::json!([{"path":["http","headers"],"value":"invalid"}]),
        ),
        ("Escape", "../outside", serde_json::json!([])),
        (
            "Protected",
            "events",
            serde_json::json!([{"path":["info","type"],"value":"graphql"}]),
        ),
    ] {
        let invalid = serde_json::json!({"name":name,"folder":folder,"edits":edits});
        assert!(
            session
                .create_request(serde_json::from_value(invalid).unwrap())
                .await
                .is_err()
        );
    }
    assert_eq!(
        collection_index(&collection.root).await.unwrap().requests,
        vec!["events/create-event.yml"]
    );
}

#[tokio::test]
async fn sends_untitled_in_memory_with_or_without_a_collection() {
    for with_collection in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let session = Session::default();
        session.init_history(directory.path().join("history.json"));
        let root = if with_collection {
            Some(
                session
                    .create_collection(directory.path().into(), "API".into(), "api".into())
                    .await
                    .unwrap()
                    .root,
            )
        } else {
            None
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        if with_collection {
            let environment = session.create_environment("local".into()).await.unwrap();
            session.save(SaveInput {revision:environment.revision,edits:vec![FieldEdit {
                path:vec!["variables".into()],value:Some(serde_json::json!([
                    {"name":"baseUrl","value":base_url},{"name":"token","secret":true,"type":"string"}
                ]))
            }]}).await.unwrap();
        }
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 2048];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8(request).unwrap().to_lowercase();
            assert!(text.starts_with("get /preview?source=dashboard "));
            assert!(text.contains("accept: application/json"));
            assert!(text.contains("authorization: bearer in-memory-secret"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .await
                .unwrap();
        });
        let response = session.send(SendInput {
            path:"Untitled".into(),method:"GET".into(),url:"{{baseUrl}}/preview".into(),
            base_url:if with_collection {String::new()}else{base_url},
            environment:with_collection.then(||"local".into()),
            secrets:BTreeMap::from([("token".into(),"in-memory-secret".into())]),
            document:Some(serde_json::json!({"info":{"name":"Untitled","type":"http"},"http":{
                "method":"GET","url":"{{baseUrl}}/preview","params":[{"name":"source","value":"dashboard","type":"query"}],
                "headers":[{"name":"Accept","value":"application/json"}],"auth":{"type":"bearer","token":if with_collection {"{{token}}"}else{"in-memory-secret"}}
            }})),..Default::default()
        }).await.unwrap();
        assert_eq!(response.status, 200);
        server.await.unwrap();
        if let Some(root) = root {
            assert!(collection_index(root).await.unwrap().requests.is_empty());
        }
        assert!(!directory.path().join("Untitled.yml").exists());
        let history = tokio::fs::read_to_string(directory.path().join("history.json"))
            .await
            .unwrap();
        assert!(!history.contains("in-memory-secret"));
        assert!(!history.contains("dashboard"));
    }
}

#[tokio::test]
async fn creates_a_collection_and_request_then_saves_sends_and_reopens_it() {
    let parent =
        std::env::temp_dir().join(format!("nimblepost-create-test-{}", std::process::id()));
    std::fs::create_dir(&parent).unwrap();
    let session = Session::default();
    assert!(
        session
            .create_request(CreateRequestInput {
                edits: Vec::new(),
                folder: String::new(),
                name: "Test".into(),
                file_name: "test".into(),
                method: "GET".into(),
                url: "https://example.com".into()
            })
            .await
            .is_err()
    );
    let collection = session
        .create_collection(parent.clone(), "My API".into(), "my-api".into())
        .await
        .unwrap();
    assert_eq!(collection.name, "My API");
    assert!(!collection.read_only);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/users", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let count = socket.read(&mut request).await.unwrap();
        assert!(request[..count].starts_with(b"GET /users HTTP/1.1"));
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]")
            .await
            .unwrap();
    });
    let created = session
        .create_request(CreateRequestInput {
            edits: Vec::new(),
            folder: String::new(),
            name: "List users".into(),
            file_name: "list-users".into(),
            method: "GET".into(),
            url: url.clone(),
        })
        .await
        .unwrap();
    assert_eq!(created.collection.requests, ["list-users.yml"]);
    assert_eq!(created.path, "list-users.yml");
    let saved = session
        .save(SaveInput {
            revision: created.request.revision,
            edits: vec![FieldEdit {
                path: vec!["info".into(), "name".into()],
                value: Some(serde_json::json!("Users")),
            }],
        })
        .await
        .unwrap();
    assert_eq!(saved.name, "Users");
    let response = session
        .send(SendInput {
            path: created.path.clone(),
            revision: Some(saved.revision),
            method: "GET".into(),
            url,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    server.await.unwrap();
    assert_eq!(
        session.open(parent.join("my-api")).await.unwrap().requests,
        ["list-users.yml"]
    );
    assert_eq!(
        session.read_request(&created.path).await.unwrap().name,
        "Users"
    );
    session.open_example(example()).await.unwrap();
    assert!(
        session
            .create_request(CreateRequestInput {
                edits: Vec::new(),
                folder: String::new(),
                name: "Test".into(),
                file_name: "test".into(),
                method: "GET".into(),
                url: "https://example.com".into()
            })
            .await
            .err()
            .unwrap()
            .contains("read-only")
    );
    std::fs::remove_dir_all(parent).unwrap();
}

#[tokio::test]
async fn organizes_nested_requests_and_rejects_read_only_or_expired_revisions() {
    let parent =
        std::env::temp_dir().join(format!("nimblepost-organize-test-{}", std::process::id()));
    std::fs::create_dir(&parent).unwrap();
    let session = Session::default();
    session
        .create_collection(parent.clone(), "API".into(), "api".into())
        .await
        .unwrap();
    let folders = session
        .create_folder("".into(), "Users".into(), "users".into())
        .await
        .unwrap();
    assert_eq!(folders.folders, ["users"]);
    let created = session
        .create_request(CreateRequestInput {
            edits: Vec::new(),
            folder: "users".into(),
            name: "List".into(),
            file_name: "list".into(),
            method: "GET".into(),
            url: "https://example.com".into(),
        })
        .await
        .unwrap();
    assert_eq!(created.path, "users/list.yml");
    let duplicated = session
        .duplicate_request(RenameRequestInput {
            revision: created.request.revision,
            name: "List copy".into(),
            file_name: "list-copy".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        duplicated.collection.requests,
        ["users/list-copy.yml", "users/list.yml"]
    );
    let renamed = session
        .rename_request(RenameRequestInput {
            revision: duplicated.request.revision,
            name: "Renamed".into(),
            file_name: "renamed".into(),
        })
        .await
        .unwrap();
    assert_eq!(renamed.path, "users/renamed.yml");
    assert_eq!(renamed.request.name, "Renamed");
    assert_eq!(
        renamed.collection.requests,
        ["users/list.yml", "users/renamed.yml"]
    );
    assert!(
        session
            .duplicate_request(RenameRequestInput {
                revision: duplicated.request.revision,
                name: "Expired".into(),
                file_name: "expired".into()
            })
            .await
            .err()
            .unwrap()
            .contains("expired")
    );
    let reopened = session.open(parent.join("api")).await.unwrap();
    assert_eq!(reopened.folders, ["users"]);
    assert_eq!(reopened.requests, renamed.collection.requests);
    std::fs::create_dir(parent.join("api/users-extra")).unwrap();
    std::fs::copy(
        parent.join("api/users/list.yml"),
        parent.join("api/users-extra/keep.yml"),
    )
    .unwrap();
    let untouched = session.read_request("users-extra/keep.yml").await.unwrap();
    let affected = session.read_request("users/renamed.yml").await.unwrap();
    let moved = session
        .rename_folder("users".into(), "members".into())
        .await
        .unwrap();
    assert_eq!(moved.folders, ["members", "users-extra"]);
    assert_eq!(
        moved.requests,
        [
            "members/list.yml",
            "members/renamed.yml",
            "users-extra/keep.yml"
        ]
    );
    assert!(
        session
            .draft(affected.revision, &[])
            .err()
            .unwrap()
            .contains("expired")
    );
    assert!(session.draft(untouched.revision, &[]).is_ok());
    assert!(session.read_request("users/renamed.yml").await.is_err());
    assert_eq!(
        session
            .read_request("members/renamed.yml")
            .await
            .unwrap()
            .name,
        "Renamed"
    );
    let read_only = session.open_example(example()).await.unwrap();
    let request = session.read_request(&read_only.requests[0]).await.unwrap();
    assert!(
        session
            .rename_folder("users".into(), "members".into())
            .await
            .err()
            .unwrap()
            .contains("read-only")
    );
    assert!(
        session
            .create_folder("".into(), "Forbidden".into(), "forbidden".into())
            .await
            .err()
            .unwrap()
            .contains("read-only")
    );
    for rename in [true, false] {
        let input = RenameRequestInput {
            revision: request.revision,
            name: "Forbidden".into(),
            file_name: "forbidden".into(),
        };
        let result = if rename {
            session.rename_request(input).await
        } else {
            session.duplicate_request(input).await
        };
        assert!(result.err().unwrap().contains("read-only"));
    }
    std::fs::remove_dir_all(parent).unwrap();
}

#[tokio::test]
async fn uses_core_for_selection_transient_edits_execution_and_chunked_response() {
    let session = Session::default();
    let collection = session.open(example()).await.unwrap();
    assert_eq!(collection.requests, ["users/create.yml", "users/list.yml"]);
    assert_eq!(collection.environments, ["local"]);
    let saved = session.read_request("users/list.yml").await.unwrap();
    assert_eq!(saved.method, "GET");
    assert_eq!(saved.url, "{{baseUrl}}/users");
    assert!(session.read_request("../private.yml").await.is_err());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let size = socket.read(&mut request).await.unwrap();
        assert!(request[..size].starts_with(b"POST /edited?limit=5 HTTP/1.1"));
        socket
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 131072\r\n\r\n")
            .await
            .unwrap();
        socket.write_all(&vec![b'x'; 128 * 1024]).await.unwrap();
    });
    let meta = session
        .send(SendInput {
            path: "users/list.yml".into(),
            environment: Some("local".into()),
            method: "POST".into(),
            url: "{{baseUrl}}/edited".into(),
            base_url: url,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(meta.status, 202);
    session.rename_workspace(1, "Renamed".into()).await.unwrap();
    let serialized = serde_json::to_value(&meta).unwrap();
    assert!(serialized.get("body").is_none());
    assert_eq!(serialized["bodyBytes"], 131072);
    let first = base64::engine::general_purpose::STANDARD
        .decode(session.read_response(meta.id, 0).unwrap())
        .unwrap();
    assert_eq!(first.len(), CHUNK_BYTES);
    assert_eq!(session.read_response(meta.id, 131072).unwrap(), "");
    assert!(session.read_response(meta.id, 131073).is_err());
    assert!(session.read_response(meta.id + 1, 0).is_err());
    assert_eq!(
        session.read_request("users/list.yml").await.unwrap().method,
        "GET"
    );
    assert!(session.inner.lock().unwrap().active.is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn cancel_and_failure_release_the_active_request() {
    let session = Session::default();
    session.open(example()).await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (ready, waiting) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0; 4096];
        assert!(socket.read(&mut buf).await.unwrap() > 0);
        ready.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    let input = SendInput {
        path: "users/list.yml".into(),
        environment: Some("local".into()),
        method: "GET".into(),
        url: "{{baseUrl}}/users".into(),
        base_url: url,
        ..Default::default()
    };
    let cancel = async {
        waiting.await.unwrap();
        assert!(session.select_workspace(1).await.is_err());
        assert!(session.rename_workspace(1, "Changed".into()).await.is_err());
        assert!(session.remove_workspace(1).await.is_err());
        assert!(session.read_workspace().await.is_err());
        assert!(session.select_collection(example()).await.is_err());
        session.cancel();
    };
    let (result, _) = tokio::join!(session.send(input), cancel);
    assert!(result.err().unwrap().contains("cancelada"));
    assert!(session.inner.lock().unwrap().active.is_none());
    let error = session
        .send(SendInput {
            path: "users/create.yml".into(),
            environment: Some("local".into()),
            method: "POST".into(),
            url: "{{baseUrl}}/users".into(),
            base_url: String::new(),
            ..Default::default()
        })
        .await
        .err()
        .unwrap();
    assert!(error.contains("token"));
    assert!(session.inner.lock().unwrap().active.is_none());
    server.abort();
}
#[tokio::test]
async fn saves_drafts_with_conflict_detection_and_never_persists_supplied_secrets() {
    let directory =
        std::env::temp_dir().join(format!("nimblepost-mvp-test-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("users")).unwrap();
    std::fs::create_dir_all(directory.join("environments")).unwrap();
    for relative in [
        "opencollection.yml",
        "users/list.yml",
        "users/create.yml",
        "users/folder.yml",
        "environments/local.yml",
    ] {
        std::fs::copy(example().join(relative), directory.join(relative)).unwrap();
    }
    let session = Session::default();
    session.init_history(directory.join("history.json"));
    session.open(directory.clone()).await.unwrap();
    let view = session.read_request("users/create.yml").await.unwrap();
    let saved_source = std::fs::read_to_string(directory.join("users/create.yml")).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = vec![];
        loop {
            let mut buffer = [0; 1024];
            let count = socket.read(&mut buffer).await.unwrap();
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.ends_with(b"secret body value") {
                break;
            }
        }
        let request = String::from_utf8(bytes).unwrap();
        assert!(request.starts_with("POST /users?query=secret+query HTTP/1.1"));
        assert!(request.contains("authorization: Bearer supplied-secret-token"));
        assert!(request.contains("x-custom: secret header value"));
        socket
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 17\r\n\r\nsecret response!!")
            .await
            .unwrap();
    });
    let edits = vec![
        FieldEdit {
            path: vec!["http".into(), "headers".into()],
            value: Some(
                serde_json::json!([{ "name": "X-Custom", "value": "secret header value" }]),
            ),
        },
        FieldEdit {
            path: vec!["http".into(), "params".into()],
            value: Some(
                serde_json::json!([{ "name": "query", "value": "secret query", "type": "query" }]),
            ),
        },
        FieldEdit {
            path: vec!["http".into(), "body".into()],
            value: Some(serde_json::json!({ "type": "text", "data": "secret body value" })),
        },
    ];
    let response = session
        .send(SendInput {
            path: "users/create.yml".into(),
            environment: Some("local".into()),
            method: "POST".into(),
            url: "{{baseUrl}}/users".into(),
            base_url: address,
            revision: Some(view.revision),
            edits: edits.clone(),
            secrets: BTreeMap::from([("token".into(), "supplied-secret-token".into())]),
            document: None,
        })
        .await
        .unwrap();
    assert_eq!(response.status, 201);
    assert_eq!(
        std::fs::read_to_string(directory.join("users/create.yml")).unwrap(),
        saved_source
    );
    let history = std::fs::read_to_string(directory.join("history.json")).unwrap();
    for forbidden in [
        "supplied-secret-token",
        "secret body value",
        "secret response",
        "secret query",
        "secret header value",
        "baseUrl",
        "127.0.0.1",
    ] {
        assert!(!history.contains(forbidden));
    }
    let saved = session
        .save(SaveInput {
            revision: view.revision,
            edits,
        })
        .await
        .unwrap();
    let after = std::fs::read_to_string(directory.join("users/create.yml")).unwrap();
    assert!(after.contains("{{token}}"));
    assert!(!after.contains("supplied-secret-token"));
    assert_eq!(saved.document["http"]["body"]["data"], "secret body value");
    std::fs::write(directory.join("users/create.yml"), "external edit").unwrap();
    assert!(
        session
            .save(SaveInput {
                revision: saved.revision,
                edits: vec![]
            })
            .await
            .err()
            .unwrap()
            .contains("Conflicto")
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("users/create.yml")).unwrap(),
        "external edit"
    );
    session.clear_history().await.unwrap();
    assert!(session.history().entries.is_empty());
    server.await.unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn included_examples_are_read_only() {
    let session = Session::default();
    assert!(session.open_example(example()).await.unwrap().read_only);
    let request = session.read_request("users/list.yml").await.unwrap();
    assert!(
        session
            .save(SaveInput {
                revision: request.revision,
                edits: vec![]
            })
            .await
            .is_err()
    );
    assert!(session.create_environment("new".into()).await.is_err());
}
