//! Implementation for the `codex queue` command.
//!
//! The top-level CLI module owns command routing; this module owns the
//! queue-specific policy for resolving target threads and writing immediate
//! messages into the SQLite state database.

use clap::Parser;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::timers::TimerDelivery;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::ThreadId;
use codex_state::StateRuntime;
use codex_tui::Cli as TuiCli;
use codex_utils_cli::CliConfigOverrides;
use std::path::Path;

#[derive(Debug, Parser)]
pub(crate) struct QueueCommand {
    /// Target thread id.
    #[arg(long = "thread", value_name = "THREAD_ID")]
    thread: String,

    /// Message text.
    #[arg(long = "message", value_name = "TEXT")]
    message: String,
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
    validate_queue_feature_flags(&config.features)?;
    let thread_id = resolve_queue_thread_id(config.codex_home.as_path(), &cmd.thread).await?;
    let state_db =
        StateRuntime::init(config.sqlite_home.clone(), config.model_provider_id.clone()).await?;
    ensure_queue_thread_belongs_to_state_db(&state_db, &thread_id).await?;
    let delivery = TimerDelivery::AfterTurn;

    let message_params = codex_state::ExternalMessageCreateParams::new(
        thread_id,
        "external".to_string(),
        cmd.message,
        /*instructions*/ None,
        "{}".to_string(),
        delivery.as_str().to_string(),
        unix_timestamp_now()?,
    );
    state_db.create_external_message(&message_params).await?;
    remove_queued_message_if_thread_missing(
        config.codex_home.as_path(),
        &state_db,
        &message_params.thread_id,
        &message_params.id,
    )
    .await?;
    println!(
        "Queued message {} for thread {}.",
        message_params.id, message_params.thread_id
    );
    Ok(())
}

async fn ensure_queue_thread_belongs_to_state_db(
    state_db: &StateRuntime,
    thread_id: &str,
) -> anyhow::Result<()> {
    let thread_id = ThreadId::from_string(thread_id)
        .map_err(|err| anyhow::anyhow!("invalid resolved thread id `{thread_id}`: {err}"))?;
    if state_db.get_thread(thread_id).await?.is_some() {
        return Ok(());
    }

    anyhow::bail!(
        "thread `{thread_id}` is not present in the configured sqlite state database; run codex queue with the profile that owns the thread"
    );
}

async fn remove_queued_message_if_thread_missing(
    codex_home: &Path,
    state_db: &StateRuntime,
    thread_id: &str,
    message_id: &str,
) -> anyhow::Result<()> {
    if codex_core::find_thread_path_by_id_str(codex_home, thread_id)
        .await?
        .is_some()
    {
        return Ok(());
    }

    state_db
        .delete_external_message(thread_id, message_id)
        .await?;
    anyhow::bail!("thread `{thread_id}` was archived before queued work could be created");
}

fn validate_queue_feature_flags(features: &Features) -> anyhow::Result<()> {
    if !features.enabled(Feature::QueuedMessages) {
        anyhow::bail!("codex queue requires the queued_messages feature");
    }
    Ok(())
}

