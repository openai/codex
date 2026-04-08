use crate::Shell;
use crate::ShellType;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::Instrument;
use tracing::info_span;

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);
pub const SNAPSHOT_RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 3);
pub const SNAPSHOT_DIR: &str = "shell_snapshots";
const EXCLUDED_EXPORT_VARS: &[&str] = &["PWD", "OLDPWD"];

type ShellSnapshotSender = watch::Sender<Option<Arc<ShellSnapshot>>>;
type ShellSnapshotReceiver = watch::Receiver<Option<Arc<ShellSnapshot>>>;

#[derive(Clone)]
pub(crate) struct ShellSnapshotState {
    shell_snapshot_tx: ShellSnapshotSender,
    shell_snapshot_rx: ShellSnapshotReceiver,
}

impl Default for ShellSnapshotState {
    fn default() -> Self {
        let (shell_snapshot_tx, shell_snapshot_rx) = watch::channel(None);
        Self {
            shell_snapshot_tx,
            shell_snapshot_rx,
        }
    }
}

impl fmt::Debug for ShellSnapshotState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShellSnapshotState")
            .field("shell_snapshot", &self.shell_snapshot())
            .finish()
    }
}

impl ShellSnapshotState {
    fn with_shell_snapshot(shell_snapshot: Option<Arc<ShellSnapshot>>) -> Self {
        let (shell_snapshot_tx, shell_snapshot_rx) = watch::channel(shell_snapshot);
        Self {
            shell_snapshot_tx,
            shell_snapshot_rx,
        }
    }

    fn shell_snapshot(&self) -> Option<Arc<ShellSnapshot>> {
        self.shell_snapshot_rx.borrow().clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub path: PathBuf,
    pub cwd: PathBuf,
}

impl Shell {
    pub fn shell_snapshot(&self) -> Option<Arc<ShellSnapshot>> {
        self.snapshot_state.shell_snapshot()
    }

    pub fn set_shell_snapshot(&mut self, shell_snapshot: Option<Arc<ShellSnapshot>>) {
        self.snapshot_state = ShellSnapshotState::with_shell_snapshot(shell_snapshot);
    }

    pub fn start_snapshotting(
        &mut self,
        codex_home: PathBuf,
        session_id: ThreadId,
        session_cwd: PathBuf,
        session_telemetry: SessionTelemetry,
    ) {
        self.snapshot_state = ShellSnapshotState::default();
        self.spawn_snapshot_task(
            codex_home,
            session_id,
            session_cwd,
            self.snapshot_state.shell_snapshot_tx.clone(),
            session_telemetry,
        );
    }

    pub fn refresh_snapshot(
        &self,
        codex_home: PathBuf,
        session_id: ThreadId,
        session_cwd: PathBuf,
        session_telemetry: SessionTelemetry,
    ) {
        self.spawn_snapshot_task(
            codex_home,
            session_id,
            session_cwd,
            self.snapshot_state.shell_snapshot_tx.clone(),
            session_telemetry,
        );
    }

    fn spawn_snapshot_task(
        &self,
        codex_home: PathBuf,
        session_id: ThreadId,
        session_cwd: PathBuf,
        shell_snapshot_tx: ShellSnapshotSender,
        session_telemetry: SessionTelemetry,
    ) {
        let snapshot_shell = self.clone();
        let snapshot_span = info_span!("shell_snapshot", thread_id = %session_id);
        tokio::spawn(
            async move {
                let timer = session_telemetry.start_timer("codex.shell_snapshot.duration_ms", &[]);
                let snapshot = ShellSnapshot::try_new(
                    &codex_home,
                    session_id,
                    session_cwd.as_path(),
                    &snapshot_shell,
                )
                .await
                .map(Arc::new);
                let success = snapshot.is_ok();
                let success_tag = if success { "true" } else { "false" };
                let _ = timer.map(|timer| timer.record(&[("success", success_tag)]));
                let mut counter_tags = vec![("success", success_tag)];
                if let Some(failure_reason) = snapshot.as_ref().err() {
                    counter_tags.push(("failure_reason", *failure_reason));
                }
                session_telemetry.counter("codex.shell_snapshot", /*inc*/ 1, &counter_tags);
                let _ = shell_snapshot_tx.send(snapshot.ok());
            }
            .instrument(snapshot_span),
        );
    }
}

impl ShellSnapshot {
    async fn try_new(
        codex_home: &Path,
        session_id: ThreadId,
        session_cwd: &Path,
        shell: &Shell,
    ) -> std::result::Result<Self, &'static str> {
        let extension = match shell.shell_type {
            ShellType::PowerShell => "ps1",
            ShellType::Zsh | ShellType::Bash | ShellType::Sh | ShellType::Cmd => "sh",
        };
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = codex_home
            .join(SNAPSHOT_DIR)
            .join(format!("{session_id}.{nonce}.{extension}"));
        let temp_path = codex_home
            .join(SNAPSHOT_DIR)
            .join(format!("{session_id}.tmp-{nonce}"));

