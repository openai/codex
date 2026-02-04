use anyhow::Result;
use anyhow::anyhow;
use app_test_support::McpProcess;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_sorts_and_includes_indices() -> Result<()> {
    // Prepare a temporary Codex home and a separate root with test files.
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    // Create files designed to have deterministic ordering for query "abe".
    std::fs::write(root.path().join("abc"), "x")?;
    std::fs::write(root.path().join("abcde"), "x")?;
    std::fs::write(root.path().join("abexy"), "x")?;
    std::fs::write(root.path().join("zzz.txt"), "x")?;
    let sub_dir = root.path().join("sub");
    std::fs::create_dir_all(&sub_dir)?;
    let sub_abce_path = sub_dir.join("abce");
    std::fs::write(&sub_abce_path, "x")?;
    let sub_abce_rel = sub_abce_path
        .strip_prefix(root.path())?
        .to_string_lossy()
        .to_string();

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    // Send fuzzyFileSearch request.
    let request_id = mcp
        .send_fuzzy_file_search_request("abe", vec![root_path.clone()], None)
        .await?;

    // Read response and verify shape and ordering.
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let value = resp.result;
    let expected_score = 72;

    assert_eq!(
        value,
        json!({
            "files": [
                {
                    "root": root_path.clone(),
                    "path": "abexy",
                    "file_name": "abexy",
                    "score": 84,
                    "indices": [0, 1, 2],
                },
                {
                    "root": root_path.clone(),
                    "path": sub_abce_rel,
                    "file_name": "abce",
                    "score": expected_score,
                    "indices": [4, 5, 7],
                },
                {
                    "root": root_path.clone(),
                    "path": "abcde",
                    "file_name": "abcde",
                    "score": 71,
                    "indices": [0, 1, 4],
                },
            ]
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_accepts_cancellation_token() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("alpha.txt"), "contents")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    let request_id = mcp
        .send_fuzzy_file_search_request("alp", vec![root_path.clone()], None)
        .await?;

    let request_id_2 = mcp
        .send_fuzzy_file_search_request(
            "alp",
            vec![root_path.clone()],
            Some(request_id.to_string()),
        )
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id_2)),
    )
    .await??;

    let files = resp
        .result
        .get("files")
        .ok_or_else(|| anyhow!("files key missing"))?
        .as_array()
        .ok_or_else(|| anyhow!("files not array"))?
        .clone();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["root"], root_path);
    assert_eq!(files[0]["path"], "alpha.txt");

    Ok(())
}
