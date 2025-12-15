use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell::get_shell;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub path: PathBuf,
}

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);

impl ShellSnapshot {
    pub async fn try_new(codex_home: &Path, shell: &Shell) -> Option<Self> {
        let extension = match shell.shell_type {
            ShellType::PowerShell => "ps1",
            _ => "sh",
        };
        let path =
            codex_home
                .join("shell_snapshots")
                .join(format!("{}.{}", Uuid::new_v4(), extension));
        match write_shell_snapshot(shell.shell_type.clone(), &path).await {
            Ok(path) => {
                tracing::info!("Shell snapshot successfully created: {}", path.display());
                Some(Self { path })
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to create shell snapshot for {}: {err:?}",
                    shell.name()
                );
                None
            }
        }
    }
}

impl Drop for ShellSnapshot {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            tracing::warn!(
                "Failed to delete shell snapshot at {:?}: {err:?}",
                self.path
            );
        }
    }
}

pub async fn write_shell_snapshot(shell_type: ShellType, output_path: &Path) -> Result<PathBuf> {
    if shell_type == ShellType::PowerShell || shell_type == ShellType::Cmd {
        bail!("Shell snapshot not supported yet for {shell_type:?}");
    }
    let shell = get_shell(shell_type.clone(), None)
        .with_context(|| format!("No available shell for {shell_type:?}"))?;

    let raw_snapshot = capture_snapshot(&shell).await?;
    let snapshot = strip_snapshot_preamble(&raw_snapshot)?;

    if let Some(parent) = output_path.parent() {
        let parent_display = parent.display();
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create snapshot parent {parent_display}"))?;
    }

    let snapshot_path = output_path.display();
    fs::write(output_path, snapshot)
        .await
        .with_context(|| format!("Failed to write snapshot to {snapshot_path}"))?;

    Ok(output_path.to_path_buf())
}

async fn capture_snapshot(shell: &Shell) -> Result<String> {
    let shell_type = shell.shell_type.clone();
    match shell_type {
        ShellType::Zsh => run_shell_script(shell, zsh_snapshot_script()).await,
        ShellType::Bash => run_shell_script(shell, bash_snapshot_script()).await,
        ShellType::Sh => run_shell_script(shell, sh_snapshot_script()).await,
        ShellType::PowerShell => run_shell_script(shell, powershell_snapshot_script()).await,
        ShellType::Cmd => bail!("Shell snapshotting is not yet supported for {shell_type:?}"),
    }
}

fn strip_snapshot_preamble(snapshot: &str) -> Result<String> {
    let marker = "# Snapshot file";
    let Some(start) = snapshot.find(marker) else {
        bail!("Snapshot output missing marker {marker}");
    };

    Ok(snapshot[start..].to_string())
}

async fn run_shell_script(shell: &Shell, script: &str) -> Result<String> {
    run_shell_script_with_timeout(shell, script, SNAPSHOT_TIMEOUT).await
}

