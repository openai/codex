use std::mem::swap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_core::CodexAuth;
use codex_core::CodexThread;
use codex_core::ModelProviderInfo;
use codex_core::ThreadManager;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_core::shell::Shell;
use codex_core::shell::get_shell_by_model_provided_path;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_features::Feature;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::MockServer;

use crate::PathBufExt;
use crate::PathExt;
use crate::RemoteEnvConfig;
use crate::TempDirExt;
use crate::get_remote_test_env;
use crate::load_default_config_for_test;
use crate::responses::WebSocketTestServer;
use crate::responses::output_value_to_text;
use crate::responses::start_mock_server;
use crate::streaming_sse::StreamingSseServer;
use crate::wait_for_event;
use crate::wait_for_event_match;
use wiremock::Match;
use wiremock::matchers::path_regex;

type ConfigMutator = dyn FnOnce(&mut Config) + Send;
type PreBuildHook = dyn FnOnce(&Path) + Send + 'static;
const TEST_MODEL_WITH_EXPERIMENTAL_TOOLS: &str = "test-gpt-5.1-codex";
const REMOTE_EXEC_SERVER_START_TIMEOUT: Duration = Duration::from_secs(5);
const REMOTE_EXEC_SERVER_POLL_INTERVAL: Duration = Duration::from_millis(25);
static REMOTE_EXEC_SERVER_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct RemoteExecServerProcess {
    container_name: String,
    pid: u32,
    remote_exec_server_path: String,
    stdout_path: String,
    cleanup_paths: Vec<String>,
}

impl Drop for RemoteExecServerProcess {
    fn drop(&mut self) {
        let cleanup_paths = self.cleanup_paths.join(" ");
        let cleanup_paths_script = if cleanup_paths.is_empty() {
            String::new()
        } else {
            format!("rm -rf {cleanup_paths}; ")
        };
        let script = format!(
            "if kill -0 {pid} 2>/dev/null; then kill {pid}; fi; {cleanup_paths_script}rm -f {remote_exec_server_path} {stdout_path}",
            pid = self.pid,
            cleanup_paths_script = cleanup_paths_script,
            remote_exec_server_path = self.remote_exec_server_path,
            stdout_path = self.stdout_path
        );
        let _ = docker_command_capture_stdout(["exec", &self.container_name, "sh", "-lc", &script]);
    }
}

impl RemoteExecServerProcess {
    fn register_cleanup_path(&mut self, path: &Path) {
        self.cleanup_paths.push(path.display().to_string());
    }
}

#[derive(Debug)]
pub struct TestEnv {
    environment: codex_exec_server::Environment,
    cwd: PathBuf,
    _local_cwd_temp_dir: Option<TempDir>,
    _remote_exec_server_process: Option<RemoteExecServerProcess>,
}

impl TestEnv {
    pub async fn local() -> Result<Self> {
        let local_cwd_temp_dir = TempDir::new()?;
        let cwd = local_cwd_temp_dir.path().to_path_buf();
        let environment = codex_exec_server::Environment::create(/*exec_server_url*/ None).await?;
        Ok(Self {
            environment,
            cwd,
            _local_cwd_temp_dir: Some(local_cwd_temp_dir),
            _remote_exec_server_process: None,
        })
    }

    pub fn environment(&self) -> &codex_exec_server::Environment {
        &self.environment
    }

    pub fn exec_server_url(&self) -> Option<&str> {
        self.environment.exec_server_url()
    }
}

pub async fn test_env() -> Result<TestEnv> {
    match get_remote_test_env() {
        Some(remote_env) => {
            let mut remote_process = start_remote_exec_server(&remote_env)?;
            let remote_ip = remote_container_ip(&remote_env.container_name)?;
            let websocket_url = rewrite_websocket_host(&remote_process.listen_url, &remote_ip)?;
            let environment = codex_exec_server::Environment::create(Some(websocket_url)).await?;
            let cwd = remote_aware_cwd_path();
            environment
                .get_filesystem()
                .create_directory(
                    &absolute_path(&cwd)?,
                    CreateDirectoryOptions { recursive: true },
                )
                .await?;
            remote_process.process.register_cleanup_path(&cwd);
            Ok(TestEnv {
                environment,
                cwd,
                _local_cwd_temp_dir: None,
                _remote_exec_server_process: Some(remote_process.process),
            })
        }
        None => TestEnv::local().await,
    }
}

