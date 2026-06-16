use super::*;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn records_completion_by_import_id() -> anyhow::Result<()> {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string()).await?;

    runtime
        .record_external_agent_config_import_completed(
            "import-1",
            &[ExternalAgentConfigImportSuccessRecord {
                item_type: "CONFIG".to_string(),
                cwd: None,
                source: Some("settings.json".to_string()),
                target: Some("config.toml".to_string()),
            }],
            &[],
        )
        .await?;
    runtime
        .record_external_agent_config_import_completed(
            "import-1",
            &[
                ExternalAgentConfigImportSuccessRecord {
                    item_type: "CONFIG".to_string(),
                    cwd: None,
                    source: Some("settings.json".to_string()),
                    target: Some("config.toml".to_string()),
                },
                ExternalAgentConfigImportSuccessRecord {
                    item_type: "MCP_SERVER_CONFIG".to_string(),
                    cwd: None,
                    source: Some("github".to_string()),
                    target: Some("github".to_string()),
                },
            ],
            &[ExternalAgentConfigImportFailureRecord {
                item_type: "MCP_SERVER_CONFIG".to_string(),
                error_type: None,
                failure_stage: "import".to_string(),
                message: "failed".to_string(),
                cwd: None,
                source: Some("broken".to_string()),
            }],
        )
        .await?;

    assert_eq!(
        runtime
            .external_agent_config_import_details_record("import-1")
            .await?,
        Some(ExternalAgentConfigImportDetailsRecord {
            successes: vec![
                ExternalAgentConfigImportSuccessRecord {
                    item_type: "CONFIG".to_string(),
                    cwd: None,
                    source: Some("settings.json".to_string()),
                    target: Some("config.toml".to_string()),
                },
                ExternalAgentConfigImportSuccessRecord {
                    item_type: "MCP_SERVER_CONFIG".to_string(),
                    cwd: None,
                    source: Some("github".to_string()),
                    target: Some("github".to_string()),
                }
            ],
            failures: vec![ExternalAgentConfigImportFailureRecord {
                item_type: "MCP_SERVER_CONFIG".to_string(),
                error_type: None,
                failure_stage: "import".to_string(),
                message: "failed".to_string(),
                cwd: None,
                source: Some("broken".to_string()),
            }],
        })
    );
    assert_eq!(
        runtime
            .external_agent_config_import_history_records(/*cursor*/ None, /*limit*/ 100)
            .await?
            .into_iter()
            .map(|record| (
                record.import_id,
                record.successes,
                record.failures,
                record.completed_at_ms > 0
            ))
            .collect::<Vec<_>>(),
        vec![(
            "import-1".to_string(),
            vec![
                ExternalAgentConfigImportSuccessRecord {
                    item_type: "CONFIG".to_string(),
                    cwd: None,
                    source: Some("settings.json".to_string()),
                    target: Some("config.toml".to_string()),
                },
                ExternalAgentConfigImportSuccessRecord {
                    item_type: "MCP_SERVER_CONFIG".to_string(),
                    cwd: None,
                    source: Some("github".to_string()),
                    target: Some("github".to_string()),
                }
            ],
            vec![ExternalAgentConfigImportFailureRecord {
                item_type: "MCP_SERVER_CONFIG".to_string(),
                error_type: None,
                failure_stage: "import".to_string(),
                message: "failed".to_string(),
                cwd: None,
                source: Some("broken".to_string()),
            }],
            true
        )]
    );

    Ok(())
}

#[tokio::test]
async fn reads_history_records_after_cursor() -> anyhow::Result<()> {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string()).await?;

    runtime
        .record_external_agent_config_import_completed("import-1", &[], &[])
        .await?;
    runtime
        .record_external_agent_config_import_completed("import-2", &[], &[])
        .await?;

    let first_page = runtime
        .external_agent_config_import_history_records(/*cursor*/ None, /*limit*/ 1)
        .await?;
    assert_eq!(first_page.len(), 1);
    let cursor = ExternalAgentConfigImportHistoryCursor {
        completed_at_ms: first_page[0].completed_at_ms,
        import_id: first_page[0].import_id.clone(),
    };
    let second_page = runtime
        .external_agent_config_import_history_records(Some(&cursor), /*limit*/ 1)
        .await?;

    let mut paged_import_ids = vec![first_page[0].import_id.clone()];
    paged_import_ids.extend(second_page.into_iter().map(|record| record.import_id));
    paged_import_ids.sort();
    assert_eq!(
        paged_import_ids,
        vec!["import-1".to_string(), "import-2".to_string()]
    );

    Ok(())
}
