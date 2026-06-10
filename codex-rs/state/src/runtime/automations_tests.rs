use super::*;
use crate::AUTOMATION_RETRY_BUDGET;
use crate::AutomationCreateParams;
use crate::AutomationDispatchMode;
use crate::AutomationDispatchOutcome;
use crate::AutomationDispatchRetryOutcome;
use crate::AutomationStatus;
use crate::AutomationTarget;
use crate::AutomationUpdateParams;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;
use chrono::Duration;
use chrono::Utc;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use uuid::Uuid;

fn thread_id() -> ThreadId {
    ThreadId::from_string(&Uuid::new_v4().to_string()).expect("valid thread id")
}

fn cron_create_params(owner_thread_id: ThreadId, cwd: PathBuf) -> AutomationCreateParams {
    AutomationCreateParams {
        owner_thread_id,
        name: "Nightly sweep".to_string(),
        prompt: "Summarize what changed.".to_string(),
        status: AutomationStatus::Active,
        rrule: Some("FREQ=DAILY;BYHOUR=9;BYMINUTE=30".to_string()),
        model: Some("gpt-5".to_string()),
        reasoning_effort: None,
        target: AutomationTarget::Cron { cwds: vec![cwd] },
        dispatch_settings: None,
    }
}

fn heartbeat_create_params(thread_id: ThreadId) -> AutomationCreateParams {
    AutomationCreateParams {
        owner_thread_id: thread_id,
        name: "Heartbeat".to_string(),
        prompt: "Check whether to continue.".to_string(),
        status: AutomationStatus::Active,
        rrule: Some("FREQ=MINUTELY;INTERVAL=30".to_string()),
        model: None,
        reasoning_effort: None,
        target: AutomationTarget::Heartbeat { thread_id },
        dispatch_settings: None,
    }
}

