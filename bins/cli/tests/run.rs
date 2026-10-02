use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::Duration,
};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nimblepost"))
}
fn collection() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/basic-http")
}

#[test]
fn runs_the_saved_collection_request_against_a_local_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buf = [0; 1024];
            let count = socket.read(&mut buf).unwrap();
            request.extend_from_slice(&buf[..count]);
            if count == 0 || request.windows(4).any(|part| part == b"\r\n\r\n") {
                break;
            }
        }
        assert!(request.starts_with(b"GET /users?limit=5 HTTP/1.1"));
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nX-Test: local\r\n\r\n{\"users\":[]}",
            )
            .unwrap();
    });
    let output = cli()
        .args(["run", "users/list.yml", "--collection"])
        .arg(collection())
        .args(["--env", "local", "--var", &format!("baseUrl={url}")])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["status"], 200);
    assert_eq!(response["bodyEncoding"], "utf8");
    assert_eq!(response["body"], "{\"users\":[]}");
    assert!(
        response["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["name"] == "x-test" && h["value"] == "local")
    );
    server.join().unwrap();
}

#[test]
fn help_bad_arguments_and_missing_variables_have_useful_exit_codes() {
    assert!(cli().arg("--help").output().unwrap().status.success());
    let output = cli()
        .args(["run", "request.yml", "--env", "local"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let output = cli()
        .arg("run")
        .arg(collection().join("users/list.yml"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("baseUrl"));
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn sigint_cancels_an_inflight_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (ready, receiver) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut data = [0u8; 4096];
        assert!(socket.read(&mut data).unwrap() > 0);
        ready.send(()).unwrap();
        // Client cancellation closes the connection; no response is sent.
        let _ = socket.read(&mut data);
    });
    let mut child = cli()
        .args(["run", "users/list.yml", "--collection"])
        .arg(collection())
        .args(["--env", "local", "--var", &format!("baseUrl={url}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(130));
            break;
        }
        if start.elapsed() > Duration::from_secs(3) {
            child.kill().unwrap();
            panic!("CLI did not cancel");
        }
        thread::sleep(Duration::from_millis(10));
    }
    server.join().unwrap();
}

#[test]
fn declared_secret_is_read_only_from_the_named_process_variable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = vec![];
        loop {
            let mut buffer = [0; 1024];
            let size = socket.read(&mut buffer).unwrap();
            if size == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..size]);
            if bytes.windows(4).any(|slice| slice == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8(bytes).unwrap();
        assert!(request.starts_with("POST /users HTTP/1.1"));
        assert!(request.contains("authorization: Bearer cli-supplied-token"));
        socket
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .unwrap();
    });
    let output = cli()
        .args(["run", "users/create.yml", "--collection"])
        .arg(collection())
        .args([
            "--env",
            "local",
            "--var",
            &format!("baseUrl={url}"),
            "--secret-env",
            "token=NIMBLEPOST_TEST_SECRET",
        ])
        .env("NIMBLEPOST_TEST_SECRET", "cli-supplied-token")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("cli-supplied-token"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["status"],
        202
    );
    server.join().unwrap();
    let output = cli()
        .args(["run", "users/create.yml", "--collection"])
        .arg(collection())
        .args([
            "--env",
            "local",
            "--secret-env",
            "token=NIMBLEPOST_TEST_SECRET",
        ])
        .env_remove("NIMBLEPOST_TEST_SECRET")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}
