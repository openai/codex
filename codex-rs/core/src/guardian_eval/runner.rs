use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::ApprovalsReviewer;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_extension_api::empty_extension_registry;
use codex_login::AuthManager;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::StreamExt;

use super::case::GuardianEvalCase;
use super::case::GuardianEvalConfig;
use super::case::GuardianEvalThreadItem;
use super::case::load_guardian_eval_cases;
use super::case::select_cases;
use super::report::GuardianEvalActual;
use super::report::GuardianEvalCaseResult;
use super::report::GuardianEvalCaseStatus;
use super::report::GuardianEvalReport;
use crate::config::Config;
use crate::guardian;
use crate::resolve_installation_id;
use crate::session::SessionSettingsUpdate;
use crate::thread_manager::ThreadManager;
use crate::thread_manager::thread_store_from_config;

#[derive(Clone)]
pub struct GuardianEvalOptions {
    pub case_ids: Vec<String>,
    pub model: Option<String>,
    pub concurrency: usize,
    pub dump_prompts: Option<PathBuf>,
    #[doc(hidden)]
    pub base_config: Option<Config>,
    #[doc(hidden)]
    pub auth_manager: Option<Arc<AuthManager>>,
}

impl Default for GuardianEvalOptions {
    fn default() -> Self {
        Self {
            case_ids: Vec::new(),
            model: None,
            concurrency: 1,
            dump_prompts: None,
            base_config: None,
            auth_manager: None,
        }
    }
}

struct GuardianEvalRuntimeOptions {
    base_config: Config,
    auth_manager: Arc<AuthManager>,
    model: Option<String>,
    dump_prompts: Option<PathBuf>,
}

pub async fn run_guardian_eval_suite(
    path: impl AsRef<Path>,
    options: GuardianEvalOptions,
) -> Result<GuardianEvalReport> {
    let concurrency = options.concurrency.max(1);
    let cases = load_guardian_eval_cases(path.as_ref())?;
    let cases = select_cases(cases, &options.case_ids)?;
    let base_config = match options.base_config {
        Some(config) => config,
        None => Config::load_with_cli_overrides(Vec::new())
            .await
            .context("load Codex config")?,
    };
    let auth_manager = match options.auth_manager {
        Some(auth_manager) => auth_manager,
        None => {
            AuthManager::shared_from_config(&base_config, /*enable_codex_api_key_env*/ false).await
        }
    };
    let runtime_options = Arc::new(GuardianEvalRuntimeOptions {
        base_config,
        auth_manager,
        model: options.model,
        dump_prompts: options.dump_prompts,
    });
    let mut results = futures::stream::iter(cases.into_iter().enumerate().map(|(index, case)| {
        let runtime_options = Arc::clone(&runtime_options);
        async move {
            let result = run_guardian_eval_case(case, runtime_options.as_ref()).await;
            (index, result)
        }
    }))
    .buffer_unordered(concurrency)
    .collect::<Vec<_>>()
    .await;
    results.sort_by_key(|(index, _)| *index);
    let results = results
        .into_iter()
        .map(|(_, result)| result)
        .collect::<Vec<_>>();
    Ok(GuardianEvalReport::from_results(results))
}

async fn run_guardian_eval_case(
    case: GuardianEvalCase,
    options: &GuardianEvalRuntimeOptions,
) -> GuardianEvalCaseResult {
    let started_at = Instant::now();
    let result = run_guardian_eval_case_inner(&case, options).await;
    let duration_ms = started_at.elapsed().as_millis();
    match result {
        Ok((actual, selected_model)) => {
            let mismatch_reason = case.expected.mismatch_reason(&actual);
            let status = if mismatch_reason.is_none() {
                GuardianEvalCaseStatus::Passed
            } else {
                GuardianEvalCaseStatus::Mismatch
            };
            GuardianEvalCaseResult {
                id: case.id,
                description: case.description,
                tags: case.tags,
                status,
                expected: case.expected,
                actual: Some(actual),
                selected_model,
                mismatch_reason,
                error: None,
                duration_ms,
            }
        }
        Err(err) => GuardianEvalCaseResult {
            id: case.id,
            description: case.description,
            tags: case.tags,
            status: GuardianEvalCaseStatus::Error,
            expected: case.expected,
            actual: None,
            selected_model: options.model.clone(),
            mismatch_reason: None,
            error: Some(format!("{err:#}")),
            duration_ms,
        },
    }
}

