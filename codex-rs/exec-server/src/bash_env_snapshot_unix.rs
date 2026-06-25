use std::collections::HashMap;
use std::fs::Metadata;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use sha2::Digest;
use sha2::Sha256;
use tempfile::TempDir;
use tokio::sync::OnceCell;
use tokio::time::timeout;
use uuid::Uuid;

use super::ExecParams;
use super::ExecServerRuntimePaths;
use crate::process_sandbox::PreparedExecRequest;
use crate::process_sandbox::prepare_exec_request;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CACHE_ENTRIES: usize = 16;
const MAX_ENV_BYTES: usize = 256 * 1024;
const MAX_SCOPE_BYTES: usize = 384 * 1024;
const MAX_PRESERVE_KEYS: usize = 128;
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
type SnapshotCell = OnceCell<Option<Arc<BashEnvSnapshot>>>;

pub(crate) struct BashEnvSnapshotCache {
    entries: Mutex<HashMap<[u8; 32], Arc<SnapshotCell>>>,
    root: Option<TempDir>,
}

struct BashEnvSnapshotRequest {
    key: [u8; 32],
    environment: HashMap<String, String>,
    preserve_env_keys: Vec<String>,
    bash_env: String,
}

struct BashEnvSnapshot {
    path: PathBuf,
    bash_env: String,
    checksum: String,
    prefix: String,
}

impl Default for BashEnvSnapshotCache {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            root: tempfile::Builder::new()
                .prefix("codex-exec-server-bash-env-")
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir()
                .ok(),
        }
    }
}

impl BashEnvSnapshotCache {
    pub(crate) async fn prepare_launch(
        &self,
        params: &ExecParams,
        environment: &HashMap<String, String>,
        runtime_paths: Option<&ExecServerRuntimePaths>,
    ) -> (Vec<String>, HashMap<String, String>) {
        let Some(request) = BashEnvSnapshotRequest::new(params, environment) else {
            return fallback(params, environment);
        };
        let preserve_env_keys = request.preserve_env_keys.clone();
        let Some(snapshot) = self.snapshot(params, request, runtime_paths).await else {
            return fallback(params, environment);
        };
        let Some(argv) = wrap_command(&params.argv, &snapshot, &preserve_env_keys) else {
            return fallback(params, environment);
        };
        let mut environment = environment.clone();
        environment.remove("BASH_ENV");
        (argv, environment)
    }

    pub(crate) fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    async fn snapshot(
        &self,
        params: &ExecParams,
        request: BashEnvSnapshotRequest,
        runtime_paths: Option<&ExecServerRuntimePaths>,
    ) -> Option<Arc<BashEnvSnapshot>> {
        let key = request.key;
        let cell = {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if entries.len() >= MAX_CACHE_ENTRIES && !entries.contains_key(&key) {
                return None;
            }
            Arc::clone(
                entries
                    .entry(key)
                    .or_insert_with(|| Arc::new(OnceCell::new())),
            )
        };
        cell.get_or_init(|| async {
            self.capture(params, request, runtime_paths)
                .await
                .map(Arc::new)
        })
        .await
        .clone()
    }

    async fn capture(
        &self,
        params: &ExecParams,
        request: BashEnvSnapshotRequest,
        runtime_paths: Option<&ExecServerRuntimePaths>,
    ) -> Option<BashEnvSnapshot> {
        let nonce = Uuid::new_v4().simple().to_string();
        let prefix = format!("__CODEX_SNAPSHOT_{}", nonce.to_ascii_uppercase());
        let marker = format!("# Codex remote Bash environment snapshot {nonce}");
        let stderr_marker = format!("# Codex remote Bash stderr complete {nonce}");
        let wrapper_end = format!("# Codex remote Bash capture complete {nonce}");
        let root = self.root.as_ref()?;
        let mut wrapper = tempfile::NamedTempFile::new_in(root.path()).ok()?;
        let script = capture_script(
            &prefix,
            &marker,
            &stderr_marker,
            &wrapper_end,
            &request.bash_env,
            &request.preserve_env_keys,
        );
        wrapper.write_all(script.as_bytes()).ok()?;
        wrapper.as_file().sync_all().ok()?;
        let mut capture = params.clone();
        capture.bash_env_snapshot = None;
        let mut environment = request.environment;
        environment.insert("BASH_ENV".into(), wrapper.path().to_string_lossy().into());
        let prepared = prepare_exec_request(&capture, environment, runtime_paths).ok()?;
        let (output, stderr) = capture_output(prepared).await?;
        let marker_offset = output
            .windows(marker.len())
            .position(|part| part == marker.as_bytes())?;
        let stdout = &output[..marker_offset];
        let snapshot =
            output[marker_offset..].strip_suffix(format!("{wrapper_end}\n").as_bytes())?;
        let stderr = &stderr[..stderr
            .windows(stderr_marker.len())
            .position(|part| part == stderr_marker.as_bytes())?];
        let mut contents = snapshot.to_vec();
        contents.extend(replay_output(stdout, /*fd*/ 3));
        contents.extend(replay_output(stderr, /*fd*/ 4));
        contents.extend_from_slice(b"builtin true\n");
        if !contents.ends_with(b"\n") {
            contents.push(b'\n');
        }
        if contents.len() > MAX_SNAPSHOT_BYTES {
            return None;
        }
        let mut temp = tempfile::NamedTempFile::new_in(root.path()).ok()?;
        temp.write_all(&contents).ok()?;
        temp.as_file().sync_all().ok()?;
        let path = root.path().join(format!("{nonce}.sh"));
        temp.persist(&path).ok()?;
        Some(BashEnvSnapshot {
            path,
            bash_env: request.bash_env,
            checksum: format!("{:x}", Sha256::digest(&contents)),
            prefix,
        })
    }
}