        let temp_path = match write_shell_snapshot(shell.shell_type, &temp_path, session_cwd).await
        {
            Ok(path) => {
                tracing::info!("Shell snapshot successfully created: {}", path.display());
                path
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to create shell snapshot for {}: {err:?}",
                    shell.name()
                );
                return Err("write_failed");
            }
        };

        let temp_snapshot = Self {
            path: temp_path.clone(),
            cwd: session_cwd.to_path_buf(),
        };

        if let Err(err) = validate_snapshot(shell, &temp_snapshot.path, session_cwd).await {
            tracing::error!("Shell snapshot validation failed: {err:?}");
            remove_snapshot_file(&temp_snapshot.path).await;
            return Err("validation_failed");
        }

        if let Err(err) = fs::rename(&temp_snapshot.path, &path).await {
            tracing::warn!("Failed to finalize shell snapshot: {err:?}");
            remove_snapshot_file(&temp_snapshot.path).await;
            return Err("write_failed");
        }

        Ok(Self {
            path,
            cwd: session_cwd.to_path_buf(),
        })
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

async fn write_shell_snapshot(
    shell_type: ShellType,
    output_path: &Path,
    cwd: &Path,
) -> Result<PathBuf> {
    if shell_type == ShellType::PowerShell || shell_type == ShellType::Cmd {
        bail!("Shell snapshot not supported yet for {shell_type:?}");
    }
    let shell = crate::get_shell(shell_type, /*path*/ None)
        .with_context(|| format!("No available shell for {shell_type:?}"))?;

    let raw_snapshot = capture_snapshot(&shell, cwd).await?;
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

async fn capture_snapshot(shell: &Shell, cwd: &Path) -> Result<String> {
    match shell.shell_type {
        ShellType::Zsh => run_shell_script(shell, &zsh_snapshot_script(), cwd).await,
        ShellType::Bash => run_shell_script(shell, &bash_snapshot_script(), cwd).await,
        ShellType::Sh => run_shell_script(shell, &sh_snapshot_script(), cwd).await,
        ShellType::PowerShell => run_shell_script(shell, powershell_snapshot_script(), cwd).await,
        ShellType::Cmd => bail!(
            "Shell snapshotting is not yet supported for {:?}",
            shell.shell_type
        ),
    }
}

fn strip_snapshot_preamble(snapshot: &str) -> Result<String> {
    let marker = "# Snapshot file";
    let Some(start) = snapshot.find(marker) else {
        bail!("Snapshot output missing marker {marker}");
    };

    Ok(snapshot[start..].to_string())
}

async fn validate_snapshot(shell: &Shell, snapshot_path: &Path, cwd: &Path) -> Result<()> {
    let snapshot_path_display = snapshot_path.display();
    let script = format!("set -e; . \"{snapshot_path_display}\"");
    run_script_with_timeout(
        shell,
        &script,
        SNAPSHOT_TIMEOUT,
        /*use_login_shell*/ false,
        cwd,
    )
    .await
    .map(|_| ())
}

async fn run_shell_script(shell: &Shell, script: &str, cwd: &Path) -> Result<String> {
    run_script_with_timeout(
        shell,
        script,
        SNAPSHOT_TIMEOUT,
        /*use_login_shell*/ true,
        cwd,
    )
    .await
}

async fn run_script_with_timeout(
    shell: &Shell,
    script: &str,
    snapshot_timeout: Duration,
    use_login_shell: bool,
    cwd: &Path,
) -> Result<String> {
    let args = shell.derive_exec_args(script, use_login_shell);
    let shell_name = shell.name();

    let mut handler = Command::new(&args[0]);
    handler.args(&args[1..]);
    handler.stdin(Stdio::null());
    handler.current_dir(cwd);
    #[cfg(unix)]
    unsafe {
        handler.pre_exec(|| {
            codex_utils_pty::process_group::detach_from_tty()?;
            Ok(())
        });
    }
    handler.kill_on_drop(true);
    let snapshot_output = timeout(snapshot_timeout, handler.output())
        .await
        .map_err(|_| anyhow!("Snapshot command timed out for {shell_name}"))?
        .with_context(|| format!("Failed to execute {shell_name}"))?;

    if !snapshot_output.status.success() {
        bail!(
            "Snapshot command exited with status {}: {}",
            snapshot_output.status,
            String::from_utf8_lossy(&snapshot_output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&snapshot_output.stdout).into_owned())
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
export_lines=$(
  while IFS= read -r name; do
    if [[ "$name" =~ ^(EXCLUDED_EXPORTS)$ ]]; then
      continue
    fi
    if [[ ! "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
      continue
    fi
    declare -xp "$name" 2>/dev/null || true
  done < <(compgen -e)
)
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

pub async fn remove_snapshot_file(path: &Path) {
    if let Err(err) = fs::remove_file(path).await {
        tracing::warn!(
            "Failed to delete stale shell snapshot {}: {err:?}",
            path.display()
        );
    }
}

pub fn snapshot_session_id_from_file_name(file_name: &str) -> Option<&str> {
    let mut parts = file_name.split('.');
    let session_id = parts.next()?;
    if uuid::Uuid::parse_str(session_id).is_ok() {
        Some(session_id)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod snapshot_tests;
