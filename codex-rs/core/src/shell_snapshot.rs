use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use crate::rollout::list::find_thread_path_by_id_str;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell::get_shell;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::Instrument;
use tracing::info_span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub path: PathBuf,
}

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);
const SNAPSHOT_RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 7); // 7 days retention.
const SNAPSHOT_DIR: &str = "shell_snapshots";
const EXCLUDED_EXPORT_VARS: &[&str] = &["PWD", "OLDPWD"];

impl ShellSnapshot {
    pub fn start_snapshotting(
        codex_home: PathBuf,
        session_id: ThreadId,
        shell: &mut Shell,
        otel_manager: OtelManager,
    ) {
        let (shell_snapshot_tx, shell_snapshot_rx) = watch::channel(None);
        shell.shell_snapshot = shell_snapshot_rx;

        let snapshot_shell = shell.clone();
        let snapshot_session_id = session_id;
        let snapshot_span = info_span!("shell_snapshot", thread_id = %snapshot_session_id);
        tokio::spawn(
            async move {
                let timer = otel_manager.start_timer("codex.shell_snapshot.duration_ms", &[]);
                let snapshot =
                    ShellSnapshot::try_new(&codex_home, snapshot_session_id, &snapshot_shell)
                        .await
                        .map(Arc::new);
                let success = if snapshot.is_some() { "true" } else { "false" };
                let _ = timer.map(|timer| timer.record(&[("success", success)]));
                otel_manager.counter("codex.shell_snapshot", 1, &[("success", success)]);
                let _ = shell_snapshot_tx.send(snapshot);
            }
            .instrument(snapshot_span),
        );
    }

