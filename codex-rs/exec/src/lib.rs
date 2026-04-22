// - In the default output mode, it is paramount that the only thing written to
//   stdout is the final message (if any).
// - In --json mode, stdout must be valid JSONL, one event per line.
// For both modes, any other output must be written to stderr.
#![deny(clippy::print_stdout)]

mod cli;
mod event_processor;
mod event_processor_with_human_output;
pub(crate) mod event_processor_with_jsonl_output;
pub(crate) mod exec_events;

pub use cli::Cli;
pub use cli::Command;
pub use cli::ReviewArgs;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::EnvironmentManager;
use codex_app_server_client::EnvironmentManagerArgs;
use codex_app_server_client::ExecServerRuntimePaths;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget as ApiReviewTarget;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::Thread as AppServerThread;
use codex_app_server_protocol::ThreadItem as AppServerThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_arg0::Arg0DispatchPaths;
use codex_cloud_requirements::cloud_requirements_loader_for_storage;
use codex_core::check_execpolicy_for_warnings;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_and_loader_overrides;
use codex_core::config::resolve_oss_provider;
use codex_core::config_loader::ConfigLoadError;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::format_config_error_with_source;
use codex_core::find_thread_meta_by_name_str;
use codex_core::format_exec_policy_error_with_source;
use codex_core::path_utils;
use codex_feedback::CodexFeedback;
use codex_git_utils::get_git_repo_root;
use codex_login::AuthConfig;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::default_client::set_default_originator;
use codex_login::enforce_login_restrictions;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use codex_otel::set_parent_from_context;
use codex_otel::traceparent_context_from_env;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_existing_preserving_symlinks;
use codex_utils_cli::SharedCliOptions;
use codex_utils_oss::ensure_oss_provider_ready;
use codex_utils_oss::get_default_model_for_oss_provider;
use event_processor_with_human_output::EventProcessorWithHumanOutput;
pub use event_processor_with_jsonl_output::CodexStatus;
pub use event_processor_with_jsonl_output::CollectedThreadEvents;
pub use event_processor_with_jsonl_output::EventProcessorWithJsonOutput;
pub use exec_events::AgentMessageItem;
pub use exec_events::CollabAgentState;
pub use exec_events::CollabAgentStatus;
pub use exec_events::CollabTool;
pub use exec_events::CollabToolCallItem;
pub use exec_events::CollabToolCallStatus;
pub use exec_events::CommandExecutionItem;
pub use exec_events::CommandExecutionStatus;
pub use exec_events::ErrorItem;
pub use exec_events::FileChangeItem;
pub use exec_events::FileUpdateChange;
pub use exec_events::ItemCompletedEvent;
pub use exec_events::ItemStartedEvent;
pub use exec_events::ItemUpdatedEvent;
pub use exec_events::McpToolCallItem;
pub use exec_events::McpToolCallItemError;
pub use exec_events::McpToolCallItemResult;
pub use exec_events::McpToolCallStatus;
pub use exec_events::PatchApplyStatus;
pub use exec_events::PatchChangeKind;
pub use exec_events::ReasoningItem;
pub use exec_events::ThreadErrorEvent;
pub use exec_events::ThreadEvent;
pub use exec_events::ThreadItem as ExecThreadItem;
pub use exec_events::ThreadItemDetails;
pub use exec_events::ThreadStartedEvent;
pub use exec_events::TodoItem;
pub use exec_events::TodoListItem;
pub use exec_events::TurnCompletedEvent;
pub use exec_events::TurnFailedEvent;
pub use exec_events::TurnStartedEvent;
pub use exec_events::Usage;
pub use exec_events::WebSearchItem;
use serde_json::Value;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use supports_color::Stream;
use tokio::sync::mpsc;
use tracing::Instrument;
use tracing::error;
use tracing::field;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;
use uuid::Uuid;

use crate::cli::Command as ExecCommand;
use crate::event_processor::EventProcessor;

const DEFAULT_ANALYTICS_ENABLED: bool = true;

enum InitialOperation {
    UserTurn {
        items: Vec<UserInput>,
        output_schema: Option<Value>,
    },
    Review {
        review_request: ReviewRequest,
    },
}

enum StdinPromptBehavior {
    /// Read stdin only when there is no positional prompt, which is the legacy
    /// `codex exec` behavior for `codex exec` with piped input.
    RequiredIfPiped,
    /// Always treat stdin as the prompt, used for the explicit `codex exec -`
    /// sentinel and similar forced-stdin call sites.
    Forced,
    /// If stdin is piped alongside a positional prompt, treat stdin as
    /// additional context to append rather than as the primary prompt.
    OptionalAppend,
}

struct RequestIdSequencer {
    next: i64,
}

impl RequestIdSequencer {
    fn new() -> Self {
        Self { next: 1 }
    }

    fn next(&mut self) -> RequestId {
        let id = self.next;
        self.next += 1;
        RequestId::Integer(id)
    }
}

struct ExecRunArgs {
    in_process_start_args: InProcessClientStartArgs,
    command: Option<ExecCommand>,
    config: Config,
    dangerously_bypass_approvals_and_sandbox: bool,
    exec_span: tracing::Span,
    images: Vec<PathBuf>,
    json_mode: bool,
    last_message_file: Option<PathBuf>,
    model_provider: Option<String>,
    oss: bool,
    output_schema_path: Option<PathBuf>,
    prompt: Option<String>,
    skip_git_repo_check: bool,
    stderr_with_ansi: bool,
}

fn exec_root_span() -> tracing::Span {
    info_span!(
        "codex.exec",
        otel.kind = "internal",
        thread.id = field::Empty,
        turn.id = field::Empty,
    )
}

