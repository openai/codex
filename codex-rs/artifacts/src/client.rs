use crate::ArtifactRuntimeError;
use crate::ArtifactRuntimeManager;
use crate::InstalledArtifactRuntime;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;
use url::Url;

const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Executes artifact build commands against a resolved runtime.
#[derive(Clone, Debug)]
pub struct ArtifactsClient {
    runtime_source: RuntimeSource,
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
enum RuntimeSource {
    Managed(ArtifactRuntimeManager),
    Installed(InstalledArtifactRuntime),
}

impl ArtifactsClient {
    /// Creates a client that lazily resolves or downloads the runtime on demand.
    pub fn from_runtime_manager(runtime_manager: ArtifactRuntimeManager) -> Self {
        Self {
            runtime_source: RuntimeSource::Managed(runtime_manager),
        }
    }

    /// Creates a client pinned to an already loaded runtime.
    pub fn from_installed_runtime(runtime: InstalledArtifactRuntime) -> Self {
        Self {
            runtime_source: RuntimeSource::Installed(runtime),
        }
    }

    /// Executes artifact-building JavaScript against the configured runtime.
    pub async fn execute_build(
        &self,
        request: ArtifactBuildRequest,
    ) -> Result<ArtifactCommandOutput, ArtifactsError> {
        let runtime = self.resolve_runtime().await?;
        let js_runtime = runtime.resolve_js_runtime()?;
        let staging_dir = TempDir::new().map_err(|source| ArtifactsError::Io {
            context: "failed to create build staging directory".to_string(),
            source,
        })?;
        let script_path = staging_dir.path().join("artifact-build.mjs");
        let build_entrypoint_url =
            Url::from_file_path(runtime.build_js_path()).map_err(|()| ArtifactsError::Io {
                context: format!(
                    "failed to convert artifact build entrypoint to a file URL: {}",
                    runtime.build_js_path().display()
                ),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid artifact build entrypoint path",
                ),
            })?;
        let wrapped_script = build_wrapped_script(&build_entrypoint_url, &request.source);
        fs::write(&script_path, wrapped_script)
            .await
            .map_err(|source| ArtifactsError::Io {
                context: format!("failed to write {}", script_path.display()),
                source,
            })?;

        let mut command = Command::new(js_runtime.executable_path());
        command.arg(&script_path).current_dir(&request.cwd);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        if js_runtime.requires_electron_run_as_node() {
            command.env("ELECTRON_RUN_AS_NODE", "1");
        }
        for (key, value) in &request.env {
            command.env(key, value);
        }

        run_command(
            command,
            request.timeout.unwrap_or(DEFAULT_EXECUTION_TIMEOUT),
        )
        .await
    }

    async fn resolve_runtime(&self) -> Result<InstalledArtifactRuntime, ArtifactsError> {
        match &self.runtime_source {
            RuntimeSource::Installed(runtime) => Ok(runtime.clone()),
            RuntimeSource::Managed(manager) => manager.ensure_installed().await.map_err(Into::into),
        }
    }
}

/// Request payload for the artifact build command.
#[derive(Clone, Debug, Default)]
pub struct ArtifactBuildRequest {
    pub source: String,
    pub cwd: PathBuf,
    pub timeout: Option<Duration>,
    pub env: BTreeMap<String, String>,
}

/// Captured stdout, stderr, and exit status from an artifact subprocess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactCommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ArtifactCommandOutput {
    /// Returns whether the subprocess exited successfully.
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Errors raised while spawning or awaiting artifact subprocesses.
#[derive(Debug, Error)]
pub enum ArtifactsError {
    #[error(transparent)]
    Runtime(#[from] ArtifactRuntimeError),
    #[error("{context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("artifact command timed out after {timeout:?}")]
    TimedOut { timeout: Duration },
}

fn build_wrapped_script(build_entrypoint_url: &Url, source: &str) -> String {
    let mut wrapped = String::new();
    wrapped.push_str("const artifactTool = await import(");
    wrapped.push_str(
        &serde_json::to_string(build_entrypoint_url.as_str()).unwrap_or_else(|error| {
            panic!("artifact build entrypoint URL must serialize: {error}")
        }),
    );
    wrapped.push_str(");\n");
    wrapped.push_str(
        r#"globalThis.artifactTool = artifactTool;
for (const [name, value] of Object.entries(artifactTool)) {
  if (name === "default" || Object.prototype.hasOwnProperty.call(globalThis, name)) {
    continue;
  }
  globalThis[name] = value;
}
"#,
    );
    wrapped.push_str(source);
    wrapped.push('\n');
    wrapped
}

async fn run_command(
    mut command: Command,
    execution_timeout: Duration,
) -> Result<ArtifactCommandOutput, ArtifactsError> {
    let mut child = command.spawn().map_err(|source| ArtifactsError::Io {
        context: "failed to spawn artifact command".to_string(),
        source,
    })?;
    let mut stdout = child.stdout.take().ok_or_else(|| ArtifactsError::Io {
        context: "artifact command stdout was not captured".to_string(),
        source: std::io::Error::other("missing stdout pipe"),
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| ArtifactsError::Io {
        context: "artifact command stderr was not captured".to_string(),
        source: std::io::Error::other("missing stderr pipe"),
    })?;
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await.map(|_| bytes)
    });

    let status = match timeout(execution_timeout, child.wait()).await {
        Ok(result) => result.map_err(|source| ArtifactsError::Io {
            context: "failed while waiting for artifact command".to_string(),
            source,
        })?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(ArtifactsError::TimedOut {
                timeout: execution_timeout,
            });
        }
    };
    let stdout_bytes = stdout_task
        .await
        .map_err(|source| ArtifactsError::Io {
            context: "failed to join stdout reader".to_string(),
            source: std::io::Error::other(source.to_string()),
        })?
        .map_err(|source| ArtifactsError::Io {
            context: "failed to read artifact command stdout".to_string(),
            source,
        })?;
    let stderr_bytes = stderr_task
        .await
        .map_err(|source| ArtifactsError::Io {
            context: "failed to join stderr reader".to_string(),
            source: std::io::Error::other(source.to_string()),
        })?
        .map_err(|source| ArtifactsError::Io {
            context: "failed to read artifact command stderr".to_string(),
            source,
        })?;

    Ok(ArtifactCommandOutput {
        exit_code: status.code(),
        stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
    })
}
