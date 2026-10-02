use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use nimblepost_core::{
    CancellationToken, Document, DocumentKind, Error, ExecutionContext, LoadedRequest,
    NetworkError, execute, load_request, prepare,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
    task::JoinHandle,
};

enum Stall {
    None,
    Headers,
    Body,
}

struct Server {
    url: String,
    received: oneshot::Receiver<Vec<u8>>,
    response_written: oneshot::Receiver<()>,
    task: JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server(response: &[u8], stall: Stall) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, received) = oneshot::channel();
    let (written_sender, response_written) = oneshot::channel();
    let response = response.to_vec();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let read = socket.read(&mut buf).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buf[..read]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map(|value| value.parse::<usize>().unwrap())
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let _ = sender.send(request);
        if matches!(stall, Stall::Headers) {
            std::future::pending::<()>().await;
        }
        socket.write_all(&response).await.unwrap();
        let _ = written_sender.send(());
        if matches!(stall, Stall::Body) {
            std::future::pending::<()>().await;
        }
    });
    Server {
        url,
        received,
        response_written,
        task,
    }
}

fn request(yaml: &str) -> LoadedRequest {
    LoadedRequest::standalone(Document::from_yaml("request.yml", yaml).unwrap()).unwrap()
}

fn basic_yaml(extra: &str) -> String {
    format!(
        "info:\n  name: Test\n  type: http\nhttp:\n  method: GET\n  url: '{{{{baseUrl}}}}/users'\n{extra}"
    )
}

fn context(url: &str) -> ExecutionContext {
    ExecutionContext {
        overrides: BTreeMap::from([("baseUrl".into(), url.into())]),
        ..Default::default()
    }
}

async fn run(yaml: &str, url: &str) -> nimblepost_core::Result<nimblepost_core::HttpResponse> {
    execute(
        prepare(&request(yaml), &context(url))?,
        &CancellationToken::new(),
    )
    .await
}

struct TempCollection(PathBuf);

impl TempCollection {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("nimblepost-core-{}-{unique}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, file: &str, content: &str) {
        let path = self.0.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
}

impl Drop for TempCollection {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn rust_validates_the_same_real_fixtures_as_the_contract_tooling() {
    let fixture_root = repo().join("tests/fixtures/opencollection");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture_root.join("manifest.json")).unwrap())
            .unwrap();
    for fixture in manifest["fixtures"].as_array().unwrap() {
        let path = fixture_root.join(fixture["path"].as_str().unwrap());
        let bytes = std::fs::read(&path).unwrap();
        let doc = Document::from_yaml(&path, String::from_utf8(bytes.clone()).unwrap()).unwrap();
        assert_eq!(doc.original().as_bytes(), bytes);
        let kind = match fixture["definition"].as_str() {
            None => DocumentKind::Collection,
            Some("HttpRequest") => DocumentKind::HttpRequest,
            Some("Environment") => DocumentKind::Environment,
            Some("Folder") => DocumentKind::Folder,
            _ => unreachable!(),
        };
        assert_eq!(
            doc.validate(kind).is_ok(),
            fixture["schemaValid"].as_bool().unwrap(),
            "{}",
            path.display()
        );
    }
}

#[test]
fn invalid_yaml_is_rejected_without_echoing_content() {
    let error = Document::from_yaml("bad.yml", "password: [TOP_SECRET")
        .err()
        .unwrap();
    assert!(matches!(error, Error::Yaml { .. }));
    assert!(!format!("{error:?} {error}").contains("TOP_SECRET"));
    for yaml in [
        "a: 1\na: 2",
        "a: &token secret\nb: *token",
        "a: !foo secret",
        "%YAML 1.1\n---\na: yes",
        "a: 1\n---\na: 2",
        "1: foo",
        "a:\n  <<: foo",
    ] {
        assert!(Document::from_yaml("bad.yml", yaml).is_err(), "{yaml}");
    }
    let source = format!("a: {}0{}", "[".repeat(70), "]".repeat(70));
    assert!(Document::from_yaml("deep.yml", source).is_err());
    assert!(Document::from_yaml("big.yml", "x".repeat(8 * 1024 * 1024 + 1)).is_err());
    // YAML 1.2 treats these words as strings, unlike YAML 1.1.
    Document::from_yaml(
        "valid.yml",
        "info: {name: No, type: http}\nhttp: {method: GET, url: 'http://localhost'}",
    )
    .unwrap()
    .validate(DocumentKind::HttpRequest)
    .unwrap();
}

