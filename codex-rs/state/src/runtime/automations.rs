use super::automation_schedule::AutomationSchedule;
use super::automation_schedule::compute_next_run_at;
use super::*;
use crate::AUTOMATION_CLAIM_LEASE_SECS;
use crate::AUTOMATION_RETRY_BACKOFF_SECS;
use crate::AUTOMATION_RETRY_BUDGET;
use crate::Automation;
use crate::AutomationCreateParams;
use crate::AutomationDispatchClaim;
use crate::AutomationDispatchMode;
use crate::AutomationDispatchOutcome;
use crate::AutomationDispatchRetryOutcome;
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

    pub async fn claim_due_automation_dispatch(
        &self,
        claimed_by: &str,
    ) -> anyhow::Result<Option<AutomationDispatchClaim>> {
        let now = Utc::now();
        let now_ts = now.timestamp();
        let lease_until = now_ts.saturating_add(AUTOMATION_CLAIM_LEASE_SECS.max(0));
        let mut tx = self.automations_pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query_as::<_, AutomationRow>(
            r#"
SELECT *
FROM automations
WHERE (
        (status = 'ACTIVE' AND in_flight_run_at IS NULL AND next_run_at IS NOT NULL AND next_run_at <= ?)
        OR
        (in_flight_run_at IS NOT NULL
         AND (status = 'ACTIVE' OR in_flight_dispatch_mode = 'manual')
         AND (retry_at IS NULL OR retry_at <= ?))
      )
  AND (claimed_by IS NULL OR lease_until IS NULL OR lease_until <= ?)
ORDER BY COALESCE(retry_at, in_flight_run_at, next_run_at) ASC, id ASC
LIMIT 1
            "#,
        )
        .bind(now_ts)
        .bind(now_ts)
        .bind(now_ts)
        .fetch_optional(&mut *tx)
        .await?;
        let claim = match row {
            Some(row) => claim_automation_row(&mut tx, row, claimed_by, now, lease_until).await?,
            None => None,
        };
        tx.commit().await?;
        Ok(claim)
    }

    pub async fn claim_automation_run_now(
        &self,
        id: &str,
        claimed_by: &str,
    ) -> anyhow::Result<AutomationDispatchOutcome> {
        self.claim_automation_run_now_inner(id, /*expected_owner_thread_id*/ None, claimed_by)
            .await
    }

    pub async fn claim_automation_run_now_if_owner(
        &self,
        id: &str,
        expected_owner_thread_id: ThreadId,
        claimed_by: &str,
    ) -> anyhow::Result<AutomationDispatchOutcome> {
        self.claim_automation_run_now_inner(id, Some(expected_owner_thread_id), claimed_by)
            .await
    }

    async fn claim_automation_run_now_inner(
        &self,
        id: &str,
        expected_owner_thread_id: Option<ThreadId>,
        claimed_by: &str,
    ) -> anyhow::Result<AutomationDispatchOutcome> {
        let now = Utc::now();
        let now_ts = now.timestamp();
        let lease_until = now_ts.saturating_add(AUTOMATION_CLAIM_LEASE_SECS.max(0));
        let mut tx = self.automations_pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(row) = load_automation_row_tx(&mut tx, id).await? else {
            tx.commit().await?;
            return Ok(AutomationDispatchOutcome::NotFound);
        };
        if let Some(expected_owner_thread_id) = expected_owner_thread_id
            && row.owner_thread_id != expected_owner_thread_id.to_string()
        {
            tx.commit().await?;
            return Ok(AutomationDispatchOutcome::NotFound);
        }
        if claim_is_active(&row, now_ts) {
            tx.commit().await?;
            return Ok(AutomationDispatchOutcome::AlreadyClaimed);
        }

        let claim = if row.in_flight_run_at.is_some() {
            claim_automation_row(&mut tx, row, claimed_by, now, lease_until).await?
        } else {
            claim_automation_row_manual(&mut tx, row, claimed_by, now, lease_until).await?
        };
        tx.commit().await?;

        Ok(match claim {
            Some(claim) => AutomationDispatchOutcome::Claimed(Box::new(claim)),
            None => AutomationDispatchOutcome::AlreadyClaimed,
        })
    }

    pub async fn mark_automation_dispatch_started(
        &self,
        automation_id: &str,
        ownership_token: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let lease_until = now.saturating_add(AUTOMATION_CLAIM_LEASE_SECS.max(0));
        let result = sqlx::query(
            r#"
UPDATE automations
SET
    last_dispatch_started_at = ?,
    lease_until = ?,
    updated_at = ?
WHERE id = ?
  AND ownership_token = ?
  AND claimed_by IS NOT NULL
            "#,
        )
        .bind(now)
        .bind(lease_until)
        .bind(now)
        .bind(automation_id)
        .bind(ownership_token)
        .execute(self.automations_pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn renew_automation_dispatch_lease(
        &self,
        automation_id: &str,
        ownership_token: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let lease_until = now.saturating_add(AUTOMATION_CLAIM_LEASE_SECS.max(0));
        let result = sqlx::query(
            r#"
UPDATE automations
SET
    lease_until = ?,
    updated_at = ?
WHERE id = ?
  AND ownership_token = ?
  AND claimed_by IS NOT NULL
            "#,
        )
        .bind(lease_until)
        .bind(now)
        .bind(automation_id)
        .bind(ownership_token)
        .execute(self.automations_pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn checkpoint_automation_dispatch_progress(
        &self,
        automation_id: &str,
        ownership_token: &str,
        next_cwd_index: usize,
        last_error: Option<&str>,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let lease_until = now.saturating_add(AUTOMATION_CLAIM_LEASE_SECS.max(0));
        let next_cwd_index =
            i64::try_from(next_cwd_index).map_err(|_| anyhow::anyhow!("cwd index overflow"))?;
        let result = sqlx::query(
            r#"
UPDATE automations
SET
    dispatch_cwd_index = ?,
    last_error = ?,
    lease_until = ?,
    updated_at = ?
WHERE id = ?
  AND ownership_token = ?
  AND claimed_by IS NOT NULL
            "#,
        )
        .bind(next_cwd_index)
        .bind(last_error)
        .bind(lease_until)
        .bind(now)
        .bind(automation_id)
        .bind(ownership_token)
        .execute(self.automations_pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn release_automation_dispatch_after_retryable_failure(
        &self,
        automation_id: &str,
        ownership_token: &str,
        error_message: &str,
    ) -> anyhow::Result<AutomationDispatchRetryOutcome> {
        let Some(row) = load_automation_row(self.automations_pool.as_ref(), automation_id).await?
        else {
            return Ok(AutomationDispatchRetryOutcome::LostClaim);
        };
        if row.ownership_token.as_deref() != Some(ownership_token) || row.claimed_by.is_none() {
            return Ok(AutomationDispatchRetryOutcome::LostClaim);
        }
        if row.attempt_count >= AUTOMATION_RETRY_BUDGET {
            let marked_terminal = self
                .mark_automation_dispatch_failed_terminal(
                    automation_id,
                    ownership_token,
                    error_message,
                )
                .await?;
            return Ok(if marked_terminal {
                AutomationDispatchRetryOutcome::MarkedTerminal
            } else {
                AutomationDispatchRetryOutcome::LostClaim
            });
        }

        let now = Utc::now().timestamp();
        let retry_at = now.saturating_add(AUTOMATION_RETRY_BACKOFF_SECS.max(0));
        let result = sqlx::query(
            r#"
UPDATE automations
SET
    claimed_by = NULL,
    ownership_token = NULL,
    lease_until = NULL,
    retry_at = ?,
    last_error = ?,
    updated_at = ?
WHERE id = ?
  AND ownership_token = ?
            "#,
        )
        .bind(retry_at)
        .bind(error_message)
        .bind(now)
        .bind(automation_id)
        .bind(ownership_token)
        .execute(self.automations_pool.as_ref())
        .await?;
        Ok(if result.rows_affected() == 1 {
            AutomationDispatchRetryOutcome::ReleasedForRetry
        } else {
            AutomationDispatchRetryOutcome::LostClaim
        })
    }

    pub async fn mark_automation_dispatch_failed_terminal(
        &self,
        automation_id: &str,
        ownership_token: &str,
        error_message: &str,
    ) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
UPDATE automations
SET
    claimed_by = NULL,
    ownership_token = NULL,
    lease_until = NULL,
    in_flight_run_at = NULL,
    in_flight_dispatch_mode = NULL,
    dispatch_cwd_index = 0,
    retry_at = NULL,
    attempt_count = 0,
    status = CASE
        WHEN in_flight_dispatch_mode = ? THEN ?
        ELSE status
    END,
    next_run_at = CASE
        WHEN in_flight_dispatch_mode = ? THEN NULL
        ELSE next_run_at
    END,
    last_error = ?,
    last_dispatch_succeeded = 0,
    last_dispatch_completed_at = ?,
    last_dispatch_failed_at = ?,
    updated_at = ?
WHERE id = ?
  AND ownership_token = ?
            "#,
        )
        .bind(AutomationDispatchMode::Scheduled.as_str())
        .bind(AutomationStatus::Paused.as_str())
        .bind(AutomationDispatchMode::Scheduled.as_str())
        .bind(error_message)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(automation_id)
        .bind(ownership_token)
        .execute(self.automations_pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn mark_automation_dispatch_completed(
        &self,
        claim: &AutomationDispatchClaim,
        last_error: Option<&str>,
    ) -> anyhow::Result<bool> {
        let mut tx = self.automations_pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(row) = load_automation_row_tx(&mut tx, claim.automation.id.as_str()).await? else {
            tx.commit().await?;
            return Ok(false);
        };
        if row.ownership_token.as_deref() != Some(claim.ownership_token.as_str()) {
            tx.commit().await?;
            return Ok(false);
        }
        let automation = Automation::try_from(row)?;
        let now = Utc::now().timestamp();
        let success = last_error.is_none();
        let next_run_at = match automation.status {
            AutomationStatus::Active => automation.next_run_at,
            AutomationStatus::Paused => None,
        };
        let result = sqlx::query(
            r#"
UPDATE automations
SET
    claimed_by = NULL,
    ownership_token = NULL,
    lease_until = NULL,
    in_flight_run_at = NULL,
    in_flight_dispatch_mode = NULL,
    dispatch_cwd_index = 0,
    retry_at = NULL,
    attempt_count = 0,
    last_error = ?,
    last_run_at = ?,
    next_run_at = ?,
    last_dispatch_succeeded = ?,
    last_dispatch_completed_at = ?,
    last_dispatch_failed_at = ?,
    updated_at = ?
WHERE id = ?
  AND ownership_token = ?
            "#,
        )
        .bind(last_error)
        .bind(now)
        .bind(next_run_at.map(|value| value.timestamp()))
        .bind(success)
        .bind(now)
        .bind((!success).then_some(now))
        .bind(now)
        .bind(automation.id.as_str())
        .bind(claim.ownership_token.as_str())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
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

async fn claim_automation_row(
    conn: &mut sqlx::SqliteConnection,
    row: AutomationRow,
    claimed_by: &str,
    now: DateTime<Utc>,
    lease_until: i64,
) -> anyhow::Result<Option<AutomationDispatchClaim>> {
    let now_ts = now.timestamp();
    let automation = Automation::try_from(row.clone())?;
    let dispatch_mode = match row.in_flight_dispatch_mode.as_deref() {
        Some(value) => AutomationDispatchMode::parse(value)?,
        None => AutomationDispatchMode::Scheduled,
    };
    let in_flight_run_at = match row.in_flight_run_at {
        Some(value) => epoch_seconds_to_datetime(value)?,
        None => now,
    };
    let next_run_at_after_claim =
        if row.in_flight_run_at.is_some() && dispatch_mode == AutomationDispatchMode::Scheduled {
            automation.next_run_at
        } else {
            compute_next_run_at_for_claim(&automation, dispatch_mode, now)?
        };
    let ownership_token = Uuid::new_v4().to_string();

    let result = if row.in_flight_run_at.is_some() {
        sqlx::query(
            r#"
UPDATE automations
SET
    claimed_by = ?,
    ownership_token = ?,
    lease_until = ?,
    retry_at = NULL,
    attempt_count = attempt_count + 1,
    updated_at = ?
WHERE id = ?
  AND in_flight_run_at IS NOT NULL
  AND (claimed_by IS NULL OR lease_until IS NULL OR lease_until <= ?)
  AND (retry_at IS NULL OR retry_at <= ?)
            "#,
        )
        .bind(claimed_by)
        .bind(ownership_token.as_str())
        .bind(lease_until)
        .bind(now_ts)
        .bind(row.id.as_str())
        .bind(now_ts)
        .bind(now_ts)
        .execute(&mut *conn)
        .await?
    } else {
        sqlx::query(
            r#"
UPDATE automations
SET
    claimed_by = ?,
    ownership_token = ?,
    lease_until = ?,
    in_flight_run_at = ?,
    in_flight_dispatch_mode = ?,
    dispatch_cwd_index = 0,
    retry_at = NULL,
    attempt_count = 1,
    last_error = NULL,
    next_run_at = ?,
    updated_at = ?
WHERE id = ?
  AND in_flight_run_at IS NULL
  AND status = 'ACTIVE'
  AND next_run_at IS NOT NULL
  AND next_run_at <= ?
  AND (claimed_by IS NULL OR lease_until IS NULL OR lease_until <= ?)
            "#,
        )
        .bind(claimed_by)
        .bind(ownership_token.as_str())
        .bind(lease_until)
        .bind(in_flight_run_at.timestamp())
        .bind(dispatch_mode.as_str())
        .bind(next_run_at_after_claim.map(|value| value.timestamp()))
        .bind(now_ts)
        .bind(row.id.as_str())
        .bind(now_ts)
        .bind(now_ts)
        .execute(&mut *conn)
        .await?
    };

    if result.rows_affected() == 0 {
        return Ok(None);
    }
    Ok(Some(AutomationDispatchClaim {
        automation,
        ownership_token,
        dispatch_mode,
        in_flight_run_at,
        dispatch_cwd_index: usize::try_from(row.dispatch_cwd_index)
            .map_err(|_| anyhow::anyhow!("invalid persisted cwd index"))?,
        attempt_count: row.attempt_count + 1,
        next_run_at_after_claim,
    }))
}

async fn claim_automation_row_manual(
    conn: &mut sqlx::SqliteConnection,
    row: AutomationRow,
    claimed_by: &str,
    now: DateTime<Utc>,
    lease_until: i64,
) -> anyhow::Result<Option<AutomationDispatchClaim>> {
    let now_ts = now.timestamp();
    let automation = Automation::try_from(row.clone())?;
    let ownership_token = Uuid::new_v4().to_string();
    let next_run_at_after_claim =
        compute_next_run_at_for_claim(&automation, AutomationDispatchMode::Manual, now)?;

    let result = sqlx::query(
        r#"
UPDATE automations
SET
    claimed_by = ?,
    ownership_token = ?,
    lease_until = ?,
    in_flight_run_at = ?,
    in_flight_dispatch_mode = ?,
    dispatch_cwd_index = 0,
    retry_at = NULL,
    attempt_count = 1,
    last_error = NULL,
    next_run_at = ?,
    updated_at = ?
WHERE id = ?
  AND in_flight_run_at IS NULL
  AND (claimed_by IS NULL OR lease_until IS NULL OR lease_until <= ?)
            "#,
    )
    .bind(claimed_by)
    .bind(ownership_token.as_str())
    .bind(lease_until)
    .bind(now_ts)
    .bind(AutomationDispatchMode::Manual.as_str())
    .bind(next_run_at_after_claim.map(|value| value.timestamp()))
    .bind(now_ts)
    .bind(row.id.as_str())
    .bind(now_ts)
    .execute(&mut *conn)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }
    Ok(Some(AutomationDispatchClaim {
        automation,
        ownership_token,
        dispatch_mode: AutomationDispatchMode::Manual,
        in_flight_run_at: now,
        dispatch_cwd_index: 0,
        attempt_count: 1,
        next_run_at_after_claim,
    }))
}

fn claim_is_active(row: &AutomationRow, now_ts: i64) -> bool {
    row.claimed_by.is_some()
        && row
            .lease_until
            .is_some_and(|lease_until| lease_until > now_ts)
}

fn compute_next_run_at_for_claim(
    automation: &Automation,
    dispatch_mode: AutomationDispatchMode,
    now: DateTime<Utc>,
) -> anyhow::Result<Option<DateTime<Utc>>> {
    if automation.status != AutomationStatus::Active {
        return Ok(None);
    }
    if dispatch_mode == AutomationDispatchMode::Manual
        && automation
            .next_run_at
            .is_some_and(|next_run_at| next_run_at > now)
    {
        return Ok(automation.next_run_at);
    }
    let schedule = AutomationSchedule::parse(automation.rrule.as_str())?;
    compute_next_run_at(
        automation.target.kind(),
        automation.id.as_str(),
        &schedule,
        Some(now),
        now,
    )
}

fn epoch_seconds_to_datetime(secs: i64) -> anyhow::Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
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
