use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use core_test_support::RemoteEnvConfig;
use core_test_support::requires_remote_env;
use pretty_assertions::assert_eq;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;

#[test]
fn remote_env_connects_creates_temp_dir_and_runs_sample_script() -> Result<()> {
    let Some(remote_env) = requires_remote_env() else {
        return Ok(());
    };

    let output = run_remote_script(
        &remote_env,
        r#"
set -euo pipefail

temp_dir="$(mktemp -d)"
script_path="${temp_dir}/sample.sh"

printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' 'echo remote-env-script-ok' > "${script_path}"
chmod +x "${script_path}"

"${script_path}" > "${temp_dir}/script.out"

echo "TEMP_DIR=${temp_dir}"
cat "${temp_dir}/script.out"
rm -rf "${temp_dir}"
"#,
    )?;

    let lines: Vec<&str> = output.lines().collect();
    ensure!(
        lines.len() >= 2,
        "remote script output must include at least two lines, got: {output:?}"
    );
    ensure!(
        lines[0].starts_with("TEMP_DIR=/"),
        "expected TEMP_DIR output from remote script, got {:?}",
        lines[0]
    );
    assert_eq!(lines[1], "remote-env-script-ok");

    Ok(())
}

fn run_remote_script(remote_env: &RemoteEnvConfig, script: &str) -> Result<String> {
    let user = &remote_env.user;
    let host = &remote_env.host;
    let ssh_target = format!("{user}@{host}");

    let mut command = Command::new("ssh");
    command
        .arg("-i")
        .arg(&remote_env.key_path)
        .arg("-p")
        .arg(remote_env.port.to_string())
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg(ssh_target)
        .arg("bash")
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .context("failed to spawn ssh to remote env")?;

    let mut stdin = child
        .stdin
        .take()
        .context("failed to open stdin for ssh command")?;
    stdin
        .write_all(script.as_bytes())
        .context("failed to write remote script to ssh stdin")?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .context("failed to wait for ssh command")?;

    ensure!(
        output.status.success(),
        "remote script failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).context("remote script stdout was not valid UTF-8")
}
