use super::*;
use codex_mcp::FileInputSource;
use codex_mcp::FileTransferMode;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_bytes;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test]
fn model_file_ref_accepts_string_and_blob_like_values() {
    assert_eq!(
        model_file_ref(&serde_json::json!("/tmp/file.txt")),
        Some("/tmp/file.txt")
    );
    assert_eq!(
        model_file_ref(&serde_json::json!({"uri": "file:///tmp/file.txt", "name": "file.txt"})),
        Some("file:///tmp/file.txt")
    );
    assert_eq!(
        model_file_ref(&serde_json::json!({"name": "file.txt"})),
        None
    );
}

#[tokio::test]
async fn transfer_rejects_non_https_remote_urls() {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    assert_eq!(
        put_transfer_file(
            &session,
            &codex_mcp::FileTransferDescriptor {
                transport: Some("https".to_string()),
                method: "PUT".to_string(),
                url: "http://example.com/upload".to_string(),
                expires_at: None,
            },
            Vec::new(),
            /*max_size*/ 0,
        )
        .await
        .expect_err("remote HTTP must be rejected"),
        "MCP transfer URL must use HTTPS"
    );
}

#[test]
fn output_file_detection_requires_a_structured_file_value() {
    let mut files = std::collections::HashMap::new();
    collect_output_files(
        &serde_json::json!({
            "file": {"uri": "mcp-file://server/file_1", "name": "report.txt"},
            "text": "mcp-file://server/not-a-file-value"
        }),
        &mut files,
    );
    assert_eq!(files.len(), 1);
    assert!(files.contains_key("mcp-file://server/file_1"));
}

#[test]
fn output_replacement_removes_transport_handle() {
    let mut value = serde_json::json!({
        "nested": [{"uri": "mcp-file://server/file_1", "name": "remote.txt"}]
    });
    replace_output_files(
        &mut value,
        &std::collections::HashMap::from([(
            "mcp-file://server/file_1".to_string(),
            serde_json::json!({"uri": "file:///tmp/local.txt", "name": "local.txt"}),
        )]),
    );
    assert_eq!(
        value,
        serde_json::json!({
            "nested": [{"uri": "file:///tmp/local.txt", "name": "local.txt"}]
        })
    );
}

#[test]
fn sanitizes_download_filenames() {
    assert_eq!(
        sanitize_filename("../../report name.txt"),
        ".._.._report_name.txt"
    );
    assert_eq!(sanitize_filename(".."), "download");
}

#[test]
fn rejects_expired_transfer_descriptors() {
    let error = validated_transfer_descriptor(
        &codex_mcp::FileTransferDescriptor {
            transport: Some("https".to_string()),
            method: "GET".to_string(),
            url: "https://example.com/file".to_string(),
            expires_at: Some("2020-01-01T00:00:00Z".to_string()),
        },
        "GET",
    )
    .expect_err("expired descriptors must fail");
    assert_eq!(error, "MCP transfer descriptor has expired");
}

#[test]
fn matches_exact_and_wildcard_mime_types() {
    assert!(mime_matches("text/plain", "text/plain"));
    assert!(mime_matches("text/*", "text/csv"));
    assert!(mime_matches("*/*", "application/pdf"));
    assert!(!mime_matches("image/*", "text/plain"));
}

#[test]
fn validates_opaque_mcp_file_uris() {
    assert_eq!(validate_mcp_file_uri("mcp-file://server/file_1"), Ok(()));
    assert_eq!(
        validate_mcp_file_uri("https://example.com/signed?secret=value"),
        Err("MCP file response returned an invalid file URI".to_string())
    );
}

#[test]
fn rejects_non_public_transfer_addresses() {
    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "100.64.0.1",
        "169.254.169.254",
        "192.168.0.1",
        "198.18.0.1",
        "224.0.0.1",
        "::1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
        "::ffff:127.0.0.1",
    ] {
        assert!(is_disallowed_transfer_address(
            address.parse().expect("IP address")
        ));
    }
    for address in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
        assert!(!is_disallowed_transfer_address(
            address.parse().expect("IP address")
        ));
    }
}

#[test]
fn direct_transfers_require_trusted_storage_hosts() {
    assert!(is_trusted_direct_transfer_host(
        "account.blob.core.windows.net"
    ));
    assert!(is_trusted_direct_transfer_host("files.oaiusercontent.com"));
    assert!(is_trusted_direct_transfer_host("FILES.OAIUSERCONTENT.COM."));
    assert!(!is_trusted_direct_transfer_host("oaiusercontent.com"));
    assert!(!is_trusted_direct_transfer_host(
        "files.oaiusercontent.com.attacker.example"
    ));
    assert!(!is_trusted_direct_transfer_host("example.com"));
}

