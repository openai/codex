use super::automation_schedule::AutomationSchedule;
use super::automation_schedule::compute_next_run_at;
use super::*;
use crate::Automation;
use crate::AutomationCreateParams;
use crate::AutomationDispatchSettings;
use crate::AutomationStatus;
use crate::AutomationTarget;
use crate::AutomationUpdateParams;
use crate::DEFAULT_AUTOMATION_RRULE;
use crate::model::AutomationRow;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use uuid::Uuid;

const AUTOMATION_NAME_MAX_CHARS: usize = 256;
const AUTOMATION_PROMPT_MAX_CHARS: usize = 2_000;

impl StateRuntime {
    pub async fn list_automations(&self) -> anyhow::Result<Vec<Automation>> {
        let rows = sqlx::query_as::<_, AutomationRow>(
            r#"
SELECT *
FROM automations
ORDER BY updated_at DESC, id ASC
            "#,
        )
        .fetch_all(self.automations_pool.as_ref())
        .await?;
        rows.into_iter()
            .map(Automation::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_automation(&self, id: &str) -> anyhow::Result<Option<Automation>> {
        let row = load_automation_row(self.automations_pool.as_ref(), id).await?;
        row.map(Automation::try_from).transpose()
    }

    pub async fn create_automation(
        &self,
        params: &AutomationCreateParams,
    ) -> anyhow::Result<Automation> {
        validate_automation_params(
            params.name.as_str(),
            params.prompt.as_str(),
            &params.target,
            params.dispatch_settings.as_ref(),
        )?;
        let mut tx = self.automations_pool.begin_with("BEGIN IMMEDIATE").await?;
        if params.status == AutomationStatus::Active {
            ensure_no_active_heartbeat_conflict(&mut tx, &params.target, /*exclude_id*/ None)
                .await?;
        }

        let now = Utc::now();
        let id = format!("automation-{}", Uuid::new_v4());
        let rrule = resolve_automation_rrule(params.rrule.as_deref());
        let next_run_at = next_run_at_for(
            id.as_str(),
            &params.target,
            params.status,
            rrule.as_str(),
            /*last_run_at*/ None,
            now,
        )?;
        let dispatch_json = DispatchSettingsJson::from(params.dispatch_settings.as_ref())?;

        sqlx::query(
            r#"
INSERT INTO automations (
    id,
    owner_thread_id,
    kind,
    name,
    prompt,
    status,
    rrule,
    next_run_at,
    last_run_at,
    created_at,
    updated_at,
    model,
    reasoning_effort,
    cron_cwds_json,
    target_thread_id,
    dispatch_workspace_roots_json,
    dispatch_approval_policy_json,
    dispatch_approvals_reviewer_json,
    dispatch_permission_profile_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.as_str())
        .bind(params.owner_thread_id.to_string())
        .bind(params.target.kind().as_str())
        .bind(params.name.trim())
        .bind(params.prompt.trim())
        .bind(params.status.as_str())
        .bind(rrule.as_str())
        .bind(next_run_at.map(|value| value.timestamp()))
        .bind(now.timestamp())
        .bind(now.timestamp())
        .bind(trim_to_option(params.model.as_deref()))
        .bind(params.reasoning_effort.as_ref().map(ToString::to_string))
        .bind(cron_cwds_json(&params.target)?.as_deref())
        .bind(target_thread_id(&params.target).as_deref())
        .bind(dispatch_json.workspace_roots.as_deref())
        .bind(dispatch_json.approval_policy.as_deref())
        .bind(dispatch_json.approvals_reviewer.as_deref())
        .bind(dispatch_json.permission_profile.as_deref())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get_automation(id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to load created automation {id}"))
    }

    pub async fn update_automation(
        &self,
        params: &AutomationUpdateParams,
    ) -> anyhow::Result<Option<Automation>> {
        self.update_automation_inner(params, /*expected_owner_thread_id*/ None)
            .await
    }

    pub async fn update_automation_if_owner(
        &self,
        params: &AutomationUpdateParams,
        expected_owner_thread_id: ThreadId,
    ) -> anyhow::Result<Option<Automation>> {
        self.update_automation_inner(params, Some(expected_owner_thread_id))
            .await
    }

    async fn update_automation_inner(
        &self,
        params: &AutomationUpdateParams,
        expected_owner_thread_id: Option<ThreadId>,
    ) -> anyhow::Result<Option<Automation>> {
        validate_automation_params(
            params.name.as_str(),
            params.prompt.as_str(),
            &params.target,
            params.dispatch_settings.as_ref(),
        )?;

        let mut tx = self.automations_pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(existing_row) = load_automation_row_tx(&mut tx, params.id.as_str()).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        if let Some(expected_owner_thread_id) = expected_owner_thread_id
            && existing_row.owner_thread_id != expected_owner_thread_id.to_string()
        {
            tx.commit().await?;
            return Ok(None);
        }
        let existing = Automation::try_from(existing_row.clone())?;
        if existing.target.kind() != params.target.kind() {
            anyhow::bail!("changing automation kind is not supported");
        }
        if params.status == AutomationStatus::Active {
            ensure_no_active_heartbeat_conflict(&mut tx, &params.target, Some(params.id.as_str()))
                .await?;
        }

        let now = Utc::now();
        let rrule = resolve_automation_rrule(params.rrule.as_deref());
        let schedule_changed = existing.rrule != rrule;
        let target_changed = existing.target != params.target;
        let next_run_at = if params.status == AutomationStatus::Active {
            if existing.status != AutomationStatus::Active || schedule_changed || target_changed {
                next_run_at_for(
                    params.id.as_str(),
                    &params.target,
                    params.status,
                    rrule.as_str(),
                    existing.last_run_at,
                    now,
                )?
            } else {
                existing.next_run_at
            }
        } else {
            None
        };
        let dispatch_json = DispatchSettingsJson::from(params.dispatch_settings.as_ref())?;

        sqlx::query(
            r#"
UPDATE automations
SET
    owner_thread_id = ?,
    name = ?,
    prompt = ?,
    status = ?,
    rrule = ?,
    next_run_at = ?,
    updated_at = ?,
    model = ?,
    reasoning_effort = ?,
    cron_cwds_json = ?,
    target_thread_id = ?,
    dispatch_workspace_roots_json = ?,
    dispatch_approval_policy_json = ?,
    dispatch_approvals_reviewer_json = ?,
    dispatch_permission_profile_json = ?
WHERE id = ?
            "#,
        )
        .bind(params.owner_thread_id.to_string())
        .bind(params.name.trim())
        .bind(params.prompt.trim())
        .bind(params.status.as_str())
        .bind(rrule.as_str())
        .bind(next_run_at.map(|value| value.timestamp()))
        .bind(now.timestamp())
        .bind(trim_to_option(params.model.as_deref()))
        .bind(params.reasoning_effort.as_ref().map(ToString::to_string))
        .bind(cron_cwds_json(&params.target)?.as_deref())
        .bind(target_thread_id(&params.target).as_deref())
        .bind(dispatch_json.workspace_roots.as_deref())
        .bind(dispatch_json.approval_policy.as_deref())
        .bind(dispatch_json.approvals_reviewer.as_deref())
        .bind(dispatch_json.permission_profile.as_deref())
        .bind(params.id.as_str())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get_automation(params.id.as_str()).await
    }

    pub async fn delete_automation(&self, id: &str) -> anyhow::Result<bool> {
        self.delete_automation_inner(id, /*expected_owner_thread_id*/ None)
            .await
    }

    pub async fn delete_automation_if_owner(
        &self,
        id: &str,
        expected_owner_thread_id: ThreadId,
    ) -> anyhow::Result<bool> {
        self.delete_automation_inner(id, Some(expected_owner_thread_id))
            .await
    }

    async fn delete_automation_inner(
        &self,
        id: &str,
        expected_owner_thread_id: Option<ThreadId>,
    ) -> anyhow::Result<bool> {
        let mut query = sqlx::query(if expected_owner_thread_id.is_some() {
            "DELETE FROM automations WHERE id = ? AND owner_thread_id = ?"
        } else {
            "DELETE FROM automations WHERE id = ?"
        })
        .bind(id);
        if let Some(expected_owner_thread_id) = expected_owner_thread_id {
            query = query.bind(expected_owner_thread_id.to_string());
        }
        let result = query.execute(self.automations_pool.as_ref()).await?;
        Ok(result.rows_affected() == 1)
    }
}

async fn load_automation_row(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> anyhow::Result<Option<AutomationRow>> {
    sqlx::query_as::<_, AutomationRow>(
        r#"
SELECT *
FROM automations
WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

async fn load_automation_row_tx(
    conn: &mut sqlx::SqliteConnection,
    id: &str,
) -> anyhow::Result<Option<AutomationRow>> {
    sqlx::query_as::<_, AutomationRow>(
        r#"
SELECT *
FROM automations
WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(Into::into)
}

async fn ensure_no_active_heartbeat_conflict(
    conn: &mut sqlx::SqliteConnection,
    target: &AutomationTarget,
    exclude_id: Option<&str>,
) -> anyhow::Result<()> {
    let AutomationTarget::Heartbeat { thread_id } = target else {
        return Ok(());
    };
    let existing_id = match exclude_id {
        Some(exclude_id) => {
            sqlx::query_scalar::<_, String>(
                r#"
SELECT id
FROM automations
WHERE kind = 'heartbeat'
  AND status = 'ACTIVE'
  AND target_thread_id = ?
  AND id != ?
LIMIT 1
                "#,
            )
            .bind(thread_id.to_string())
            .bind(exclude_id)
            .fetch_optional(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_scalar::<_, String>(
                r#"
SELECT id
FROM automations
WHERE kind = 'heartbeat'
  AND status = 'ACTIVE'
  AND target_thread_id = ?
LIMIT 1
                "#,
            )
            .bind(thread_id.to_string())
            .fetch_optional(&mut *conn)
            .await?
        }
    };
    if let Some(existing_id) = existing_id {
        anyhow::bail!("active heartbeat already exists for thread {thread_id}: {existing_id}");
    }
    Ok(())
}

fn validate_automation_params(
    name: &str,
    prompt: &str,
    target: &AutomationTarget,
    dispatch_settings: Option<&AutomationDispatchSettings>,
) -> anyhow::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("automation name is required");
    }
    if name.chars().count() > AUTOMATION_NAME_MAX_CHARS {
        anyhow::bail!("automation name is too long");
    }
    let prompt = prompt.trim();
    if prompt.is_empty() {
        anyhow::bail!("automation prompt is required");
    }
    if prompt.chars().count() > AUTOMATION_PROMPT_MAX_CHARS {
        anyhow::bail!("automation prompt is too long");
    }
    match target {
        AutomationTarget::Cron { cwds } => {
            if cwds.is_empty() {
                anyhow::bail!("cron automations require at least one cwd");
            }
            for cwd in cwds {
                if !cwd.is_absolute() {
                    anyhow::bail!("cron automation cwd must be absolute: {}", cwd.display());
                }
            }
        }
        AutomationTarget::Heartbeat { .. } => {
            if dispatch_settings.is_some() {
                anyhow::bail!("heartbeat automations do not use dispatch settings");
            }
        }
    }
    Ok(())
}

fn resolve_automation_rrule(rrule: Option<&str>) -> String {
    trim_to_option(rrule).unwrap_or_else(|| DEFAULT_AUTOMATION_RRULE.to_string())
}

fn next_run_at_for(
    automation_id: &str,
    target: &AutomationTarget,
    status: AutomationStatus,
    rrule: &str,
    last_run_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> anyhow::Result<Option<DateTime<Utc>>> {
    let schedule = AutomationSchedule::parse(rrule)?;
    if status != AutomationStatus::Active {
        return Ok(None);
    }
    compute_next_run_at(target.kind(), automation_id, &schedule, last_run_at, now)
}

fn trim_to_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn cron_cwds_json(target: &AutomationTarget) -> anyhow::Result<Option<String>> {
    match target {
        AutomationTarget::Cron { cwds } => {
            let cwds = cwds
                .iter()
                .map(|cwd| cwd.display().to_string())
                .collect::<Vec<_>>();
            Ok(Some(serde_json::to_string(&cwds)?))
        }
        AutomationTarget::Heartbeat { .. } => Ok(None),
    }
}

fn target_thread_id(target: &AutomationTarget) -> Option<String> {
    match target {
        AutomationTarget::Cron { .. } => None,
        AutomationTarget::Heartbeat { thread_id } => Some(thread_id.to_string()),
    }
}

struct DispatchSettingsJson {
    workspace_roots: Option<String>,
    approval_policy: Option<String>,
    approvals_reviewer: Option<String>,
    permission_profile: Option<String>,
}

impl DispatchSettingsJson {
    fn from(settings: Option<&AutomationDispatchSettings>) -> anyhow::Result<Self> {
        Ok(match settings {
            Some(settings) => Self {
                workspace_roots: Some(serde_json::to_string(&settings.workspace_roots)?),
                approval_policy: Some(serde_json::to_string(&settings.approval_policy)?),
                approvals_reviewer: Some(serde_json::to_string(&settings.approvals_reviewer)?),
                permission_profile: Some(serde_json::to_string(&settings.permission_profile)?),
            },
            None => Self {
                workspace_roots: None,
                approval_policy: None,
                approvals_reviewer: None,
                permission_profile: None,
            },
        })
    }
}

#[cfg(test)]
#[path = "automations_tests.rs"]
mod tests;
