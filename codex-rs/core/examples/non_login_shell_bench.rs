use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use codex_core::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
use codex_core::exec::ExecExpiration;
use codex_core::exec::ExecParams;
use codex_core::exec::process_exec_tool_call;
use codex_core::protocol::SandboxPolicy;
use codex_core::shell::default_user_shell;

fn parse_iterations() -> usize {
    let mut args = std::env::args().skip(1);
    let mut iterations = 5usize;
    while let Some(arg) = args.next() {
        if arg == "--iterations" {
            if let Some(value) = args.next() {
                if let Ok(parsed) = value.parse::<usize>() {
                    iterations = parsed.max(1);
                }
            }
        }
    }
    iterations
}

async fn measure(command: &str, use_login_shell: bool) -> anyhow::Result<Duration> {
    let cwd = std::env::current_dir()?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let shell = default_user_shell();
    let args = shell.derive_exec_args(command, use_login_shell);
    let params = ExecParams {
        command: args,
        cwd,
        expiration: ExecExpiration::from(DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
        env,
        with_escalated_permissions: None,
        justification: None,
        arg0: None,
    };
    let start = Instant::now();
    let _ = process_exec_tool_call(
        params,
        &SandboxPolicy::DangerFullAccess,
        &std::env::current_dir()?,
        &None,
        None,
    )
    .await?;
    Ok(start.elapsed())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let iterations = parse_iterations();
    let commands = ["ls", "rg --files", "git status"];

    println!("Non-login shell heuristic benchmark (iterations: {iterations})");
    for command in commands {
        let mut login_times = Vec::with_capacity(iterations);
        let mut non_login_times = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            login_times.push(measure(command, true).await?);
            non_login_times.push(measure(command, false).await?);
        }
        let avg_login_ms =
            login_times.iter().map(Duration::as_secs_f64).sum::<f64>() * 1000.0 / iterations as f64;
        let avg_non_login_ms = non_login_times
            .iter()
            .map(Duration::as_secs_f64)
            .sum::<f64>()
            * 1000.0
            / iterations as f64;
        println!(
            "{command:12} login: {avg_login_ms:>8.3} ms | non-login: {avg_non_login_ms:>8.3} ms"
        );
    }

    Ok(())
}