#[tokio::test]
async fn inline_rewrite_turns_a_local_path_into_a_data_uri() {
    let (session, mut turn_context) = crate::session::tests::make_session_and_context().await;
    turn_context.permission_profile = codex_protocol::models::PermissionProfile::Disabled;
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("report.txt");
    tokio::fs::write(&path, b"hello")
        .await
        .expect("write test file");
    let spec = FileInputSpec {
        path: "file".to_string(),
        accepts: vec!["text/plain".to_string()],
        max_size: Some(32),
        transfer_modes: BTreeSet::from([FileTransferMode::Inline]),
        sources: BTreeSet::from([FileInputSource::Mcp]),
        is_array: false,
    };

    let rewritten = rewrite_mcp_file_arguments(
        &session,
        &turn_context,
        "test",
        Some(serde_json::json!({"file": path})),
        &[spec],
    )
    .await
    .expect("inline rewrite succeeds");

    assert_eq!(
        rewritten,
        Some(serde_json::json!({
            "file": "data:text/plain;base64,aGVsbG8="
        }))
    );
}

#[tokio::test]
async fn upload_transfer_streams_the_exact_file_bytes() {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/upload"))
        .and(header("content-length", "9"))
        .and(body_bytes(b"stream me"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    put_transfer_file(
        &session,
        &codex_mcp::FileTransferDescriptor {
            transport: Some("https".to_string()),
            method: "PUT".to_string(),
            url: format!("{}/upload", server.uri()),
            expires_at: None,
        },
        b"stream me".to_vec(),
        /*max_size*/ 32,
    )
    .await
    .expect("upload succeeds");
}

#[tokio::test]
async fn upload_transfer_accepts_post_descriptors() {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload"))
        .and(header("content-length", "9"))
        .and(body_bytes(b"stream me"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;
    put_transfer_file(
        &session,
        &codex_mcp::FileTransferDescriptor {
            transport: None,
            method: "POST".to_string(),
            url: format!("{}/upload", server.uri()),
            expires_at: None,
        },
        b"stream me".to_vec(),
        /*max_size*/ 32,
    )
    .await
    .expect("upload succeeds");
}

#[tokio::test]
async fn download_transfer_materializes_exact_bytes() {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"download me"))
        .expect(1)
        .mount(&server)
        .await;
    let directory = tempdir().expect("temp dir");
    let output = directory.path().join("download.txt");

    let size = download_transfer_file(
        &session,
        &codex_mcp::FileTransferDescriptor {
            transport: Some("https".to_string()),
            method: "GET".to_string(),
            url: format!("{}/download", server.uri()),
            expires_at: None,
        },
        &output,
        /*max_size*/ 32,
    )
    .await
    .expect("download succeeds");

    assert_eq!(size, 11);
    assert_eq!(
        tokio::fs::read(&output).await.expect("read download"),
        b"download me"
    );
}

#[tokio::test]
async fn transfer_errors_do_not_expose_signed_urls() {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/download"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let directory = tempdir().expect("temp dir");
    let output = directory.path().join("download.txt");
    let secret = "sensitive-signature";

    let error = download_transfer_file(
        &session,
        &codex_mcp::FileTransferDescriptor {
            transport: None,
            method: "GET".to_string(),
            url: format!("{}/download?sig={secret}", server.uri()),
            expires_at: None,
        },
        &output,
        /*max_size*/ 32,
    )
    .await
    .expect_err("failed transfer");

    assert_eq!(
        error,
        "MCP download transfer returned HTTP 500 Internal Server Error"
    );
    assert!(!error.contains(secret));
}

#[tokio::test]
async fn failed_download_removes_partial_file() {
    let (session, _) = crate::session::tests::make_session_and_context().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 33]))
        .mount(&server)
        .await;
    let directory = tempdir().expect("temp dir");
    let output = directory.path().join("download.txt");

    let error = download_transfer_file(
        &session,
        &codex_mcp::FileTransferDescriptor {
            transport: None,
            method: "GET".to_string(),
            url: format!("{}/download", server.uri()),
            expires_at: None,
        },
        &output,
        /*max_size*/ 32,
    )
    .await
    .expect_err("oversized transfer");

    assert_eq!(error, "MCP download exceeds the 32-byte limit");
    assert!(!output.with_extension("part").exists());
}