async fn resolve_queue_thread_id(codex_home: &Path, target: &str) -> anyhow::Result<String> {
    if let Ok(thread_id) = ThreadId::from_string(target) {
        if codex_core::find_thread_path_by_id_str(codex_home, &thread_id.to_string())
            .await?
            .is_none()
        {
            anyhow::bail!("no thread with id `{thread_id}`");
        }
        return Ok(thread_id.to_string());
    }

    let mut active_thread_ids = Vec::new();
    for thread_id in codex_core::find_thread_ids_by_name(codex_home, target).await? {
        if codex_core::find_thread_path_by_id_str(codex_home, &thread_id.to_string())
            .await?
            .is_some()
        {
            active_thread_ids.push(thread_id);
        }
    }

    match active_thread_ids.as_slice() {
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

    fn write_test_rollout(codex_home: &Path, thread_id: ThreadId) {
        let sessions_dir = codex_home
            .join("sessions")
            .join("2026")
            .join("04")
            .join("10");
        std::fs::create_dir_all(&sessions_dir).expect("create sessions dir");
        std::fs::write(
            sessions_dir.join(format!("rollout-2026-04-10T12-00-00-{thread_id}.jsonl")),
            "",
        )
        .expect("write rollout");
    }

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
    }

    #[test]
    fn queue_without_required_args_is_subcommand_error() {
        let err = MultitoolCli::try_parse_from(["codex", "queue"])
            .expect_err("queue should be parsed as a subcommand, not as an interactive prompt");
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn queue_requires_queued_messages_feature() {
        let mut features = Features::with_defaults();

        let err = validate_queue_feature_flags(&features)
            .expect_err("queue should require queued_messages");

        assert_eq!(
            err.to_string(),
            "codex queue requires the queued_messages feature"
        );

        features.enable(Feature::QueuedMessages);
        validate_queue_feature_flags(&features)
            .expect("queued messages feature should permit immediate queue command");
    }

    #[tokio::test]
    async fn queue_requires_thread_in_configured_state_db() {
        let sqlite_home = TempDir::new().expect("sqlite home tempdir");
        let runtime = StateRuntime::init(
            sqlite_home.path().to_path_buf(),
            "test-provider".to_string(),
        )
        .await
        .expect("initialize state runtime");
        let thread_id = ThreadId::new().to_string();

        let err = ensure_queue_thread_belongs_to_state_db(&runtime, &thread_id)
            .await
            .expect_err("missing state thread should fail");

        assert_eq!(
            err.to_string(),
            format!(
                "thread `{thread_id}` is not present in the configured sqlite state database; run codex queue with the profile that owns the thread"
            )
        );
    }

    #[tokio::test]
    async fn queue_thread_resolves_thread_name() {
        let temp = TempDir::new().expect("tempdir");
        let thread_id = ThreadId::new();
        write_test_rollout(temp.path(), thread_id);
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
    async fn queue_thread_id_requires_existing_thread() {
        let temp = TempDir::new().expect("tempdir");
        let thread_id = ThreadId::new();
        write_test_rollout(temp.path(), thread_id);

        assert_eq!(
            resolve_queue_thread_id(temp.path(), &thread_id.to_string())
                .await
                .expect("resolve"),
            thread_id.to_string()
        );

        let missing = ThreadId::new();
        assert_eq!(
            resolve_queue_thread_id(temp.path(), &missing.to_string())
                .await
                .expect_err("missing id should fail")
                .to_string(),
            format!("no thread with id `{missing}`")
        );
    }

    #[tokio::test]
    async fn queue_thread_name_rejects_missing_and_ambiguous_names() {
        let temp = TempDir::new().expect("tempdir");
        let first = ThreadId::new();
        let second = ThreadId::new();
        write_test_rollout(temp.path(), first);
        write_test_rollout(temp.path(), second);
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

    #[tokio::test]
    async fn queue_thread_name_ignores_names_without_rollouts() {
        let temp = TempDir::new().expect("tempdir");
        let stale = ThreadId::new();
        let active = ThreadId::new();
        write_test_rollout(temp.path(), active);
        codex_core::append_thread_name(temp.path(), stale, "same")
            .await
            .expect("append stale name");
        codex_core::append_thread_name(temp.path(), stale, "stale")
            .await
            .expect("append stale-only name");
        codex_core::append_thread_name(temp.path(), active, "same")
            .await
            .expect("append active name");

        assert_eq!(
            resolve_queue_thread_id(temp.path(), "same")
                .await
                .expect("resolve"),
            active.to_string()
        );

        assert_eq!(
            resolve_queue_thread_id(temp.path(), "stale")
                .await
                .expect_err("stale name should fail")
                .to_string(),
            "no thread named `stale`"
        );
    }
}