impl BashEnvSnapshotRequest {
    fn new(params: &ExecParams, environment: &HashMap<String, String>) -> Option<Self> {
        let snapshot = params.bash_env_snapshot.as_ref()?;
        if params.tty
            || params.pipe_stdin
            || params.arg0.is_some()
            || params.argv.len() < 3
            || params.argv.get(1).map(String::as_str) != Some("-c")
            || !params.cwd.starts_with(&snapshot.workspace_root)
            || environment.iter().any(|(key, value)| {
                (key == "builtin" || key.starts_with("BASH_FUNC_builtin"))
                    && value.trim_start().starts_with("()")
            })
        {
            return None;
        }
        let shell = Path::new(params.argv.first()?);
        let bash_env = Path::new(environment.get("BASH_ENV")?);
        if !shell.is_absolute()
            || shell.file_name()?.to_str()? != "bash"
            || !shell.is_file()
            || !bash_env.is_absolute()
            || !bash_env.is_file()
        {
            return None;
        }
        let bash_env_source = std::fs::read(bash_env).ok()?;
        if bash_env_source.len() > MAX_SNAPSHOT_BYTES || unsupported_startup(&bash_env_source) {
            return None;
        }
        let mut preserve_env_keys = snapshot.preserve_env_keys.clone();
        if preserve_env_keys.len() > MAX_PRESERVE_KEYS
            || preserve_env_keys
                .iter()
                .any(|key| key.len() > 128 || !valid_variable(key))
        {
            return None;
        }
        preserve_env_keys.retain(|key| key != "BASH_ENV");
        preserve_env_keys.sort_unstable();
        preserve_env_keys.dedup();

        let mut environment_scope = environment
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        environment_scope.sort_unstable();
        let environment_bytes = environment_scope
            .iter()
            .try_fold(0usize, |size, (key, value)| {
                size.checked_add(key.len())?.checked_add(value.len())
            })?;
        if environment_bytes > MAX_ENV_BYTES {
            return None;
        }
        let mut scope = serde_json::to_value((
            unsafe { libc::geteuid() },
            &snapshot.workspace_root,
            &params.cwd,
            shell.to_string_lossy(),
            file_identity(&shell.metadata().ok()?),
            bash_env.to_string_lossy(),
            file_identity(&bash_env.metadata().ok()?),
            environment_scope,
            &params.env_policy,
            &params.sandbox,
            params.enforce_managed_network,
            &params.managed_network,
            &preserve_env_keys,
        ))
        .ok()?;
        sort_json(&mut scope);
        let scope = serde_json::to_vec(&scope).ok()?;
        if scope.len() > MAX_SCOPE_BYTES {
            return None;
        }
        let key = Sha256::digest(scope).into();
        Some(Self {
            key,
            environment: environment.clone(),
            preserve_env_keys,
            bash_env: bash_env.to_string_lossy().into_owned(),
        })
    }
}

