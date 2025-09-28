use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::find_rollout_by_conversation_id;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Submission;
use codex_protocol::mcp_protocol::ConversationId;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tracing::error;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Parser)]
pub struct ProtoCli {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Resume a previous session by conversation UUID.
    #[arg(
        long = "session",
        value_name = "UUID",
        conflicts_with = "resume_rollout",
        help = "Resume a saved Codex session by its conversation UUID (printed in exec/tui banners)."
    )]
    pub session: Option<String>,

    /// Resume a previous session from a rollout file on disk.
    #[arg(
        long = "resume-rollout",
        value_name = "FILE",
        help = "Resume from an explicit rollout file, e.g. ~/.codex/sessions/.../rollout-2025-09-27T11-22-33-<uuid>.jsonl"
    )]
    pub resume_rollout: Option<PathBuf>,
}

pub async fn run_main(opts: ProtoCli) -> anyhow::Result<()> {
    if std::io::stdin().is_terminal() {
        anyhow::bail!("Protocol mode expects stdin to be a pipe, not a terminal");
    }

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let ProtoCli {
        mut config_overrides,
        session,
        resume_rollout,
    } = opts;

    let parse_cli_overrides =
        |overrides: &CliConfigOverrides| overrides.parse_overrides().map_err(anyhow::Error::msg);

    let cli_overrides_before_resume = parse_cli_overrides(&config_overrides)?;
    if (session.is_some() || resume_rollout.is_some())
        && cli_overrides_before_resume
            .iter()
            .any(|(key, _)| key == "experimental_resume")
    {
        anyhow::bail!(
            "--session/--resume-rollout cannot be combined with -c experimental_resume overrides"
        );
    }

    let resume_path_override = if let Some(path) = resume_rollout {
        Some(path)
    } else if let Some(session_id) = session {
        let trimmed = session_id.trim();
        let session_uuid =
            Uuid::parse_str(trimmed).with_context(|| format!("Invalid session UUID: {trimmed}"))?;
        let codex_home = find_codex_home().context("Failed to locate Codex home directory")?;
        let conversation_id = ConversationId(session_uuid);
        let found_path = find_rollout_by_conversation_id(&codex_home, &conversation_id)
            .await
            .with_context(|| format!("Failed to search sessions under {}", codex_home.display()))?;
        let path =
            found_path.ok_or_else(|| anyhow::anyhow!("No rollout found for session {trimmed}"))?;
        Some(path)
    } else {
        None
    };

    if let Some(path) = resume_path_override {
        let canonical = std::fs::canonicalize(&path)
            .with_context(|| format!("Failed to resolve rollout path {}", path.display()))?;
        let normalized = if cfg!(windows) {
            canonical.to_string_lossy().replace('\\', "/")
        } else {
            canonical.to_string_lossy().into_owned()
        };
        config_overrides
            .raw_overrides
            .push(format!("experimental_resume=\"{normalized}\""));
    }

    let overrides_vec = parse_cli_overrides(&config_overrides)?;

    let config = Config::load_with_cli_overrides(overrides_vec, ConfigOverrides::default())?;
    // Use conversation_manager API to start a conversation
    let conversation_manager = ConversationManager::new(AuthManager::shared(
        config.codex_home.clone(),
        config.preferred_auth_method,
    ));
    let NewConversation {
        conversation_id: _,
        conversation,
        session_configured,
    } = conversation_manager.new_conversation(config).await?;

    // Simulate streaming the session_configured event.
    let synthetic_event = Event {
        // Fake id value.
        id: "".to_string(),
        msg: EventMsg::SessionConfigured(session_configured),
    };
    let session_configured_event = match serde_json::to_string(&synthetic_event) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to serialize session_configured: {e}");
            return Err(anyhow::Error::from(e));
        }
    };
    println!("{session_configured_event}");

    // Task that reads JSON lines from stdin and forwards to Submission Queue
    let sq_fut = {
        let conversation = conversation.clone();
        async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let mut lines = stdin.lines();
            loop {
                let result = tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        break
                    },
                    res = lines.next_line() => res,
                };

                match result {
                    Ok(Some(line)) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Submission>(line) {
                            Ok(sub) => {
                                if let Err(e) = conversation.submit_with_id(sub).await {
                                    error!("{e:#}");
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("invalid submission: {e}");
                            }
                        }
                    }
                    _ => {
                        info!("Submission queue closed");
                        break;
                    }
                }
            }
        }
    };

    // Task that reads events from the agent and prints them as JSON lines to stdout
    let eq_fut = async move {
        loop {
            let event = tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                event = conversation.next_event() => event,
            };
            match event {
                Ok(event) => {
                    let event_str = match serde_json::to_string(&event) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize event: {e}");
                            continue;
                        }
                    };
                    println!("{event_str}");
                }
                Err(e) => {
                    error!("{e:#}");
                    break;
                }
            }
        }
        info!("Event queue closed");
    };

    tokio::join!(sq_fut, eq_fut);
    Ok(())
}