pub async fn run_main(cli: Cli, arg0_paths: Arg0DispatchPaths) -> anyhow::Result<()> {
    if let Err(err) = set_default_originator("codex_exec".to_string()) {
        tracing::warn!(?err, "Failed to set codex exec originator override {err:?}");
    }

    let Cli {
        command,
        shared,
        skip_git_repo_check,
        ephemeral,
        ignore_user_config,
        ignore_rules,
        color,
        last_message_file,
        json: json_mode,
        prompt,
        output_schema: output_schema_path,
        config_overrides,
    } = cli;
    let shared = shared.into_inner();
    let SharedCliOptions {
        images,
        model: model_cli_arg,
        oss,
        oss_provider,
        config_profile,
        sandbox_mode: sandbox_mode_cli_arg,
        full_auto,
        dangerously_bypass_approvals_and_sandbox,
        cwd,
        add_dir,
    } = shared;

    let (_stdout_with_ansi, stderr_with_ansi) = match color {
        cli::Color::Always => (true, true),
        cli::Color::Never => (false, false),
        cli::Color::Auto => (
            supports_color::on_cached(Stream::Stdout).is_some(),
            supports_color::on_cached(Stream::Stderr).is_some(),
        ),
    };
    // Build fmt layer (existing logging) to compose with OTEL layer.
    let default_level = "error";

    // Build env_filter separately and attach via with_filter.
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(stderr_with_ansi)
        .with_writer(std::io::stderr)
        .with_filter(env_filter);

    let sandbox_mode = if full_auto {
        Some(SandboxMode::WorkspaceWrite)
    } else if dangerously_bypass_approvals_and_sandbox {
        Some(SandboxMode::DangerFullAccess)
    } else {
        sandbox_mode_cli_arg.map(Into::<SandboxMode>::into)
    };

    // Parse `-c` overrides from the CLI.
    let cli_kv_overrides = match config_overrides.parse_overrides() {
        Ok(v) => v,
        #[allow(clippy::print_stderr)]
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let resolved_cwd = cwd.clone();
    let config_cwd = match resolved_cwd.as_deref() {
        Some(path) => {
            AbsolutePathBuf::from_absolute_path(canonicalize_existing_preserving_symlinks(path)?)?
        }
        None => AbsolutePathBuf::current_dir()?,
    };

    // we load config.toml here to determine project state.
    #[allow(clippy::print_stderr)]
    let codex_home = match find_codex_home() {
        Ok(codex_home) => codex_home,
        Err(err) => {
            eprintln!("Error finding codex home: {err}");
            std::process::exit(1);
        }
    };

    #[allow(clippy::print_stderr)]
    let loader_overrides = LoaderOverrides {
        ignore_user_config,
        ignore_user_and_project_exec_policy_rules: ignore_rules,
        ..Default::default()
    };

    let config_toml = match load_config_as_toml_with_cli_and_loader_overrides(
        &codex_home,
        Some(&config_cwd),
        cli_kv_overrides.clone(),
        loader_overrides.clone(),
    )
    .await
    {
        Ok(config_toml) => config_toml,
        Err(err) => {
            let config_error = err
                .get_ref()
                .and_then(|err| err.downcast_ref::<ConfigLoadError>())
                .map(ConfigLoadError::config_error);
            if let Some(config_error) = config_error {
                eprintln!(
                    "Error loading config.toml:\n{}",
                    format_config_error_with_source(config_error)
                );
            } else {
                eprintln!("Error loading config.toml: {err}");
            }
            std::process::exit(1);
        }
    };

    let chatgpt_base_url = config_toml
        .chatgpt_base_url
        .clone()
        .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string());
    // TODO(gt): Make cloud requirements failures blocking once we can fail-closed.
    let cloud_requirements = cloud_requirements_loader_for_storage(
        codex_home.to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        config_toml.cli_auth_credentials_store.unwrap_or_default(),
        chatgpt_base_url,
    );
    let run_cli_overrides = cli_kv_overrides.clone();
    let run_loader_overrides = loader_overrides.clone();
    let run_cloud_requirements = cloud_requirements.clone();

    let model_provider = if oss {
        let resolved = resolve_oss_provider(
            oss_provider.as_deref(),
            &config_toml,
            config_profile.clone(),
        );

        if let Some(provider) = resolved {
            Some(provider)
        } else {
            return Err(anyhow::anyhow!(
                "No default OSS provider configured. Use --local-provider=provider or set oss_provider to one of: {LMSTUDIO_OSS_PROVIDER_ID}, {OLLAMA_OSS_PROVIDER_ID} in config.toml"
            ));
        }
    } else {
        None // No OSS mode enabled
    };

    // When using `--oss`, let the bootstrapper pick the model based on selected provider
    let model = if let Some(model) = model_cli_arg {
        Some(model)
    } else if oss {
        model_provider
            .as_ref()
            .and_then(|provider_id| get_default_model_for_oss_provider(provider_id))
            .map(std::borrow::ToOwned::to_owned)
    } else {
        None // No model specified, will use the default.
    };

    // Load configuration and determine approval policy
    let overrides = ConfigOverrides {
        model,
        review_model: None,
        config_profile,
        // Default to never ask for approvals in headless mode. Feature flags can override.
        approval_policy: Some(AskForApproval::Never),
        approvals_reviewer: None,
        sandbox_mode,
        permission_profile: None,
        cwd: resolved_cwd,
        model_provider: model_provider.clone(),
        service_tier: None,
        codex_self_exe: arg0_paths.codex_self_exe.clone(),
        codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
        main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
        js_repl_node_path: None,
        js_repl_node_module_dirs: None,
        zsh_path: None,
        base_instructions: None,
        developer_instructions: None,
        personality: None,
        compact_prompt: None,
        include_apply_patch_tool: None,
        show_raw_agent_reasoning: oss.then_some(true),
        tools_web_search_request: None,
        ephemeral: ephemeral.then_some(true),
        additional_writable_roots: add_dir,
    };

    let config = ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides)
        .harness_overrides(overrides)
        .loader_overrides(loader_overrides)
        .cloud_requirements(cloud_requirements)
        .build()
        .await?;

    #[allow(clippy::print_stderr)]
    match check_execpolicy_for_warnings(&config.config_layer_stack).await {
        Ok(None) => {}
        Ok(Some(err)) | Err(err) => {
            eprintln!(
                "Error loading rules:\n{}",
                format_exec_policy_error_with_source(&err)
            );
            std::process::exit(1);
        }
    }

    set_default_client_residency_requirement(config.enforce_residency.value());

    if let Err(err) = enforce_login_restrictions(&AuthConfig {
        codex_home: config.codex_home.to_path_buf(),
        auth_credentials_store_mode: config.cli_auth_credentials_store_mode,
        forced_login_method: config.forced_login_method,
        forced_chatgpt_workspace_id: config.forced_chatgpt_workspace_id.clone(),
    }) {
        eprintln!("{err}");
        std::process::exit(1);
    }

    let otel = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        codex_core::otel_init::build_provider(
            &config,
            env!("CARGO_PKG_VERSION"),
            /*service_name_override*/ None,
            DEFAULT_ANALYTICS_ENABLED,
        )
    })) {
        Ok(Ok(otel)) => otel,
        Ok(Err(e)) => {
            eprintln!("Could not create otel exporter: {e}");
            None
        }
        Err(_) => {
            eprintln!("Could not create otel exporter: panicked during initialization");
            None
        }
    };

    let otel_logger_layer = otel.as_ref().and_then(|o| o.logger_layer());

    let otel_tracing_layer = otel.as_ref().and_then(|o| o.tracing_layer());

    let _ = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(otel_tracing_layer)
        .with(otel_logger_layer)
        .try_init();

    let exec_span = exec_root_span();
    if let Some(context) = traceparent_context_from_env() {
        set_parent_from_context(&exec_span, context);
    }
    let config_warnings: Vec<ConfigWarningNotification> = config
        .startup_warnings
        .iter()
        .map(|warning| ConfigWarningNotification {
            summary: warning.clone(),
            details: None,
            path: None,
            range: None,
        })
        .collect();
    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        arg0_paths.codex_self_exe.clone(),
        arg0_paths.codex_linux_sandbox_exe.clone(),
    )?;
    let in_process_start_args = InProcessClientStartArgs {
        arg0_paths,
        config: std::sync::Arc::new(config.clone()),
        cli_overrides: run_cli_overrides,
        loader_overrides: run_loader_overrides,
        cloud_requirements: run_cloud_requirements,
        feedback: CodexFeedback::new(),
        log_db: None,
        environment_manager: std::sync::Arc::new(EnvironmentManager::new(
            EnvironmentManagerArgs::from_env(local_runtime_paths),
        )),
        config_warnings,
        session_source: SessionSource::Exec,
        enable_codex_api_key_env: true,
        client_name: "codex_exec".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    };
    run_exec_session(ExecRunArgs {
        in_process_start_args,
        command,
        config,
        dangerously_bypass_approvals_and_sandbox,
        exec_span: exec_span.clone(),
        images,
        json_mode,
        last_message_file,
        model_provider,
        oss,
        output_schema_path,
        prompt,
        skip_git_repo_check,
        stderr_with_ansi,
    })
    .instrument(exec_span)
    .await
}

