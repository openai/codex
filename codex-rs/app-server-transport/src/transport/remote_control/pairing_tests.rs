use super::pairing::RemoteControlPairingClient;
use super::protocol::RemoteControlTarget;
use super::protocol::StartRemoteControlPairingRequest;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use pretty_assertions::assert_eq;
use serde_json::json;
use time::OffsetDateTime;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;

#[tokio::test]
async fn start_remote_control_pairing_uses_server_token_and_maps_response() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let pair_url = format!(
        "http://{}/backend-api/wham/remote/control/server/pair",
        listener.local_addr().expect("listener should have addr")
    );
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("request should arrive");
        let mut reader = BufReader::new(stream);

        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .await
            .expect("request line should read");
        assert_eq!(
            request_line.trim_end(),
            "POST /backend-api/wham/remote/control/server/pair HTTP/1.1"
        );

        let mut authorization = None;
        let mut content_length = None;
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("header line should read");
            if line == "\r\n" {
                break;
            }
            let (name, value) = line
                .trim_end()
                .split_once(": ")
                .expect("header should split");
            match name.to_ascii_lowercase().as_str() {
                "authorization" => authorization = Some(value.to_string()),
                "content-length" => {
                    content_length =
                        Some(value.parse::<usize>().expect("content length should parse"))
                }
                _ => {}
            }
        }
        assert_eq!(
            authorization,
            Some("Bearer remote-control-token".to_string())
        );

        let mut body = vec![0; content_length.expect("request should have body")];
        reader
            .read_exact(&mut body)
            .await
            .expect("request body should read");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body)
                .expect("request body should be json"),
            json!({ "manual_code": true })
        );

        let response_body = json!({
            "pairing_code": "pairing-code",
            "manual_pairing_code": "ABCD-EFGH",
            "server_id": "server-id",
            "environment_id": "environment-id",
            "expires_at": "3026-05-22T12:34:56Z",
        })
        .to_string();
        reader
            .get_mut()
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{response_body}",
                    response_body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("response should write");
    });
    let client = RemoteControlPairingClient::new(
        &RemoteControlTarget {
            websocket_url: "ws://unused".to_string(),
            enroll_url: "http://unused".to_string(),
            refresh_url: "http://unused".to_string(),
            pair_url,
        },
        "remote-control-token".to_string(),
        OffsetDateTime::from_unix_timestamp(33_336_362_096).expect("future timestamp should parse"),
    );

    let response = client
        .start(StartRemoteControlPairingRequest { manual_code: true })
        .await
        .expect("pairing should succeed");
    server_task.await.expect("server task should finish");

    assert_eq!(
        response,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "environment-id".to_string(),
            expires_at: 33_336_362_096,
        }
    );
}

#[tokio::test]
async fn start_remote_control_pairing_preserves_backend_error_context() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let pair_url = format!(
        "http://{}/backend-api/wham/remote/control/server/pair",
        listener.local_addr().expect("listener should have addr")
    );
    let expected_pair_url = pair_url.clone();
    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("request should arrive");
        let mut reader = BufReader::new(stream);

        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("request line should read");
            if line == "\r\n" {
                break;
            }
        }

        let response_body = "pairing unavailable";
        reader
            .get_mut()
            .write_all(
                format!(
                    "HTTP/1.1 503 Service Unavailable\r\nx-request-id: request-123\r\ncf-ray: ray-123\r\ncontent-length: {}\r\n\r\n{response_body}",
                    response_body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("response should write");
    });
    let client = RemoteControlPairingClient::new(
        &RemoteControlTarget {
            websocket_url: "ws://unused".to_string(),
            enroll_url: "http://unused".to_string(),
            refresh_url: "http://unused".to_string(),
            pair_url,
        },
        "remote-control-token".to_string(),
        OffsetDateTime::from_unix_timestamp(33_336_362_096).expect("future timestamp should parse"),
    );

    let err = client
        .start(StartRemoteControlPairingRequest { manual_code: false })
        .await
        .expect_err("pairing should fail");
    server_task.await.expect("server task should finish");

    assert_eq!(
        err.to_string(),
        format!(
            "remote control pairing failed at `{expected_pair_url}`: HTTP 503 Service Unavailable, request-id: request-123, cf-ray: ray-123, body: pairing unavailable"
        )
    );
}
