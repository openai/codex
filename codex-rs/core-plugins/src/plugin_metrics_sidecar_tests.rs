use super::*;
use crate::PluginMeasurementDefinition;
use crate::PluginMetricsOperation;
use codex_plugin::PluginId;
use codex_protocol::models::LegacyReadWriteRoots;
use pretty_assertions::assert_eq;
use serde_json::json;

fn create_sidecar() -> PluginMetricsSidecar {
    PluginMetricsSidecar::create(resolved_operation()).expect("create sidecar")
}

#[test]
fn sidecar_is_created_in_system_temp_with_private_permissions() {
    let output_dir =
        AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).expect("absolute temp directory");
    let sidecar = PluginMetricsSidecar::create(resolved_operation()).expect("create sidecar");

    assert_eq!(
        sidecar.absolute_output_path().parent(),
        Some(sidecar.absolute_output_dir.clone())
    );
    assert_eq!(sidecar.absolute_output_dir.parent(), Some(output_dir));
    assert!(sidecar.absolute_output_dir.as_path().is_dir());
    let roots = sidecar
        .additional_permissions()
        .file_system
        .expect("file system permissions")
        .legacy_read_write_roots()
        .expect("legacy roots");
    assert_eq!(
        roots,
        LegacyReadWriteRoots {
            read: None,
            write: Some(vec![sidecar.absolute_output_dir]),
        }
    );
}

fn resolved_operation() -> ResolvedPluginMetricsOperation {
    ResolvedPluginMetricsOperation {
        plugin_id: PluginId::parse("security@openai-curated").expect("valid plugin id"),
        operation: PluginMetricsOperation {
            operation_name: "security_scan".to_string(),
            measurements: BTreeMap::from([
                (
                    "finding_count".to_string(),
                    PluginMeasurementDefinition {
                        enum_dimensions: BTreeMap::from([(
                            "severity".to_string(),
                            BTreeSet::from(["high".to_string(), "low".to_string()]),
                        )]),
                    },
                ),
                (
                    "files_scanned".to_string(),
                    PluginMeasurementDefinition {
                        enum_dimensions: BTreeMap::new(),
                    },
                ),
            ]),
        },
    }
}

#[tokio::test]
async fn sidecar_keeps_valid_rows_and_first_duplicate_then_cleans_up() {
    let sidecar = create_sidecar();
    let path = sidecar.absolute_output_path();
    std::fs::write(
        path.as_path(),
        json!({
            "version": 1,
            "measurements": [
                {"name": "finding_count", "value": 3, "dimensions": {"severity": "high"}},
                {"name": "unknown", "value": 1},
                {"name": "finding_count", "value": 4},
                {"name": "finding_count", "value": 5, "dimensions": {"severity": "critical"}},
                {"name": "finding_count", "value": 6, "dimensions": {"severity": "high", "extra": "x"}},
                {"name": "finding_count", "value": 99, "dimensions": {"severity": "high"}},
                {"name": "files_scanned", "value": 17},
                {"name": "files_scanned", "value": "not-a-number"},
                {"name": "files_scanned", "value": 18, "unknown": true}
            ]
        })
        .to_string(),
    )
    .expect("write output");

    let batch = sidecar
        .finish(/*exit_code*/ 0)
        .await
        .expect("valid measurements");
    let execution_id = batch.execution_id.clone();
    assert_eq!(
        batch,
        PluginMeasurementBatch {
            plugin_id: "security@openai-curated".to_string(),
            execution_id: execution_id.clone(),
            operation: "security_scan".to_string(),
            rows: vec![
                PluginMeasurementRow {
                    measurement_name: "finding_count".to_string(),
                    number_value: 3.0,
                    dimensions: BTreeMap::from([("severity".to_string(), "high".to_string(),)]),
                },
                PluginMeasurementRow {
                    measurement_name: "files_scanned".to_string(),
                    number_value: 17.0,
                    dimensions: BTreeMap::new(),
                },
            ],
        }
    );
    assert_eq!(
        Uuid::parse_str(&execution_id)
            .expect("execution id UUID")
            .get_version(),
        Some(uuid::Version::Random)
    );
    assert!(!path.exists());
}

#[tokio::test]
async fn malformed_oversized_and_nonzero_outputs_are_ignored_and_cleaned_up() {
    for output in [
        r#"{"version":2,"measurements":[]}"#.as_bytes().to_vec(),
        r#"{"version":1,"measurements":[],"unknown":true}"#.as_bytes().to_vec(),
        json!({
            "version": 1,
            "measurements": vec![json!({"name": "files_scanned", "value": 1}); MAX_OUTPUT_ROWS + 1]
        })
        .to_string()
        .into_bytes(),
        vec![b' '; MAX_OUTPUT_BYTES as usize + 1],
    ] {
        let sidecar = create_sidecar();
        let path = sidecar.absolute_output_path();
        std::fs::write(path.as_path(), output).expect("write output");
        assert_eq!(sidecar.finish(/*exit_code*/ 0).await, None);
        assert!(!path.exists());
    }

    let sidecar = create_sidecar();
    let path = sidecar.absolute_output_path();
    std::fs::write(
        path.as_path(),
        r#"{"version":1,"measurements":[{"name":"files_scanned","value":1}]}"#,
    )
    .expect("write output");
    assert_eq!(sidecar.finish(/*exit_code*/ 1).await, None);
    assert!(!path.exists());
}

#[test]
fn reserved_output_env_is_absent_without_sidecar_and_cannot_be_overridden() {
    let mut env = HashMap::from([
        (
            PLUGIN_METRICS_OUTPUT_ENV_VAR.to_string(),
            "/user/path".to_string(),
        ),
        ("KEEP".to_string(), "value".to_string()),
    ]);
    strip_output_env(&mut env);
    assert_eq!(
        env,
        HashMap::from([("KEEP".to_string(), "value".to_string())])
    );

    let sidecar = create_sidecar();
    let path = sidecar.absolute_output_path();
    sidecar.install_output_env(&mut env);
    assert_eq!(
        env.get(PLUGIN_METRICS_OUTPUT_ENV_VAR).map(String::as_str),
        path.as_path().to_str()
    );
    drop(sidecar);
    assert!(!path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn sidecar_reads_the_original_file_after_path_replacement() {
    let sidecar = create_sidecar();
    let path = sidecar.absolute_output_path();
    std::fs::remove_file(path.as_path()).expect("remove original output path");
    std::fs::write(
        path.as_path(),
        r#"{"version":1,"measurements":[{"name":"files_scanned","value":99}]}"#,
    )
    .expect("write replacement output");

    assert_eq!(sidecar.finish(/*exit_code*/ 0).await, None);
    assert!(!path.exists());
}