async fn run_guardian_eval_case_inner(
    case: &GuardianEvalCase,
    options: &GuardianEvalRuntimeOptions,
) -> Result<(GuardianEvalActual, Option<String>)> {
    let mut config = options.base_config.clone();
    config.ephemeral = true;
    config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    config.guardian_policy_config = case.config.guardian_policy_config.clone();
    config.cwd = fixture_cwd(&case.config)?;

    let environment_manager = match ExecServerRuntimePaths::from_optional_paths(
        config.codex_self_exe.clone(),
        config.codex_linux_sandbox_exe.clone(),
    ) {
        Ok(local_runtime_paths) => EnvironmentManager::from_codex_home(
            config.codex_home.clone(),
            Some(local_runtime_paths),
        )
        .await
        .context("initialize environment manager")?,
        Err(_) => EnvironmentManager::without_environments(),
    };
    let state_db = None;
    let thread_store = thread_store_from_config(&config, state_db.clone());
    let installation_id = resolve_installation_id(&config.codex_home)
        .await
        .context("resolve installation id")?;
    let thread_manager = ThreadManager::new(
        &config,
        Arc::clone(&options.auth_manager),
        SessionSource::Exec,
        Arc::new(environment_manager),
        empty_extension_registry(),
        /*analytics_events_client*/ None,
        thread_store,
        state_db,
        installation_id,
        /*attestation_provider*/ None,
    );
    let thread = thread_manager
        .start_thread(config)
        .await
        .context("start eval thread")?;
    let session = Arc::clone(&thread.thread.codex.session);
    let mut turn = session
        .new_turn_with_sub_id(
            format!("guardian-eval-{}", case.id),
            SessionSettingsUpdate::default(),
        )
        .await
        .context("create eval turn")?;
    if let Some(model) = &options.model {
        Arc::get_mut(&mut turn)
            .context("eval turn unexpectedly shared before model override")?
            .model_info
            .auto_review_model_override = Some(model.clone());
    }

    let history = case
        .thread
        .iter()
        .map(GuardianEvalThreadItem::to_response_item)
        .collect::<Result<Vec<_>>>()?;
    if !history.is_empty() {
        session
            .record_conversation_items(turn.as_ref(), &history)
            .await;
    }

    let request = case.action.to_guardian_request();
    if let Some(prompt_dir) = &options.dump_prompts {
        dump_guardian_prompt(
            prompt_dir,
            case,
            session.as_ref(),
            turn.as_ref(),
            request.clone(),
        )
        .await?;
    }

    let (outcome, analytics) = guardian::run_guardian_review_session_with_retry(
        Arc::clone(&session),
        turn,
        request,
        case.retry_reason.clone(),
        guardian::guardian_output_schema(),
        /*external_cancel*/ None,
        guardian::GUARDIAN_REVIEW_MAX_ATTEMPTS,
    )
    .await;

    let shutdown = thread.thread.shutdown_and_wait().await;
    let _removed = thread_manager.remove_thread(&thread.thread_id).await;
    shutdown.context("shutdown eval thread")?;

    match outcome {
        guardian::GuardianReviewOutcome::Completed(assessment) => Ok((
            GuardianEvalActual::from_assessment(assessment),
            analytics.guardian_model,
        )),
        guardian::GuardianReviewOutcome::Error(err) => {
            anyhow::bail!("guardian review failed: {err:?}")
        }
    }
}

async fn dump_guardian_prompt(
    prompt_dir: &Path,
    case: &GuardianEvalCase,
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    request: guardian::GuardianApprovalRequest,
) -> Result<()> {
    let prompt = guardian::build_guardian_prompt_items_with_parent_turn(
        session,
        Some(turn),
        case.retry_reason.clone(),
        request,
        guardian::GuardianPromptMode::Full,
    )
    .await
    .context("build guardian prompt for dump")?;
    tokio::fs::create_dir_all(prompt_dir)
        .await
        .with_context(|| format!("create prompt dump directory {}", prompt_dir.display()))?;
    let prompt_path = prompt_dir.join(format!("{}.txt", sanitize_case_id(&case.id)));
    tokio::fs::write(&prompt_path, render_prompt_items(&prompt.items))
        .await
        .with_context(|| format!("write prompt dump {}", prompt_path.display()))?;
    Ok(())
}

fn render_prompt_items(items: &[UserInput]) -> String {
    let mut rendered = String::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            rendered.push_str("\n---\n");
        }
        match item {
            UserInput::Text { text, .. } => rendered.push_str(text),
            other => {
                rendered.push_str(
                    &serde_json::to_string_pretty(other)
                        .unwrap_or_else(|_| "<non-text item>".into()),
                );
            }
        }
    }
    rendered
}

fn fixture_cwd(config: &GuardianEvalConfig) -> Result<AbsolutePathBuf> {
    let cwd = config
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join("codex-guardian-eval"));
    let cwd = if cwd.is_absolute() {
        cwd
    } else {
        std::env::current_dir()
            .context("resolve current directory")?
            .join(cwd)
    };
    std::fs::create_dir_all(&cwd).with_context(|| format!("create eval cwd {}", cwd.display()))?;
    AbsolutePathBuf::try_from(cwd).context("eval cwd must be absolute")
}

fn sanitize_case_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