async fn run_exec_session(args: ExecRunArgs) -> anyhow::Result<()> {
    let ExecRunArgs {
        in_process_start_args,
        command,
        config,
        dangerously_bypass_approvals_and_sandbox,
        exec_span,
        images,
        json_mode,
        last_message_file,
        model_provider,
        oss,
        output_schema_path,
        prompt,
        skip_git_repo_check,
        stderr_with_ansi,
    } = args;

    let mut event_processor: Box<dyn EventProcessor> = match json_mode {
        true => Box::new(EventProcessorWithJsonOutput::new(last_message_file.clone())),
        _ => Box::new(EventProcessorWithHumanOutput::create_with_ansi(
            stderr_with_ansi,
            &config,
            last_message_file.clone(),
        )),
    };
    if oss {
        // We're in the oss section, so provider_id should be Some
        // Let's handle None case gracefully though just in case
        let provider_id = match model_provider.as_ref() {
            Some(id) => id,
            None => {
                error!("OSS provider unexpectedly not set when oss flag is used");
                return Err(anyhow::anyhow!(
                    "OSS provider not set but oss flag was used"
                ));
            }
        };
        ensure_oss_provider_ready(provider_id, &config)
            .await
            .map_err(|e| anyhow::anyhow!("OSS setup failed: {e}"))?;
    }

    let default_cwd = config.cwd.to_path_buf();
    let default_approval_policy = config.permissions.approval_policy.value();
    let default_sandbox_policy = config.permissions.sandbox_policy.get();
    let default_effort = config.model_reasoning_effort;

    let (initial_operation, prompt_summary) = match (command.as_ref(), prompt, images) {
        (Some(ExecCommand::Review(review_cli)), _, _) => {
            let review_request = build_review_request(review_cli)?;
            let summary = codex_core::review_prompts::user_facing_hint(&review_request.target);
            (InitialOperation::Review { review_request }, summary)
        }
        (Some(ExecCommand::Resume(args)), root_prompt, imgs) => {
            let prompt_arg = args
                .prompt
                .clone()
                .or_else(|| {
                    if args.last {
                        args.session_id.clone()
                    } else {
                        None
                    }
                })
                .or(root_prompt);
            let prompt_text = resolve_prompt(prompt_arg);
            let mut items: Vec<UserInput> = imgs
                .into_iter()
                .chain(args.images.iter().cloned())
                .map(|path| UserInput::LocalImage { path })
                .collect();
            items.push(UserInput::Text {
                text: prompt_text.clone(),
                // CLI input doesn't track UI element ranges, so none are available here.
                text_elements: Vec::new(),
            });
            let output_schema = load_output_schema(output_schema_path.clone());
            (
                InitialOperation::UserTurn {
                    items,
                    output_schema,
                },
                prompt_text,
            )
        }
        (None, root_prompt, imgs) => {
            let prompt_text = resolve_root_prompt(root_prompt);
            let mut items: Vec<UserInput> = imgs
                .into_iter()
                .map(|path| UserInput::LocalImage { path })
                .collect();
            items.push(UserInput::Text {
                text: prompt_text.clone(),
                // CLI input doesn't track UI element ranges, so none are available here.
                text_elements: Vec::new(),
            });
            let output_schema = load_output_schema(output_schema_path);
            (
                InitialOperation::UserTurn {
                    items,
                    output_schema,
                },
                prompt_text,
            )
        }
    };

    // When --yolo (dangerously_bypass_approvals_and_sandbox) is set, also skip the git repo check
    // since the user is explicitly running in an externally sandboxed environment.
    if !skip_git_repo_check
        && !dangerously_bypass_approvals_and_sandbox
        && get_git_repo_root(&default_cwd).is_none()
    {
        eprintln!("Not inside a trusted directory and --skip-git-repo-check was not specified.");
        std::process::exit(1);
    }

    let mut request_ids = RequestIdSequencer::new();
    let mut client = InProcessAppServerClient::start(in_process_start_args)
        .await
        .map_err(|err| {
            anyhow::anyhow!("failed to initialize in-process app-server client: {err}")
        })?;

    // Handle resume subcommand through existing `thread/list` + `thread/resume`
    // APIs so exec no longer reaches into rollout storage directly.
    let (primary_thread_id, fallback_session_configured) =
        if let Some(ExecCommand::Resume(args)) = command.as_ref() {
            if let Some(thread_id) = resolve_resume_thread_id(&client, &config, args).await? {
                let response: ThreadResumeResponse = send_request_with_response(
                    &client,
                    ClientRequest::ThreadResume {
                        request_id: request_ids.next(),
                        params: thread_resume_params_from_config(&config, thread_id),
                    },
                    "thread/resume",
                )
                .await
                .map_err(anyhow::Error::msg)?;
                let session_configured = session_configured_from_thread_resume_response(&response)
                    .map_err(anyhow::Error::msg)?;
                (session_configured.session_id, session_configured)
            } else {
                let response: ThreadStartResponse = send_request_with_response(
                    &client,
                    ClientRequest::ThreadStart {
                        request_id: request_ids.next(),
                        params: thread_start_params_from_config(&config),
                    },
                    "thread/start",
                )
                .await
                .map_err(anyhow::Error::msg)?;
                let session_configured = session_configured_from_thread_start_response(&response)
                    .map_err(anyhow::Error::msg)?;
                (session_configured.session_id, session_configured)
            }
        } else {
            let response: ThreadStartResponse = send_request_with_response(
                &client,
                ClientRequest::ThreadStart {
                    request_id: request_ids.next(),
                    params: thread_start_params_from_config(&config),
                },
                "thread/start",
            )
            .await
            .map_err(anyhow::Error::msg)?;
            let session_configured = session_configured_from_thread_start_response(&response)
                .map_err(anyhow::Error::msg)?;
            (session_configured.session_id, session_configured)
        };

    let primary_thread_id_for_span = primary_thread_id.to_string();
    // Use the start/resume response as the authoritative bootstrap payload.
    // Waiting for a later streamed `SessionConfigured` event adds up to 10s of
    // avoidable startup latency on the in-process path.
    let session_configured = fallback_session_configured;

    exec_span.record("thread.id", primary_thread_id_for_span.as_str());

    // Print the effective configuration and initial request so users can see what Codex
    // is using.
    event_processor.print_config_summary(&config, &prompt_summary, &session_configured);
    if !json_mode
        && let Some(message) =
            codex_core::config::system_bwrap_warning(config.permissions.sandbox_policy.get())
    {
        event_processor.process_warning(message);
    }

    info!("Codex initialized with event: {session_configured:?}");

    let (interrupt_tx, mut interrupt_rx) = mpsc::unbounded_channel::<()>();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::debug!("Keyboard interrupt");
            let _ = interrupt_tx.send(());
        }
    });

    let task_id = match initial_operation {
        InitialOperation::UserTurn {
            items,
            output_schema,
        } => {
            let (sandbox_policy, permission_profile) = if matches!(
                default_sandbox_policy,
                SandboxPolicy::ExternalSandbox { .. }
            ) {
                (Some(default_sandbox_policy.clone().into()), None)
            } else {
                let permission_profile = PermissionProfile::from_legacy_sandbox_policy(
                    default_sandbox_policy,
                    &default_cwd,
                );
                (None, Some(permission_profile.into()))
            };
            let response: TurnStartResponse = send_request_with_response(
                &client,
                ClientRequest::TurnStart {
                    request_id: request_ids.next(),
                    params: TurnStartParams {
                        thread_id: primary_thread_id_for_span.clone(),
                        input: items.into_iter().map(Into::into).collect(),
                        responsesapi_client_metadata: None,
                        cwd: Some(default_cwd),
                        approval_policy: Some(default_approval_policy.into()),
                        approvals_reviewer: None,
                        sandbox_policy,
                        permission_profile,
                        model: None,
                        service_tier: None,
                        effort: default_effort,
                        summary: None,
                        personality: None,
                        output_schema,
                        collaboration_mode: None,
                    },
                },
                "turn/start",
            )
            .await
            .map_err(anyhow::Error::msg)?;
            let task_id = response.turn.id;
            info!("Sent prompt with event ID: {task_id}");
            task_id
        }
        InitialOperation::Review { review_request } => {
            let response: ReviewStartResponse = send_request_with_response(
                &client,
                ClientRequest::ReviewStart {
                    request_id: request_ids.next(),
                    params: ReviewStartParams {
                        thread_id: primary_thread_id_for_span.clone(),
                        target: review_target_to_api(review_request.target),
                        delivery: None,
                    },
                },
                "review/start",
            )
            .await
            .map_err(anyhow::Error::msg)?;
            let _ = event_processor.process_server_notification(ServerNotification::TurnStarted(
                TurnStartedNotification {
                    thread_id: response.review_thread_id.clone(),
                    turn: response.turn.clone(),
                },
            ));
            let task_id = response.turn.id;
            info!("Sent review request with event ID: {task_id}");
            task_id
        }
    };
    exec_span.record("turn.id", task_id.as_str());

    // Run the loop until the task is complete.
    // Track whether a fatal error was reported by the server so we can
    // exit with a non-zero status for automation-friendly signaling.
    let mut error_seen = false;
    let mut interrupt_channel_open = true;
    let primary_thread_id_for_requests = primary_thread_id.to_string();
    loop {
        let server_event = tokio::select! {
            maybe_interrupt = interrupt_rx.recv(), if interrupt_channel_open => {
                if maybe_interrupt.is_none() {
                    interrupt_channel_open = false;
                    continue;
                }
                if let Err(err) = send_request_with_response::<TurnInterruptResponse>(
                    &client,
                    ClientRequest::TurnInterrupt {
                        request_id: request_ids.next(),
                        params: TurnInterruptParams {
                            thread_id: primary_thread_id_for_requests.clone(),
                            turn_id: task_id.clone(),
                        },
                    },
                    "turn/interrupt",
                )
                .await
                {
                    warn!("turn/interrupt failed: {err}");
                }
                continue;
            }
            maybe_event = client.next_event() => maybe_event,
        };

        let Some(server_event) = server_event else {
            break;
        };

        match server_event {
            InProcessServerEvent::ServerRequest(request) => {
                handle_server_request(&client, request, &mut error_seen).await;
            }
            InProcessServerEvent::ServerNotification(mut notification) => {
                if let ServerNotification::Error(payload) = &notification {
                    if payload.thread_id == primary_thread_id_for_requests
                        && payload.turn_id == task_id
                        && !payload.will_retry
                    {
                        error_seen = true;
                    }
                } else if let ServerNotification::TurnCompleted(payload) = &notification
                    && payload.thread_id == primary_thread_id_for_requests
                    && payload.turn.id == task_id
                    && matches!(
                        payload.turn.status,
                        codex_app_server_protocol::TurnStatus::Failed
                            | codex_app_server_protocol::TurnStatus::Interrupted
                    )
                {
                    error_seen = true;
                }

                maybe_backfill_turn_completed_items(
                    config.ephemeral,
                    &client,
                    &mut request_ids,
                    &mut notification,
                )
                .await;

                if should_process_notification(
                    &notification,
                    &primary_thread_id_for_requests,
                    &task_id,
                ) {
                    match event_processor.process_server_notification(notification) {
                        CodexStatus::Running => {}
                        CodexStatus::InitiateShutdown => {
                            if let Err(err) = request_shutdown(
                                &client,
                                &mut request_ids,
                                &primary_thread_id_for_requests,
                            )
                            .await
                            {
                                warn!("thread/unsubscribe failed during shutdown: {err}");
                            }
                            break;
                        }
                    }
                }
            }
            InProcessServerEvent::Lagged { skipped } => {
                let message = lagged_event_warning_message(skipped);
                warn!("{message}");
                event_processor.process_warning(message);
            }
        }
    }

    if let Err(err) = client.shutdown().await {
        warn!("in-process app-server shutdown failed: {err}");
    }
    event_processor.print_final_output();
    if error_seen {
        std::process::exit(1);
    }

    Ok(())
}

