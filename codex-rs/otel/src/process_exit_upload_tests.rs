use super::*;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::fs;
use std::io::Cursor;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::mpsc;
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn upload_statsig_metrics_sends_exact_body_and_headers() {
    let server = MockServer::start().await;
    let body = serde_json::json!({"resourceMetrics": []});
    Mock::given(method("POST"))
        .and(path("/v1/metrics"))
        .and(header("content-type", "application/json"))
        .and(header(STATSIG_API_KEY_HEADER, STATSIG_API_KEY))
        .and(body_json(&body))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let endpoint = format!("{}/v1/metrics", server.uri());
    let body = body.to_string();

    tokio::task::spawn_blocking(move || upload_statsig_metrics(Cursor::new(body), &endpoint))
        .await
        .expect("join upload task")
        .expect("upload metrics");
}

#[cfg(unix)]
#[test]
fn detached_uploader_outlives_parent_process() {
    const CHILD_ENV: &str = "CODEX_TEST_DETACHED_STATSIG_UPLOAD";
    if let Some(executable) = std::env::var_os(CHILD_ENV) {
        let mut payload = tempfile::tempfile().expect("create upload payload");
        payload.write_all(b"detached payload").unwrap();
        payload.seek(SeekFrom::Start(0)).unwrap();
        spawn_uploader(executable.into(), payload).expect("spawn detached uploader");
        std::process::exit(0);
    }

    let temp_dir = tempfile::tempdir().expect("create temporary directory");
    let executable = temp_dir.path().join("upload-helper");
    let output = temp_dir.path().join("upload-helper.output");
    fs::write(&executable, "#!/bin/sh\nsleep 0.2\ncat > \"$0.output\"\n")
        .expect("write upload helper");
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make upload helper executable");

    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process_exit_upload::tests::detached_uploader_outlives_parent_process",
        ])
        .env(CHILD_ENV, &executable)
        .status()
        .expect("run uploader parent process");
    assert!(status.success());

    for _ in 0..200 {
        if fs::read(&output).is_ok_and(|body| body == b"detached payload") {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("detached uploader did not finish after its parent exited");
}

#[test]
fn process_exit_client_does_not_fall_back_when_helper_spawn_fails() {
    let upload = StatsigUpload::new();
    upload.configure_executable(PathBuf::from("/path/that/does/not/exist/codex"));
    let client = StatsigUploadClient::new(reqwest::blocking::Client::new(), upload);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("build current-thread runtime");

    let result = runtime.block_on(client.send_bytes(statsig_request()));
    let retry = runtime.block_on(client.send_bytes(statsig_request()));

    assert!(result.is_err());
    assert!(retry.is_err());
}

#[cfg(unix)]
#[test]
fn statsig_uploader_backpressures_normal_uploads_and_reserves_exit_helpers() {
    let temp_dir = tempfile::tempdir().expect("create temporary directory");
    let executable = temp_dir.path().join("upload-helper");
    fs::write(&executable, "#!/bin/sh\ncat >/dev/null\nsleep 1\n").expect("write upload helper");
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).expect("make upload helper executable");

    let upload = StatsigUpload::new();
    upload.configure_executable(executable);
    let client = Arc::new(StatsigUploadClient::new(
        reqwest::blocking::Client::new(),
        upload.clone(),
    ));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("build current-thread runtime");

    let first = runtime.block_on(client.send_bytes(statsig_request()));
    assert!(first.is_ok());

    let (sender, receiver) = mpsc::channel();
    let waiting_client = Arc::clone(&client);
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread runtime");
        sender
            .send(runtime.block_on(waiting_client.send_bytes(statsig_request())))
            .expect("send waiting upload result");
    });
    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());

    assert!(upload.prepare());
    let waiting_upload = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("waiting upload should be handed off");
    let final_upload = runtime.block_on(client.send_bytes(statsig_request()));
    let over_limit = runtime.block_on(client.send_bytes(statsig_request()));
    assert!(waiting_upload.is_ok());
    assert!(final_upload.is_ok());
    assert!(over_limit.is_err());
}

#[test]
fn upload_statsig_metrics_rejects_oversized_body() {
    let result = upload_statsig_metrics(
        Cursor::new(vec![0; MAX_UPLOAD_BYTES + 1]),
        "http://127.0.0.1:1/v1/metrics",
    );

    assert_eq!(
        result.unwrap_err().to_string(),
        format!("process-exit metrics payload exceeded the {MAX_UPLOAD_BYTES}-byte limit")
    );
}

fn statsig_request() -> Request<Bytes> {
    Request::builder()
        .method(http::Method::POST)
        .uri(STATSIG_OTLP_HTTP_ENDPOINT)
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from_static(br#"{"resourceMetrics":[]}"#))
        .expect("build Statsig request")
}
