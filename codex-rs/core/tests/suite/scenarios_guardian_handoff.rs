//! Guardian sees root windows delivered along a nested delegation chain.
use super::*;

#[test_case::test_case(false; "text_history")]
#[test_case::test_case(true; "native_history")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn guardian_handoff_delegation_request_history(native_history: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let requests = tokio::time::timeout(
        Duration::from_secs(/*secs*/ 30),
        super::super::guardian_subagent_authorization::root_handoff::handoff_scenario(
            native_history,
        ),
    )
    .await??;
    let mut snapshot = context_snapshot::format_request_history_snapshot(
        "Complete Guardian request history: root delegates through alpha to leaf and separately to beta. Root handoffs to alpha update leaf immediately. Every branch keeps genuine root user messages, including cancellation followed by unrelated status questions. Handoff windows and the latest three root messages select assistant context. If compaction removes the handoff calls, the existing root context is used instead. The assessment is mocked.",
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    for (pattern, replacement) in [
        (
            r#"(?m)^(\s*"environment_id": )"(?:local|remote)""#,
            "$1\"<ENVIRONMENT>\"",
        ),
        (
            r#"(The active permission profile for environment )"(?:local|remote)""#,
            "$1\"<ENVIRONMENT>\"",
        ),
        (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
        (
            r#""command": \[\s*(?:"[^"]*",\s*)*"exit 0"\s*\]"#,
            "\"command\": [\"<SHELL>\", \"exit 0\"]",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    let snapshot_name = if native_history {
        "guardian_handoff_delegation_native"
    } else {
        "guardian_handoff_delegation"
    };
    insta::assert_snapshot!(snapshot_name, snapshot);
    Ok(())
}
