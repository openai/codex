use std::ffi::OsStr;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::Environment;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use tempfile::TempDir;

pub const TEST_ENVIRONMENT_ENV_VAR: &str = "CODEX_TEST_ENVIRONMENT";
pub const LEGACY_REMOTE_ENV_ENV_VAR: &str = "CODEX_TEST_REMOTE_ENV";
pub const DOCKER_CONTAINER_ENV_VAR: &str = "CODEX_TEST_REMOTE_ENV_CONTAINER_NAME";
const REMOTE_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_TEST_REMOTE_EXEC_SERVER_URL";
static REMOTE_TEST_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestEnvironment {
    Local,
    Docker { container_name: String },
    WineExec,
}

impl TestEnvironment {
    pub fn is_remote(&self) -> bool {
        !matches!(self, Self::Local)
    }

    pub fn docker_container_name(&self) -> Option<&str> {
        match self {
            Self::Docker { container_name } => Some(container_name),
            Self::Local | Self::WineExec => None,
        }
    }
}

/// Owns the execution environment and workspace selected for an integration test.
///
/// Local tests receive a temporary local directory. Docker and Wine-exec tests receive a unique
/// directory created through the configured remote exec server.
#[derive(Debug)]
pub struct TestExecutionEnvironment {
    environment: Environment,
    cwd: AbsolutePathBuf,
    environment_cwd: LegacyAppPathString,
    state: TestExecutionEnvironmentState,
}

impl TestExecutionEnvironment {
    pub async fn new() -> Result<Self> {
        match test_environment() {
            TestEnvironment::Local => Self::local().await,
            TestEnvironment::Docker { container_name } => {
                Self::remote(RemoteTestEnvironment::Docker { container_name }).await
            }
            TestEnvironment::WineExec => Self::remote(RemoteTestEnvironment::WineExec).await,
        }
    }

    async fn remote(remote: RemoteTestEnvironment) -> Result<Self> {
        let websocket_url = remote_exec_server_url()?;
        let environment = Environment::create_for_tests(Some(websocket_url))?;
        let instance_id = remote_test_instance_id();
        let (cwd_uri, path_convention) = match &remote {
            RemoteTestEnvironment::Docker { .. } => (
                PathUri::parse(&format!("file:///tmp/codex-test-cwd-{instance_id}"))?,
                PathConvention::Posix,
            ),
            RemoteTestEnvironment::WineExec => {
                // Each Wine-exec test process has an isolated filesystem root, so this drive-root
                // path cannot collide with a different Bazel shard.
                (
                    PathUri::parse(&format!("file:///C:/codex-test-cwd-{instance_id}"))?,
                    PathConvention::Windows,
                )
            }
        };
        let environment_cwd = LegacyAppPathString::from_path_uri(&cwd_uri, path_convention)?;
        environment
            .get_filesystem()
            .create_directory(
                &cwd_uri,
                CreateDirectoryOptions { recursive: true },
                /*sandbox*/ None,
            )
            .await?;
        let cwd = match &remote {
            RemoteTestEnvironment::Docker { .. } => cwd_uri.to_abs_path()?,
            RemoteTestEnvironment::WineExec => {
                // TODO(anp): Change `Config::cwd` to `LegacyAppPathString` so this host-compatible
                // projection can be removed.
                let path = cwd_uri.to_url().to_file_path().map_err(|()| {
                    anyhow!("remote test cwd URI cannot be projected onto the host: {cwd_uri}")
                })?;
                AbsolutePathBuf::try_from(path)?
            }
        };
        let state = match remote {
            RemoteTestEnvironment::Docker { container_name } => {
                TestExecutionEnvironmentState::Docker { container_name }
            }
            RemoteTestEnvironment::WineExec => TestExecutionEnvironmentState::WineExec,
        };
        Ok(Self {
            environment,
            cwd,
            environment_cwd,
            state,
        })
    }

    pub async fn local() -> Result<Self> {
        let local_cwd_temp_dir = Arc::new(TempDir::new()?);
        let cwd = AbsolutePathBuf::from_absolute_path_checked(local_cwd_temp_dir.path())?;
        let environment = Environment::create_for_tests(/*exec_server_url*/ None)?;
        Ok(Self {
            environment,
            environment_cwd: LegacyAppPathString::from_abs_path(&cwd),
            cwd,
            state: TestExecutionEnvironmentState::Local {
                temp_dir: local_cwd_temp_dir,
            },
        })
    }