#[tokio::test]
async fn create_update_delete_cron_automation_round_trips() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize");
    let owner_thread_id = thread_id();
    let cwd = codex_home.join("workspace");
    let created = runtime
        .create_automation(&cron_create_params(owner_thread_id, cwd.clone()))
        .await
        .expect("create automation");

    assert_eq!(created.owner_thread_id, owner_thread_id);
    assert_eq!(created.name, "Nightly sweep");
    assert_eq!(created.prompt, "Summarize what changed.");
    assert_eq!(created.status, AutomationStatus::Active);
    assert_eq!(
        created.target,
        AutomationTarget::Cron {
            cwds: vec![cwd.clone()]
        }
    );
    assert!(created.next_run_at.is_some());

    let updated = runtime
        .update_automation(&AutomationUpdateParams {
            id: created.id.clone(),
            owner_thread_id,
            name: "Paused sweep".to_string(),
            prompt: "Wait for now.".to_string(),
            status: AutomationStatus::Paused,
            rrule: Some("FREQ=DAILY;BYHOUR=10;BYMINUTE=0".to_string()),
            model: None,
            reasoning_effort: None,
            target: AutomationTarget::Cron { cwds: vec![cwd] },
            dispatch_settings: None,
        })
        .await
        .expect("update automation")
        .expect("automation should exist");

    assert_eq!(updated.name, "Paused sweep");
    assert_eq!(updated.status, AutomationStatus::Paused);
    assert_eq!(updated.next_run_at, None);
    assert_eq!(runtime.list_automations().await.expect("list").len(), 1);
    assert!(
        runtime
            .delete_automation(created.id.as_str())
            .await
            .expect("delete")
    );
    assert_eq!(
        runtime
            .get_automation(created.id.as_str())
            .await
            .expect("get"),
        None
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn active_heartbeat_targets_are_unique() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize");
    let owner_thread_id = thread_id();
    let target_thread_id = thread_id();
    let params = AutomationCreateParams {
        owner_thread_id,
        name: "Heartbeat".to_string(),
        prompt: "Check whether to continue.".to_string(),
        status: AutomationStatus::Active,
        rrule: Some("FREQ=HOURLY;INTERVAL=1".to_string()),
        model: None,
        reasoning_effort: None,
        target: AutomationTarget::Heartbeat {
            thread_id: target_thread_id,
        },
        dispatch_settings: None,
    };

    runtime
        .create_automation(&params)
        .await
        .expect("create first heartbeat");
    let duplicate = runtime
        .create_automation(&params)
        .await
        .expect_err("duplicate active heartbeat should fail");

    assert!(
        duplicate
            .to_string()
            .contains("active heartbeat already exists"),
        "{duplicate:#}"
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn due_claim_advances_schedule_and_completes() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize");
    let automation = runtime
        .create_automation(&cron_create_params(
            thread_id(),
            codex_home.join("workspace"),
        ))
        .await
        .expect("create automation");
    sqlx::query("UPDATE automations SET next_run_at = ? WHERE id = ?")
        .bind((Utc::now() - Duration::seconds(1)).timestamp())
        .bind(automation.id.as_str())
        .execute(runtime.automations_pool.as_ref())
        .await
        .expect("force due automation");

    let claim = runtime
        .claim_due_automation_dispatch("worker-a")
        .await
        .expect("claim due automation")
        .expect("automation should be due");

    assert_eq!(claim.automation.id, automation.id);
    assert_eq!(claim.dispatch_mode, AutomationDispatchMode::Scheduled);
    assert_eq!(claim.dispatch_cwd_index, 0);
    assert_eq!(claim.attempt_count, 1);
    assert!(claim.next_run_at_after_claim.is_some());
    assert_eq!(
        runtime
            .claim_due_automation_dispatch("worker-b")
            .await
            .expect("second claim should not error"),
        None
    );
    assert!(
        runtime
            .mark_automation_dispatch_started(
                claim.automation.id.as_str(),
                claim.ownership_token.as_str(),
            )
            .await
            .expect("mark started")
    );
    assert!(
        runtime
            .checkpoint_automation_dispatch_progress(
                claim.automation.id.as_str(),
                claim.ownership_token.as_str(),
                /*next_cwd_index*/ 1,
                /*last_error*/ None,
            )
            .await
            .expect("checkpoint")
    );
    assert!(
        runtime
            .mark_automation_dispatch_completed(&claim, /*last_error*/ None)
            .await
            .expect("mark completed")
    );
    let reloaded = runtime
        .get_automation(automation.id.as_str())
        .await
        .expect("load automation")
        .expect("automation should exist");
    assert_eq!(reloaded.last_run_at.is_some(), true);
    assert_eq!(reloaded.next_run_at, claim.next_run_at_after_claim);

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn retryable_failure_requeues_until_retry_budget_is_exhausted() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize");
    let automation = runtime
        .create_automation(&cron_create_params(
            thread_id(),
            codex_home.join("workspace"),
        ))
        .await
        .expect("create automation");
    sqlx::query("UPDATE automations SET next_run_at = ? WHERE id = ?")
        .bind((Utc::now() - Duration::seconds(1)).timestamp())
        .bind(automation.id.as_str())
        .execute(runtime.automations_pool.as_ref())
        .await
        .expect("force due automation");

    let claim = runtime
        .claim_due_automation_dispatch("worker-a")
        .await
        .expect("claim due automation")
        .expect("automation should be due");
    assert_eq!(
        runtime
            .release_automation_dispatch_after_retryable_failure(
                claim.automation.id.as_str(),
                claim.ownership_token.as_str(),
                "temporary failure",
            )
            .await
            .expect("release retry"),
        AutomationDispatchRetryOutcome::ReleasedForRetry
    );
    assert_eq!(
        runtime
            .claim_due_automation_dispatch("worker-b")
            .await
            .expect("retry should wait for retry_at"),
        None
    );
    sqlx::query("UPDATE automations SET retry_at = ?, attempt_count = ? WHERE id = ?")
        .bind((Utc::now() - Duration::seconds(1)).timestamp())
        .bind(AUTOMATION_RETRY_BUDGET)
        .bind(automation.id.as_str())
        .execute(runtime.automations_pool.as_ref())
        .await
        .expect("make retry terminal");

    let terminal_claim = runtime
        .claim_due_automation_dispatch("worker-c")
        .await
        .expect("claim retry")
        .expect("retry should be due");
    assert_eq!(
        runtime
            .release_automation_dispatch_after_retryable_failure(
                terminal_claim.automation.id.as_str(),
                terminal_claim.ownership_token.as_str(),
                "still failing",
            )
            .await
            .expect("terminal retry"),
        AutomationDispatchRetryOutcome::MarkedTerminal
    );
    let reloaded = runtime
        .get_automation(automation.id.as_str())
        .await
        .expect("load automation")
        .expect("automation should exist");
    assert_eq!(reloaded.status, AutomationStatus::Paused);
    assert_eq!(reloaded.next_run_at, None);

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn deferred_scheduled_heartbeat_reopens_next_run_without_consuming() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize");
    let automation = runtime
        .create_automation(&heartbeat_create_params(thread_id()))
        .await
        .expect("create automation");
    sqlx::query("UPDATE automations SET next_run_at = ? WHERE id = ?")
        .bind((Utc::now() - Duration::seconds(1)).timestamp())
        .bind(automation.id.as_str())
        .execute(runtime.automations_pool.as_ref())
        .await
        .expect("force due automation");

    let claim = runtime
        .claim_due_automation_dispatch("worker-a")
        .await
        .expect("claim due automation")
        .expect("automation should be due");
    assert_eq!(claim.dispatch_mode, AutomationDispatchMode::Scheduled);
    assert!(
        runtime
            .mark_automation_dispatch_started(
                claim.automation.id.as_str(),
                claim.ownership_token.as_str(),
            )
            .await
            .expect("mark started")
    );
    let retry_at = Utc::now() + Duration::seconds(60);

    assert!(
        runtime
            .defer_scheduled_automation_dispatch(&claim, retry_at, "busy")
            .await
            .expect("defer heartbeat")
    );

    let reloaded = runtime
        .get_automation(automation.id.as_str())
        .await
        .expect("load automation")
        .expect("automation should exist");
    assert_eq!(reloaded.last_run_at, None);
    assert_eq!(
        reloaded
            .next_run_at
            .expect("defer should set next run")
            .timestamp(),
        retry_at.timestamp()
    );
    assert_eq!(
        runtime
            .claim_due_automation_dispatch("worker-b")
            .await
            .expect("deferred heartbeat should not be immediately due"),
        None
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn scheduled_heartbeat_cooldown_defers_after_recent_thread_activity() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize");
    let now = Utc::now();
    let target_thread_id = thread_id();
    let thread_updated_at = now - Duration::minutes(5);
    let mut metadata =
        test_thread_metadata(&codex_home, target_thread_id, codex_home.join("workspace"));
    metadata.updated_at = thread_updated_at;
    runtime
        .upsert_thread(&metadata)
        .await
        .expect("store target thread metadata");

    let automation = runtime
        .create_automation(&heartbeat_create_params(target_thread_id))
        .await
        .expect("create automation");
    sqlx::query("UPDATE automations SET next_run_at = ? WHERE id = ?")
        .bind((now - Duration::seconds(1)).timestamp())
        .bind(automation.id.as_str())
        .execute(runtime.automations_pool.as_ref())
        .await
        .expect("force due automation");

    let claim = runtime
        .claim_due_automation_dispatch("worker-a")
        .await
        .expect("claim due automation")
        .expect("automation should be due");
    assert!(
        runtime
            .mark_automation_dispatch_started(
                claim.automation.id.as_str(),
                claim.ownership_token.as_str(),
            )
            .await
            .expect("mark started")
    );

    assert!(
        runtime
            .defer_scheduled_heartbeat_for_cooldown_at(&claim, target_thread_id, now)
            .await
            .expect("defer heartbeat for cooldown")
    );

    let reloaded = runtime
        .get_automation(automation.id.as_str())
        .await
        .expect("load automation")
        .expect("automation should exist");
    assert_eq!(reloaded.last_run_at, None);
    assert_eq!(
        reloaded
            .next_run_at
            .expect("cooldown should set next run")
            .timestamp(),
        (thread_updated_at + Duration::minutes(30)).timestamp()
    );
    assert_eq!(
        runtime
            .claim_due_automation_dispatch("worker-b")
            .await
            .expect("cooldown heartbeat should not be immediately due"),
        None
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn run_now_dispatches_paused_automation_once() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize");
    let mut params = cron_create_params(thread_id(), codex_home.join("workspace"));
    params.status = AutomationStatus::Paused;
    let automation = runtime
        .create_automation(&params)
        .await
        .expect("create automation");

    let outcome = runtime
        .claim_automation_run_now(automation.id.as_str(), "worker-a")
        .await
        .expect("claim run now");
    let AutomationDispatchOutcome::Claimed(claim) = outcome else {
        panic!("expected paused automation to be claimed");
    };
    assert_eq!(claim.dispatch_mode, AutomationDispatchMode::Manual);
    assert_eq!(claim.next_run_at_after_claim, None);
    assert!(
        runtime
            .mark_automation_dispatch_completed(&claim, /*last_error*/ None)
            .await
            .expect("mark completed")
    );

    let reloaded = runtime
        .get_automation(automation.id.as_str())
        .await
        .expect("load automation")
        .expect("automation should exist");
    assert_eq!(reloaded.status, AutomationStatus::Paused);
    assert_eq!(reloaded.last_run_at.is_some(), true);
    assert_eq!(reloaded.next_run_at, None);

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}