impl Drop for BashEnvSnapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn capture_output(prepared: PreparedExecRequest) -> Option<(Vec<u8>, Vec<u8>)> {
    timeout(CAPTURE_TIMEOUT, async move {
        let (program, args) = prepared.command.split_first()?;
        let spawned = codex_utils_pty::spawn_pipe_process_no_stdin(
            program,
            args,
            prepared.cwd.as_path(),
            &prepared.env,
            &prepared.arg0,
        )
        .await
        .ok()?;
        let codex_utils_pty::SpawnedProcess {
            session: _session,
            stdout_rx,
            stderr_rx,
            exit_rx,
        } = spawned;
        let (stdout, stderr, exit) = tokio::join!(
            collect_output(stdout_rx),
            collect_output(stderr_rx),
            exit_rx
        );
        (exit.ok()? == 0).then_some((stdout?, stderr?))
    })
    .await
    .ok()
    .flatten()
}

async fn collect_output(mut receiver: tokio::sync::mpsc::Receiver<Vec<u8>>) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    while let Some(chunk) = receiver.recv().await {
        if output.len().checked_add(chunk.len())? > MAX_SNAPSHOT_BYTES {
            return None;
        }
        output.extend_from_slice(&chunk);
    }
    Some(output)
}

fn capture_script(
    prefix: &str,
    marker: &str,
    stderr_marker: &str,
    wrapper_end: &str,
    bash_env: &str,
    preserve_env_keys: &[String],
) -> String {
    let mut excluded = ":BASH:BASHOPTS:BASHPID:BASH_ALIASES:BASH_ARGC:BASH_ARGV:BASH_ARGV0:\
BASH_CMDS:BASH_COMMAND:BASH_ENV:BASH_EXECUTION_STRING:BASH_LINENO:BASH_REMATCH:BASH_SOURCE:BASH_SUBSHELL:\
BASH_VERSINFO:BASH_VERSION:COPROC:DIRSTACK:EPOCHREALTIME:EPOCHSECONDS:EUID:FUNCNAME:GROUPS:\
HISTCMD:HOSTTYPE:LINENO:MACHTYPE:OLDPWD:OSTYPE:PIPESTATUS:PPID:PWD:RANDOM:SECONDS:SHELLOPTS:SHLVL:SRANDOM:UID:_:"
        .to_string();
    for key in preserve_env_keys {
        excluded.push_str(key);
        excluded.push(':');
    }
    r#"if ! builtin type -P sha256sum >/dev/null 2>&1 && ! builtin type -P shasum >/dev/null 2>&1; then builtin exit 125; fi
builtin unset __CODEX_PREFIX___EXCLUDED __CODEX_PREFIX___P_NAMES __CODEX_PREFIX___NAME __CODEX_PREFIX___DECL __CODEX_PREFIX___ALIASES __CODEX_PREFIX___OPTS __CODEX_PREFIX___SHOPTS __CODEX_PREFIX___FUNC_ATTRS __CODEX_PREFIX___CWD __CODEX_PREFIX___UMASK
builtin export BASH_ENV='__CODEX_BASH_ENV__'
__CODEX_PREFIX___EXCLUDED='__CODEX_EXCLUDED__'; __CODEX_PREFIX___P_NAMES=; __CODEX_PREFIX___NAME=; __CODEX_PREFIX___DECL=
__CODEX_PREFIX___ALIASES=; __CODEX_PREFIX___OPTS=; __CODEX_PREFIX___SHOPTS=; __CODEX_PREFIX___FUNC_ATTRS=; __CODEX_PREFIX___CWD=; __CODEX_PREFIX___UMASK=
while IFS= builtin read -r __CODEX_PREFIX___NAME; do
  [[ "$__CODEX_PREFIX___NAME" != __CODEX_PREFIX___* && "$__CODEX_PREFIX___EXCLUDED" != *":$__CODEX_PREFIX___NAME:"* && "$__CODEX_PREFIX___NAME" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
  __CODEX_PREFIX___P_NAMES="${__CODEX_PREFIX___P_NAMES}${__CODEX_PREFIX___NAME}"$'\n'
done < <(builtin compgen -A variable)
builtin source "$BASH_ENV"
if [[ $- == *x* || $- == *v* ]]; then builtin exit 125; fi
builtin printf '%s\n' '__CODEX_MARKER__'; builtin printf '%s\n' '__CODEX_STDERR_MARKER__' >&2
__CODEX_PREFIX___ALIASES=$(builtin alias -p 2>/dev/null || builtin true)
__CODEX_PREFIX___OPTS=$(builtin set +o); if [[ $- == *e* ]]; then __CODEX_PREFIX___OPTS=${__CODEX_PREFIX___OPTS/set +o errexit/set -o errexit}; fi
__CODEX_PREFIX___SHOPTS=$(builtin shopt -p 2>/dev/null || builtin true)
builtin shopt -u extdebug 2>/dev/null || builtin true; __CODEX_PREFIX___FUNC_ATTRS=$(builtin declare -F)
__CODEX_PREFIX___CWD=$(builtin pwd -L); __CODEX_PREFIX___UMASK=$(builtin umask)
builtin set +e; builtin set +u; builtin unalias -a 2>/dev/null || builtin true
__CODEX_PREFIX___ALIASES=${__CODEX_PREFIX___ALIASES//$'\n'/$'\nbuiltin '}; __CODEX_PREFIX___OPTS=${__CODEX_PREFIX___OPTS//$'\n'/$'\nbuiltin '}; __CODEX_PREFIX___SHOPTS=${__CODEX_PREFIX___SHOPTS//$'\n'/$'\nbuiltin '}
[[ -z "$__CODEX_PREFIX___ALIASES" ]] || __CODEX_PREFIX___ALIASES="builtin $__CODEX_PREFIX___ALIASES"
[[ -z "$__CODEX_PREFIX___OPTS" ]] || __CODEX_PREFIX___OPTS="builtin $__CODEX_PREFIX___OPTS"
[[ -z "$__CODEX_PREFIX___SHOPTS" ]] || __CODEX_PREFIX___SHOPTS="builtin $__CODEX_PREFIX___SHOPTS"
builtin printf 'builtin cd -L -- %q || builtin return 1\n' "$__CODEX_PREFIX___CWD"
builtin declare -f
while IFS= builtin read -r __CODEX_PREFIX___DECL; do
  [[ "$__CODEX_PREFIX___DECL" == "declare -f "* ]] && continue
  builtin printf 'builtin %s\n' "$__CODEX_PREFIX___DECL"
done <<< "$__CODEX_PREFIX___FUNC_ATTRS"
builtin printf '\n'
while IFS= builtin read -r __CODEX_PREFIX___NAME; do
  [[ "$__CODEX_PREFIX___NAME" != __CODEX_PREFIX___* && "$__CODEX_PREFIX___EXCLUDED" != *":$__CODEX_PREFIX___NAME:"* && "$__CODEX_PREFIX___NAME" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
  __CODEX_PREFIX___DECL=$(builtin declare -p "$__CODEX_PREFIX___NAME" 2>/dev/null) || continue; builtin printf 'builtin unset -v %s\n' "$__CODEX_PREFIX___NAME"; __CODEX_PREFIX___NAME=${__CODEX_PREFIX___DECL#declare }; __CODEX_PREFIX___NAME=${__CODEX_PREFIX___NAME%% *}
  if [[ "$__CODEX_PREFIX___NAME" == -*[aA]* ]]; then __CODEX_PREFIX___DECL=${__CODEX_PREFIX___DECL#declare }; __CODEX_PREFIX___DECL=${__CODEX_PREFIX___DECL#* }; if [[ "$__CODEX_PREFIX___NAME" == *A* ]]; then __CODEX_PREFIX___CWD=A; else __CODEX_PREFIX___CWD=a; fi
    if [[ "${__CODEX_PREFIX___DECL#*=}" == \(* ]]; then builtin printf 'builtin declare -%s %q\n' "$__CODEX_PREFIX___CWD" "$__CODEX_PREFIX___DECL"; else builtin printf 'builtin declare -%s %s\n' "$__CODEX_PREFIX___CWD" "$__CODEX_PREFIX___DECL"; fi; builtin printf 'builtin declare %s %s\n' "$__CODEX_PREFIX___NAME" "${__CODEX_PREFIX___DECL%%=*}"; else builtin printf 'builtin %s\n' "$__CODEX_PREFIX___DECL"; fi
done < <(builtin compgen -A variable)
while IFS= builtin read -r __CODEX_PREFIX___NAME; do
  [[ -z "$__CODEX_PREFIX___NAME" ]] && continue
  builtin declare -p "$__CODEX_PREFIX___NAME" >/dev/null 2>&1 || builtin printf 'builtin unset -v %s\n' "$__CODEX_PREFIX___NAME"
done < <(builtin printf '%s' "$__CODEX_PREFIX___P_NAMES")
builtin printf '\n'
if __CODEX_PREFIX___DECL=$(builtin declare -p BASH_ENV 2>/dev/null); then builtin printf 'builtin %s\n' "$__CODEX_PREFIX___DECL"; else builtin printf 'builtin unset -v BASH_ENV\n'; fi
builtin printf '\nbuiltin umask %s\n' "$__CODEX_PREFIX___UMASK"
builtin printf '%s\n' "$__CODEX_PREFIX___OPTS"
builtin printf '\n%s\n\n' "$__CODEX_PREFIX___SHOPTS"
[[ -z "$__CODEX_PREFIX___ALIASES" ]] || builtin printf '%s\n' "$__CODEX_PREFIX___ALIASES"
builtin printf '%s\n' '__CODEX_WRAPPER_END__'; builtin exit 0
"#
        .replace("__CODEX_PREFIX__", prefix)
        .replace("__CODEX_MARKER__", marker)
        .replace("__CODEX_STDERR_MARKER__", stderr_marker)
        .replace("__CODEX_WRAPPER_END__", wrapper_end)
        .replace("__CODEX_BASH_ENV__", &quote(bash_env))
        .replace("__CODEX_EXCLUDED__", &excluded)
}

fn replay_output(output: &[u8], fd: u8) -> Vec<u8> {
    if output.is_empty() {
        return Vec::new();
    }
    let escaped = output
        .iter()
        .map(|byte| format!("\\{byte:03o}"))
        .collect::<String>();
    format!("builtin printf '%b' '{escaped}' >&{fd}\n").into_bytes()
}

fn wrap_command(
    argv: &[String],
    snapshot: &BashEnvSnapshot,
    preserve_env_keys: &[String],
) -> Option<Vec<String>> {
    let mut script = String::new();
    for (index, key) in preserve_env_keys.iter().enumerate() {
        script.push_str(&format!(
            "{prefix}_{index}_SET=\"${{{key}+x}}\"\n{prefix}_{index}_VALUE=\"${{{key}-}}\"\n",
            prefix = snapshot.prefix,
        ));
    }
    script.push_str("builtin unset BASH_ENV\n");
    script.push_str(&format!(
        "{prefix}_HASH=$(if builtin command -v sha256sum >/dev/null 2>&1; then builtin command sha256sum '{path}'; elif builtin command -v shasum >/dev/null 2>&1; then builtin command shasum -a 256 '{path}'; fi)\n\
         {prefix}_HASH=${{{prefix}_HASH%% *}}\n\
         if [[ \"${{{prefix}_HASH}}\" = '{checksum}' ]] && builtin source '{path}' 3>&1 4>&2 >/dev/null 2>&1; then builtin true; else builtin export BASH_ENV='{bash_env}'; builtin source \"$BASH_ENV\" || builtin true; fi\n\
         builtin unset {prefix}_HASH\n",
        prefix = snapshot.prefix,
        path = quote(&snapshot.path.to_string_lossy()),
        checksum = snapshot.checksum,
        bash_env = quote(&snapshot.bash_env),
    ));
    for (index, key) in preserve_env_keys.iter().enumerate() {
        script.push_str(&format!(
                "if [[ \"${{{prefix}_{index}_SET}}\" = x ]]; then builtin export {key}=\"${{{prefix}_{index}_VALUE}}\"; else builtin unset {key}; fi\nbuiltin unset {prefix}_{index}_SET {prefix}_{index}_VALUE\n",
                prefix = snapshot.prefix,
            ));
    }
    script.push_str(&format!(
        "BASH_EXECUTION_STRING='{}'\n",
        quote(argv.get(2)?)
    ));
    script.push_str(argv.get(2)?);
    let mut wrapped = vec![argv.first()?.clone(), "-c".to_string(), script];
    wrapped.extend(argv.get(3..)?.iter().cloned());
    Some(wrapped)
}

fn file_identity(metadata: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64, u32) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        metadata.mode(),
    )
}

fn sort_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => values.iter_mut().for_each(sort_json),
        serde_json::Value::Object(map) => {
            map.values_mut().for_each(sort_json);
            map.sort_keys();
        }
        _ => {}
    }
}

fn valid_variable(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('_') | Some('a'..='z') | Some('A'..='Z'))
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn unsupported_startup(source: &[u8]) -> bool {
    let positional = |byte| matches!(byte, b'0'..=b'9' | b'@' | b'*' | b'#');
    source
        .windows(b"BASH_EXECUTION_STRING".len())
        .any(|part| part == b"BASH_EXECUTION_STRING")
        || source
            .windows(2)
            .any(|part| part[0] == b'$' && positional(part[1]))
        || source
            .windows(3)
            .any(|part| part.starts_with(b"${") && positional(part[2]))
        || source
            .split(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
            .any(|word| word == b"trap" || word == b"builtin")
}

fn quote(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn fallback(
    params: &ExecParams,
    environment: &HashMap<String, String>,
) -> (Vec<String>, HashMap<String, String>) {
    (params.argv.clone(), environment.clone())
}
