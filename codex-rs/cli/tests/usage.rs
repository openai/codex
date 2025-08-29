use codex_cli::usage::run_usage_command;
use codex_common::CliConfigOverrides;

#[tokio::test]
async fn usage_command_runs() {
    // Should not error; prints a helpful message even if not logged in.
    let overrides = CliConfigOverrides::default();
    run_usage_command(overrides)
        .await
        .expect("usage should run");
}
