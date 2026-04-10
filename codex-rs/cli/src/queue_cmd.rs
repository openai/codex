//! Implementation for the `codex queue` command.
//!
//! The top-level CLI module owns command routing; this module owns the
//! queue-specific policy for resolving target threads and writing either
//! immediate messages or one-shot timers into the SQLite state database.

use clap::Parser;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::messages::MessagePayload;
use codex_core::timers::ThreadTimerStorageCreateParams;
use codex_core::timers::ThreadTimerTrigger;
use codex_core::timers::TimerDelivery;
use codex_core::timers::build_thread_timer_create_params;
use codex_core::timers::normalize_thread_timer_dtstart_input;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::ThreadId;
use codex_state::StateRuntime;
use codex_tui::Cli as TuiCli;
use codex_utils_cli::CliConfigOverrides;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Parser)]
pub(crate) struct QueueCommand {
    /// Target thread id.
    #[arg(long = "thread", value_name = "THREAD_ID")]
    thread: String,

    /// Message text.
    #[arg(long = "message", value_name = "TEXT")]
    message: String,

    /// Queue a one-shot timer for a local datetime or time, e.g. 2026-04-10T09:30:00 or 09:30.
    #[arg(long = "at", value_name = "WHEN")]
    at: Option<String>,
}

pub(crate) async fn run_queue_command(
    cmd: QueueCommand,
    root_config_overrides: &CliConfigOverrides,
    interactive: &TuiCli,
) -> anyhow::Result<()> {
    let cli_kv_overrides = root_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let overrides = ConfigOverrides {
        config_profile: interactive.config_profile.clone(),
        ..Default::default()
    };
    let config =
        Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides).await?;
    let queue_mode = if cmd.at.is_some() {
        QueueMode::Timer
    } else {
        QueueMode::ImmediateMessage
    };
    validate_queue_feature_flags(&config.features, queue_mode)?;
    let thread_id = resolve_queue_thread_id(config.codex_home.as_path(), &cmd.thread).await?;
    let state_db =
        StateRuntime::init(config.sqlite_home.clone(), config.model_provider_id.clone()).await?;
    let delivery = TimerDelivery::AfterTurn;

    if let Some(at) = cmd.at {
        let dtstart = normalize_thread_timer_dtstart_input(&at).map_err(anyhow::Error::msg)?;
        let timer_params = build_thread_timer_create_params(ThreadTimerStorageCreateParams {
            thread_id,
            source: "external".to_string(),
            client_id: "codex-cli".to_string(),
            trigger: ThreadTimerTrigger::Schedule {
                dtstart: Some(dtstart.clone()),
                rrule: None,
            },
            payload: MessagePayload {
                content: cmd.message,
                instructions: None,
                meta: BTreeMap::new(),
            },
            delivery,
        })
        .map_err(anyhow::Error::msg)?;
        state_db.create_thread_timer(&timer_params).await?;
        println!(
            "{}",
            queue_timer_success_message(
                &timer_params.id,
                &timer_params.thread_id,
                &dtstart,
                timer_params.pending_run,
            )
        );
        return Ok(());
    }

    let message_params = codex_state::ThreadMessageCreateParams::new(
        thread_id,
        "external".to_string(),
        cmd.message,
        /*instructions*/ None,
        "{}".to_string(),
        delivery.as_str().to_string(),
        unix_timestamp_now()?,
    );
    state_db.create_thread_message(&message_params).await?;
    println!(
        "Queued message {} for thread {}.",
        message_params.id, message_params.thread_id
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueMode {
    ImmediateMessage,
    Timer,
}

fn validate_queue_feature_flags(features: &Features, mode: QueueMode) -> anyhow::Result<()> {
    if !features.enabled(Feature::QueuedMessages) {
        anyhow::bail!("codex queue requires the queued_messages feature");
    }
    if mode == QueueMode::Timer && !features.enabled(Feature::Timers) {
        anyhow::bail!("codex queue --at requires the timers feature");
    }
    Ok(())
}

fn queue_timer_success_message(
    timer_id: &str,
    thread_id: &str,
    dtstart: &str,
    pending_run: bool,
) -> String {
    if pending_run {
        format!("Queued timer {timer_id} for thread {thread_id}; it is due now.")
    } else {
        let local_time = dtstart.replace('T', " ");
        format!(
            "Queued timer {timer_id} for thread {thread_id}; it will fire at {local_time} local time."
        )
    }
}

async fn resolve_queue_thread_id(codex_home: &Path, target: &str) -> anyhow::Result<String> {
    if let Ok(thread_id) = ThreadId::from_string(target) {
        return Ok(thread_id.to_string());
    }

    let thread_ids = codex_core::find_thread_ids_by_name(codex_home, target).await?;
    match thread_ids.as_slice() {
        [] => anyhow::bail!("no thread named `{target}`"),
        [thread_id] => Ok(thread_id.to_string()),
        _ => anyhow::bail!("more than one thread is named `{target}`; use a thread id instead"),
    }
}

fn unix_timestamp_now() -> anyhow::Result<i64> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| anyhow::anyhow!("system clock is before unix epoch: {err}"))?;
    i64::try_from(duration.as_secs()).map_err(|_| anyhow::anyhow!("current time is out of range"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MultitoolCli;
    use crate::Subcommand;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn queue_command_parses_immediate_message() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "queue",
            "--thread",
            "thread-1",
            "--message",
            "do work",
        ])
        .expect("parse");
        let Some(Subcommand::Queue(cmd)) = cli.subcommand else {
            unreachable!()
        };

        assert_eq!(cmd.thread, "thread-1");
        assert_eq!(cmd.message, "do work");
        assert_eq!(cmd.at, None);
    }

    #[test]
    fn queue_without_required_args_is_subcommand_error() {
        let err = MultitoolCli::try_parse_from(["codex", "queue"])
            .expect_err("queue should be parsed as a subcommand, not as an interactive prompt");
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn queue_rejects_legacy_content_flag() {
        assert!(
            MultitoolCli::try_parse_from([
                "codex",
                "queue",
                "--thread",
                "thread-1",
                "--content",
                "do work",
            ])
            .is_err()
        );
    }

    #[test]
    fn queue_rejects_meta_flag() {
        assert!(
            MultitoolCli::try_parse_from([
                "codex",
                "queue",
                "--thread",
                "thread-1",
                "--message",
                "do work",
                "--meta",
                "ticket=ABC_123",
            ])
            .is_err()
        );
    }

    #[test]
    fn queue_rejects_instructions_flag() {
        assert!(
            MultitoolCli::try_parse_from([
                "codex",
                "queue",
                "--thread",
                "thread-1",
                "--message",
                "do work",
                "--instructions",
                "be brief",
            ])
            .is_err()
        );
    }

    #[test]
    fn queue_rejects_steer_flag() {
        assert!(
            MultitoolCli::try_parse_from([
                "codex",
                "queue",
                "--thread",
                "thread-1",
                "--message",
                "do work",
                "--steer",
            ])
            .is_err()
        );
    }

    #[test]
    fn queue_at_parses_without_instructions() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "queue",
            "--thread",
            "thread-1",
            "--message",
            "do work",
            "--at",
            "2026-04-10T12:00:00",
        ])
        .expect("parse");
        let Some(Subcommand::Queue(cmd)) = cli.subcommand else {
            unreachable!()
        };

        assert_eq!(cmd.at, Some("2026-04-10T12:00:00".to_string()));
    }

    #[test]
    fn queue_requires_queued_messages_feature() {
        let mut features = Features::with_defaults();

        let err = validate_queue_feature_flags(&features, QueueMode::ImmediateMessage)
            .expect_err("queue should require queued_messages");

        assert_eq!(
            err.to_string(),
            "codex queue requires the queued_messages feature"
        );

        features.enable(Feature::QueuedMessages);
        validate_queue_feature_flags(&features, QueueMode::ImmediateMessage)
            .expect("queued messages feature should permit immediate queue command");
    }

    #[test]
    fn queue_at_requires_timers_feature() {
        let mut features = Features::with_defaults();
        features.enable(Feature::QueuedMessages);

        let err = validate_queue_feature_flags(&features, QueueMode::Timer)
            .expect_err("queue --at should require timers");

        assert_eq!(
            err.to_string(),
            "codex queue --at requires the timers feature"
        );

        features.enable(Feature::Timers);
        validate_queue_feature_flags(&features, QueueMode::Timer)
            .expect("timers feature should permit queue --at");
    }

    #[test]
    fn queued_timer_output_reports_fire_time() {
        assert_eq!(
            queue_timer_success_message("timer-1", "thread-1", "2026-04-10T08:59:00", false),
            "Queued timer timer-1 for thread thread-1; it will fire at 2026-04-10 08:59:00 local time."
        );
    }

    #[test]
    fn queued_timer_output_reports_due_now() {
        assert_eq!(
            queue_timer_success_message("timer-1", "thread-1", "2026-04-10T08:59:00", true),
            "Queued timer timer-1 for thread thread-1; it is due now."
        );
    }

    #[tokio::test]
    async fn queue_thread_resolves_thread_name() {
        let temp = TempDir::new().expect("tempdir");
        let thread_id = ThreadId::new();
        codex_core::append_thread_name(temp.path(), thread_id, "named-thread")
            .await
            .expect("append thread name");

        assert_eq!(
            resolve_queue_thread_id(temp.path(), "named-thread")
                .await
                .expect("resolve"),
            thread_id.to_string()
        );
    }

    #[tokio::test]
    async fn queue_thread_name_rejects_missing_and_ambiguous_names() {
        let temp = TempDir::new().expect("tempdir");
        let first = ThreadId::new();
        let second = ThreadId::new();
        codex_core::append_thread_name(temp.path(), first, "same")
            .await
            .expect("append first name");
        codex_core::append_thread_name(temp.path(), second, "same")
            .await
            .expect("append second name");

        assert_eq!(
            resolve_queue_thread_id(temp.path(), "missing")
                .await
                .expect_err("missing name should fail")
                .to_string(),
            "no thread named `missing`"
        );
        assert_eq!(
            resolve_queue_thread_id(temp.path(), "same")
                .await
                .expect_err("ambiguous name should fail")
                .to_string(),
            "more than one thread is named `same`; use a thread id instead"
        );
    }
}