fn sandbox_mode_from_policy(
    sandbox_policy: &codex_protocol::protocol::SandboxPolicy,
) -> Option<codex_app_server_protocol::SandboxMode> {
    match sandbox_policy {
        codex_protocol::protocol::SandboxPolicy::DangerFullAccess => {
            Some(codex_app_server_protocol::SandboxMode::DangerFullAccess)
        }
        codex_protocol::protocol::SandboxPolicy::ReadOnly { .. } => {
            Some(codex_app_server_protocol::SandboxMode::ReadOnly)
        }
        codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. } => {
            Some(codex_app_server_protocol::SandboxMode::WorkspaceWrite)
        }
        codex_protocol::protocol::SandboxPolicy::ExternalSandbox { .. } => None,
    }
}

fn thread_start_params_from_config(config: &Config) -> ThreadStartParams {
    let permission_profile = permission_profile_override_from_config(config);
    let sandbox = permission_profile
        .is_none()
        .then(|| sandbox_mode_from_policy(config.permissions.sandbox_policy.get()))
        .flatten();
    ThreadStartParams {
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.to_string_lossy().to_string()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: approvals_reviewer_override_from_config(config),
        sandbox,
        permission_profile,
        config: config_request_overrides_from_config(config),
        ephemeral: Some(config.ephemeral),
        ..ThreadStartParams::default()
    }
}

