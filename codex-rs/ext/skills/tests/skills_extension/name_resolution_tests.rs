//! Display names resolve through the same policy-checked routes as package locators.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn skill_names_preserve_package_and_alias_precedence_in_both_versions() -> TestResult {
    for discovery_v2 in [false, true] {
        let cloud = cloud_skill("work").with_alias_root("skill://Plugin_123");
        let mut hidden = cloud_skill("hidden").hidden_from_prompt();
        hidden.name = cloud.name.clone();
        let mut disabled = cloud_skill("disabled").disabled();
        disabled.name = cloud.name.clone();
        let disabled_package = disabled.id.0.clone();
        let mut entries = vec![cloud.clone(), hidden, disabled];
        for (id, name) in [
            ("package-shadow", cloud.id.0.as_str()),
            ("alias-shadow", "c0/work"),
            ("disabled-package-shadow", disabled_package.as_str()),
            ("disabled-alias-shadow", "c0/disabled"),
        ] {
            let mut shadow = cloud_skill(id);
            shadow.name = name.to_string();
            entries.push(shadow);
        }
        let mut harness =
            MergeHarness::new(provider(vec![]), provider(entries), default_config()).await?;
        if !discovery_v2 {
            harness.snapshots = None;
        }
        harness.turn_store.insert(selected_plugins(&[None], &[]));
        let tools = harness.tools(&harness.step_store).await;
        for tool in if discovery_v2 {
            vec!["read", "installations"]
        } else {
            vec!["read"]
        } {
            let expected = call_tool(&tools, tool, json!({"package": cloud.id.0})).await?;
            for package in [cloud.name.as_str(), "c0/work"] {
                assert_eq!(
                    call_tool(&tools, tool, json!({"package": package})).await?,
                    expected,
                    "{tool}: {package} (discovery_v2={discovery_v2})",
                );
            }
            if tool == "read" {
                assert_eq!(expected["resource"], cloud.main_prompt.as_str());
            }
            for package in [disabled_package.as_str(), "c0/disabled"] {
                assert_eq!(
                    call_tool(&tools, tool, json!({"package": package})).await,
                    Err(FunctionCallError::RespondToModel(
                        "skill package is not available".to_string()
                    )),
                );
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn listed_skill_names_preserve_merged_locations_and_source_filters() -> TestResult {
    let mut cloud = cloud_skill("work");
    let mut executor = installed_skill("first", "work")?;
    // A valid qualified name can exceed the byte limit in skills.list.
    cloud.name = format!("x:{}", "🦀".repeat(64));
    executor.name.clone_from(&cloud.name);
    let harness = MergeHarness::new(
        provider(vec![executor]),
        provider(vec![cloud.clone()]),
        default_config(),
    )
    .await?;
    harness
        .turn_store
        .insert(selected_plugins(&[None, Some("env-1:first")], &[]));
    let tools = harness.tools(&harness.step_store).await;
    let listed = call_tool(&tools, "list", json!({"authority": {"kind": "cloud"}})).await?;
    for tool in ["read", "installations"] {
        for source in [
            Value::Null,
            json!({"kind": "cloud"}),
            json!({"kind": "executor", "environment_id": "env-1"}),
        ] {
            assert_eq!(
                call_tool(
                    &tools,
                    tool,
                    json!({"package": listed["skills"][0]["name"], "source": source})
                )
                .await?,
                call_tool(
                    &tools,
                    tool,
                    json!({"package": cloud.id.0, "source": source})
                )
                .await?,
                "{tool}: {source}",
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn ambiguous_skill_names_require_a_package_locator() -> TestResult {
    let prefix = format!("x:{}", "🦀".repeat(63));
    let mut first = cloud_skill("work");
    first.name = format!("{prefix}🦀");
    let mut second = cloud_skill("another");
    second.name = first.name.clone();
    let mut shorter = cloud_skill("shorter");
    shorter.name = prefix.clone();
    let hidden = cloud_skill("hidden").hidden_from_prompt();
    let disabled = cloud_skill("disabled").disabled();
    let harness = MergeHarness::new(
        provider(vec![]),
        provider(vec![first.clone(), second, shorter, hidden, disabled]),
        default_config(),
    )
    .await?;
    harness.turn_store.insert(selected_plugins(&[None], &[]));
    let tools = harness.tools(&harness.step_store).await;
    for tool in ["read", "installations"] {
        for name in [&first.name, &prefix] {
            assert_eq!(
                call_tool(&tools, tool, json!({"package": name})).await,
                Err(FunctionCallError::RespondToModel(
                    "skill name is ambiguous; use skills.list and pass the package for the intended skill".to_string(),
                )),
            );
        }
        for package in ["deploy:hidden", "deploy:disabled"] {
            assert_eq!(
                call_tool(&tools, tool, json!({"package": package})).await,
                Err(FunctionCallError::RespondToModel(
                    "skill package is not available".to_string()
                )),
            );
        }
        call_tool(&tools, tool, json!({"package": first.id.0})).await?;
    }
    Ok(())
}
