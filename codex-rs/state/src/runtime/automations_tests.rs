use super::*;
use crate::AutomationCreateParams;
use crate::AutomationStatus;
use crate::AutomationTarget;
use crate::AutomationUpdateParams;
use crate::runtime::test_support::unique_temp_dir;
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