async fn run_shell_script_with_timeout(
    shell: &Shell,
    script: &str,
    snapshot_timeout: Duration,
) -> Result<String> {
    let args = shell.derive_exec_args(script, true);
    let shell_name = shell.name();

    // Handler is kept as guard to control the drop. The `mut` pattern is required because .args()
    // returns a ref of handler.
    let mut handler = Command::new(&args[0]);
    handler.args(&args[1..]);
    handler.kill_on_drop(true);
    let output = timeout(snapshot_timeout, handler.output())
        .await
        .map_err(|_| anyhow!("Snapshot command timed out for {shell_name}"))?
        .with_context(|| format!("Failed to execute {shell_name}"))?;

    if !output.status.success() {
        let status = output.status;
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Snapshot command exited with status {status}: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn zsh_snapshot_script() -> &'static str {
    r##"print '# Snapshot file'
print '# Unset all aliases to avoid conflicts with functions'
print 'unalias -a 2>/dev/null || true'
print '# Functions'
functions
print ''
setopt_count=$(setopt | wc -l | tr -d ' ')
print "# setopts $setopt_count"
setopt | sed 's/^/setopt /'
print ''
alias_count=$(alias -L | wc -l | tr -d ' ')
print "# aliases $alias_count"
alias -L
print ''
export_count=$(export -p | wc -l | tr -d ' ')
print "# exports $export_count"
export -p
"##
}

fn bash_snapshot_script() -> &'static str {
    r##"echo '# Snapshot file'
echo '# Unset all aliases to avoid conflicts with functions'
unalias -a 2>/dev/null || true
echo '# Functions'
declare -f
echo ''
bash_opts=$(set -o | awk '$2=="on"{print $1}')
bash_opt_count=$(printf '%s\n' "$bash_opts" | sed '/^$/d' | wc -l | tr -d ' ')
echo "# setopts $bash_opt_count"
if [ -n "$bash_opts" ]; then
  printf 'set -o %s\n' $bash_opts
fi
echo ''
alias_count=$(alias -p | wc -l | tr -d ' ')
echo "# aliases $alias_count"
alias -p
echo ''
export_count=$(export -p | wc -l | tr -d ' ')
echo "# exports $export_count"
export -p
"##
}

fn sh_snapshot_script() -> &'static str {
    r##"echo '# Snapshot file'
echo '# Unset all aliases to avoid conflicts with functions'
unalias -a 2>/dev/null || true
echo '# Functions'
if command -v typeset >/dev/null 2>&1; then
  typeset -f
elif command -v declare >/dev/null 2>&1; then
  declare -f
fi
echo ''
if set -o >/dev/null 2>&1; then
  sh_opts=$(set -o | awk '$2=="on"{print $1}')
  sh_opt_count=$(printf '%s\n' "$sh_opts" | sed '/^$/d' | wc -l | tr -d ' ')
  echo "# setopts $sh_opt_count"
  if [ -n "$sh_opts" ]; then
    printf 'set -o %s\n' $sh_opts
  fi
else
  echo '# setopts 0'
fi
echo ''
if alias >/dev/null 2>&1; then
  alias_count=$(alias | wc -l | tr -d ' ')
  echo "# aliases $alias_count"
  alias
  echo ''
else
  echo '# aliases 0'
fi
if export -p >/dev/null 2>&1; then
  export_count=$(export -p | wc -l | tr -d ' ')
  echo "# exports $export_count"
  export -p
else
  export_count=$(env | wc -l | tr -d ' ')
  echo "# exports $export_count"
  env | sort | while IFS='=' read -r key value; do
    escaped=$(printf "%s" "$value" | sed "s/'/'\"'\"'/g")
    printf "export %s='%s'\n" "$key" "$escaped"
  done
fi
"##
}

fn powershell_snapshot_script() -> &'static str {
    r##"$ErrorActionPreference = 'Stop'
