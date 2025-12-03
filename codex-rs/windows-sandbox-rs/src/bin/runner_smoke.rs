use anyhow::Result;
use codex_windows_sandbox::{run_windows_sandbox_capture, SandboxPolicy};
use std::collections::HashMap;

fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let codex_home = dirs_next::home_dir().unwrap_or(cwd.clone()).join(".codex");
    let policy = SandboxPolicy::ReadOnly;
    let policy_json = serde_json::to_string(&policy)?;

    let mut env_map = HashMap::new();
    env_map.insert("SBX_DEBUG".to_string(), "1".to_string());

    let res = run_windows_sandbox_capture(
        &policy_json,
        &cwd,
        &codex_home,
        vec![
            "cmd".to_string(),
            "/c".to_string(),
            "echo smoke-runner".to_string(),
        ],
        &cwd,
        env_map,
        Some(10_000),
    )?;

    println!("exit_code={}", res.exit_code);
    println!("stdout={}", String::from_utf8_lossy(&res.stdout));
    println!("stderr={}", String::from_utf8_lossy(&res.stderr));
    println!("timed_out={}", res.timed_out);
    Ok(())
}