fn thread_resume_params_from_config(config: &Config, thread_id: String) -> ThreadResumeParams {
    let permission_profile = permission_profile_override_from_config(config);
    let sandbox = permission_profile
        .is_none()
        .then(|| sandbox_mode_from_policy(config.permissions.sandbox_policy.get()))
        .flatten();
    ThreadResumeParams {
        thread_id,
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.to_string_lossy().to_string()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: approvals_reviewer_override_from_config(config),
        sandbox,
        permission_profile,
        config: config_request_overrides_from_config(config),
        ..ThreadResumeParams::default()
    }
}

fn permission_profile_override_from_config(
    config: &Config,
) -> Option<codex_app_server_protocol::PermissionProfile> {
    if matches!(
        config.permissions.sandbox_policy.get(),
        SandboxPolicy::ExternalSandbox { .. }
    ) {
        None
    } else {
        Some(config.permissions.permission_profile().into())
    }
}

fn config_request_overrides_from_config(config: &Config) -> Option<HashMap<String, Value>> {
    config
        .active_profile
        .as_ref()
        .map(|profile| HashMap::from([("profile".to_string(), Value::String(profile.clone()))]))
}

fn approvals_reviewer_override_from_config(
    config: &Config,
) -> Option<codex_app_server_protocol::ApprovalsReviewer> {
    Some(config.approvals_reviewer.into())
}

async fn send_request_with_response<T>(
    client: &InProcessAppServerClient,
    request: ClientRequest,
    method: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    client.request_typed(request).await.map_err(|err| {
        if method.is_empty() {
            err.to_string()
        } else {
            format!("{method}: {err}")
        }
    })
}

fn session_configured_from_thread_start_response(
    response: &ThreadStartResponse,
) -> Result<SessionConfiguredEvent, String> {
    session_configured_from_thread_response(
        &response.thread.id,
        response.thread.name.clone(),
        response.thread.path.clone(),
        response.model.clone(),
        response.model_provider.clone(),
        response.service_tier,
        response.approval_policy.to_core(),
        response.approvals_reviewer.to_core(),
        response.sandbox.to_core(),
        response.cwd.clone(),
        response.reasoning_effort,
    )
}

fn session_configured_from_thread_resume_response(
    response: &ThreadResumeResponse,
) -> Result<SessionConfiguredEvent, String> {
    session_configured_from_thread_response(
        &response.thread.id,
        response.thread.name.clone(),
        response.thread.path.clone(),
        response.model.clone(),
        response.model_provider.clone(),
        response.service_tier,
        response.approval_policy.to_core(),
        response.approvals_reviewer.to_core(),
        response.sandbox.to_core(),
        response.cwd.clone(),
        response.reasoning_effort,
    )
}

fn review_target_to_api(target: ReviewTarget) -> ApiReviewTarget {
    match target {
        ReviewTarget::UncommittedChanges => ApiReviewTarget::UncommittedChanges,
        ReviewTarget::BaseBranch { branch } => ApiReviewTarget::BaseBranch { branch },
        ReviewTarget::Commit { sha, title } => ApiReviewTarget::Commit { sha, title },
        ReviewTarget::Custom { instructions } => ApiReviewTarget::Custom { instructions },
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "session mapping keeps explicit fields"
)]
fn session_configured_from_thread_response(
    thread_id: &str,
    thread_name: Option<String>,
    rollout_path: Option<PathBuf>,
    model: String,
    model_provider_id: String,
    service_tier: Option<codex_protocol::config_types::ServiceTier>,
    approval_policy: AskForApproval,
    approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer,
    sandbox_policy: SandboxPolicy,
    cwd: AbsolutePathBuf,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
) -> Result<SessionConfiguredEvent, String> {
    let session_id = codex_protocol::ThreadId::from_string(thread_id)
        .map_err(|err| format!("thread id `{thread_id}` is invalid: {err}"))?;

    Ok(SessionConfiguredEvent {
        session_id,
        forked_from_id: None,
        thread_name,
        model,
        model_provider_id,
        service_tier,
        approval_policy,
        approvals_reviewer,
        sandbox_policy,
        cwd,
        reasoning_effort,
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path,
    })
}