#[test]
fn missing_malformed_recursive_and_duplicate_variables_fail_before_network() {
    let loaded = request(&basic_yaml(""));
    assert!(
        matches!(prepare(&loaded, &ExecutionContext::default()), Err(Error::MissingVariable { name, .. }) if name == "baseUrl")
    );
    for value in ["http://{{nested}}", "http://localhost/}}"] {
        assert!(prepare(&loaded, &context(value)).is_err());
    }
    for url in [
        "{{baseUrl",
        "{{}}",
        "{{bad name}}",
        "{{baseUrl}}/}}",
        "file:///tmp/test",
        "http://user:password@localhost/",
    ] {
        let yaml = basic_yaml("").replace("{{baseUrl}}/users", url);
        assert!(prepare(&request(&yaml), &context("http://localhost")).is_err());
    }
    let yaml = basic_yaml(
        "runtime:\n  variables:\n    - {name: baseUrl, value: 'http://localhost'}\n    - {name: baseUrl, value: 'http://localhost'}\n",
    );
    assert!(prepare(&request(&yaml), &ExecutionContext::default()).is_err());
}

#[test]
fn unsupported_active_configuration_is_not_silently_ignored() {
    for extra in [
        "runtime:\n  scripts: [{type: before-request, code: 'throw 1'}]",
        "runtime:\n  assertions: [{expression: res.status, operator: equals, value: '200'}]",
        "settings: {timeout: 0}",
        "settings: {timeout: 1.5}",
        "settings: {maxRedirects: 21}",
        "settings: {encodeUrl: false}",
        "settings: {forwardAuthorizationHeader: true}",
        "runtime:\n  variables: [{name: x, value: {type: number, data: '1'}}]",
    ] {
        assert!(
            prepare(&request(&basic_yaml(extra)), &context("http://localhost")).is_err(),
            "{extra}"
        );
    }
    let yaml = basic_yaml("").replace("  url:", "  unknown: secret\n  url:");
    let doc = Document::from_yaml("request.yml", &yaml).unwrap();
    assert_eq!(doc.original(), yaml);
    assert!(LoadedRequest::standalone(doc).is_err());
}

#[tokio::test]
async fn get_returns_status_duplicate_headers_and_binary_body() {
    let mut response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 3\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\nConnection: close\r\n\r\n".to_vec();
    response.extend_from_slice(&[0, 255, 65]);
    let mut mock = server(&response, Stall::None).await;
    let result = run(&basic_yaml(""), &mock.url).await.unwrap();
    assert_eq!(result.status, 404);
    assert_eq!(result.body, [0, 255, 65]);
    assert_eq!(
        result
            .headers
            .iter()
            .filter(|header| header.name == "set-cookie")
            .count(),
        2
    );
    let sent = (&mut mock.received).await.unwrap();
    assert!(sent.starts_with(b"GET /users HTTP/1.1\r\n"));
}

#[tokio::test]
async fn post_interpolates_body_query_and_bearer_auth() {
    let mut mock = server(
        b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\n{}",
        Stall::None,
    )
    .await;
    let yaml = "info: {name: Create, type: http}\nhttp:\n  method: POST\n  url: '{{ baseUrl }}/users?existing=1'\n  params:\n    - {name: tag, value: 'a b&c', type: query}\n    - {name: tag, value: second, type: query}\n    - {name: ignore, value: '{{missing}}', type: query, disabled: true}\n  auth: {type: bearer, token: '{{token}}'}\n  body: {type: json, data: '{\"name\":\"{{name}}\"}'}\n";
    let mut ctx = context(&mock.url);
    ctx.overrides.insert("token".into(), "private-token".into());
    ctx.overrides.insert("name".into(), "Ada".into());
    let prepared = prepare(&request(yaml), &ctx).unwrap();
    let result = execute(prepared, &CancellationToken::new()).await.unwrap();
    assert_eq!(result.status, 201);
    let sent = String::from_utf8((&mut mock.received).await.unwrap()).unwrap();
    assert!(sent.starts_with("POST /users?existing=1&tag=a+b%26c&tag=second HTTP/1.1"));
    assert!(sent.contains("authorization: Bearer private-token\r\n"));
    assert!(sent.contains("content-type: application/json\r\n"));
    assert!(sent.ends_with("{\"name\":\"Ada\"}"));
}

