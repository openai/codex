use super::StateRuntime;
use crate::model::datetime_to_epoch_millis;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use sqlx::Row;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAgentConfigImportSuccessRecord {
    pub item_type: String,
    pub cwd: Option<PathBuf>,
    pub source: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAgentConfigImportFailureRecord {
    pub item_type: String,
    pub failure_stage: String,
    pub message: String,
    pub cwd: Option<PathBuf>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAgentConfigImportDetailsRecord {
    pub successes: Vec<ExternalAgentConfigImportSuccessRecord>,
    pub failures: Vec<ExternalAgentConfigImportFailureRecord>,
}

impl StateRuntime {
    pub async fn record_external_agent_config_import_completed(
        &self,
        import_id: &str,
        successes: &[ExternalAgentConfigImportSuccessRecord],
        failures: &[ExternalAgentConfigImportFailureRecord],
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO external_agent_config_imports (
    import_id,
    completed_at_ms,
    successes,
    failures
) VALUES (?, ?, ?, ?)
ON CONFLICT(import_id) DO UPDATE SET
    completed_at_ms = excluded.completed_at_ms,
    successes = excluded.successes,
    failures = excluded.failures
"#,
        )
        .bind(import_id)
        .bind(datetime_to_epoch_millis(Utc::now()))
        .bind(serde_json::to_string(successes)?)
        .bind(serde_json::to_string(failures)?)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    pub async fn external_agent_config_import_details_record(
        &self,
        import_id: &str,
    ) -> anyhow::Result<Option<ExternalAgentConfigImportDetailsRecord>> {
        let row = sqlx::query(
            r#"
SELECT
    successes,
    failures
FROM external_agent_config_imports
WHERE import_id = ?
"#,
        )
        .bind(import_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| {
            let successes: String = row.try_get("successes")?;
            let failures: String = row.try_get("failures")?;
            Ok(ExternalAgentConfigImportDetailsRecord {
                successes: serde_json::from_str(&successes)?,
                failures: serde_json::from_str(&failures)?,
            })
        })
        .transpose()
    }
}

#[cfg(test)]
#[path = "external_agent_config_imports_tests.rs"]
mod tests;