    async fn try_new(codex_home: &Path, session_id: ThreadId, shell: &Shell) -> Option<Self> {
        // File to store the snapshot
        let extension = match shell.shell_type {
            ShellType::PowerShell => "ps1",
            _ => "sh",
        };
        let path = codex_home
            .join(SNAPSHOT_DIR)
            .join(format!("{session_id}.{extension}"));

        // Clean the (unlikely) leaked snapshot files.
        let codex_home = codex_home.to_path_buf();
        let cleanup_session_id = session_id;
        tokio::spawn(async move {
            if let Err(err) = cleanup_stale_snapshots(&codex_home, cleanup_session_id).await {
                tracing::warn!("Failed to clean up shell snapshots: {err:?}");
            }
        });

        // Make the new snapshot.
        let snapshot = match write_shell_snapshot(shell.shell_type.clone(), &path).await {
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
        };

        if let Some(snapshot) = snapshot.as_ref()
            && let Err(err) = validate_snapshot(shell, &snapshot.path).await
        {
            tracing::error!("Shell snapshot validation failed: {err:?}");
            return None;
        }

        snapshot
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

async fn write_shell_snapshot(shell_type: ShellType, output_path: &Path) -> Result<PathBuf> {
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
        ShellType::Zsh => run_shell_script(shell, &zsh_snapshot_script()).await,
        ShellType::Bash => run_shell_script(shell, &bash_snapshot_script()).await,
        ShellType::Sh => run_shell_script(shell, &sh_snapshot_script()).await,
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

async fn validate_snapshot(shell: &Shell, snapshot_path: &Path) -> Result<()> {
    let snapshot_path_display = snapshot_path.display();
    let script = format!("set -e; . \"{snapshot_path_display}\"");
    run_script_with_timeout(shell, &script, SNAPSHOT_TIMEOUT, false)
        .await
        .map(|_| ())
}

async fn run_shell_script(shell: &Shell, script: &str) -> Result<String> {
    run_script_with_timeout(shell, script, SNAPSHOT_TIMEOUT, true).await
}

async fn run_script_with_timeout(
    shell: &Shell,
    script: &str,
    snapshot_timeout: Duration,
    use_login_shell: bool,
) -> Result<String> {
    let args = shell.derive_exec_args(script, use_login_shell);
    let shell_name = shell.name();

    // Handler is kept as guard to control the drop. The `mut` pattern is required because .args()
    // returns a ref of handler.
    let mut handler = Command::new(&args[0]);
    handler.args(&args[1..]);
    handler.stdin(Stdio::null());
    #[cfg(unix)]
    unsafe {
        handler.pre_exec(|| {
            codex_utils_pty::process_group::detach_from_tty()?;
            Ok(())
        });
    }
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

fn excluded_exports_regex() -> String {
    EXCLUDED_EXPORT_VARS.join("|")
}

fn zsh_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [[ -n "$ZDOTDIR" ]]; then
  rc="$ZDOTDIR/.zshrc"
else
  rc="$HOME/.zshrc"
fi
[[ -r "$rc" ]] && . "$rc"
print '# Snapshot file'
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
export_lines=$(export -p | awk '
/^(export|declare -x|typeset -x) / {
  line=$0
  name=line
  sub(/^(export|declare -x|typeset -x) /, "", name)
  sub(/=.*/, "", name)
  if (name ~ /^(EXCLUDED_EXPORTS)$/) {
    next
  }
  if (name ~ /^[A-Za-z_][A-Za-z0-9_]*$/) {
    print line
  }
}')
export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
print "# exports $export_count"
if [[ -n "$export_lines" ]]; then
  print -r -- "$export_lines"
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn bash_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [ -z "$BASH_ENV" ] && [ -r "$HOME/.bashrc" ]; then
  . "$HOME/.bashrc"
fi
echo '# Snapshot file'
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
export_lines=$(export -p | awk '
/^(export|declare -x|typeset -x) / {
  line=$0
  name=line
  sub(/^(export|declare -x|typeset -x) /, "", name)
  sub(/=.*/, "", name)
  if (name ~ /^(EXCLUDED_EXPORTS)$/) {
    next
  }
  if (name ~ /^[A-Za-z_][A-Za-z0-9_]*$/) {
    print line
  }
}')
export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
echo "# exports $export_count"
if [ -n "$export_lines" ]; then
  printf '%s\n' "$export_lines"
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn sh_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [ -n "$ENV" ] && [ -r "$ENV" ]; then
  . "$ENV"
fi
echo '# Snapshot file'
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
  export_lines=$(export -p | awk '
/^(export|declare -x|typeset -x) / {
  line=$0
  name=line
  sub(/^(export|declare -x|typeset -x) /, "", name)
  sub(/=.*/, "", name)
  if (name ~ /^(EXCLUDED_EXPORTS)$/) {
    next
  }
  if (name ~ /^[A-Za-z_][A-Za-z0-9_]*$/) {
    print line
  }
}')
  export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
  echo "# exports $export_count"
  if [ -n "$export_lines" ]; then
    printf '%s\n' "$export_lines"
  fi
else
  export_count=$(env | sort | awk -F= '$1 ~ /^[A-Za-z_][A-Za-z0-9_]*$/ { count++ } END { print count }')
  echo "# exports $export_count"
  env | sort | while IFS='=' read -r key value; do
    case "$key" in
      ""|[0-9]*|*[!A-Za-z0-9_]*|EXCLUDED_EXPORTS) continue ;;
    esac
    escaped=$(printf "%s" "$value" | sed "s/'/'\"'\"'/g")
    printf "export %s='%s'\n" "$key" "$escaped"
  done
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
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

/// Removes shell snapshots that either lack a matching session rollout file or
/// whose rollouts have not been updated within the retention window.
/// The active session id is exempt from cleanup.
pub async fn cleanup_stale_snapshots(codex_home: &Path, active_session_id: ThreadId) -> Result<()> {
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);

    let mut entries = match fs::read_dir(&snapshot_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    let now = SystemTime::now();
    let active_session_id = active_session_id.to_string();

    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }

        let path = entry.path();

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let (session_id, _) = match file_name.rsplit_once('.') {
            Some((stem, ext)) => (stem, ext),
            None => {
                remove_snapshot_file(&path).await;
                continue;
            }
        };
        if session_id == active_session_id {
            continue;
        }

        let rollout_path = find_thread_path_by_id_str(codex_home, session_id).await?;
        let Some(rollout_path) = rollout_path else {
            remove_snapshot_file(&path).await;
            continue;
        };

        let modified = match fs::metadata(&rollout_path).await.and_then(|m| m.modified()) {
            Ok(modified) => modified,
            Err(err) => {
                tracing::warn!(
                    "Failed to check rollout age for snapshot {}: {err:?}",
                    path.display()
                );
                continue;
            }
        };

        if now
            .duration_since(modified)
            .ok()
            .is_some_and(|age| age >= SNAPSHOT_RETENTION)
        {
            remove_snapshot_file(&path).await;
        }
    }

    Ok(())
}

async fn remove_snapshot_file(path: &Path) {
    if let Err(err) = fs::remove_file(path).await {
        tracing::warn!("Failed to delete shell snapshot at {:?}: {err:?}", path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    #[cfg(unix)]
    use std::process::Command;
    #[cfg(target_os = "linux")]
    use std::process::Command as StdCommand;

    use tempfile::tempdir;

    #[cfg(unix)]
    struct BlockingStdinPipe {
        original: i32,
        write_end: i32,
    }

    #[cfg(unix)]
    impl BlockingStdinPipe {
        fn install() -> Result<Self> {
            let mut fds = [0i32; 2];
            if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
                return Err(std::io::Error::last_os_error()).context("create stdin pipe");
            }

            let original = unsafe { libc::dup(libc::STDIN_FILENO) };
            if original == -1 {
                let err = std::io::Error::last_os_error();
                unsafe {
                    libc::close(fds[0]);
                    libc::close(fds[1]);
                }
                return Err(err).context("dup stdin");
            }

            if unsafe { libc::dup2(fds[0], libc::STDIN_FILENO) } == -1 {
                let err = std::io::Error::last_os_error();
                unsafe {
                    libc::close(fds[0]);
                    libc::close(fds[1]);
                    libc::close(original);
                }
                return Err(err).context("replace stdin");
            }

            unsafe {
                libc::close(fds[0]);
            }

            Ok(Self {
                original,
                write_end: fds[1],
            })
        }
    }

    #[cfg(unix)]
    impl Drop for BlockingStdinPipe {
        fn drop(&mut self) {
            unsafe {
                libc::dup2(self.original, libc::STDIN_FILENO);
                libc::close(self.original);
                libc::close(self.write_end);
            }
        }
    }

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
    #[test]
    fn bash_snapshot_filters_invalid_exports() -> Result<()> {
        let output = Command::new("/bin/bash")
            .arg("-c")
            .arg(bash_snapshot_script())
            .env("BASH_ENV", "/dev/null")
            .env("VALID_NAME", "ok")
            .env("PWD", "/tmp/stale")
            .env("NEXTEST_BIN_EXE_codex-write-config-schema", "/path/to/bin")
            .env("BAD-NAME", "broken")
            .output()?;

        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("VALID_NAME"));
        assert!(!stdout.contains("PWD=/tmp/stale"));
        assert!(!stdout.contains("NEXTEST_BIN_EXE_codex-write-config-schema"));
        assert!(!stdout.contains("BAD-NAME"));

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn try_new_creates_and_deletes_snapshot_file() -> Result<()> {
        let dir = tempdir()?;
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        };

        let snapshot = ShellSnapshot::try_new(dir.path(), ThreadId::new(), &shell)
            .await
            .expect("snapshot should be created");
        let path = snapshot.path.clone();
        assert!(path.exists());

        drop(snapshot);

        assert!(!path.exists());

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn snapshot_shell_does_not_inherit_stdin() -> Result<()> {
        let _stdin_guard = BlockingStdinPipe::install()?;

        let dir = tempdir()?;
        let home = dir.path();
        fs::write(home.join(".bashrc"), "read -r ignored\n").await?;

        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        };

        let home_display = home.display();
        let script = format!(
            "HOME=\"{home_display}\"; export HOME; {}",
            bash_snapshot_script()
        );
        let output = run_script_with_timeout(&shell, &script, Duration::from_millis(500), true)
            .await
            .context("run snapshot command")?;

        assert!(
            output.contains("# Snapshot file"),
            "expected snapshot marker in output; output={output:?}"
        );

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
        let pid_path = dir.path().join("pid");
        let script = format!("echo $$ > \"{}\"; sleep 30", pid_path.display());

        let shell = Shell {
            shell_type: ShellType::Sh,
            shell_path: PathBuf::from("/bin/sh"),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        };

        let err = run_script_with_timeout(&shell, &script, Duration::from_secs(1), true)
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

    async fn write_rollout_stub(codex_home: &Path, session_id: ThreadId) -> Result<PathBuf> {
        let dir = codex_home
            .join("sessions")
            .join("2025")
            .join("01")
            .join("01");
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("rollout-2025-01-01T00-00-00-{session_id}.jsonl"));
        fs::write(&path, "").await?;
        Ok(path)
    }

    #[tokio::test]
    async fn cleanup_stale_snapshots_removes_orphans_and_keeps_live() -> Result<()> {
        let dir = tempdir()?;
        let codex_home = dir.path();
        let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
        fs::create_dir_all(&snapshot_dir).await?;

        let live_session = ThreadId::new();
        let orphan_session = ThreadId::new();
        let live_snapshot = snapshot_dir.join(format!("{live_session}.sh"));
        let orphan_snapshot = snapshot_dir.join(format!("{orphan_session}.sh"));
        let invalid_snapshot = snapshot_dir.join("not-a-snapshot.txt");

        write_rollout_stub(codex_home, live_session).await?;
        fs::write(&live_snapshot, "live").await?;
        fs::write(&orphan_snapshot, "orphan").await?;
        fs::write(&invalid_snapshot, "invalid").await?;

        cleanup_stale_snapshots(codex_home, ThreadId::new()).await?;

        assert_eq!(live_snapshot.exists(), true);
        assert_eq!(orphan_snapshot.exists(), false);
        assert_eq!(invalid_snapshot.exists(), false);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cleanup_stale_snapshots_removes_stale_rollouts() -> Result<()> {
        let dir = tempdir()?;
        let codex_home = dir.path();
        let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
        fs::create_dir_all(&snapshot_dir).await?;

        let stale_session = ThreadId::new();
        let stale_snapshot = snapshot_dir.join(format!("{stale_session}.sh"));
        let rollout_path = write_rollout_stub(codex_home, stale_session).await?;
        fs::write(&stale_snapshot, "stale").await?;

        set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

        cleanup_stale_snapshots(codex_home, ThreadId::new()).await?;

        assert_eq!(stale_snapshot.exists(), false);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cleanup_stale_snapshots_skips_active_session() -> Result<()> {
        let dir = tempdir()?;
        let codex_home = dir.path();
        let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
        fs::create_dir_all(&snapshot_dir).await?;

        let active_session = ThreadId::new();
        let active_snapshot = snapshot_dir.join(format!("{active_session}.sh"));
        let rollout_path = write_rollout_stub(codex_home, active_session).await?;
        fs::write(&active_snapshot, "active").await?;

        set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

        cleanup_stale_snapshots(codex_home, active_session).await?;

        assert_eq!(active_snapshot.exists(), true);
        Ok(())
    }

    #[cfg(unix)]
    fn set_file_mtime(path: &Path, age: Duration) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs()
            .saturating_sub(age.as_secs());
        let tv_sec = now
            .try_into()
            .map_err(|_| anyhow!("Snapshot mtime is out of range for libc::timespec"))?;
        let ts = libc::timespec { tv_sec, tv_nsec: 0 };
        let times = [ts, ts];
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        let result = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
}