fn lagged_event_warning_message(skipped: usize) -> String {
    format!("in-process app-server event stream lagged; dropped {skipped} events")
}

fn should_process_notification(
    notification: &ServerNotification,
    thread_id: &str,
    turn_id: &str,
) -> bool {
    match notification {
        ServerNotification::ConfigWarning(_) | ServerNotification::DeprecationNotice(_) => true,
        ServerNotification::Error(notification) => {
            notification.thread_id == thread_id && notification.turn_id == turn_id
        }
        ServerNotification::HookCompleted(notification) => {
            notification.thread_id == thread_id
                && notification
                    .turn_id
                    .as_deref()
                    .is_none_or(|candidate| candidate == turn_id)
        }
        ServerNotification::HookStarted(notification) => {
            notification.thread_id == thread_id
                && notification
                    .turn_id
                    .as_deref()
                    .is_none_or(|candidate| candidate == turn_id)
        }
        ServerNotification::ItemCompleted(notification) => {
            notification.thread_id == thread_id && notification.turn_id == turn_id
        }
        ServerNotification::ItemStarted(notification) => {
            notification.thread_id == thread_id && notification.turn_id == turn_id
        }
        ServerNotification::ModelRerouted(notification) => {
            notification.thread_id == thread_id && notification.turn_id == turn_id
        }
        ServerNotification::ThreadTokenUsageUpdated(notification) => {
            notification.thread_id == thread_id && notification.turn_id == turn_id
        }
        ServerNotification::TurnCompleted(notification) => {
            notification.thread_id == thread_id && notification.turn.id == turn_id
        }
        ServerNotification::TurnDiffUpdated(notification) => {
            notification.thread_id == thread_id && notification.turn_id == turn_id
        }
        ServerNotification::TurnPlanUpdated(notification) => {
            notification.thread_id == thread_id && notification.turn_id == turn_id
        }
        ServerNotification::TurnStarted(notification) => {
            notification.thread_id == thread_id && notification.turn.id == turn_id
        }
        _ => false,
    }
}

async fn maybe_backfill_turn_completed_items(
    thread_ephemeral: bool,
    client: &InProcessAppServerClient,
    request_ids: &mut RequestIdSequencer,
    notification: &mut ServerNotification,
) {
    // In-process delivery may drop non-terminal item notifications under backpressure while still
    // guaranteeing `turn/completed`. Because app-server currently emits that completion with an
    // empty `turn.items`, exec does one last `thread/read` here so human/json output can recover
    // the final message and reconcile any still-running items before shutdown.
    if !should_backfill_turn_completed_items(thread_ephemeral, notification) {
        return;
    }

    let ServerNotification::TurnCompleted(payload) = notification else {
        return;
    };

    let response = send_request_with_response::<ThreadReadResponse>(
        client,
        ClientRequest::ThreadRead {
            request_id: request_ids.next(),
            params: ThreadReadParams {
                thread_id: payload.thread_id.clone(),
                include_turns: true,
            },
        },
        "thread/read",
    )
    .await;

    match response {
        Ok(response) => {
            if let Some(items) = turn_items_for_thread(&response.thread, &payload.turn.id) {
                payload.turn.items = items;
            }
        }
        Err(err) => {
            warn!("thread/read failed while backfilling turn items for turn completion: {err}");
        }
    }
}

/// Returns true only when `exec` can safely recover missing turn items from
/// rollout-backed thread history.
fn should_backfill_turn_completed_items(
    thread_ephemeral: bool,
    notification: &ServerNotification,
) -> bool {
    let ServerNotification::TurnCompleted(payload) = notification else {
        return false;
    };

    !thread_ephemeral && payload.turn.items.is_empty()
}

fn turn_items_for_thread(
    thread: &AppServerThread,
    turn_id: &str,
) -> Option<Vec<AppServerThreadItem>> {
    thread
        .turns
        .iter()
        .find(|turn| turn.id == turn_id)
        .map(|turn| turn.items.clone())
}

fn all_thread_source_kinds() -> Vec<ThreadSourceKind> {
    vec![
        ThreadSourceKind::Cli,
        ThreadSourceKind::VsCode,
        ThreadSourceKind::Exec,
        ThreadSourceKind::AppServer,
        ThreadSourceKind::SubAgent,
        ThreadSourceKind::SubAgentReview,
        ThreadSourceKind::SubAgentCompact,
        ThreadSourceKind::SubAgentThreadSpawn,
        ThreadSourceKind::SubAgentOther,
        ThreadSourceKind::Unknown,
    ]
}

async fn latest_thread_cwd(thread: &AppServerThread) -> PathBuf {
    if let Some(path) = thread.path.as_deref()
        && let Some(cwd) = parse_latest_turn_context_cwd(path).await
    {
        return cwd;
    }
    thread.cwd.to_path_buf()
}

async fn parse_latest_turn_context_cwd(path: &Path) -> Option<PathBuf> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(rollout_line) = serde_json::from_str::<RolloutLine>(trimmed) else {
            continue;
        };
        if let RolloutItem::TurnContext(item) = rollout_line.item {
            return Some(item.cwd);
        }
    }
    None
}

fn cwds_match(current_cwd: &Path, session_cwd: &Path) -> bool {
    path_utils::paths_match_after_normalization(current_cwd, session_cwd)
}

