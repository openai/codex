#![cfg(not(target_os = "windows"))]

use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disabled until we enable read_file tool"]
async fn read_file_tool_returns_requested_lines() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex().build(&server).await?;

    let file_path = test.cwd.path().join("sample.txt");
    std::fs::write(&file_path, "first\nsecond\nthird\nfourth\n")?;
    let file_path = file_path.to_string_lossy().to_string();

    let call_id = "read-file-call";
    let arguments = json!({
        "file_path": file_path,
        "offset": 2,
        "limit": 2,
    })
    .to_string();

    let mocks = mount_function_call_agent_response(&server, call_id, &arguments, "read_file").await;

    test.submit_turn("please inspect sample.txt").await?;

    let req = mocks.completion.single_request();
    let (output_text_opt, _) = req
        .function_call_output_content_and_success(call_id)
        .expect("output present");
    let output_text = output_text_opt.expect("output text present");
    assert_eq!(output_text, "L2: second\nL3: third");

    Ok(())
}
