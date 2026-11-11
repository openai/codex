//! Observe the saved choice at native spawn and admission, before a helper can sample.

use crate::CodexThread;
use crate::StartThreadOptions;
use crate::ThreadManager;
use crate::agent::helper_daybreak::SavedIntent;
use crate::agent::helper_daybreak::saved_choice;
use crate::agent::helper_daybreak::turn::HelperTurnAdmission;
use crate::agent::next_thread_spawn_depth;
use crate::agent::types::SpawnAgentOptions;
use crate::agent::types::SpawnProgramSource;
use crate::config::Config;
use crate::config::test_config;
use crate::session::turn_context::NewTurnContextOptions;
use crate::session::turn_context::TurnContext;
use crate::tools::handlers::multi_agents_common::thread_spawn_source;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStartPhase;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::openai_models::SavedDaybreakChoice;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::turn_input::CyberAccessProgram;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnStartOptions;
use codex_protocol::user_input::UserInput;
use codex_thread_store::ThreadMetadataPatch;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;

struct PauseAdmittedTurn(mpsc::UnboundedSender<String>);

impl TurnLifecycleContributor for PauseAdmittedTurn {
    fn turn_start_phase(&self, _: &ExtensionData) -> TurnStartPhase {
        TurnStartPhase::RegularTaskStart
    }

    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            self.0
                .send(input.turn_id.to_owned())
                .expect("test is waiting for admission");
            std::future::pending().await
        })
    }
}

fn text() -> Vec<UserInput> {
    vec![UserInput::Text {
        text: "Continue".into(),
        text_elements: Vec::new(),
    }]
}

async fn admitted_turn(
    turns: &mut mpsc::UnboundedReceiver<String>,
    thread: &CodexThread,
) -> Arc<TurnContext> {
    let id = tokio::time::timeout(std::time::Duration::from_secs(10), turns.recv())
        .await
        .expect("regular task should start")
        .expect("turn observer");
    thread
        .session
        .turn_context_for_sub_id(&id)
        .await
        .expect("turn is paused after admission")
}

async fn spawn_child(
    manager: &ThreadManager,
    config: &Config,
    parent: &CodexThread,
    spawning_turn: &TurnContext,
    program_source: SpawnProgramSource,
) -> codex_protocol::error::Result<Arc<CodexThread>> {
    let parent_id = parent.session.thread_id();
    let source = thread_spawn_source(
        parent_id,
        &parent.session_source,
        next_thread_spawn_depth(&parent.session_source),
        /*agent_role*/ None,
        /*task_name*/ None,
    )
    .expect("native child source");
    let child = parent
        .session
        .services
        .local_agent_runtime
        .control(parent.session.session_id())
        .spawn_agent_with_metadata(
            config.clone(),
            text(),
            source,
            SpawnAgentOptions {
                parent_thread_id: Some(parent_id),
                parent_turn_id: Some(spawning_turn.sub_id.clone()),
                cyber_access_program: spawning_turn.cyber_access_program,
                cyber_access_program_source: program_source,
                ..Default::default()
            },
        )
        .await?;
    Ok(manager
        .get_thread(child.thread_id)
        .await
        .expect("loaded child"))
}

async fn save_choice(thread: &CodexThread, enabled: bool) {
    thread.ensure_rollout_materialized().await;
    let stored = thread
        .update_thread_metadata(
            ThreadMetadataPatch {
                daybreak_enabled: Some(enabled),
                ..Default::default()
            },
            /*include_archived*/ false,
        )
        .await
        .expect("save chat preference");
    assert_eq!(stored.daybreak_enabled, Some(enabled));
}

fn saved(store: &ExtensionData) -> Option<SavedDaybreakChoice> {
    store.get::<SavedIntent>().map(|intent| {
        assert!(intent.owner.is_current());
        intent.choice.clone().expect("read saved choice")
    })
}

