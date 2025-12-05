use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;
use uuid::Uuid;

use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell::get_shell;

#[derive(Clone, Debug)]
pub struct ShellSnapshot {
    pub path: PathBuf,
}

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
            Ok(path) => Some(Self { path }),
            Err(err) => {
                warn!(
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
            debug!(
                "Failed to delete shell snapshot at {:?}: {err:?}",
                self.path
            );
        }
    }
}

/// Wraps an existing shell command so that it is executed after applying a
/// previously captured shell snapshot.
///
/// The snapshot script at `snapshot_path` replays functions, aliases, and
/// environment variables from an earlier shell session. This helper builds a
/// new command line that:
///   1. Starts the user's shell in non-login mode,
///   2. Sources or runs the snapshot script, and then
///   3. Executes the original `command` with its arguments.
///
/// The wrapper shell always runs in non-login mode; callers control login
/// behavior for the final command itself when they construct `command`.
pub fn wrap_command_with_snapshot(
    shell: &Shell,
    snapshot_path: &Path,
    command: &[String],
) -> Vec<String> {
    if command.is_empty() {
        return command.to_vec();
    }

    match shell.shell_type {
        ShellType::Zsh | ShellType::Bash | ShellType::Sh => {
            // `. "$1" && shift && exec "$@"`:
            //   1. source the snapshot script passed as the first argument,
            //   2. drop that argument so "$@" becomes the original command and args,
            //   3. exec the original command, replacing the wrapper shell.
            let mut args = shell.derive_exec_args(". \"$1\" && shift && exec \"$@\"", false);
            args.push("codex-shell-snapshot".to_string());
            args.push(snapshot_path.to_string_lossy().to_string());
            args.extend_from_slice(command);
            args
        }
        ShellType::PowerShell => {
            let mut args = shell.derive_exec_args("param($snapshot) . $snapshot; & @args", false);
            args.push(snapshot_path.to_string_lossy().to_string());
            args.extend_from_slice(command);
            args
        }
        ShellType::Cmd => command.to_vec(),
    }
}

pub async fn write_shell_snapshot(shell_type: ShellType, output_path: &Path) -> Result<PathBuf> {
    let shell = get_shell(shell_type.clone(), None)
        .with_context(|| format!("No available shell for {shell_type:?}"))?;

    let snapshot = capture_snapshot(&shell).await?;

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

async fn run_shell_script(shell: &Shell, script: &str) -> Result<String> {
    let args = shell.derive_exec_args(script, true);
    let shell_name = shell.name();
    let output = timeout(
        Duration::from_secs(10),
        Command::new(&args[0]).args(&args[1..]).output(),
    )
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
    use tempfile::tempdir;

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

    #[cfg(unix)]
    #[test]
    fn wrap_command_with_snapshot_wraps_bash_shell() {
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };
        let snapshot_path = PathBuf::from("/tmp/snapshot.sh");
        let original_command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo hello".to_string(),
        ];

        let wrapped = wrap_command_with_snapshot(&shell, &snapshot_path, &original_command);

        let mut expected = shell.derive_exec_args(". \"$1\" && shift && exec \"$@\"", false);
        expected.push("codex-shell-snapshot".to_string());
        expected.push(snapshot_path.to_string_lossy().to_string());
        expected.extend_from_slice(&original_command);

        assert_eq!(wrapped, expected);
    }

    #[test]
    fn wrap_command_with_snapshot_preserves_cmd_shell() {
        let shell = Shell {
            shell_type: ShellType::Cmd,
            shell_path: PathBuf::from("cmd"),
        };
        let snapshot_path = PathBuf::from("C:\\snapshot.cmd");
        let original_command = vec![
            "cmd".to_string(),
            "/c".to_string(),
            "echo hello".to_string(),
        ];

        let wrapped = wrap_command_with_snapshot(&shell, &snapshot_path, &original_command);

        assert_eq!(wrapped, original_command);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn try_new_creates_and_deletes_snapshot_file() -> Result<()> {
        let dir = tempdir()?;
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
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
    #[tokio::test]
    async fn windows_powershell_snapshot_includes_sections() -> Result<()> {
        let snapshot = get_snapshot(ShellType::PowerShell).await?;
        assert!(snapshot.contains("# Snapshot file"));
        assert!(snapshot.contains("aliases "));
        assert!(snapshot.contains("exports "));
        Ok(())
    }
}
