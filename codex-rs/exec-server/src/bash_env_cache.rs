use std::collections::HashMap;

use crate::ExecServerRuntimePaths;
use crate::protocol::ExecParams;

#[cfg(unix)]
mod imp {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use codex_file_system::FileSystemSandboxContext;
    use codex_network_proxy::ManagedNetworkSandboxContext;
    use codex_utils_path_uri::PathUri;
    use tokio::sync::Mutex;
    use tokio::sync::OnceCell;
    use tokio::time::timeout;
    use uuid::Uuid;

    use super::*;
    use crate::process_sandbox::prepare_exec_request;

    const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
    const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

    #[derive(Default)]
    pub(crate) struct BashEnvCache {
        entry: Mutex<Option<CacheEntry>>,
    }

    struct CacheEntry {
        key: CacheKey,
        environment: Arc<OnceCell<Option<HashMap<String, String>>>>,
    }

    #[derive(PartialEq, Eq)]
    struct CacheKey {
        scope: PathUri,
        shell: String,
        cwd: PathUri,
        environment: HashMap<String, String>,
        sandbox: Option<FileSystemSandboxContext>,
        enforce_managed_network: bool,
        managed_network: Option<ManagedNetworkSandboxContext>,
    }

    impl BashEnvCache {
        pub(crate) async fn environment_for_launch(
            &self,
            params: &ExecParams,
            environment: HashMap<String, String>,
            runtime_paths: Option<&ExecServerRuntimePaths>,
        ) -> HashMap<String, String> {
            let Some(key) = CacheKey::new(params, &environment) else {
                return environment;
            };

            let cached_environment = {
                let mut entry = self.entry.lock().await;
                if let Some(cached) = entry.as_ref()
                    && cached.key == key
                {
                    Arc::clone(&cached.environment)
                } else {
                    let environment = Arc::new(OnceCell::new());
                    *entry = Some(CacheEntry {
                        key,
                        environment: Arc::clone(&environment),
                    });
                    environment
                }
            };
            cached_environment
                .get_or_init(|| capture_environment(params, &environment, runtime_paths))
                .await
                .clone()
                .unwrap_or(environment)
        }
    }

    impl CacheKey {
        fn new(params: &ExecParams, environment: &HashMap<String, String>) -> Option<Self> {
            let scope = params.env_policy.as_ref()?.bash_env_cache_scope.as_ref()?;
            let [shell, option, _script] = params.argv.as_slice() else {
                return None;
            };
            if params.tty
                || params.pipe_stdin
                || params.arg0.is_some()
                || option != "-c"
                || Path::new(shell).file_name()?.to_str()? != "bash"
                || !params.cwd.starts_with(scope)
                || environment.get("BASH_ENV").is_none_or(String::is_empty)
            {
                return None;
            }

            Some(Self {
                scope: scope.clone(),
                shell: shell.clone(),
                cwd: params.cwd.clone(),
                environment: environment.clone(),
                sandbox: params.sandbox.clone(),
                enforce_managed_network: params.enforce_managed_network,
                managed_network: params.managed_network.clone(),
            })
        }
    }

    async fn capture_environment(
        params: &ExecParams,
        environment: &HashMap<String, String>,
        runtime_paths: Option<&ExecServerRuntimePaths>,
    ) -> Option<HashMap<String, String>> {
        let nonce = Uuid::new_v4().simple().to_string();
        let start_marker = format!("__CODEX_BASH_ENV_START_{nonce}__");
        let end_marker = format!("__CODEX_BASH_ENV_END_{nonce}__");
        let mut capture = params.clone();
        capture.argv = vec![
            params.argv.first()?.clone(),
            "-c".to_string(),
            format!(
                "builtin printf '%s' '{start_marker}'; if builtin command env -0; then builtin printf '%s' '{end_marker}'; else builtin exit 125; fi"
            ),
        ];
        let prepared = prepare_exec_request(&capture, environment.clone(), runtime_paths).ok()?;
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

        let captured = timeout(CAPTURE_TIMEOUT, async move {
            let _session = spawned.session;
            let (stdout, stderr, exit_code) = tokio::join!(
                collect_output(spawned.stdout_rx),
                collect_output(spawned.stderr_rx),
                spawned.exit_rx,
            );
            (exit_code.ok()? == 0 && stderr?.is_empty()).then_some(parse_capture(
                stdout?,
                &start_marker,
                &end_marker,
            )?)
        })
        .await
        .ok()
        .flatten()?;

        Some(sanitize_environment(captured, environment))
    }

    async fn collect_output(mut receiver: tokio::sync::mpsc::Receiver<Vec<u8>>) -> Option<Vec<u8>> {
        let mut output = Vec::new();
        while let Some(chunk) = receiver.recv().await {
            if output.len().checked_add(chunk.len())? > MAX_CAPTURE_BYTES {
                return None;
            }
            output.extend_from_slice(&chunk);
        }
        Some(output)
    }

    fn parse_capture(
        output: Vec<u8>,
        start_marker: &str,
        end_marker: &str,
    ) -> Option<HashMap<String, String>> {
        let payload = output.strip_prefix(start_marker.as_bytes())?;
        let payload = payload.strip_suffix(end_marker.as_bytes())?;
        if !payload.is_empty() && !payload.ends_with(&[0]) {
            return None;
        }

        payload
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let separator = entry.iter().position(|byte| *byte == b'=')?;
                let key = std::str::from_utf8(&entry[..separator]).ok()?.to_string();
                let value = std::str::from_utf8(&entry[separator + 1..])
                    .ok()?
                    .to_string();
                Some((key, value))
            })
            .collect()
    }

    fn sanitize_environment(
        mut captured: HashMap<String, String>,
        original: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        captured.remove("BASH_ENV");
        captured.retain(|key, _| !shell_managed(key));
        captured.extend(
            original
                .iter()
                .filter(|(key, _)| shell_managed(key))
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        captured
    }

    fn shell_managed(key: &str) -> bool {
        key.starts_with("BASH_FUNC_")
            || matches!(
                key,
                "BASHOPTS"
                    | "BASH_ARGV0"
                    | "BASH_EXECUTION_STRING"
                    | "OLDPWD"
                    | "PWD"
                    | "SHELLOPTS"
                    | "SHLVL"
                    | "_"
            )
    }
}

#[cfg(unix)]
pub(crate) use imp::BashEnvCache;

#[cfg(not(unix))]
#[derive(Default)]
pub(crate) struct BashEnvCache;

#[cfg(not(unix))]
impl BashEnvCache {
    pub(crate) async fn environment_for_launch(
        &self,
        _params: &ExecParams,
        environment: HashMap<String, String>,
        _runtime_paths: Option<&ExecServerRuntimePaths>,
    ) -> HashMap<String, String> {
        environment
    }
}