struct RemoteExecServerStart {
    process: RemoteExecServerProcess,
    listen_url: String,
}

fn start_remote_exec_server(remote_env: &RemoteEnvConfig) -> Result<RemoteExecServerStart> {
    let container_name = remote_env.container_name.as_str();
    let instance_id = remote_exec_server_instance_id();
    let remote_exec_server_path = format!("/tmp/codex-exec-server-{instance_id}");
    let stdout_path = format!("/tmp/codex-exec-server-{instance_id}.stdout");
    let local_binary = codex_utils_cargo_bin::cargo_bin("codex-exec-server")
        .context("resolve codex-exec-server binary")?;
    let local_binary = local_binary.to_string_lossy().to_string();
    let remote_binary = format!("{container_name}:{remote_exec_server_path}");

    docker_command_success(["cp", &local_binary, &remote_binary])?;
    docker_command_success([
        "exec",
        container_name,
        "chmod",
        "+x",
        &remote_exec_server_path,
    ])?;

    let start_script = format!(
        "rm -f {stdout_path}; \
nohup {remote_exec_server_path} --listen ws://0.0.0.0:0 > {stdout_path} 2>&1 & \
echo $!"
    );
    let pid_output =
        docker_command_capture_stdout(["exec", container_name, "sh", "-lc", &start_script])?;
    let pid = pid_output
        .trim()
        .parse::<u32>()
        .with_context(|| format!("parse remote exec-server PID from {pid_output:?}"))?;

    let listen_url = wait_for_remote_listen_url(container_name, &stdout_path)?;

    Ok(RemoteExecServerStart {
        process: RemoteExecServerProcess {
            container_name: container_name.to_string(),
            pid,
            remote_exec_server_path,
            stdout_path,
            cleanup_paths: Vec::new(),
        },
        listen_url,
    })
}

fn remote_aware_cwd_path() -> PathBuf {
    PathBuf::from(format!(
        "/tmp/codex-core-test-cwd-{}",
        remote_exec_server_instance_id()
    ))
}

fn wait_for_remote_listen_url(container_name: &str, stdout_path: &str) -> Result<String> {
    let deadline = Instant::now() + REMOTE_EXEC_SERVER_START_TIMEOUT;
    loop {
        let line = docker_command_capture_stdout([
            "exec",
            container_name,
            "sh",
            "-lc",
            &format!("head -n 1 {stdout_path} 2>/dev/null || true"),
        ])?;
        let listen_url = line.trim();
        if listen_url.starts_with("ws://") {
            return Ok(listen_url.to_string());
        }

        if Instant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for remote exec-server listen URL in container `{container_name}` after {REMOTE_EXEC_SERVER_START_TIMEOUT:?}"
            ));
        }
        std::thread::sleep(REMOTE_EXEC_SERVER_POLL_INTERVAL);
    }
}

fn remote_exec_server_instance_id() -> String {
    let instance = REMOTE_EXEC_SERVER_INSTANCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{instance}", std::process::id())
}

fn remote_container_ip(container_name: &str) -> Result<String> {
    let ip = docker_command_capture_stdout([
        "inspect",
        "-f",
        "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
        container_name,
    ])?;
    let ip = ip.trim();
    if ip.is_empty() {
        return Err(anyhow!(
            "container `{container_name}` has no IP address; cannot connect to remote exec-server"
        ));
    }
    Ok(ip.to_string())
}

fn rewrite_websocket_host(listen_url: &str, host: &str) -> Result<String> {
    let Some(address) = listen_url.strip_prefix("ws://") else {
        return Err(anyhow!(
            "unexpected websocket listen URL `{listen_url}`; expected ws://IP:PORT"
        ));
    };
    let Some((_, port)) = address.rsplit_once(':') else {
        return Err(anyhow!(
            "unexpected websocket listen URL `{listen_url}`; expected ws://IP:PORT"
        ));
    };
    Ok(format!("ws://{host}:{port}"))
}