async fn resolve_resume_thread_id(
    client: &InProcessAppServerClient,
    config: &Config,
    args: &crate::cli::ResumeArgs,
) -> anyhow::Result<Option<String>> {
    let model_providers = resume_lookup_model_providers(config, args);

    if args.last {
        let mut cursor = None;
        loop {
            let response: ThreadListResponse = send_request_with_response(
                client,
                ClientRequest::ThreadList {
                    request_id: RequestId::Integer(0),
                    params: ThreadListParams {
                        cursor,
                        limit: Some(100),
                        sort_key: Some(ThreadSortKey::UpdatedAt),
                        sort_direction: None,
                        model_providers: model_providers.clone(),
                        source_kinds: Some(all_thread_source_kinds()),
                        archived: Some(false),
                        cwd: None,
                        search_term: None,
                    },
                },
                "thread/list",
            )
            .await
            .map_err(anyhow::Error::msg)?;
            for thread in response.data {
                let latest_cwd = latest_thread_cwd(&thread).await;
                if args.all || cwds_match(config.cwd.as_path(), latest_cwd.as_path()) {
                    return Ok(Some(thread.id));
                }
            }
            let Some(next_cursor) = response.next_cursor else {
                return Ok(None);
            };
            cursor = Some(next_cursor);
        }
    }

    let Some(session_id) = args.session_id.as_deref() else {
        return Ok(None);
    };
    if Uuid::parse_str(session_id).is_ok() {
        return Ok(Some(session_id.to_string()));
    }
    if let Some(state_db) = codex_core::get_state_db(config).await {
        let cwd = (!args.all).then_some(config.cwd.as_path());
        let resolved = state_db
            .find_thread_by_exact_title(
                session_id,
                &[],
                /*model_providers*/ None,
                /*archived_only*/ false,
                cwd,
            )
            .await?;
        if let Some(thread) = resolved {
            return Ok(Some(thread.id.to_string()));
        }
        if let Some((_, session_meta)) =
            find_thread_meta_by_name_str(&config.codex_home, session_id).await?
            && (args.all || cwds_match(config.cwd.as_path(), &session_meta.meta.cwd))
        {
            return Ok(Some(session_meta.meta.id.to_string()));
        }
    }

    let mut cursor = None;
    loop {
        let response: ThreadListResponse = send_request_with_response(
            client,
            ClientRequest::ThreadList {
                request_id: RequestId::Integer(0),
                params: ThreadListParams {
                    cursor,
                    limit: Some(100),
                    sort_key: Some(ThreadSortKey::UpdatedAt),
                    sort_direction: None,
                    model_providers: model_providers.clone(),
                    source_kinds: Some(all_thread_source_kinds()),
                    archived: Some(false),
                    cwd: None,
                    search_term: Some(session_id.to_string()),
                },
            },
            "thread/list",
        )
        .await
        .map_err(anyhow::Error::msg)?;
        for thread in response.data {
            if thread.name.as_deref() != Some(session_id) {
                continue;
            }
            let latest_cwd = latest_thread_cwd(&thread).await;
            if args.all || cwds_match(config.cwd.as_path(), latest_cwd.as_path()) {
                return Ok(Some(thread.id));
            }
        }
        let Some(next_cursor) = response.next_cursor else {
            return Ok(None);
        };
        cursor = Some(next_cursor);
    }
}

fn resume_lookup_model_providers(
    config: &Config,
    args: &crate::cli::ResumeArgs,
) -> Option<Vec<String>> {
    if args.last {
        Some(vec![config.model_provider_id.clone()])
    } else {
        None
    }
}

fn canceled_mcp_server_elicitation_response() -> Result<Value, String> {
    serde_json::to_value(McpServerElicitationRequestResponse {
        action: McpServerElicitationAction::Cancel,
        content: None,
        meta: None,
    })
    .map_err(|err| format!("failed to encode mcp elicitation response: {err}"))
}

async fn request_shutdown(
    client: &InProcessAppServerClient,
    request_ids: &mut RequestIdSequencer,
    thread_id: &str,
) -> Result<(), String> {
    let request = ClientRequest::ThreadUnsubscribe {
        request_id: request_ids.next(),
        params: ThreadUnsubscribeParams {
            thread_id: thread_id.to_string(),
        },
    };
    send_request_with_response::<ThreadUnsubscribeResponse>(client, request, "thread/unsubscribe")
        .await
        .map(|_| ())
}

async fn resolve_server_request(
    client: &InProcessAppServerClient,
    request_id: RequestId,
    value: serde_json::Value,
    method: &str,
) -> Result<(), String> {
    client
        .resolve_server_request(request_id, value)
        .await
        .map_err(|err| format!("failed to resolve `{method}` server request: {err}"))
}

async fn reject_server_request(
    client: &InProcessAppServerClient,
    request_id: RequestId,
    method: &str,
    reason: String,
) -> Result<(), String> {
    client
        .reject_server_request(
            request_id,
            JSONRPCErrorError {
                code: -32000,
                message: reason,
                data: None,
            },
        )
        .await
        .map_err(|err| format!("failed to reject `{method}` server request: {err}"))
}

fn server_request_method_name(request: &ServerRequest) -> String {
    serde_json::to_value(request)
        .ok()
        .and_then(|value| {
            value
                .get("method")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

async fn handle_server_request(
    client: &InProcessAppServerClient,
    request: ServerRequest,
    error_seen: &mut bool,
) {
    let method = server_request_method_name(&request);
    let handle_result = match request {
        ServerRequest::McpServerElicitationRequest { request_id, .. } => {
            // Exec auto-cancels elicitation instead of surfacing it
            // interactively. Preserve that behavior for attached subagent
            // threads too so we do not turn a cancel into a decline/error.
            match canceled_mcp_server_elicitation_response() {
                Ok(value) => {
                    resolve_server_request(
                        client,
                        request_id,
                        value,
                        "mcpServer/elicitation/request",
                    )
                    .await
                }
                Err(err) => Err(err),
            }
        }
        ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
            reject_server_request(
                client,
                request_id,
                &method,
                format!(
                    "command execution approval is not supported in exec mode for thread `{}`",
                    params.thread_id
                ),
            )
            .await
        }
        ServerRequest::FileChangeRequestApproval { request_id, params } => {
            reject_server_request(
                client,
                request_id,
                &method,
                format!(
                    "file change approval is not supported in exec mode for thread `{}`",
                    params.thread_id
                ),
            )
            .await
        }
        ServerRequest::ToolRequestUserInput { request_id, params } => {
            reject_server_request(
                client,
                request_id,
                &method,
                format!(
                    "request_user_input is not supported in exec mode for thread `{}`",
                    params.thread_id
                ),
            )
            .await
        }
        ServerRequest::DynamicToolCall { request_id, params } => {
            reject_server_request(
                client,
                request_id,
                &method,
                format!(
                    "dynamic tool calls are not supported in exec mode for thread `{}`",
                    params.thread_id
                ),
            )
            .await
        }
        ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } => {
            reject_server_request(
                client,
                request_id,
                &method,
                "chatgpt auth token refresh is not supported in exec mode".to_string(),
            )
            .await
        }
        ServerRequest::ApplyPatchApproval { request_id, params } => {
            reject_server_request(
                client,
                request_id,
                &method,
                format!(
                    "apply_patch approval is not supported in exec mode for thread `{}`",
                    params.conversation_id
                ),
            )
            .await
        }
        ServerRequest::ExecCommandApproval { request_id, params } => {
            reject_server_request(
                client,
                request_id,
                &method,
                format!(
                    "exec command approval is not supported in exec mode for thread `{}`",
                    params.conversation_id
                ),
            )
            .await
        }
        ServerRequest::PermissionsRequestApproval { request_id, params } => {
            reject_server_request(
                client,
                request_id,
                &method,
                format!(
                    "permissions approval is not supported in exec mode for thread `{}`",
                    params.thread_id
                ),
            )
            .await
        }
    };

    if let Err(err) = handle_result {
        *error_seen = true;
        warn!("{err}");
    }
}

fn load_output_schema(path: Option<PathBuf>) -> Option<Value> {
    let path = path?;

    let schema_str = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!(
                "Failed to read output schema file {}: {err}",
                path.display()
            );
            std::process::exit(1);
        }
    };

    match serde_json::from_str::<Value>(&schema_str) {
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!(
                "Output schema file {} is not valid JSON: {err}",
                path.display()
            );
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PromptDecodeError {
    InvalidUtf8 { valid_up_to: usize },
    InvalidUtf16 { encoding: &'static str },
    UnsupportedBom { encoding: &'static str },
}

impl std::fmt::Display for PromptDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptDecodeError::InvalidUtf8 { valid_up_to } => write!(
                f,
                "input is not valid UTF-8 (invalid byte at offset {valid_up_to}). Convert it to UTF-8 and retry (e.g., `iconv -f <ENC> -t UTF-8 prompt.txt`)."
            ),
            PromptDecodeError::InvalidUtf16 { encoding } => write!(
                f,
                "input looked like {encoding} but could not be decoded. Convert it to UTF-8 and retry."
            ),
            PromptDecodeError::UnsupportedBom { encoding } => write!(
                f,
                "input appears to be {encoding}. Convert it to UTF-8 and retry."
            ),
        }
    }
}

