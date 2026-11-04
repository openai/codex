//! Checks bounded outcome metrics for skill reads and installation listings.

use super::*;
use pretty_assertions::assert_eq;

fn read_result(outcome: &str, version: &str) -> RecordedCounter {
    RecordedCounter {
        name: "codex.skills.read.result".to_string(),
        increment: 1,
        tags: vec![
            ("outcome".to_string(), outcome.to_string()),
            ("discovery_version".to_string(), version.to_string()),
        ],
    }
}

fn installations_result(outcome: &str, source_filter: &str) -> RecordedCounter {
    RecordedCounter {
        name: "codex.skills.installations.result".to_string(),
        increment: 1,
        tags: vec![
            ("outcome".to_string(), outcome.to_string()),
            ("source_filter".to_string(), source_filter.to_string()),
            ("discovery_version".to_string(), "v2".to_string()),
        ],
    }
}

#[tokio::test]
async fn reads_report_outcomes_for_each_discovery_version() -> TestResult {
    for discovery_version in ["v1", "v2"] {
        let mut executor = installed_skill("executor", "local")?
            .with_remote_plugin_id(/*remote_plugin_id*/ None)
            .with_plugin_id(/*plugin_id*/ None);
        executor.main_prompt = SkillResourceId::environment(
            executor.main_prompt.as_str(),
            "missing-environment",
            PathUri::parse("file:///skills/local/SKILL.md")?,
        );
        let mut harness = MergeHarness::new(
            provider(vec![executor]),
            provider(vec![
                cloud_skill("work").with_alias_root("skill://Plugin_123"),
            ]),
            default_config(),
        )
        .await?;
        if discovery_version == "v1" {
            harness.snapshots = None;
        }
        harness.turn_store.insert(selected_plugins(&[None], &[]));
        let tools = harness.tools(&harness.step_store).await;
        let mut expected = Vec::new();
        for (arguments, budget, outcome) in [
            (
                json!({"package": "skill://Plugin_123/work"}),
                10_000,
                "success",
            ),
            (json!({}), 10_000, "invalid_arguments"),
            (json!({"package": "missing"}), 10_000, "package_unavailable"),
            (json!({"package": "c99/work"}), 10_000, "unresolved_alias"),
            (
                json!({"package": "skill://executor/local"}),
                10_000,
                "environment_unavailable",
            ),
            (
                json!({"package": "skill://Plugin_123/work", "source": {"kind": "executor", "environment_id": "missing"}}),
                10_000,
                "source_unavailable",
            ),
            (
                json!({"package": "skill://Plugin_123/work", "cursor": "invalid"}),
                10_000,
                "invalid_cursor",
            ),
            (
                json!({"package": "skill://Plugin_123/work"}),
                1,
                "response_budget",
            ),
            (
                json!({"package": "skill://Plugin_123/work"}),
                10_000,
                "read_failed",
            ),
        ] {
            if discovery_version == "v1" && outcome != "success" {
                continue;
            }
            if outcome == "read_failed" {
                let config = default_config();
                harness.registry.config_contributors()[0].on_config_changed(
                    &harness.session_store,
                    &harness.thread_store,
                    &config,
                    &TestConfig {
                        cloud_skill_enabled: false,
                        ..config.clone()
                    },
                );
            }
            let result =
                call_tool_with_budget(&tools, "read", arguments, TruncationPolicy::Bytes(budget))
                    .await;
            assert_eq!(
                result.is_ok(),
                outcome == "success",
                "{outcome}: {result:?}"
            );
            expected.push(read_result(outcome, discovery_version));
            assert_eq!(harness.catalog_metrics.counters(), expected);
        }
    }
    Ok(())
}

#[tokio::test]
async fn installations_report_outcomes_for_each_discovery_version() -> TestResult {
    let harness = MergeHarness::new(
        provider(Vec::new()),
        provider(vec![
            cloud_skill("work").with_alias_root("skill://Plugin_123"),
        ]),
        default_config(),
    )
    .await?;
    harness.turn_store.insert(selected_plugins(&[None], &[]));
    let tools = harness.tools(&harness.step_store).await;
    let mut expected = Vec::new();
    for (arguments, budget, outcome, source_filter) in [
        (
            json!({"package": "skill://Plugin_123/work"}),
            10_000,
            "success",
            "all",
        ),
        (
            json!({"package": "c0/work", "source": {"kind": "cloud"}}),
            10_000,
            "success",
            "cloud",
        ),
        (
            json!({"package": "c0/work", "source": {"kind": "executor", "environment_id": "missing"}}),
            10_000,
            "empty",
            "executor",
        ),
        (json!({}), 10_000, "invalid_arguments", "unparsed"),
        (json!({"package": ""}), 10_000, "invalid_package", "all"),
        (
            json!({"package": "c0/work", "source": {"kind": "executor", "environment_id": ""}}),
            10_000,
            "invalid_source",
            "executor",
        ),
        (
            json!({"package": "c0/missing"}),
            10_000,
            "package_unavailable",
            "all",
        ),
        (
            json!({"package": "c99/work"}),
            10_000,
            "unresolved_alias",
            "all",
        ),
        (
            json!({"package": "c0/work", "cursor": "invalid"}),
            10_000,
            "invalid_cursor",
            "all",
        ),
        (json!({"package": "c0/work"}), 1, "response_budget", "all"),
    ] {
        let result = call_tool_with_budget(
            &tools,
            "installations",
            arguments,
            TruncationPolicy::Bytes(budget),
        )
        .await;
        assert_eq!(
            result.is_ok(),
            matches!(outcome, "success" | "empty"),
            "{outcome}: {result:?}"
        );
        expected.push(installations_result(outcome, source_filter));
        assert_eq!(harness.catalog_metrics.counters(), expected);
    }
    Ok(())
}