async fn paused_helpers(
    home: &std::path::Path,
    programs: Option<Vec<CyberAccessProgram>>,
) -> (Config, ThreadManager, mpsc::UnboundedReceiver<String>) {
    let mut config = test_config().await;
    config.codex_home = home.abs();
    config.cwd = home.abs();
    config.agent_max_depth = 2;
    config.model_provider.supports_websockets = false;
    config.model_provider.base_url = Some("http://127.0.0.1:9/v1".to_owned());
    config.chatgpt_base_url = "http://127.0.0.1:9".to_owned();
    let mut catalog = codex_models_manager::bundled_models_response().expect("local catalog");
    if let Some(programs) = programs {
        for model in &mut catalog.models {
            model.available_access_programs =
                Some(codex_protocol::openai_models::ModelAccessPrograms {
                    cyber: programs.clone(),
                });
        }
    }
    config.model_catalog = Some(catalog);
    config
        .features
        .enable(Feature::DaybreakProgramPropagation)
        .expect("enable propagation");
    let mut manager = ThreadManager::with_models_provider_home_and_state_for_tests(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        config.model_provider.clone(),
        config.codex_home.to_path_buf(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        crate::init_state_db(&config).await,
    );
    let (tx, turns) = mpsc::unbounded_channel();
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_lifecycle_contributor(Arc::new(PauseAdmittedTurn(tx)));
    Arc::get_mut(&mut manager.state)
        .expect("unshared manager")
        .extensions = Arc::new(extensions.build());

    (config, manager, turns)
}

#[test_case::test_case(Some(false), None, None; "saved off")]
#[test_case::test_case(None, Some(false), None; "pending off before materialization")]
#[test_case::test_case(None, None, None; "unset")]
#[test_case::test_case(None, None, Some(CyberAccessProgram::DaybreakRed); "explicit does not invent a choice")]
#[test_case::test_case(Some(true), None, None; "saved on survives toggles and nested spawn")]
#[tokio::test]
async fn native_spawn_captures_the_saved_choice(
    stored: Option<bool>,
    pending: Option<bool>,
    explicit: Option<CyberAccessProgram>,
) {
    let home = tempdir().expect("create test home");
    let (config, manager, mut turns) = paused_helpers(home.path(), /*programs*/ None).await;
    let mut start_options = StartThreadOptions::new(config.clone());
    let thread_store = &manager.state.thread_store;
    if let Some(enabled) = pending {
        let thread_id = manager.reserve_thread_id();
        thread_store
            .stage_pending_thread_metadata(
                thread_id,
                ThreadMetadataPatch {
                    daybreak_enabled: Some(enabled),
                    ..Default::default()
                },
            )
            .await
            .expect("stage chat preference");
        start_options.reserved_thread_id = Some(thread_id);
    }
    let root = manager
        .start_thread(start_options)
        .await
        .expect("start root");
    if let Some(enabled) = stored {
        save_choice(&root.thread, enabled).await;
    }
    if pending.is_some() {
        // Admitting the first user turn can materialize the parent and consume staging.
        let path = root.thread.rollout_path().expect("local rollout path");
        assert!(
            !path.exists(),
            "parent rollout should still be unmaterialized"
        );
        assert_eq!(saved_choice(&root.thread.session).await, Ok(pending));
    }
    root.thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(text()).on_start(TurnStartOptions {
                cyber_access_program: explicit,
                ..Default::default()
            }),
        )
        .await
        .expect("admit root turn");
    let root_turn = admitted_turn(&mut turns, &root.thread).await;
    assert_eq!(saved(&root_turn.extension_data), None);
    if stored == Some(true) {
        // Keep the writer's live file intact while making metadata reads fail.
        let path = root.thread.rollout_path().expect("root rollout");
        let backup = path.with_extension("backup");
        std::fs::rename(&path, &backup).expect("move healthy rollout");
        std::fs::write(&path, b"invalid metadata\n").expect("inject failed metadata read");
        let error = spawn_child(
            &manager,
            &config,
            &root.thread,
            &root_turn,
            SpawnProgramSource::ParentTurn,
        )
        .await
        .err()
        .expect("required read must fail spawn");
        assert!(
            error
                .to_string()
                .contains("Could not read the Daybreak preference"),
            "{error}"
        );
        assert_eq!(manager.list_thread_ids().await, vec![root.thread_id]);
        std::fs::remove_file(&path).expect("remove invalid rollout");
        std::fs::rename(backup, path).expect("restore storage before retry");
    }
    let child = spawn_child(
        &manager,
        &config,
        &root.thread,
        &root_turn,
        SpawnProgramSource::ParentTurn,
    )
    .await
    .expect("start child");
    let child_turn = admitted_turn(&mut turns, &child).await;
    let expected = stored
        .or(pending)
        .map(|enabled| SavedDaybreakChoice::from(Some(enabled)));
    assert_eq!(saved(child.thread_extension_data()), expected);
    assert_eq!(saved(&child_turn.extension_data), expected);
    assert_eq!(child_turn.cyber_access_program, explicit);

    if stored == Some(true) {
        let constructed = child
            .session
            .new_turn_with_default_settings(
                "constructed".into(),
                NewTurnContextOptions {
                    helper_admission: HelperTurnAdmission::capture(&child.session),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(
            saved(&constructed.extension_data),
            Some(SavedDaybreakChoice::On)
        );
        save_choice(&root.thread, /*enabled*/ false).await;
        let sibling = spawn_child(
            &manager,
            &config,
            &root.thread,
            &root_turn,
            SpawnProgramSource::ParentTurn,
        )
        .await
        .expect("start sibling after preference change");
        let sibling_turn = admitted_turn(&mut turns, &sibling).await;
        assert_eq!(
            saved(&sibling_turn.extension_data),
            Some(SavedDaybreakChoice::Off)
        );
        sibling
            .shutdown_and_wait()
            .await
            .expect("shut down sibling");

        let current = root.thread.session.get_config().await;
        let mut refreshed = current.as_ref().clone();
        refreshed
            .features
            .disable(Feature::DaybreakProgramPropagation)
            .unwrap();
        assert_eq!(
            root.thread
                .session
                .refresh_runtime_config(current, refreshed)
                .await,
            crate::ConfigRefreshOutcome::Published
        );
        let unenrolled = spawn_child(
            &manager,
            &config,
            &root.thread,
            &root_turn,
            SpawnProgramSource::ParentTurn,
        )
        .await
        .expect("start child after parent rollout withdrawal");
        let unenrolled_turn = admitted_turn(&mut turns, &unenrolled).await;
        assert_eq!(saved(&unenrolled_turn.extension_data), None);
        assert!(
            unenrolled
                .thread_extension_data()
                .get::<crate::agent::helper_daybreak::Enrollment>()
                .is_none()
        );
        unenrolled
            .shutdown_and_wait()
            .await
            .expect("shut down unenrolled child");

        assert!(manager.remove_thread(&root.thread_id).await.is_some());
        assert!(manager.get_thread(root.thread_id).await.is_err());
        child
            .session
            .abort_all_tasks(TurnAbortReason::Interrupted)
            .await;

        let direct = Some(CyberAccessProgram::DaybreakRed);
        child
            .start_or_steer_turn(
                TurnInputRequest::user_input(text()).on_start(TurnStartOptions {
                    cyber_access_program: direct,
                    ..Default::default()
                }),
            )
            .await
            .expect("start explicit helper turn after parent unload");
        let explicit_turn = admitted_turn(&mut turns, &child).await;
        assert_eq!(explicit_turn.cyber_access_program, direct);
        assert_eq!(
            saved(&explicit_turn.extension_data),
            Some(SavedDaybreakChoice::On)
        );

        // A local toggle affects later turns. This nested spawn belongs to the turn above.
        save_choice(&child, /*enabled*/ false).await;
        let nested = spawn_child(
            &manager,
            &config,
            &child,
            &explicit_turn,
            SpawnProgramSource::ParentTurn,
        )
        .await
        .expect("start nested child");
        let nested_turn = admitted_turn(&mut turns, &nested).await;
        assert_eq!(nested_turn.cyber_access_program, direct);
        assert_eq!(
            saved(nested.thread_extension_data()),
            Some(SavedDaybreakChoice::On)
        );
        assert_eq!(
            saved(&nested_turn.extension_data),
            Some(SavedDaybreakChoice::On)
        );

        child
            .session
            .abort_all_tasks(TurnAbortReason::Interrupted)
            .await;
        child
            .start_or_steer_turn(TurnInputRequest::user_input(text()))
            .await
            .expect("admit next helper turn");
        let next = admitted_turn(&mut turns, &child).await;
        assert_eq!(next.cyber_access_program, None);
        assert_eq!(saved(&next.extension_data), Some(SavedDaybreakChoice::Off));
        assert_eq!(
            saved(child.thread_extension_data()),
            Some(SavedDaybreakChoice::On)
        );
        nested
            .shutdown_and_wait()
            .await
            .expect("shut down nested helper");
    }
    child.shutdown_and_wait().await.expect("shut down helper");
    root.thread
        .shutdown_and_wait()
        .await
        .expect("shut down root");
}

#[tokio::test]
async fn copied_program_rechecks_the_spawning_turn_owner() {
    let home = tempdir().expect("create test home");
    let blue = CyberAccessProgram::DaybreakBlue;
    let (config, mut manager, mut turns) =
        paused_helpers(home.path(), Some(vec![blue, CyberAccessProgram::Standard])).await;
    Arc::get_mut(&mut manager.state)
        .expect("unshared manager")
        .auth_manager = codex_login::AuthManager::from_auth_for_testing_with_home(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        home.path().to_owned(),
    );
    let root = manager
        .start_thread(StartThreadOptions::new(config.clone()))
        .await
        .expect("start root");
    save_choice(&root.thread, /*enabled*/ true).await;
    root.thread
        .start_or_steer_turn(TurnInputRequest::user_input(text()))
        .await
        .expect("start root turn");
    let root_turn = admitted_turn(&mut turns, &root.thread).await;
    let child = spawn_child(
        &manager,
        &config,
        &root.thread,
        &root_turn,
        SpawnProgramSource::ParentTurn,
    )
    .await
    .expect("start child");
    let turn = admitted_turn(&mut turns, &child).await;
    assert_eq!(turn.cyber_access_program, Some(blue));
    assert_eq!(
        crate::agent::helper_daybreak::turn::explicit_program_for_spawn(&turn),
        None
    );

    let auth = manager.auth_manager();
    let previous = auth.auth_owner();
    codex_login::auth::login_with_chatgpt_auth_tokens(
        home.path(),
        "header.e30.changed",
        "other-account",
        /*chatgpt_plan_type*/ None,
    )
    .expect("switch account");
    auth.reload().await;
    assert!(!previous.is_current());
    assert_eq!(
        auth.auth_cached()
            .and_then(|auth| auth.get_account_id())
            .as_deref(),
        Some("other-account")
    );
    let err = spawn_child(
        &manager,
        &config,
        &child,
        &turn,
        SpawnProgramSource::ParentTurn,
    )
    .await
    .err()
    .expect("reject copied program from the previous account");
    assert!(
        err.to_string()
            .contains("saved Daybreak choice has changed"),
        "{err}"
    );
    let explicit = spawn_child(
        &manager,
        &config,
        &child,
        &turn,
        SpawnProgramSource::Explicit,
    )
    .await
    .expect("accept program explicitly chosen for this child");
    let explicit_turn = admitted_turn(&mut turns, &explicit).await;
    assert_eq!(explicit_turn.cyber_access_program, Some(blue));
    assert!(
        explicit_turn
            .extension_data
            .get::<crate::agent::helper_daybreak::turn::SavedProgramResolution>()
            .is_none()
    );
    explicit
        .shutdown_and_wait()
        .await
        .expect("stop explicit child");
    child.shutdown_and_wait().await.expect("stop child");
    root.thread.shutdown_and_wait().await.expect("stop root");
}
