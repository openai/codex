// - In the default output mode, it is paramount that the only thing written to
//   stdout is the final message (if any).
// - In --json mode, stdout must be valid JSONL, one event per line.
// For both modes, any other output must be written to stderr.
#![deny(clippy::print_stdout)]

mod cli;
mod event_processor;
mod event_processor_with_human_output;
pub mod event_processor_with_jsonl_output;
pub mod exec_events;

pub use cli::Cli;
pub use cli::Command;
pub use cli::ReviewArgs;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget as ApiReviewTarget;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_arg0::Arg0DispatchPaths;
use codex_cloud_requirements::cloud_requirements_loader;
use codex_core::AuthManager;
use codex_core::LMSTUDIO_OSS_PROVIDER_ID;
use codex_core::OLLAMA_OSS_PROVIDER_ID;
use codex_core::auth::enforce_login_restrictions;
use codex_core::check_execpolicy_for_warnings;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_overrides;
use codex_core::config::resolve_oss_provider;
use codex_core::config_loader::ConfigLoadError;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::format_config_error_with_source;
use codex_core::format_exec_policy_error_with_source;
use codex_core::git_info::get_git_repo_root;
use codex_feedback::CodexFeedback;
use codex_otel::set_parent_from_context;
use codex_otel::traceparent_context_from_env;
use codex_protocol::account::PlanType as AccountPlanType;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_oss::ensure_oss_provider_ready;
use codex_utils_oss::get_default_model_for_oss_provider;
use event_processor_with_human_output::EventProcessorWithHumanOutput;
use event_processor_with_jsonl_output::EventProcessorWithJsonOutput;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::io::IsTerminal;
use std::io::Read;
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
use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use codex_core::default_client::set_default_client_residency_requirement;
use codex_core::default_client::set_default_originator;
use codex_core::find_thread_path_by_id_str;
use codex_core::find_thread_path_by_name_str;

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
    cursor_ansi: bool,
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
        images,
        model: model_cli_arg,
        oss,
        oss_provider,
        config_profile,
        full_auto,
        dangerously_bypass_approvals_and_sandbox,
        cwd,
        skip_git_repo_check,
        add_dir,
        ephemeral,
        color,
        last_message_file,
        json: json_mode,
        sandbox_mode: sandbox_mode_cli_arg,
        prompt,
        output_schema: output_schema_path,
        config_overrides,
        progress_cursor,
    } = cli;

    let (_stdout_with_ansi, stderr_with_ansi) = match color {
        cli::Color::Always => (true, true),
        cli::Color::Never => (false, false),
        cli::Color::Auto => (
            supports_color::on_cached(Stream::Stdout).is_some(),
            supports_color::on_cached(Stream::Stderr).is_some(),
        ),
    };
    let cursor_ansi = if progress_cursor {
        true
    } else {
        match color {
            cli::Color::Never => false,
            cli::Color::Always => true,
            cli::Color::Auto => {
                if stderr_with_ansi || std::io::stderr().is_terminal() {
                    true
                } else {
                    match std::env::var("TERM") {
                        Ok(term) => !term.is_empty() && term != "dumb",
                        Err(_) => false,
                    }
                }
            }
        }
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
        Some(path) => AbsolutePathBuf::from_absolute_path(path.canonicalize()?)?,
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
    let config_toml = match load_config_as_toml_with_cli_overrides(
        &codex_home,
        &config_cwd,
        cli_kv_overrides.clone(),
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

    let cloud_auth_manager = AuthManager::shared(
        codex_home.clone(),
        false,
        config_toml.cli_auth_credentials_store.unwrap_or_default(),
    );
    let chatgpt_base_url = config_toml
        .chatgpt_base_url
        .clone()
        .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string());
    // TODO(gt): Make cloud requirements failures blocking once we can fail-closed.
    let cloud_requirements =
        cloud_requirements_loader(cloud_auth_manager, chatgpt_base_url, codex_home.clone());
    let run_cli_overrides = cli_kv_overrides.clone();
    let run_loader_overrides = LoaderOverrides::default();
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
        cwd: resolved_cwd,
        model_provider: model_provider.clone(),
        service_tier: None,
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

    if let Err(err) = enforce_login_restrictions(&config) {
        eprintln!("{err}");
        std::process::exit(1);
    }

    let otel = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        codex_core::otel_init::build_provider(
            &config,
            env!("CARGO_PKG_VERSION"),
            None,
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
    let in_process_start_args = InProcessClientStartArgs {
        arg0_paths,
        config: std::sync::Arc::new(config.clone()),
        cli_overrides: run_cli_overrides,
        loader_overrides: run_loader_overrides,
        cloud_requirements: run_cloud_requirements,
        feedback: CodexFeedback::new(),
        config_warnings,
        session_source: SessionSource::Exec,
        enable_codex_api_key_env: true,
        client_name: "codex-exec".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    };
    run_exec_session(ExecRunArgs {
        in_process_start_args,
        command,
        config,
        cursor_ansi,
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
        cursor_ansi,
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
            cursor_ansi,
            &config,
            last_message_file.clone(),
        )),
    };
    let required_mcp_servers: HashSet<String> = config
        .mcp_servers
        .get()
        .iter()
        .filter(|(_, server)| server.enabled && server.required)
        .map(|(name, _)| name.clone())
        .collect();

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

    // Handle resume subcommand by resolving a rollout path and using explicit resume API.
    let (primary_thread_id, fallback_session_configured) =
        if let Some(ExecCommand::Resume(args)) = command.as_ref() {
            let resume_path = resolve_resume_path(&config, args).await?;

            if let Some(path) = resume_path {
                let response: ThreadResumeResponse = send_request_with_response(
                    &client,
                    ClientRequest::ThreadResume {
                        request_id: request_ids.next(),
                        params: thread_resume_params_from_config(&config, Some(path)),
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
    let mut buffered_events = VecDeque::new();
    // Use the start/resume response as the authoritative bootstrap payload.
    // Waiting for a later streamed `SessionConfigured` event adds up to 10s of
    // avoidable startup latency on the in-process path.
    let session_configured = fallback_session_configured;

    exec_span.record("thread.id", primary_thread_id_for_span.as_str());

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
            let prompt_text = resolve_prompt(root_prompt);
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

    // Print the effective configuration and initial request so users can see what Codex
    // is using.
    event_processor.print_config_summary(&config, &prompt_summary, &session_configured);

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
            let response: TurnStartResponse = send_request_with_response(
                &client,
                ClientRequest::TurnStart {
                    request_id: request_ids.next(),
                    params: TurnStartParams {
                        thread_id: primary_thread_id_for_span.clone(),
                        input: items.into_iter().map(Into::into).collect(),
                        cwd: Some(default_cwd),
                        approval_policy: Some(default_approval_policy.into()),
                        approvals_reviewer: None,
                        sandbox_policy: Some(default_sandbox_policy.clone().into()),
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
        let server_event = if let Some(event) = buffered_events.pop_front() {
            Some(event)
        } else {
            tokio::select! {
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
            }
        };

        let Some(server_event) = server_event else {
            break;
        };

        match server_event {
            InProcessServerEvent::ServerRequest(request) => {
                handle_server_request(
                    &client,
                    request,
                    &config,
                    &primary_thread_id_for_requests,
                    &mut error_seen,
                )
                .await;
            }
            InProcessServerEvent::ServerNotification(notification) => {
                if let ServerNotification::Error(payload) = &notification
                    && payload.thread_id == primary_thread_id_for_requests
                    && payload.turn_id == task_id
                    && !payload.will_retry
                {
                    error_seen = true;
                }
            }
            InProcessServerEvent::LegacyNotification(notification) => {
                let decoded = match decode_legacy_notification(notification) {
                    Ok(event) => event,
                    Err(err) => {
                        warn!("{err}");
                        continue;
                    }
                };
                if decoded.conversation_id.as_deref()
                    != Some(primary_thread_id_for_requests.as_str())
                    && decoded.conversation_id.is_some()
                {
                    continue;
                }
                let event = decoded.event;
                if matches!(event.msg, EventMsg::SessionConfigured(_)) {
                    continue;
                }
                if matches!(event.msg, EventMsg::Error(_)) {
                    // The legacy bridge still carries fatal turn failures for
                    // exec. Preserve the non-zero exit behavior until this
                    // path is fully replaced by typed server notifications.
                    error_seen = true;
                }
                match &event.msg {
                    EventMsg::TurnComplete(payload) => {
                        if payload.turn_id != task_id {
                            continue;
                        }
                    }
                    EventMsg::TurnAborted(payload) => {
                        if payload.turn_id.as_deref() != Some(task_id.as_str()) {
                            continue;
                        }
                    }
                    EventMsg::McpStartupUpdate(update) => {
                        if required_mcp_servers.contains(&update.server)
                            && let codex_protocol::protocol::McpStartupStatus::Failed { error } =
                                &update.status
                        {
                            error_seen = true;
                            eprintln!(
                                "Required MCP server '{}' failed to initialize: {error}",
                                update.server
                            );
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
                    _ => {}
                }

                match event_processor.process_event(event) {
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
                    CodexStatus::Shutdown => {
                        // `ShutdownComplete` does not identify which attached
                        // thread emitted it, so subagent shutdowns must not end
                        // the primary exec loop early.
                    }
                }
            }
            InProcessServerEvent::Lagged { skipped } => {
                let message = lagged_event_warning_message(skipped);
                warn!("{message}");
                let _ = event_processor.process_event(Event {
                    id: String::new(),
                    msg: EventMsg::Warning(codex_protocol::protocol::WarningEvent { message }),
                });
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
    ThreadStartParams {
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.to_string_lossy().to_string()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: approvals_reviewer_override_from_config(config),
        sandbox: sandbox_mode_from_policy(config.permissions.sandbox_policy.get()),
        config: config_request_overrides_from_config(config),
        ephemeral: Some(config.ephemeral),
        ..ThreadStartParams::default()
    }
}

fn thread_resume_params_from_config(config: &Config, path: Option<PathBuf>) -> ThreadResumeParams {
    ThreadResumeParams {
        thread_id: "resume".to_string(),
        path,
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.to_string_lossy().to_string()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: approvals_reviewer_override_from_config(config),
        sandbox: sandbox_mode_from_policy(config.permissions.sandbox_policy.get()),
        config: config_request_overrides_from_config(config),
        ..ThreadResumeParams::default()
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

#[expect(
    clippy::too_many_arguments,
    reason = "session mapping keeps explicit fields"
)]
/// Synthesizes startup session metadata from `thread/start` or `thread/resume`.
///
/// This is a compatibility bridge for the current in-process architecture.
/// Some session fields are not available synchronously from the start/resume
/// response, so callers must treat the result as a best-effort fallback until
/// a later `SessionConfigured` event proves otherwise.
/// TODO(architecture): stop synthesizing a partial `SessionConfiguredEvent`
/// here. Either return the authoritative session-configured payload from
/// `thread/start`/`thread/resume`, or introduce a smaller bootstrap type for
/// exec so this path cannot accidentally depend on placeholder fields.
fn session_configured_from_thread_response(
    thread_id: &str,
    thread_name: Option<String>,
    rollout_path: Option<PathBuf>,
    model: String,
    model_provider_id: String,
    service_tier: Option<codex_protocol::config_types::ServiceTier>,
    approval_policy: AskForApproval,
    approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer,
    sandbox_policy: codex_protocol::protocol::SandboxPolicy,
    cwd: PathBuf,
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

fn review_target_to_api(target: ReviewTarget) -> ApiReviewTarget {
    match target {
        ReviewTarget::UncommittedChanges => ApiReviewTarget::UncommittedChanges,
        ReviewTarget::BaseBranch { branch } => ApiReviewTarget::BaseBranch { branch },
        ReviewTarget::Commit { sha, title } => ApiReviewTarget::Commit { sha, title },
        ReviewTarget::Custom { instructions } => ApiReviewTarget::Custom { instructions },
    }
}

fn normalize_legacy_notification_method(method: &str) -> &str {
    method.strip_prefix("codex/event/").unwrap_or(method)
}

fn lagged_event_warning_message(skipped: usize) -> String {
    format!("in-process app-server event stream lagged; dropped {skipped} events")
}

struct DecodedLegacyNotification {
    conversation_id: Option<String>,
    event: Event,
}

fn decode_legacy_notification(
    notification: JSONRPCNotification,
) -> Result<DecodedLegacyNotification, String> {
    let value = notification
        .params
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let method = notification.method;
    let normalized_method = normalize_legacy_notification_method(&method).to_string();
    let serde_json::Value::Object(mut object) = value else {
        return Err(format!(
            "legacy notification `{method}` params were not an object"
        ));
    };
    let conversation_id = object
        .get("conversationId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let mut event_payload = if let Some(serde_json::Value::Object(msg_payload)) = object.get("msg")
    {
        serde_json::Value::Object(msg_payload.clone())
    } else {
        object.remove("conversationId");
        serde_json::Value::Object(object)
    };
    let serde_json::Value::Object(ref mut object) = event_payload else {
        return Err(format!(
            "legacy notification `{method}` event payload was not an object"
        ));
    };
    object.insert(
        "type".to_string(),
        serde_json::Value::String(normalized_method),
    );

    let msg: EventMsg = serde_json::from_value(event_payload)
        .map_err(|err| format!("failed to decode event: {err}"))?;
    Ok(DecodedLegacyNotification {
        conversation_id,
        event: Event {
            id: String::new(),
            msg,
        },
    })
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
    config: &Config,
    _thread_id: &str,
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
        ServerRequest::ChatgptAuthTokensRefresh { request_id, params } => {
            let refresh_result = tokio::task::spawn_blocking({
                let config = config.clone();
                move || local_external_chatgpt_tokens(&config)
            })
            .await;

            match refresh_result {
                Err(err) => {
                    reject_server_request(
                        client,
                        request_id,
                        &method,
                        format!("local chatgpt auth refresh task failed in exec: {err}"),
                    )
                    .await
                }
                Ok(Err(reason)) => reject_server_request(client, request_id, &method, reason).await,
                Ok(Ok(response)) => {
                    if let Some(previous_account_id) = params.previous_account_id.as_deref()
                        && previous_account_id != response.chatgpt_account_id
                    {
                        warn!(
                            "local auth refresh account mismatch: expected `{previous_account_id}`, got `{}`",
                            response.chatgpt_account_id
                        );
                    }
                    match serde_json::to_value(response) {
                        Ok(value) => {
                            resolve_server_request(
                                client,
                                request_id,
                                value,
                                "account/chatgptAuthTokens/refresh",
                            )
                            .await
                        }
                        Err(err) => Err(format!(
                            "failed to serialize chatgpt auth refresh response: {err}"
                        )),
                    }
                }
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

fn local_external_chatgpt_tokens(
    config: &Config,
) -> Result<ChatgptAuthTokensRefreshResponse, String> {
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    auth_manager.set_forced_chatgpt_workspace_id(config.forced_chatgpt_workspace_id.clone());
    auth_manager.reload();

    let auth = auth_manager
        .auth_cached()
        .ok_or_else(|| "no cached auth available for local token refresh".to_string())?;
    if !auth.is_external_chatgpt_tokens() {
        return Err("external ChatGPT token auth is not active".to_string());
    }

    let access_token = auth
        .get_token()
        .map_err(|err| format!("failed to read external access token: {err}"))?;
    let chatgpt_account_id = auth
        .get_account_id()
        .ok_or_else(|| "external token auth is missing chatgpt account id".to_string())?;
    let chatgpt_plan_type = auth.account_plan_type().map(|plan_type| match plan_type {
        AccountPlanType::Free => "free".to_string(),
        AccountPlanType::Go => "go".to_string(),
        AccountPlanType::Plus => "plus".to_string(),
        AccountPlanType::Pro => "pro".to_string(),
        AccountPlanType::Team => "team".to_string(),
        AccountPlanType::Business => "business".to_string(),
        AccountPlanType::Enterprise => "enterprise".to_string(),
        AccountPlanType::Edu => "edu".to_string(),
        AccountPlanType::Unknown => "unknown".to_string(),
    });

    Ok(ChatgptAuthTokensRefreshResponse {
        access_token,
        chatgpt_account_id,
        chatgpt_plan_type,
    })
}

async fn resolve_resume_path(
    config: &Config,
    args: &crate::cli::ResumeArgs,
) -> anyhow::Result<Option<PathBuf>> {
    if args.last {
        let default_provider_filter = vec![config.model_provider_id.clone()];
        let filter_cwd = if args.all {
            None
        } else {
            Some(config.cwd.as_path())
        };
        match codex_core::RolloutRecorder::find_latest_thread_path(
            config,
            1,
            None,
            codex_core::ThreadSortKey::UpdatedAt,
            &[],
            Some(default_provider_filter.as_slice()),
            &config.model_provider_id,
            filter_cwd,
        )
        .await
        {
            Ok(path) => Ok(path),
            Err(e) => {
                error!("Error listing threads: {e}");
                Ok(None)
            }
        }
    } else if let Some(id_str) = args.session_id.as_deref() {
        if Uuid::parse_str(id_str).is_ok() {
            let path = find_thread_path_by_id_str(&config.codex_home, id_str).await?;
            Ok(path)
        } else {
            let path = find_thread_path_by_name_str(&config.codex_home, id_str).await?;
            Ok(path)
        }
    } else {
        Ok(None)
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

fn resolve_prompt(prompt_arg: Option<String>) -> String {
    match prompt_arg {
        Some(p) if p != "-" => p,
        maybe_dash => {
            let force_stdin = matches!(maybe_dash.as_deref(), Some("-"));

            if std::io::stdin().is_terminal() && !force_stdin {
                eprintln!(
                    "No prompt provided. Either specify one as an argument or pipe the prompt into stdin."
                );
                std::process::exit(1);
            }

            if !force_stdin {
                eprintln!("Reading prompt from stdin...");
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
                eprintln!("No prompt provided via stdin.");
                std::process::exit(1);
            }
            buffer
        }
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
mod tests {
    use super::*;
    use codex_otel::set_parent_from_w3c_trace_context;
    use codex_protocol::config_types::ApprovalsReviewer;
    use opentelemetry::trace::TraceContextExt;
    use opentelemetry::trace::TraceId;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    fn test_tracing_subscriber() -> impl tracing::Subscriber + Send + Sync {
        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("codex-exec-tests");
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer))
    }

    #[test]
    fn exec_defaults_analytics_to_enabled() {
        assert_eq!(DEFAULT_ANALYTICS_ENABLED, true);
    }

    #[test]
    fn exec_root_span_can_be_parented_from_trace_context() {
        let subscriber = test_tracing_subscriber();
        let _guard = tracing::subscriber::set_default(subscriber);

        let parent = codex_protocol::protocol::W3cTraceContext {
            traceparent: Some("00-00000000000000000000000000000077-0000000000000088-01".into()),
            tracestate: Some("vendor=value".into()),
        };
        let exec_span = exec_root_span();
        assert!(set_parent_from_w3c_trace_context(&exec_span, &parent));

        let trace_id = exec_span.context().span().span_context().trace_id();
        assert_eq!(
            trace_id,
            TraceId::from_hex("00000000000000000000000000000077").expect("trace id")
        );
    }

    #[test]
    fn builds_uncommitted_review_request() {
        let args = ReviewArgs {
            uncommitted: true,
            base: None,
            commit: None,
            commit_title: None,
            prompt: None,
        };
        let request = build_review_request(&args).expect("builds uncommitted review request");

        let expected = ReviewRequest {
            target: ReviewTarget::UncommittedChanges,
            user_facing_hint: None,
        };

        assert_eq!(request, expected);
    }

    #[test]
    fn builds_commit_review_request_with_title() {
        let args = ReviewArgs {
            uncommitted: false,
            base: None,
            commit: Some("123456789".to_string()),
            commit_title: Some("Add review command".to_string()),
            prompt: None,
        };
        let request = build_review_request(&args).expect("builds commit review request");

        let expected = ReviewRequest {
            target: ReviewTarget::Commit {
                sha: "123456789".to_string(),
                title: Some("Add review command".to_string()),
            },
            user_facing_hint: None,
        };

        assert_eq!(request, expected);
    }

    #[test]
    fn builds_custom_review_request_trims_prompt() {
        let args = ReviewArgs {
            uncommitted: false,
            base: None,
            commit: None,
            commit_title: None,
            prompt: Some("  custom review instructions  ".to_string()),
        };
        let request = build_review_request(&args).expect("builds custom review request");

        let expected = ReviewRequest {
            target: ReviewTarget::Custom {
                instructions: "custom review instructions".to_string(),
            },
            user_facing_hint: None,
        };

        assert_eq!(request, expected);
    }

    #[test]
    fn decode_prompt_bytes_strips_utf8_bom() {
        let input = [0xEF, 0xBB, 0xBF, b'h', b'i', b'\n'];

        let out = decode_prompt_bytes(&input).expect("decode utf-8 with BOM");

        assert_eq!(out, "hi\n");
    }

    #[test]
    fn decode_prompt_bytes_decodes_utf16le_bom() {
        // UTF-16LE BOM + "hi\n"
        let input = [0xFF, 0xFE, b'h', 0x00, b'i', 0x00, b'\n', 0x00];

        let out = decode_prompt_bytes(&input).expect("decode utf-16le with BOM");

        assert_eq!(out, "hi\n");
    }

    #[test]
    fn decode_prompt_bytes_decodes_utf16be_bom() {
        // UTF-16BE BOM + "hi\n"
        let input = [0xFE, 0xFF, 0x00, b'h', 0x00, b'i', 0x00, b'\n'];

        let out = decode_prompt_bytes(&input).expect("decode utf-16be with BOM");

        assert_eq!(out, "hi\n");
    }

    #[test]
    fn decode_prompt_bytes_rejects_utf32le_bom() {
        // UTF-32LE BOM + "hi\n"
        let input = [
            0xFF, 0xFE, 0x00, 0x00, b'h', 0x00, 0x00, 0x00, b'i', 0x00, 0x00, 0x00, b'\n', 0x00,
            0x00, 0x00,
        ];

        let err = decode_prompt_bytes(&input).expect_err("utf-32le should be rejected");

        assert_eq!(
            err,
            PromptDecodeError::UnsupportedBom {
                encoding: "UTF-32LE"
            }
        );
    }

    #[test]
    fn decode_prompt_bytes_rejects_utf32be_bom() {
        // UTF-32BE BOM + "hi\n"
        let input = [
            0x00, 0x00, 0xFE, 0xFF, 0x00, 0x00, 0x00, b'h', 0x00, 0x00, 0x00, b'i', 0x00, 0x00,
            0x00, b'\n',
        ];

        let err = decode_prompt_bytes(&input).expect_err("utf-32be should be rejected");

        assert_eq!(
            err,
            PromptDecodeError::UnsupportedBom {
                encoding: "UTF-32BE"
            }
        );
    }

    #[test]
    fn decode_prompt_bytes_rejects_invalid_utf8() {
        // Invalid UTF-8 sequence: 0xC3 0x28
        let input = [0xC3, 0x28];

        let err = decode_prompt_bytes(&input).expect_err("invalid utf-8 should fail");

        assert_eq!(err, PromptDecodeError::InvalidUtf8 { valid_up_to: 0 });
    }

    #[test]
    fn lagged_event_warning_message_is_explicit() {
        assert_eq!(
            lagged_event_warning_message(7),
            "in-process app-server event stream lagged; dropped 7 events".to_string()
        );
    }

    #[test]
    fn decode_legacy_notification_preserves_conversation_id() {
        let decoded = decode_legacy_notification(JSONRPCNotification {
            method: "codex/event/error".to_string(),
            params: Some(serde_json::json!({
                "conversationId": "thread-123",
                "msg": {
                    "message": "boom"
                }
            })),
        })
        .expect("legacy notification should decode");

        assert_eq!(decoded.conversation_id.as_deref(), Some("thread-123"));
        assert!(matches!(
            decoded.event.msg,
            EventMsg::Error(codex_protocol::protocol::ErrorEvent {
                message,
                codex_error_info: None,
            }) if message == "boom"
        ));
    }

    #[test]
    fn canceled_mcp_server_elicitation_response_uses_cancel_action() {
        let value = canceled_mcp_server_elicitation_response()
            .expect("mcp elicitation cancel response should serialize");
        let response: McpServerElicitationRequestResponse =
            serde_json::from_value(value).expect("cancel response should deserialize");

        assert_eq!(
            response,
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Cancel,
                content: None,
                meta: None,
            }
        );
    }

    #[tokio::test]
    async fn thread_start_params_include_review_policy_when_review_policy_is_manual_only() {
        let codex_home = tempdir().expect("create temp codex home");
        let cwd = tempdir().expect("create temp cwd");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .fallback_cwd(Some(cwd.path().to_path_buf()))
            .build()
            .await
            .expect("build default config");

        let params = thread_start_params_from_config(&config);

        assert_eq!(
            params.approvals_reviewer,
            Some(codex_app_server_protocol::ApprovalsReviewer::User)
        );
    }

    #[tokio::test]
    async fn thread_start_params_include_review_policy_when_auto_review_is_enabled() {
        let codex_home = tempdir().expect("create temp codex home");
        let cwd = tempdir().expect("create temp cwd");
        std::fs::write(
            codex_home.path().join("config.toml"),
            "approvals_reviewer = \"guardian_subagent\"\n",
        )
        .expect("write auto-review config");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .fallback_cwd(Some(cwd.path().to_path_buf()))
            .build()
            .await
            .expect("build auto-review config");

        let params = thread_start_params_from_config(&config);

        assert_eq!(
            params.approvals_reviewer,
            Some(codex_app_server_protocol::ApprovalsReviewer::GuardianSubagent)
        );
    }

    #[test]
    fn session_configured_from_thread_response_uses_review_policy_from_response() {
        let response = ThreadStartResponse {
            thread: codex_app_server_protocol::Thread {
                id: "67e55044-10b1-426f-9247-bb680e5fe0c8".to_string(),
                preview: String::new(),
                ephemeral: false,
                model_provider: "openai".to_string(),
                created_at: 0,
                updated_at: 0,
                status: codex_app_server_protocol::ThreadStatus::Idle,
                path: Some(PathBuf::from("/tmp/rollout.jsonl")),
                cwd: PathBuf::from("/tmp"),
                cli_version: "0.0.0".to_string(),
                source: codex_app_server_protocol::SessionSource::Cli,
                agent_nickname: None,
                agent_role: None,
                git_info: None,
                name: Some("thread".to_string()),
                turns: vec![],
            },
            model: "gpt-5.4".to_string(),
            model_provider: "openai".to_string(),
            service_tier: None,
            cwd: PathBuf::from("/tmp"),
            approval_policy: codex_app_server_protocol::AskForApproval::OnRequest,
            approvals_reviewer: codex_app_server_protocol::ApprovalsReviewer::GuardianSubagent,
            sandbox: codex_app_server_protocol::SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: codex_app_server_protocol::ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            reasoning_effort: None,
        };

        let event = session_configured_from_thread_start_response(&response)
            .expect("build bootstrap session configured event");

        assert_eq!(
            event.approvals_reviewer,
            ApprovalsReviewer::GuardianSubagent
        );
    }
}