    pub fn environment_id(&self) -> &'static str {
        match self.state {
            TestExecutionEnvironmentState::Local { .. } => LOCAL_ENVIRONMENT_ID,
            TestExecutionEnvironmentState::Docker { .. }
            | TestExecutionEnvironmentState::WineExec => REMOTE_ENVIRONMENT_ID,
        }
    }

    pub fn exec_server_url(&self) -> Option<&str> {
        self.environment.exec_server_url()
    }

    pub fn cwd(&self) -> &AbsolutePathBuf {
        &self.cwd
    }

    pub fn environment_cwd(&self) -> &LegacyAppPathString {
        &self.environment_cwd
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub(crate) fn local_cwd_temp_dir(&self) -> Option<Arc<TempDir>> {
        match &self.state {
            TestExecutionEnvironmentState::Local { temp_dir, .. } => Some(Arc::clone(temp_dir)),
            TestExecutionEnvironmentState::Docker { .. }
            | TestExecutionEnvironmentState::WineExec => None,
        }
    }
}

impl Drop for TestExecutionEnvironment {
    fn drop(&mut self) {
        if let TestExecutionEnvironmentState::Docker { container_name } = &self.state {
            let script = format!("rm -rf {}", self.cwd.as_path().display());
            let _ = docker_command_capture_stdout(["exec", container_name, "sh", "-lc", &script]);
        }
    }
}

#[derive(Debug)]
enum TestExecutionEnvironmentState {
    Local { temp_dir: Arc<TempDir> },
    Docker { container_name: String },
    WineExec,
}

#[derive(Debug)]
enum RemoteTestEnvironment {
    Docker { container_name: String },
    WineExec,
}

pub async fn test_execution_environment() -> Result<TestExecutionEnvironment> {
    TestExecutionEnvironment::new().await
}

pub fn test_environment() -> TestEnvironment {
    let environment = parse_test_environment(
        std::env::var_os(TEST_ENVIRONMENT_ENV_VAR).as_deref(),
        std::env::var_os(LEGACY_REMOTE_ENV_ENV_VAR).as_deref(),
        std::env::var_os(DOCKER_CONTAINER_ENV_VAR).as_deref(),
    )
    .expect("invalid test environment configuration");

    if matches!(environment, TestEnvironment::WineExec) && !cfg!(target_os = "linux") {
        panic!("{TEST_ENVIRONMENT_ENV_VAR}=wine-exec is only supported on Linux");
    }

    environment
}

pub fn get_remote_test_env() -> Option<TestEnvironment> {
    let environment = test_environment();
    environment.is_remote().then_some(environment)
}

fn parse_test_environment(
    configured_environment: Option<&OsStr>,
    legacy_remote_environment: Option<&OsStr>,
    docker_container: Option<&OsStr>,
) -> Result<TestEnvironment, String> {
    let configured_environment = configured_environment
        .map(|value| {
            value
                .to_str()
                .ok_or_else(|| format!("{TEST_ENVIRONMENT_ENV_VAR} must contain valid UTF-8"))
        })
        .transpose()?;

    match configured_environment {
        None => match legacy_remote_environment {
            Some(container_name) => Ok(TestEnvironment::Docker {
                container_name: non_empty_utf8(LEGACY_REMOTE_ENV_ENV_VAR, container_name)?,
            }),
            None => Ok(TestEnvironment::Local),
        },
        Some("local") => Ok(TestEnvironment::Local),
        Some("docker") => {
            let (container_name_env_var, container_name) = match docker_container {
                Some(container_name) => (DOCKER_CONTAINER_ENV_VAR, container_name),
                None => (
                    LEGACY_REMOTE_ENV_ENV_VAR,
                    legacy_remote_environment.ok_or_else(|| {
                        format!(
                            "{DOCKER_CONTAINER_ENV_VAR} must be set when {TEST_ENVIRONMENT_ENV_VAR}=docker"
                        )
                    })?,
                ),
            };
            Ok(TestEnvironment::Docker {
                container_name: non_empty_utf8(container_name_env_var, container_name)?,
            })
        }
        Some("wine-exec") => Ok(TestEnvironment::WineExec),
        Some(value) => Err(format!(
            "{TEST_ENVIRONMENT_ENV_VAR} must be one of local, docker, or wine-exec; got {value:?}"
        )),
    }
}

fn non_empty_utf8(name: &str, value: &OsStr) -> Result<String, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{name} must contain valid UTF-8"))?;
    if value.trim().is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    Ok(value.to_string())
}

fn remote_exec_server_url() -> Result<String> {
    let listen_url = std::env::var(REMOTE_EXEC_SERVER_URL_ENV_VAR).with_context(|| {
        format!("{REMOTE_EXEC_SERVER_URL_ENV_VAR} must be set for remote tests")
    })?;
    let listen_url = listen_url.trim();
    if listen_url.is_empty() {
        return Err(anyhow!(
            "{REMOTE_EXEC_SERVER_URL_ENV_VAR} must not be empty"
        ));
    }
    Ok(listen_url.to_string())
}

fn remote_test_instance_id() -> String {
    let instance = REMOTE_TEST_INSTANCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{instance}", std::process::id())
}

fn docker_command_capture_stdout<const N: usize>(args: [&str; N]) -> Result<String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .with_context(|| format!("run docker {args:?}"))?;
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

#[cfg(test)]
#[path = "test_environment_tests.rs"]
mod tests;