fn docker_command_success<const N: usize>(args: [&str; N]) -> Result<()> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .with_context(|| format!("run docker {:?}", args))?;
    if !output.status.success() {
        return Err(anyhow!(
            "docker {:?} failed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn docker_command_capture_stdout<const N: usize>(args: [&str; N]) -> Result<String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .with_context(|| format!("run docker {:?}", args))?;
    if !output.status.success() {
        return Err(anyhow!(
            "docker {:?} failed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).context("docker stdout must be utf-8")
}

fn absolute_path(path: &Path) -> Result<AbsolutePathBuf> {
    Ok(path.abs())
}

/// A collection of different ways the model can output an apply_patch call
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApplyPatchModelOutput {
    Freeform,
    Function,
    Shell,
    ShellViaHeredoc,
    ShellCommandViaHeredoc,
}

/// A collection of different ways the model can output an apply_patch call
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShellModelOutput {
    Shell,
    ShellCommand,
    LocalShell,
    // UnifiedExec has its own set of tests
}

pub struct TestCodexBuilder {
    config_mutators: Vec<Box<ConfigMutator>>,
    auth: CodexAuth,
    pre_build_hooks: Vec<Box<PreBuildHook>>,
    home: Option<Arc<TempDir>>,
    user_shell_override: Option<Shell>,
}

impl TestCodexBuilder {
    pub fn with_config<T>(mut self, mutator: T) -> Self
    where
        T: FnOnce(&mut Config) + Send + 'static,
    {
        self.config_mutators.push(Box::new(mutator));
        self
    }

    pub fn with_auth(mut self, auth: CodexAuth) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_model(self, model: &str) -> Self {
        let new_model = model.to_string();
        self.with_config(move |config| {
            config.model = Some(new_model.clone());
        })
    }

    pub fn with_pre_build_hook<F>(mut self, hook: F) -> Self
    where
        F: FnOnce(&Path) + Send + 'static,
    {
        self.pre_build_hooks.push(Box::new(hook));
        self
    }

    pub fn with_home(mut self, home: Arc<TempDir>) -> Self {
        self.home = Some(home);
        self
    }

    pub fn with_user_shell(mut self, user_shell: Shell) -> Self {
        self.user_shell_override = Some(user_shell);
        self
    }

    pub fn with_windows_cmd_shell(self) -> Self {
        if cfg!(windows) {
            self.with_user_shell(get_shell_by_model_provided_path(&PathBuf::from("cmd.exe")))
        } else {
            self
        }
    }

    pub async fn build(&mut self, server: &wiremock::MockServer) -> anyhow::Result<TestCodex> {
        let home = match self.home.clone() {
            Some(home) => home,
            None => Arc::new(TempDir::new()?),
        };
        Box::pin(self.build_with_home(server, home, /*resume_from*/ None)).await
    }

    pub async fn build_remote_aware(
        &mut self,
        server: &wiremock::MockServer,
    ) -> anyhow::Result<TestCodex> {
        let test_env = test_env().await?;
        let home = match self.home.clone() {
            Some(home) => home,
            None => Arc::new(TempDir::new()?),
        };
        let base_url = format!("{}/v1", server.uri());
        let cwd = test_env.cwd.to_path_buf();
        self.config_mutators.push(Box::new(move |config| {
            config.cwd = cwd.abs();
        }));
        let (config, cwd) = self.prepare_config(base_url, &home).await?;
        Box::pin(self.build_from_config(config, cwd, home, /*resume_from*/ None, test_env)).await
    }

    pub async fn build_with_streaming_server(
        &mut self,
        server: &StreamingSseServer,
    ) -> anyhow::Result<TestCodex> {
        let base_url = server.uri();
        let home = match self.home.clone() {
            Some(home) => home,
            None => Arc::new(TempDir::new()?),
        };
        Box::pin(self.build_with_home_and_base_url(
            format!("{base_url}/v1"),
            home,
            /*resume_from*/ None,
        ))
        .await
    }

    pub async fn build_with_websocket_server(
        &mut self,
        server: &WebSocketTestServer,
    ) -> anyhow::Result<TestCodex> {
        let base_url = format!("{}/v1", server.uri());
        let home = match self.home.clone() {
            Some(home) => home,
            None => Arc::new(TempDir::new()?),
        };
        let base_url_clone = base_url.clone();
        self.config_mutators.push(Box::new(move |config| {
            config.model_provider.base_url = Some(base_url_clone);
            config.model_provider.supports_websockets = true;
            config.experimental_realtime_ws_model = Some("realtime-test-model".to_string());
        }));
        Box::pin(self.build_with_home_and_base_url(base_url, home, /*resume_from*/ None)).await
    }

    pub async fn resume(
        &mut self,
        server: &wiremock::MockServer,
        home: Arc<TempDir>,
        rollout_path: PathBuf,
    ) -> anyhow::Result<TestCodex> {
        Box::pin(self.build_with_home(server, home, Some(rollout_path))).await
    }

    async fn build_with_home(
        &mut self,
        server: &wiremock::MockServer,
        home: Arc<TempDir>,
        resume_from: Option<PathBuf>,
    ) -> anyhow::Result<TestCodex> {
        let base_url = format!("{}/v1", server.uri());
        let (config, cwd) = self.prepare_config(base_url, &home).await?;
        Box::pin(self.build_from_config(config, cwd, home, resume_from, TestEnv::local().await?))
            .await
    }

    async fn build_with_home_and_base_url(
        &mut self,
        base_url: String,
        home: Arc<TempDir>,
        resume_from: Option<PathBuf>,
    ) -> anyhow::Result<TestCodex> {
        let (config, cwd) = self.prepare_config(base_url, &home).await?;
        Box::pin(self.build_from_config(config, cwd, home, resume_from, TestEnv::local().await?))
            .await
    }

    async fn build_from_config(
        &mut self,
        config: Config,
        cwd: Arc<TempDir>,
        home: Arc<TempDir>,
        resume_from: Option<PathBuf>,
        test_env: TestEnv,
    ) -> anyhow::Result<TestCodex> {
        let auth = self.auth.clone();
        let environment_manager = Arc::new(codex_exec_server::EnvironmentManager::new(
            test_env.exec_server_url().map(str::to_owned),
        ));
        let thread_manager = if config.model_catalog.is_some() {
            ThreadManager::new(
                &config,
                codex_core::test_support::auth_manager_from_auth(auth.clone()),
                SessionSource::Exec,
                CollaborationModesConfig::default(),
                Arc::clone(&environment_manager),
            )
        } else {
            codex_core::test_support::thread_manager_with_models_provider_and_home(
                auth.clone(),
                config.model_provider.clone(),
                config.codex_home.clone(),
                Arc::clone(&environment_manager),
            )
        };
        let thread_manager = Arc::new(thread_manager);
        let user_shell_override = self.user_shell_override.clone();

        let new_conversation = match (resume_from, user_shell_override) {
            (Some(path), Some(user_shell_override)) => {
                let auth_manager = codex_core::test_support::auth_manager_from_auth(auth);
                Box::pin(
                    codex_core::test_support::resume_thread_from_rollout_with_user_shell_override(
                        thread_manager.as_ref(),
                        config.clone(),
                        path,
                        auth_manager,
                        user_shell_override,
                    ),
                )
                .await?
            }
            (Some(path), None) => {
                let auth_manager = codex_core::test_support::auth_manager_from_auth(auth);
                Box::pin(thread_manager.resume_thread_from_rollout(
                    config.clone(),
                    path,
                    auth_manager,
                    /*parent_trace*/ None,
                ))
                .await?
            }
            (None, Some(user_shell_override)) => {
                Box::pin(
                    codex_core::test_support::start_thread_with_user_shell_override(
                        thread_manager.as_ref(),
                        config.clone(),
                        user_shell_override,
                    ),
                )
                .await?
            }
            (None, None) => Box::pin(thread_manager.start_thread(config.clone())).await?,
        };

        Ok(TestCodex {
            home,
            cwd,
            config,
            codex: new_conversation.thread,
            session_configured: new_conversation.session_configured,
            thread_manager,
            _test_env: test_env,
        })
    }

    async fn prepare_config(
        &mut self,
        base_url: String,
        home: &TempDir,
    ) -> anyhow::Result<(Config, Arc<TempDir>)> {
        let model_provider = ModelProviderInfo {
            base_url: Some(base_url),
            // Most core tests use SSE-only mock servers, so keep websocket transport off unless
            // a test explicitly opts into websocket coverage.
            supports_websockets: false,
            ..built_in_model_providers(/*openai_base_url*/ None)["openai"].clone()
        };
        let cwd = Arc::new(TempDir::new()?);
        let mut config = load_default_config_for_test(home).await;
        config.cwd = cwd.abs();
        config.model_provider = model_provider;
        for hook in self.pre_build_hooks.drain(..) {
            hook(home.path());
        }
        if let Ok(path) = codex_utils_cargo_bin::cargo_bin("codex") {
            config.codex_self_exe = Some(path);
        } else if let Ok(path) = codex_utils_cargo_bin::cargo_bin("codex-exec") {
            // `codex-exec` also supports `--codex-run-as-apply-patch`, so use it
            // when the multitool binary is not available in test builds.
            config.codex_self_exe = Some(path);
        } else if let Ok(exe) = std::env::current_exe()
            && let Some(bin_dir) = exe.parent().and_then(|parent| parent.parent())
        {
            let codex = bin_dir.join("codex");
            let codex_exec = bin_dir.join("codex-exec");
            if codex.is_file() {
                config.codex_self_exe = Some(codex);
            } else if codex_exec.is_file() {
                config.codex_self_exe = Some(codex_exec);
            }
        }
        if let Some(path) = find_codex_cli_exe() {
            config.codex_linux_sandbox_exe = Some(path);
        }

        let mut mutators = vec![];
        swap(&mut self.config_mutators, &mut mutators);
        for mutator in mutators {
            mutator(&mut config);
        }
        ensure_test_model_catalog(&mut config)?;

        if config.include_apply_patch_tool {
            config.features.enable(Feature::ApplyPatchFreeform)?;
        } else {
            config.features.disable(Feature::ApplyPatchFreeform)?;
        }

        Ok((config, cwd))
    }
}

fn find_codex_cli_exe() -> Option<PathBuf> {
    codex_utils_cargo_bin::cargo_bin("codex")
        .ok()
        .filter(|path| path.file_stem().is_some_and(|stem| stem == "codex"))
        .or_else(|| {
            std::env::current_exe().ok().and_then(|exe| {
                exe.parent()
                    .and_then(|parent| parent.parent())
                    .map(|parent| parent.join(format!("codex{}", std::env::consts::EXE_SUFFIX)))
                    .filter(|path| path.is_file())
            })
        })
}

fn ensure_test_model_catalog(config: &mut Config) -> Result<()> {
    if config.model.as_deref() != Some(TEST_MODEL_WITH_EXPERIMENTAL_TOOLS)
        || config.model_catalog.is_some()
    {
        return Ok(());
    }

    let bundled_models_path = codex_utils_cargo_bin::find_resource!("../../models.json")
        .context("bundled models.json")?;
    let bundled_models_contents =
        std::fs::read_to_string(&bundled_models_path).with_context(|| {
            format!(
                "read bundled models.json from {}",
                bundled_models_path.display()
            )
        })?;
    let bundled_models: ModelsResponse =
        serde_json::from_str(&bundled_models_contents).context("parse bundled models.json")?;
    let mut model = bundled_models
        .models
        .iter()
        .find(|candidate| candidate.slug == "gpt-5.1-codex")
        .cloned()
        .unwrap_or_else(|| panic!("missing bundled model gpt-5.1-codex"));
    model.slug = TEST_MODEL_WITH_EXPERIMENTAL_TOOLS.to_string();
    model.display_name = TEST_MODEL_WITH_EXPERIMENTAL_TOOLS.to_string();
    model.experimental_supported_tools = vec!["test_sync_tool".to_string()];
    config.model_catalog = Some(ModelsResponse {
        models: vec![model],
    });
    Ok(())
}

pub struct TestCodex {
    pub home: Arc<TempDir>,
    pub cwd: Arc<TempDir>,
    pub codex: Arc<CodexThread>,
    pub session_configured: SessionConfiguredEvent,
    pub config: Config,
    pub thread_manager: Arc<ThreadManager>,
    _test_env: TestEnv,
}

impl TestCodex {
    pub fn cwd_path(&self) -> &Path {
        self.cwd.path()
    }

    pub fn codex_home_path(&self) -> &Path {
        self.config.codex_home.as_path()
    }

    pub fn workspace_path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.cwd_path().join(rel)
    }

    pub fn executor_environment(&self) -> &TestEnv {
        &self._test_env
    }

    pub fn fs(&self) -> Arc<dyn ExecutorFileSystem> {
        self._test_env.environment().get_filesystem()
    }

    pub async fn submit_turn(&self, prompt: &str) -> Result<()> {
        self.submit_turn_with_policies(
            prompt,
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
        )
        .await
    }

    pub async fn submit_turn_with_policy(
        &self,
        prompt: &str,
        sandbox_policy: SandboxPolicy,
    ) -> Result<()> {
        self.submit_turn_with_policies(prompt, AskForApproval::Never, sandbox_policy)
            .await
    }

    pub async fn submit_turn_with_service_tier(
        &self,
        prompt: &str,
        service_tier: Option<ServiceTier>,
    ) -> Result<()> {
        self.submit_turn_with_context(
            prompt,
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
            Some(service_tier),
        )
        .await
    }

    pub async fn submit_turn_with_policies(
        &self,
        prompt: &str,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
    ) -> Result<()> {
        self.submit_turn_with_context(
            prompt,
            approval_policy,
            sandbox_policy,
            /*service_tier*/ None,
        )
        .await
    }

    async fn submit_turn_with_context(
        &self,
        prompt: &str,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
        service_tier: Option<Option<ServiceTier>>,
    ) -> Result<()> {
        let session_model = self.session_configured.model.clone();
        self.codex
            .submit(Op::UserTurn {
                items: vec![UserInput::Text {
                    text: prompt.into(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                cwd: self.config.cwd.to_path_buf(),
                approval_policy,
                approvals_reviewer: None,
                sandbox_policy,
                model: session_model,
                effort: None,
                summary: None,
                service_tier,
                collaboration_mode: None,
                personality: None,
            })
            .await?;

        let turn_id = wait_for_event_match(&self.codex, |event| match event {
            EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
            _ => None,
        })
        .await;
        wait_for_event(&self.codex, |event| match event {
            EventMsg::TurnComplete(event) => event.turn_id == turn_id,
            _ => false,
        })
        .await;
        Ok(())
    }
}

pub struct TestCodexHarness {
    server: MockServer,
    test: TestCodex,
}

impl TestCodexHarness {
    pub async fn new() -> Result<Self> {
        Self::with_builder(test_codex()).await
    }

    pub async fn with_config(mutator: impl FnOnce(&mut Config) + Send + 'static) -> Result<Self> {
        Self::with_builder(test_codex().with_config(mutator)).await
    }

    pub async fn with_builder(mut builder: TestCodexBuilder) -> Result<Self> {
        let server = start_mock_server().await;
        let test = builder.build(&server).await?;
        Ok(Self { server, test })
    }

    pub fn server(&self) -> &MockServer {
        &self.server
    }

    pub fn test(&self) -> &TestCodex {
        &self.test
    }

    pub fn cwd(&self) -> &Path {
        self.test.cwd_path()
    }

    pub fn path(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.test.workspace_path(rel)
    }

    pub async fn submit(&self, prompt: &str) -> Result<()> {
        // Box the submit-and-wait path so callers do not inline the full turn
        // future into their own async state.
        Box::pin(self.test.submit_turn(prompt)).await
    }

    pub async fn submit_with_policy(
        &self,
        prompt: &str,
        sandbox_policy: SandboxPolicy,
    ) -> Result<()> {
        self.test
            .submit_turn_with_policy(prompt, sandbox_policy)
            .await
    }

    pub async fn request_bodies(&self) -> Vec<Value> {
        let path_matcher = path_regex(".*/responses$");
        self.server
            .received_requests()
            .await
            .expect("mock server should not fail")
            .into_iter()
            .filter(|req| path_matcher.matches(req))
            .map(|req| {
                req.body_json::<Value>()
                    .expect("request body to be valid JSON")
            })
            .collect()
    }

    pub async fn function_call_output_value(&self, call_id: &str) -> Value {
        let bodies = self.request_bodies().await;
        function_call_output(&bodies, call_id).clone()
    }

    pub async fn function_call_stdout(&self, call_id: &str) -> String {
        self.function_call_output_value(call_id)
            .await
            .get("output")
            .and_then(Value::as_str)
            .expect("output string")
            .to_string()
    }

    pub async fn custom_tool_call_output(&self, call_id: &str) -> String {
        let bodies = self.request_bodies().await;
        custom_tool_call_output_text(&bodies, call_id)
    }

    pub async fn apply_patch_output(
        &self,
        call_id: &str,
        output_type: ApplyPatchModelOutput,
    ) -> String {
        // Box the awaited output helpers so callers do not inline request
        // capture and response parsing into their own async state.
        match output_type {
            ApplyPatchModelOutput::Freeform => {
                Box::pin(self.custom_tool_call_output(call_id)).await
            }
            ApplyPatchModelOutput::Function
            | ApplyPatchModelOutput::Shell
            | ApplyPatchModelOutput::ShellViaHeredoc
            | ApplyPatchModelOutput::ShellCommandViaHeredoc => {
                Box::pin(self.function_call_stdout(call_id)).await
            }
        }
    }
}

fn custom_tool_call_output<'a>(bodies: &'a [Value], call_id: &str) -> &'a Value {
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("custom_tool_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
                {
                    return item;
                }
            }
        }
    }
    panic!("custom_tool_call_output {call_id} not found");
}

fn custom_tool_call_output_text(bodies: &[Value], call_id: &str) -> String {
    let output = custom_tool_call_output(bodies, call_id)
        .get("output")
        .unwrap_or_else(|| panic!("custom_tool_call_output {call_id} missing output"));
    output_value_to_text(output)
        .unwrap_or_else(|| panic!("custom_tool_call_output {call_id} missing text output"))
}

fn function_call_output<'a>(bodies: &'a [Value], call_id: &str) -> &'a Value {
    for body in bodies {
        if let Some(items) = body.get("input").and_then(Value::as_array) {
            for item in items {
                if item.get("type").and_then(Value::as_str) == Some("function_call_output")
                    && item.get("call_id").and_then(Value::as_str) == Some(call_id)
                {
                    return item;
                }
            }
        }
    }
    panic!("function_call_output {call_id} not found");
}

pub fn test_codex() -> TestCodexBuilder {
    TestCodexBuilder {
        config_mutators: vec![],
        auth: CodexAuth::from_api_key("dummy"),
        pre_build_hooks: vec![],
        home: None,
        user_shell_override: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn custom_tool_call_output_text_returns_output_text() {
        let bodies = vec![json!({
            "input": [{
                "type": "custom_tool_call_output",
                "call_id": "call-1",
                "output": "hello"
            }]
        })];

        assert_eq!(custom_tool_call_output_text(&bodies, "call-1"), "hello");
    }

    #[test]
    #[should_panic(expected = "custom_tool_call_output call-2 missing output")]
    fn custom_tool_call_output_text_panics_when_output_is_missing() {
        let bodies = vec![json!({
            "input": [{
                "type": "custom_tool_call_output",
                "call_id": "call-2"
            }]
        })];

        let _ = custom_tool_call_output_text(&bodies, "call-2");
    }
}
