//! Exercises dual-write and v2 cutover through model requests and committed versioned state.

use super::*;
use codex_protocol::MemoryVersion;
use core_test_support::responses::sse_response;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[tokio::test]
async fn dual_write_runs_both_versions_until_v2_is_selected() -> anyhow::Result<()> {
    for (version, expected_pipelines) in [(Some("v1"), 2), (Some("v2"), 1), (None, 1)] {
        let server = start_mock_server().await;
        let home = Arc::new(TempDir::new()?);
        let version_config = version
            .map(|version| format!("version = {version:?}"))
            .unwrap_or_default();
        tokio::fs::write(
            home.path().join("config.toml"),
            format!(
                "[features]\nmemories = true\n\
                 [memories]\ndual_write = true\nmax_raw_memories_for_consolidation = 1\n\
                 min_rollout_idle_hours = 0\n{version_config}\n"
            ),
        )
        .await?;
        let test = test_codex()
            .with_home(Arc::clone(&home))
            .build(&server)
            .await?;
        let selected_version = test.config.memories.version;
        assert_eq!(
            selected_version,
            if version == Some("v1") {
                MemoryVersion::V1
            } else {
                MemoryVersion::V2
            }
        );
        let db = test.codex.state_db().expect("memory state");
        let source = seed_stage1_candidate(
            &db,
            home.path(),
            chrono::Utc::now() - chrono::Duration::hours(2),
            "dual-write",
        )
        .await?;
        let v1_root = home.path().join("memories");
        let v2_root = home.path().join("memories_v2");
        if version.is_some() {
            seed_required_memory_artifacts(&v1_root).await?;
        }
        tokio::fs::create_dir_all(&v2_root).await?;
        tokio::fs::write(
            v2_root.join("memory_summary.md"),
            "v1\n\n## User Profile\nV2 user\n\n## User preferences\nV2 preference\n\n## General Tips\nV2 tip\n\n## What's in Memory\nV2 source\n",
        )
        .await?;
        // Dispatch by the output contract rather than request order: both pipelines
        // run concurrently, and each phase two can start before the other phase one.
        Mock::given(method("POST"))
            .and(path_regex(".*/responses$"))
            .respond_with(|request: &wiremock::Request| {
                let body: serde_json::Value = request.body_json().expect("response request");
                let properties = &body["text"]["format"]["schema"]["properties"];
                let output = if properties.get("raw_memory").is_some() {
                    r#"{"raw_memory":"legacy fact","rollout_summary":"legacy rollout","rollout_slug":"legacy"}"#
                } else if properties.get("rollout_summary").is_some() {
                    r#"{"rollout_summary":"v2 rollout","rollout_slug":"v2"}"#
                } else {
                    "consolidation complete"
                };
                sse_response(sse(vec![
                    ev_response_created("response"),
                    ev_assistant_message("message", output),
                    ev_completed("response"),
                ]))
            })
            .mount(&server)
            .await;

        trigger_memories_startup(&test).await;
        for (version, root, expected_raw, expected_summary) in [
            (MemoryVersion::V2, &v2_root, "", "v2 rollout"),
            (MemoryVersion::V1, &v1_root, "legacy fact", "legacy rollout"),
        ] {
            let store = db.memories_for_version(version).await?;
            if selected_version == MemoryVersion::V2 && version == MemoryVersion::V1 {
                assert_eq!(
                    store.list_stage1_outputs_for_global(/*n*/ 10).await?,
                    vec![]
                );
                assert_eq!(store.max_consolidated_thread_count().await?, 0);
                assert!(!root.join("rollout_summaries").exists());
                continue;
            }
            // Both pipelines initialize Git workspaces and run consolidation agents.
            // Allow for process startup under contention on Windows CI runners.
            let deadline = Instant::now() + Duration::from_secs(60);
            while store.max_consolidated_thread_count().await? == 0 {
                anyhow::ensure!(
                    Instant::now() < deadline,
                    "pipeline {version:?} did not consolidate"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let outputs = store.list_stage1_outputs_for_global(/*n*/ 10).await?;
            assert_eq!(
                outputs
                    .iter()
                    .map(|output| (
                        output.thread_id,
                        output.raw_memory.as_str(),
                        output.rollout_summary.as_str()
                    ))
                    .collect::<Vec<_>>(),
                vec![(source, expected_raw, expected_summary)]
            );
            let summaries = read_rollout_summary_bodies(root).await?;
            assert_eq!(summaries.len(), 1);
            assert!(summaries[0].contains(expected_summary));
        }
        let requests = server.received_requests().await.expect("recorded requests");
        let bodies = requests
            .iter()
            .filter(|request| request.url.path().ends_with("/responses"))
            .map(|request| {
                request
                    .body_json::<serde_json::Value>()
                    .expect("request JSON")
            })
            .collect::<Vec<_>>();
        assert_eq!(bodies.len(), expected_pipelines * 2);
        assert_eq!(
            bodies
                .iter()
                .filter(|body| body["text"]["format"]["schema"].is_null())
                .count(),
            expected_pipelines
        );
        assert!(!v2_root.join("MEMORY.md").exists());
        assert!(!v2_root.join("raw_memories.md").exists());
        if version.is_none() {
            assert!(!v1_root.exists());
        }
        shutdown_test_codex(&test).await?;
    }
    Ok(())
}