#[tokio::test]
async fn basic_and_api_key_auth_are_sent_and_manual_collisions_are_rejected() {
    for (auth, expected) in [
        (
            "{type: basic, username: user, password: pass}",
            "authorization: Basic dXNlcjpwYXNz\r\n",
        ),
        (
            "{type: apikey, key: X-Api-Key, value: key, placement: header}",
            "x-api-key: key\r\n",
        ),
        (
            "{type: apikey, key: api_key, value: 'a b', placement: query}",
            "GET /users?api_key=a+b HTTP/1.1\r\n",
        ),
    ] {
        let mut mock = server(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n", Stall::None).await;
        let yaml =
            basic_yaml("").replace("  method: GET", &format!("  auth: {auth}\n  method: GET"));
        assert_eq!(run(&yaml, &mock.url).await.unwrap().status, 200);
        let sent = String::from_utf8((&mut mock.received).await.unwrap()).unwrap();
        assert!(sent.contains(expected), "{sent}");
    }
    for yaml in [
        "info: {type: http}\nhttp: {method: GET, url: 'http://localhost', auth: {type: bearer, token: secret}, headers: [{name: authorization, value: manual}]}",
        "info: {type: http}\nhttp: {method: GET, url: 'http://localhost?key=manual', auth: {type: apikey, key: key, value: secret, placement: query}}",
        "info: {type: http}\nhttp: {method: GET, url: 'http://localhost', headers: [{name: Content-Length, value: '99'}]}",
        "info: {type: http}\nhttp: {method: POST, url: 'http://localhost', body: {type: json, data: '{invalid'}}",
    ] {
        assert!(prepare(&request(yaml), &ExecutionContext::default()).is_err());
    }
}

#[tokio::test]
async fn collection_environment_folder_request_and_override_precedence() {
    let mut mock = server(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n", Stall::None).await;
    let temp = TempCollection::new();
    temp.write("opencollection.yml", "opencollection: '1.0.0'\ninfo: {name: Test}\nrequest:\n  variables: [{name: baseUrl, value: 'http://collection.invalid'}, {name: limit, value: '1'}]\n  headers: [{name: Accept, value: old}, {name: X-Removed, value: old}]\n  settings: {http: {timeout: 5000}}\n");
    temp.write("users/folder.yml", "info: {name: Users, type: folder}\nrequest:\n  variables: [{name: baseUrl, value: 'http://folder.invalid'}, {name: limit, value: '2'}]\n");
    temp.write("environments/dev.yml", &format!("name: dev\nvariables:\n  - {{name: baseUrl, value: '{}'}}\n  - {{name: token, secret: true, type: string}}\n", mock.url));
    temp.write("users/list.yml", "info: {name: List, type: http}\nhttp:\n  method: GET\n  url: '{{baseUrl}}/users'\n  params: [{name: limit, value: '{{limit}}', type: query}]\n  headers:\n    - {name: ACCEPT, value: new}\n    - {name: x-removed, value: ignored, disabled: true}\n    - {name: X-Duplicate, value: one}\n    - {name: X-Duplicate, value: two}\nruntime:\n  variables: [{name: limit, value: '3'}]\nsettings: {timeout: inherit}\n");
    let loaded = load_request(&temp.0, "users/list.yml", Some("dev"))
        .await
        .unwrap();
    let before = std::fs::read(temp.0.join("users/list.yml")).unwrap();
    let prepared = prepare(&loaded, &ExecutionContext::default()).unwrap();
    assert_eq!(prepared.variable_origins()["baseUrl"].scope, "environment");
    assert_eq!(prepared.variable_origins()["limit"].scope, "request");
    assert!(prepared.variable_origins()["token"].secret);
    let result = execute(prepared, &CancellationToken::new()).await.unwrap();
    assert_eq!(result.status, 200);
    let sent = String::from_utf8((&mut mock.received).await.unwrap()).unwrap();
    assert!(sent.starts_with("GET /users?limit=3 HTTP/1.1"));
    assert!(sent.contains("accept: new\r\n"));
    assert!(!sent.contains("x-removed:"));
    assert_eq!(sent.matches("x-duplicate:").count(), 2);
    assert_eq!(
        before,
        std::fs::read(temp.0.join("users/list.yml")).unwrap()
    );
    let override_ctx = ExecutionContext {
        overrides: BTreeMap::from([("baseUrl".into(), "http://runtime.invalid".into())]),
        ..Default::default()
    };
    assert_eq!(
        prepare(&loaded, &override_ctx).unwrap().variable_origins()["baseUrl"].scope,
        "runtime"
    );
}

#[tokio::test]
async fn timeout_applies_before_headers_and_while_reading_body() {
    for stage in [Stall::Headers, Stall::Body] {
        let mock = server(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nx", stage).await;
        let result = run(&basic_yaml("settings: {timeout: 80}"), &mock.url).await;
        assert!(matches!(result, Err(Error::Timeout { timeout_ms: 80 })));
    }
}

#[tokio::test]
async fn cancellation_applies_before_headers_and_while_reading_body() {
    for stage in [Stall::Headers, Stall::Body] {
        let during_body = matches!(stage, Stall::Body);
        let mut mock = server(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nx", stage).await;
        let prepared = prepare(
            &request(&basic_yaml("settings: {timeout: 5000}")),
            &context(&mock.url),
        )
        .unwrap();
        let token = CancellationToken::new();
        let trigger = token.clone();
        let task = tokio::spawn(async move { execute(prepared, &token).await });
        tokio::time::timeout(Duration::from_secs(2), &mut mock.received)
            .await
            .unwrap()
            .unwrap();
        if during_body {
            (&mut mock.response_written).await.unwrap();
            // Let the HTTP task consume headers and reach the deliberately incomplete body.
            tokio::time::sleep(Duration::from_millis(20)).await;
            assert!(!task.is_finished());
        }
        trigger.cancel();
        let result = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(result, Err(Error::Cancelled)));
    }
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let prepared = prepare(&request(&basic_yaml("")), &context("http://127.0.0.1:1")).unwrap();
    assert!(matches!(
        execute(prepared, &cancelled).await,
        Err(Error::Cancelled)
    ));
}

#[tokio::test]
async fn connection_failure_is_readable_and_does_not_leak_url_values() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let yaml = basic_yaml("").replace("/users", "/users?token=TOP_SECRET");
    let error = run(&yaml, &url).await.err().unwrap();
    assert!(matches!(error, Error::Network(NetworkError::Connect)));
    assert!(!format!("{error:?} {error}").contains("TOP_SECRET"));
}

#[tokio::test]
async fn oversized_and_truncated_bodies_are_errors() {
    let mock = server(
        b"HTTP/1.1 200 OK\r\nContent-Length: 16777217\r\n\r\n",
        Stall::Body,
    )
    .await;
    assert!(matches!(
        run(&basic_yaml(""), &mock.url).await,
        Err(Error::BodyTooLarge { .. })
    ));
    let mock = server(
        b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nx",
        Stall::None,
    )
    .await;
    assert!(matches!(
        run(&basic_yaml(""), &mock.url).await,
        Err(Error::Network(NetworkError::Response))
    ));
}

#[tokio::test]
async fn cross_origin_redirect_returns_3xx_without_sending_credentials_to_target() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = format!("http://{}/target", target.local_addr().unwrap());
    let response =
        format!("HTTP/1.1 302 Found\r\nLocation: {destination}\r\nContent-Length: 0\r\n\r\n");
    let mock = server(response.as_bytes(), Stall::None).await;
    let yaml = "info: {name: Auth, type: http}\nhttp:\n  method: GET\n  url: '{{baseUrl}}/users'\n  auth: {type: apikey, key: X-Credential, value: secret, placement: header}\n";
    assert_eq!(run(yaml, &mock.url).await.unwrap().status, 302);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), target.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn file_loader_rejects_wrong_versions_missing_environments_and_path_escape() {
    let temp = TempCollection::new();
    temp.write(
        "opencollection.yml",
        "opencollection: '2.0.0'\ninfo: {name: Test}\n",
    );
    temp.write("request.yml", &basic_yaml(""));
    assert!(load_request(&temp.0, "request.yml", None).await.is_err());
    temp.write(
        "opencollection.yml",
        "opencollection: '1.0.0'\ninfo: {name: Test}\n",
    );
    assert!(load_request(&temp.0, "../request.yml", None).await.is_err());
    assert!(
        load_request(&temp.0, "request.yml", Some("missing"))
            .await
            .is_err()
    );
    temp.write("environments/a.yml", "name: dev\nvariables: []\n");
    temp.write("environments/b.yml", "name: dev\nvariables: []\n");
    assert!(
        load_request(&temp.0, "request.yml", Some("dev"))
            .await
            .is_err()
    );
    #[cfg(unix)]
    {
        let outside = TempCollection::new();
        outside.write("request.yml", &basic_yaml(""));
        std::os::unix::fs::symlink(outside.0.join("request.yml"), temp.0.join("escape.yml"))
            .unwrap();
        assert!(load_request(&temp.0, "escape.yml", None).await.is_err());
    }
}

#[tokio::test]
async fn examples_load_and_secrets_remain_outside_source_documents() {
    let root = repo().join("examples/basic-http");
    let loaded = load_request(&root, "users/create.yml", Some("local"))
        .await
        .unwrap();
    assert!(
        matches!(prepare(&loaded, &ExecutionContext::default()), Err(Error::MissingVariable { name, .. }) if name == "token")
    );
    let context = ExecutionContext {
        secrets: BTreeMap::from([("token".into(), "TOP_SECRET".into())]),
        ..Default::default()
    };
    let prepared = prepare(&loaded, &context).unwrap();
    assert!(prepared.variable_origins()["token"].secret);
    assert!(!loaded.request.original().contains("TOP_SECRET"));
    assert!(loaded.request.original().contains("{{token}}"));
    let mut override_ctx = ExecutionContext::default();
    override_ctx
        .overrides
        .insert("token".into(), "OTHER_SECRET".into());
    assert!(prepare(&loaded, &override_ctx).unwrap().variable_origins()["token"].secret);
    let loaded = load_request(&root, "users/list.yml", Some("local"))
        .await
        .unwrap();
    prepare(&loaded, &ExecutionContext::default()).unwrap();
}