Write-Output '# Snapshot file'
Write-Output '# Unset all aliases to avoid conflicts with functions'
Write-Output 'Remove-Item Alias:* -ErrorAction SilentlyContinue'
Write-Output '# Functions'
Get-ChildItem Function: | ForEach-Object {
    "function {0} {{`n{1}`n}}" -f $_.Name, $_.Definition
}
Write-Output ''
$aliases = Get-Alias
Write-Output ("# aliases " + $aliases.Count)
$aliases | ForEach-Object {
    "Set-Alias -Name {0} -Value {1}" -f $_.Name, $_.Definition
}
Write-Output ''
$envVars = Get-ChildItem Env:
Write-Output ("# exports " + $envVars.Count)
$envVars | ForEach-Object {
    $escaped = $_.Value -replace "'", "''"
    "`$env:{0}='{1}'" -f $_.Name, $escaped
}
"##
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(target_os = "linux")]
    use std::process::Command as StdCommand;

    use tempfile::tempdir;

    #[cfg(not(target_os = "windows"))]
    fn assert_posix_snapshot_sections(snapshot: &str) {
        assert!(snapshot.contains("# Snapshot file"));
        assert!(snapshot.contains("aliases "));
        assert!(snapshot.contains("exports "));
        assert!(
            snapshot.contains("PATH"),
            "snapshot should capture a PATH export"
        );
        assert!(snapshot.contains("setopts "));
    }

    async fn get_snapshot(shell_type: ShellType) -> Result<String> {
        let dir = tempdir()?;
        let path = dir.path().join("snapshot.sh");
        write_shell_snapshot(shell_type, &path).await?;
        let content = fs::read_to_string(&path).await?;
        Ok(content)
    }

    #[test]
    fn strip_snapshot_preamble_removes_leading_output() {
        let snapshot = "noise\n# Snapshot file\nexport PATH=/bin\n";
        let cleaned = strip_snapshot_preamble(snapshot).expect("snapshot marker exists");
        assert_eq!(cleaned, "# Snapshot file\nexport PATH=/bin\n");
    }

    #[test]
    fn strip_snapshot_preamble_requires_marker() {
        let result = strip_snapshot_preamble("missing header");
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn try_new_creates_and_deletes_snapshot_file() -> Result<()> {
        let dir = tempdir()?;
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: None,
        };

        let snapshot = ShellSnapshot::try_new(dir.path(), &shell)
            .await
            .expect("snapshot should be created");
        let path = snapshot.path.clone();
        assert!(path.exists());

        drop(snapshot);

        assert!(!path.exists());

        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn timed_out_snapshot_shell_is_terminated() -> Result<()> {
        use std::process::Stdio;
        use tokio::time::Duration as TokioDuration;
        use tokio::time::Instant;
        use tokio::time::sleep;

        let dir = tempdir()?;
        let shell_path = dir.path().join("hanging-shell.sh");
        let pid_path = dir.path().join("pid");

        let script = format!(
            "#!/bin/sh\n\
             echo $$ > {}\n\
             sleep 30\n",
            pid_path.display()
        );
        fs::write(&shell_path, script).await?;
        let mut permissions = std::fs::metadata(&shell_path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&shell_path, permissions)?;

        let shell = Shell {
            shell_type: ShellType::Sh,
            shell_path,
            shell_snapshot: None,
        };

        let err = run_shell_script_with_timeout(&shell, "ignored", Duration::from_millis(500))
            .await
            .expect_err("snapshot shell should time out");
        assert!(
            err.to_string().contains("timed out"),
            "expected timeout error, got {err:?}"
        );

        let pid = fs::read_to_string(&pid_path)
            .await
            .expect("snapshot shell writes its pid before timing out")
            .trim()
            .parse::<i32>()?;

        let deadline = Instant::now() + TokioDuration::from_secs(1);
        loop {
            let kill_status = StdCommand::new("kill")
                .arg("-0")
                .arg(pid.to_string())
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .status()?;
            if !kill_status.success() {
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out snapshot shell is still alive after grace period");
            }
            sleep(TokioDuration::from_millis(50)).await;
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn macos_zsh_snapshot_includes_sections() -> Result<()> {
        let snapshot = get_snapshot(ShellType::Zsh).await?;
        assert_posix_snapshot_sections(&snapshot);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn linux_bash_snapshot_includes_sections() -> Result<()> {
        let snapshot = get_snapshot(ShellType::Bash).await?;
        assert_posix_snapshot_sections(&snapshot);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn linux_sh_snapshot_includes_sections() -> Result<()> {
        let snapshot = get_snapshot(ShellType::Sh).await?;
        assert_posix_snapshot_sections(&snapshot);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    #[ignore]
    #[tokio::test]
    async fn windows_powershell_snapshot_includes_sections() -> Result<()> {
        let snapshot = get_snapshot(ShellType::PowerShell).await?;
        assert!(snapshot.contains("# Snapshot file"));
        assert!(snapshot.contains("aliases "));
        assert!(snapshot.contains("exports "));
        Ok(())
    }
}
