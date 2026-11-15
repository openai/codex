//! Execute a noninteractive turn on an existing app server, using its configuration and credentials.

use super::*;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;

pub(super) async fn run(cli: Cli, endpoint: RemoteAppServerEndpoint) -> anyhow::Result<()> {
    anyhow::ensure!(
        !cli.trust
            && !cli.ignore_user_config
            && !cli.ignore_rules
            && !cli.strict_config
            && !cli.ephemeral
            && cli.thread_source.is_none()
            && cli.cyber_access_program.is_none()
            && cli.output_schema.is_none()
            && cli.config_overrides.raw_overrides.is_empty()
            && cli.images.is_empty()
            && cli.model.is_none()
            && !cli.oss
            && cli.oss_provider.is_none()
            && cli.config_profile_v2.is_none()
            && cli.sandbox_mode.is_none()
            && !cli.auto_review
            && !cli.dangerously_bypass_approvals_and_sandbox
            && !cli.bypass_hook_trust
            && cli.cwd.is_none()
            && !cli.worktree
            && cli.add_dir.is_empty(),
        "--remote currently supports only prompt, resume by UUID, and output options; configure the agent on the server"
    );
    let (session_id, prompt) = match cli.command {
        None => (None, resolve_root_prompt(cli.prompt)),
        Some(ExecCommand::Resume(args)) => {
            anyhow::ensure!(
                !args.last && !args.all && args.images.is_empty(),
                "remote resume requires a thread UUID"
            );
            let id = args
                .session_id
                .filter(|id| Uuid::parse_str(id).is_ok())
                .ok_or_else(|| anyhow::anyhow!("remote resume requires a thread UUID"))?;
            (Some(id), resolve_prompt(args.prompt.or(cli.prompt)))
        }
        _ => anyhow::bail!("--remote supports exec and exec resume"),
    };
    let bound_output = cli
        .last_message_file
        .as_deref()
        .map(codex_exec_server::BoundOutput::open)
        .transpose()?;
    let mut processor: Box<dyn EventProcessor> = if cli.json {
        Box::new(EventProcessorWithJsonOutput::new(cli.last_message_file))
    } else {
        let ansi = match cli.color {
            cli::Color::Always => true,
            cli::Color::Never => false,
            cli::Color::Auto => supports_color::on_cached(Stream::Stderr).is_some(),
        };
        Box::new(EventProcessorWithHumanOutput::create_with_ansi(
            ansi,
            /*config*/ None,
            cli.last_message_file,
        ))
    };
    let client = AppServerClient::Remote(
        RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
            required_human_approval: false,
            endpoint,
            client_name: "codex_exec".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            experimental_api: true,
            mcp_server_openai_form_elicitation: false,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
        })
        .await?,
    );
    let mut ids = RequestIdSequencer::new();
    let thread = if let Some(thread_id) = session_id {
        client
            .request_typed::<ThreadResumeResponse>(ClientRequest::ThreadResume {
                request_id: ids.next(),
                params: ThreadResumeParams {
                    thread_id,
                    exclude_turns: true,
                    ..Default::default()
                },
            })
            .await?
            .thread
    } else {
        client
            .request_typed::<ThreadStartResponse>(ClientRequest::ThreadStart {
                request_id: ids.next(),
                params: ThreadStartParams {
                    approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
                    ..Default::default()
                },
            })
            .await?
            .thread
    };
    anyhow::ensure!(
        !matches!(
            thread.status,
            codex_app_server_protocol::ThreadStatus::Active { .. }
        ),
        "remote thread already has an active turn"
    );
    processor.print_thread_started(&thread.id);
    let (interrupt_tx, interrupt_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = interrupt_tx.send(());
        }
    });
    let response: TurnStartResponse = client
        .request_typed(ClientRequest::TurnStart {
            request_id: ids.next(),
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                turn_trigger: Some("exec".into()),
                input: vec![codex_app_server_protocol::UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                approval_policy: Some(codex_app_server_protocol::AskForApproval::Never),
                ..Default::default()
            },
        })
        .await?;
    finish_turn(
        client,
        ids,
        ExecTurn {
            thread_id: thread.id,
            turn_id: response.turn.id,
            ephemeral: thread.ephemeral,
            history_mode: thread.history_mode,
        },
        processor,
        bound_output,
        interrupt_rx,
    )
    .await
}