fn decode_prompt_bytes(input: &[u8]) -> Result<String, PromptDecodeError> {
    let input = input.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(input);

    if input.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return Err(PromptDecodeError::UnsupportedBom {
            encoding: "UTF-32LE",
        });
    }

    if input.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return Err(PromptDecodeError::UnsupportedBom {
            encoding: "UTF-32BE",
        });
    }

    if let Some(rest) = input.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, "UTF-16LE", u16::from_le_bytes);
    }

    if let Some(rest) = input.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, "UTF-16BE", u16::from_be_bytes);
    }

    std::str::from_utf8(input)
        .map(str::to_string)
        .map_err(|e| PromptDecodeError::InvalidUtf8 {
            valid_up_to: e.valid_up_to(),
        })
}

fn decode_utf16(
    input: &[u8],
    encoding: &'static str,
    decode_unit: fn([u8; 2]) -> u16,
) -> Result<String, PromptDecodeError> {
    if !input.len().is_multiple_of(2) {
        return Err(PromptDecodeError::InvalidUtf16 { encoding });
    }

    let units: Vec<u16> = input
        .chunks_exact(2)
        .map(|chunk| decode_unit([chunk[0], chunk[1]]))
        .collect();

    String::from_utf16(&units).map_err(|_| PromptDecodeError::InvalidUtf16 { encoding })
}

fn read_prompt_from_stdin(behavior: StdinPromptBehavior) -> Option<String> {
    let stdin_is_terminal = std::io::stdin().is_terminal();

    match behavior {
        StdinPromptBehavior::RequiredIfPiped if stdin_is_terminal => {
            eprintln!(
                "No prompt provided. Either specify one as an argument or pipe the prompt into stdin."
            );
            std::process::exit(1);
        }
        StdinPromptBehavior::RequiredIfPiped => {
            eprintln!("Reading prompt from stdin...");
        }
        StdinPromptBehavior::Forced => {}
        StdinPromptBehavior::OptionalAppend if stdin_is_terminal => return None,
        StdinPromptBehavior::OptionalAppend => {
            eprintln!("Reading additional input from stdin...");
        }
    }

    let mut bytes = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut bytes) {
        eprintln!("Failed to read prompt from stdin: {e}");
        std::process::exit(1);
    }

    let buffer = match decode_prompt_bytes(&bytes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read prompt from stdin: {e}");
            std::process::exit(1);
        }
    };

    if buffer.trim().is_empty() {
        match behavior {
            StdinPromptBehavior::OptionalAppend => None,
            StdinPromptBehavior::RequiredIfPiped | StdinPromptBehavior::Forced => {
                eprintln!("No prompt provided via stdin.");
                std::process::exit(1);
            }
        }
    } else {
        Some(buffer)
    }
}

fn prompt_with_stdin_context(prompt: &str, stdin_text: &str) -> String {
    let mut combined = format!("{prompt}\n\n<stdin>\n{stdin_text}");
    if !stdin_text.ends_with('\n') {
        combined.push('\n');
    }
    combined.push_str("</stdin>");
    combined
}

fn resolve_prompt(prompt_arg: Option<String>) -> String {
    match prompt_arg {
        Some(p) if p != "-" => p,
        maybe_dash => {
            let behavior = if matches!(maybe_dash.as_deref(), Some("-")) {
                StdinPromptBehavior::Forced
            } else {
                StdinPromptBehavior::RequiredIfPiped
            };
            let Some(prompt) = read_prompt_from_stdin(behavior) else {
                unreachable!("required stdin prompt should produce content");
            };
            prompt
        }
    }
}

fn resolve_root_prompt(prompt_arg: Option<String>) -> String {
    match prompt_arg {
        Some(prompt) if prompt != "-" => {
            if let Some(stdin_text) = read_prompt_from_stdin(StdinPromptBehavior::OptionalAppend) {
                prompt_with_stdin_context(&prompt, &stdin_text)
            } else {
                prompt
            }
        }
        maybe_dash => resolve_prompt(maybe_dash),
    }
}

fn build_review_request(args: &ReviewArgs) -> anyhow::Result<ReviewRequest> {
    let target = if args.uncommitted {
        ReviewTarget::UncommittedChanges
    } else if let Some(branch) = args.base.clone() {
        ReviewTarget::BaseBranch { branch }
    } else if let Some(sha) = args.commit.clone() {
        ReviewTarget::Commit {
            sha,
            title: args.commit_title.clone(),
        }
    } else if let Some(prompt_arg) = args.prompt.clone() {
        let prompt = resolve_prompt(Some(prompt_arg)).trim().to_string();
        if prompt.is_empty() {
            anyhow::bail!("Review prompt cannot be empty");
        }
        ReviewTarget::Custom {
            instructions: prompt,
        }
    } else {
        anyhow::bail!(
            "Specify --uncommitted, --base, --commit, or provide custom review instructions"
        );
    };

    Ok(ReviewRequest {
        target,
        user_facing_hint: None,
    })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
