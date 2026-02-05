use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use app_test_support::McpProcess;
use codex_app_server_protocol::FindFilesStreamChunkNotification;
use codex_app_server_protocol::FindFilesStreamResponse;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CHUNK_METHOD: &str = "findFilesStream/chunk";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_single_root_single_match() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("alpha.rs"), "fn alpha() {}")?;
    std::fs::write(root.path().join("beta.rs"), "fn beta() {}")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    let request_id = mcp
        .send_find_files_stream_request("alp", vec![root_path.clone()], None)
        .await?;

    let chunks = collect_final_chunks(&mut mcp, request_id).await?;
    let files = flatten_files(&chunks);

    assert_eq!(files.len(), 1, "files={files:?}");
    assert_eq!(files[0].root, root_path);
    assert_eq!(files[0].path, "alpha.rs");
    assert_eq!(files[0].file_name, "alpha.rs");
    assert!(files[0].indices.is_some(), "expected indices for match");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_empty_query_emits_single_empty_chunk() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("alpha.rs"), "fn alpha() {}")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_find_files_stream_request("", vec![root.path().to_string_lossy().to_string()], None)
        .await?;

    let response = read_response(&mut mcp, request_id).await?;
    let parsed: FindFilesStreamResponse = serde_json::from_value(response.result)?;
    assert_eq!(parsed, FindFilesStreamResponse {});

    let (chunks, mismatched_count) = collect_chunks_until_complete(&mut mcp, request_id).await?;
    assert_eq!(mismatched_count, 0, "unexpected mismatched notifications");
    assert_eq!(chunks.len(), 1, "chunks={chunks:?}");
    let chunk = &chunks[0];
    assert_eq!(chunk.files.len(), 0);
    assert_eq!(chunk.total_match_count, 0);
    assert_eq!(chunk.chunk_index, 0);
    assert_eq!(chunk.chunk_count, 1);
    assert!(!chunk.running);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_empty_roots_emits_single_empty_chunk() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_find_files_stream_request("alp", Vec::new(), None)
        .await?;

    let chunks = collect_final_chunks(&mut mcp, request_id).await?;
    assert_eq!(chunks.len(), 1, "chunks={chunks:?}");
    let chunk = &chunks[0];
    assert_eq!(chunk.files.len(), 0);
    assert_eq!(chunk.total_match_count, 0);
    assert_eq!(chunk.chunk_count, 1);
    assert!(!chunk.running);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_no_matches_returns_empty_files() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("alpha.rs"), "fn alpha() {}")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_find_files_stream_request(
            "zzz",
            vec![root.path().to_string_lossy().to_string()],
            None,
        )
        .await?;

    let chunks = collect_final_chunks(&mut mcp, request_id).await?;
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.chunk_index == 0)
        .ok_or_else(|| anyhow!("missing chunk 0"))?;

    assert_eq!(chunk.files.len(), 0);
    assert_eq!(chunk.total_match_count, 0);
    assert!(!chunk.running);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_merges_results_across_roots() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root_a = TempDir::new()?;
    let root_b = TempDir::new()?;

    std::fs::write(root_a.path().join("alpha.rs"), "fn alpha() {}")?;
    std::fs::write(root_b.path().join("alpine.rs"), "fn alpine() {}")?;
    std::fs::write(root_b.path().join("beta.rs"), "fn beta() {}")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_a_path = root_a.path().to_string_lossy().to_string();
    let root_b_path = root_b.path().to_string_lossy().to_string();

    let request_id = mcp
        .send_find_files_stream_request("alp", vec![root_a_path.clone(), root_b_path.clone()], None)
        .await?;

    let chunks = collect_final_chunks(&mut mcp, request_id).await?;
    let files = flatten_files(&chunks);

    let observed: BTreeSet<(String, String)> = files
        .into_iter()
        .map(|file| (file.root, file.path))
        .collect();
    let expected: BTreeSet<(String, String)> = BTreeSet::from([
        (root_a_path, "alpha.rs".to_string()),
        (root_b_path, "alpine.rs".to_string()),
    ]);

    assert_eq!(observed, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_same_token_updates_request_id_and_query() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("alpha.rs"), "fn alpha() {}")?;
    std::fs::write(root.path().join("beta.rs"), "fn beta() {}")?;

    // Create enough extra files to keep the stream active while we issue a follow-up query.
    write_matching_files(root.path(), "alpha-extra", 150)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    let token = "shared-token".to_string();

    let first_request_id = mcp
        .send_find_files_stream_request("alp", vec![root_path.clone()], Some(token.clone()))
        .await?;
    let _first_response = read_response(&mut mcp, first_request_id).await?;

    let second_request_id = mcp
        .send_find_files_stream_request("bet", vec![root_path.clone()], Some(token))
        .await?;

    let (chunks, _mismatched_count) =
        collect_chunks_until_complete(&mut mcp, second_request_id).await?;
    assert_eq!(
        chunks[0].request_id,
        RequestId::Integer(second_request_id),
        "expected notifications to adopt latest request id"
    );
    assert_eq!(chunks[0].query, "bet");

    let files = flatten_files(&chunks);
    assert!(files.iter().any(|file| file.path == "beta.rs"));
    assert!(
        chunks
            .iter()
            .all(|chunk| chunk.request_id == RequestId::Integer(second_request_id))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_same_token_with_different_roots_cancels_old_stream() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root_a = TempDir::new()?;
    let root_b = TempDir::new()?;

    std::fs::write(root_a.path().join("alpha.rs"), "fn alpha() {}")?;
    std::fs::write(root_b.path().join("beta.rs"), "fn beta() {}")?;

    write_matching_files(root_a.path(), "alpha-extra", 120)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let token = "root-swap-token".to_string();
    let root_a_path = root_a.path().to_string_lossy().to_string();
    let root_b_path = root_b.path().to_string_lossy().to_string();

    let first_request_id = mcp
        .send_find_files_stream_request("alp", vec![root_a_path], Some(token.clone()))
        .await?;
    let _first_response = read_response(&mut mcp, first_request_id).await?;

    let second_request_id = mcp
        .send_find_files_stream_request("alp", vec![root_b_path.clone()], Some(token))
        .await?;

    let (chunks, _mismatched_count) =
        collect_chunks_until_complete(&mut mcp, second_request_id).await?;

    let files = flatten_files(&chunks);
    assert!(files.iter().all(|file| file.root == root_b_path));
    assert!(files.iter().all(|file| file.path != "alpha.rs"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_enforces_limit_per_root() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    write_matching_files(root.path(), "limit-case", 60)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_find_files_stream_request(
            "limit-case",
            vec![root.path().to_string_lossy().to_string()],
            None,
        )
        .await?;

    let chunks = collect_final_chunks(&mut mcp, request_id).await?;
    let files = flatten_files(&chunks);

    assert_eq!(
        files.len(),
        50,
        "expected limit-per-root to cap emitted matches"
    );
    assert!(
        chunks[0].total_match_count >= 60,
        "expected total match count to reflect all matches"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_chunks_results_when_over_chunk_size() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root_a = TempDir::new()?;
    let root_b = TempDir::new()?;
    let root_c = TempDir::new()?;

    write_matching_files(root_a.path(), "chunk-case", 55)?;
    write_matching_files(root_b.path(), "chunk-case", 55)?;
    write_matching_files(root_c.path(), "chunk-case", 55)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_find_files_stream_request(
            "chunk-case",
            vec![
                root_a.path().to_string_lossy().to_string(),
                root_b.path().to_string_lossy().to_string(),
                root_c.path().to_string_lossy().to_string(),
            ],
            None,
        )
        .await?;

    let chunks = collect_final_chunks(&mut mcp, request_id).await?;
    let chunk_indices: BTreeSet<usize> = chunks.iter().map(|chunk| chunk.chunk_index).collect();

    assert_eq!(chunks[0].chunk_count, 2);
    assert_eq!(chunk_indices, BTreeSet::from([0, 1]));
    assert_eq!(flatten_files(&chunks).len(), 150);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_files_stream_emits_sorted_unique_indices() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("abcde.rs"), "fn main() {}")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_find_files_stream_request(
            "ace",
            vec![root.path().to_string_lossy().to_string()],
            None,
        )
        .await?;

    let chunks = collect_final_chunks(&mut mcp, request_id).await?;
    let files = flatten_files(&chunks);

    assert_eq!(files.len(), 1, "files={files:?}");
    let indices = files[0]
        .indices
        .clone()
        .ok_or_else(|| anyhow!("missing indices"))?;
    assert_eq!(indices, vec![0, 2, 4]);
    assert!(is_sorted_unique(&indices));

    Ok(())
}

async fn collect_final_chunks(
    mcp: &mut McpProcess,
    request_id: i64,
) -> anyhow::Result<Vec<FindFilesStreamChunkNotification>> {
    let _response = read_response(mcp, request_id).await?;
    let (chunks, mismatched_count) = collect_chunks_until_complete(mcp, request_id).await?;
    if mismatched_count != 0 {
        anyhow::bail!("saw {mismatched_count} notifications for other request ids");
    }
    Ok(chunks)
}

async fn read_response(mcp: &mut McpProcess, request_id: i64) -> anyhow::Result<JSONRPCResponse> {
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await?
}

async fn collect_chunks_until_complete(
    mcp: &mut McpProcess,
    request_id: i64,
) -> anyhow::Result<(Vec<FindFilesStreamChunkNotification>, usize)> {
    let mut latest_query = String::new();
    let mut latest_chunk_count = 0usize;
    let mut latest_chunks = std::collections::BTreeMap::new();
    let mut mismatched_count = 0usize;

    loop {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message(CHUNK_METHOD),
        )
        .await??;
        let chunk = parse_chunk(notification)?;

        if chunk.request_id != RequestId::Integer(request_id) {
            mismatched_count += 1;
            continue;
        }

        if chunk.query != latest_query || chunk.chunk_count != latest_chunk_count {
            latest_query.clear();
            latest_query.push_str(&chunk.query);
            latest_chunk_count = chunk.chunk_count;
            latest_chunks.clear();
        }

        latest_chunks.insert(chunk.chunk_index, chunk.clone());

        if !chunk.running && latest_chunks.len() == latest_chunk_count {
            let chunks = latest_chunks.into_values().collect();
            return Ok((chunks, mismatched_count));
        }
    }
}

fn parse_chunk(
    notification: JSONRPCNotification,
) -> anyhow::Result<FindFilesStreamChunkNotification> {
    let params = notification
        .params
        .ok_or_else(|| anyhow!("notification missing params"))?;
    let chunk = serde_json::from_value::<FindFilesStreamChunkNotification>(params)?;
    Ok(chunk)
}

fn flatten_files(
    chunks: &[FindFilesStreamChunkNotification],
) -> Vec<codex_app_server_protocol::FuzzyFileSearchResult> {
    let mut files = Vec::new();
    for chunk in chunks {
        files.extend(chunk.files.clone());
    }
    files
}

fn write_matching_files(root: &std::path::Path, prefix: &str, count: usize) -> Result<()> {
    for index in 0..count {
        let file_name = format!("{prefix}-{index:03}.rs");
        std::fs::write(root.join(file_name), "fn main() {}")?;
    }
    Ok(())
}

fn is_sorted_unique(indices: &[u32]) -> bool {
    indices.windows(2).all(|pair| pair[0] < pair[1])
}
