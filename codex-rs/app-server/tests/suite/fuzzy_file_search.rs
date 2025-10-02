use app_test_support::McpProcess;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_sorts_and_includes_indices() {
    // Prepare a temporary Codex home and a separate root with test files.
    let codex_home = TempDir::new().expect("create temp codex home");
    let root = TempDir::new().expect("create temp search root");

    // Create files designed to have deterministic ordering for query "abc".
    std::fs::write(root.path().join("abc"), "x").expect("write file abc");
    std::fs::write(root.path().join("abcde"), "x").expect("write file abcx");
    std::fs::write(root.path().join("abexy"), "x").expect("write file abcx");
    std::fs::write(root.path().join("zzz.txt"), "x").expect("write file zzz");

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await.expect("spawn mcp");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    let root_path = root.path().to_string_lossy().to_string();
    // Send fuzzyFileSearch request.
    let request_id = mcp
        .send_fuzzy_file_search_request("abe", vec![root_path.clone()], None)
        .await
        .expect("send fuzzyFileSearch");

    // Read response and verify shape and ordering.
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("fuzzyFileSearch timeout")
    .expect("fuzzyFileSearch resp");

    let value = resp.result;
    assert_eq!(
        value,
        json!({
            "files": [
                { "root": root_path.clone(), "path": "abexy", "score": 88, "indices": [0, 1, 2] },
                { "root": root_path.clone(), "path": "abcde", "score": 74, "indices": [0, 1, 4] },
            ]
        })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_accepts_cancellation_token() {
    let codex_home = TempDir::new().expect("create temp codex home");
    let root = TempDir::new().expect("create temp search root");

    std::fs::write(root.path().join("alpha.txt"), "contents").expect("write alpha");

    let mut mcp = McpProcess::new(codex_home.path()).await.expect("spawn mcp");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    let root_path = root.path().to_string_lossy().to_string();
    let request_id = mcp
        .send_fuzzy_file_search_request("alp", vec![root_path.clone()], None)
        .await
        .expect("send fuzzyFileSearch");

    let request_id_2 = mcp
        .send_fuzzy_file_search_request(
            "alp",
            vec![root_path.clone()],
            Some(request_id.to_string()),
        )
        .await
        .expect("send fuzzyFileSearch");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id_2)),
    )
    .await
    .expect("fuzzyFileSearch timeout")
    .expect("fuzzyFileSearch resp");

    let files = resp
        .result
        .get("files")
        .and_then(|value| value.as_array())
        .cloned()
        .expect("files array");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["root"], root_path);
    assert_eq!(files[0]["path"], "alpha.txt");
}
