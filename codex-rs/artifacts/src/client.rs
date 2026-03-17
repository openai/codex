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

const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const VISUAL_ARTIFACTS_MANIFEST_PREFIX: &str = "__CODEX_VISUAL_ARTIFACTS__";

/// Executes artifact build and render commands against a resolved runtime.
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
        let wrapped_script = build_wrapped_script(&request.source);
        fs::write(&script_path, wrapped_script)
            .await
            .map_err(|source| ArtifactsError::Io {
                context: format!("failed to write {}", script_path.display()),
                source,
            })?;

        let mut command = Command::new(js_runtime.executable_path());
        command
            .arg(&script_path)
            .current_dir(&request.cwd)
            .env("CODEX_ARTIFACT_BUILD_ENTRYPOINT", runtime.build_js_path())
            .env(
                "CODEX_ARTIFACT_RENDER_ENTRYPOINT",
                runtime.render_cli_path(),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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

    /// Executes the artifact render CLI against the configured runtime.
    pub async fn execute_render(
        &self,
        request: ArtifactRenderCommandRequest,
    ) -> Result<ArtifactCommandOutput, ArtifactsError> {
        let runtime = self.resolve_runtime().await?;
        let js_runtime = runtime.resolve_js_runtime()?;
        let mut command = Command::new(js_runtime.executable_path());
        command
            .arg(runtime.render_cli_path())
            .args(request.target.to_args())
            .current_dir(&request.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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

/// Request payload for the artifact render CLI.
#[derive(Clone, Debug)]
pub struct ArtifactRenderCommandRequest {
    pub cwd: PathBuf,
    pub timeout: Option<Duration>,
    pub env: BTreeMap<String, String>,
    pub target: ArtifactRenderTarget,
}

/// Render targets supported by the packaged artifact runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactRenderTarget {
    Presentation(PresentationRenderTarget),
    Spreadsheet(SpreadsheetRenderTarget),
}

impl ArtifactRenderTarget {
    /// Converts a render target to the CLI args expected by `render_cli.mjs`.
    pub fn to_args(&self) -> Vec<String> {
        match self {
            Self::Presentation(target) => {
                vec![
                    "pptx".to_string(),
                    "render".to_string(),
                    "--in".to_string(),
                    target.input_path.display().to_string(),
                    "--slide".to_string(),
                    target.slide_number.to_string(),
                    "--out".to_string(),
                    target.output_path.display().to_string(),
                ]
            }
            Self::Spreadsheet(target) => {
                let mut args = vec![
                    "xlsx".to_string(),
                    "render".to_string(),
                    "--in".to_string(),
                    target.input_path.display().to_string(),
                    "--sheet".to_string(),
                    target.sheet_name.clone(),
                    "--out".to_string(),
                    target.output_path.display().to_string(),
                ];
                if let Some(range) = &target.range {
                    args.push("--range".to_string());
                    args.push(range.clone());
                }
                args
            }
        }
    }
}

/// Presentation render request parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationRenderTarget {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub slide_number: u32,
}

/// Spreadsheet render request parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpreadsheetRenderTarget {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub sheet_name: String,
    pub range: Option<String>,
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

fn build_wrapped_script(source: &str) -> String {
    let mut wrapped = concat!(
        "import { pathToFileURL } from \"node:url\";\n",
        "const artifactTool = await import(pathToFileURL(process.env.CODEX_ARTIFACT_BUILD_ENTRYPOINT).href);\n",
        "const __codexVisualArtifacts = [];\n",
        "let __codexArtifactDocument;\n",
        "const emitVisual = (spec) => {\n",
        "  if (!spec || typeof spec !== \"object\" || Array.isArray(spec)) {\n",
        "    throw new TypeError(\"codex.emitVisual(...) expects an object\");\n",
        "  }\n",
        "  const title = typeof spec.title === \"string\" && spec.title.trim()\n",
        "    ? spec.title.trim()\n",
        "    : \"Interactive visual\";\n",
        "  const html = typeof spec.html === \"string\" ? spec.html : \"\";\n",
        "  if (!html.trim()) {\n",
        "    throw new TypeError(\"codex.emitVisual(...) requires a non-empty html string\");\n",
        "  }\n",
        "  const summary = typeof spec.summary === \"string\" && spec.summary.trim()\n",
        "    ? spec.summary.trim()\n",
        "    : undefined;\n",
        "  const heightPx = Number.isFinite(spec.heightPx)\n",
        "    ? Math.max(160, Math.min(1600, Math.trunc(spec.heightPx)))\n",
        "    : undefined;\n",
        "  const visual = { title, html, summary, heightPx };\n",
        "  __codexVisualArtifacts.push(visual);\n",
        "  return visual;\n",
        "};\n",
        "const setDocument = (spec) => {\n",
        "  const normalized = typeof spec === \"string\" ? { markdown: spec } : spec;\n",
        "  if (!normalized || typeof normalized !== \"object\" || Array.isArray(normalized)) {\n",
        "    throw new TypeError(\"codex.setDocument(...) expects a string or object\");\n",
        "  }\n",
        "  const markdown = typeof normalized.markdown === \"string\" ? normalized.markdown : \"\";\n",
        "  if (!markdown.trim()) {\n",
        "    throw new TypeError(\"codex.setDocument(...) requires a non-empty markdown string\");\n",
        "  }\n",
        "  const title = typeof normalized.title === \"string\" && normalized.title.trim()\n",
        "    ? normalized.title.trim()\n",
        "    : \"Markdown workspace\";\n",
        "  const document = { title, markdown };\n",
        "  __codexArtifactDocument = document;\n",
        "  return document;\n",
        "};\n",
        "globalThis.codex = { ...(globalThis.codex ?? {}), emitVisual, setDocument };\n",
        "globalThis.emitVisual = emitVisual;\n",
        "globalThis.setDocument = setDocument;\n",
        "globalThis.artifactTool = artifactTool;\n",
        "globalThis.artifacts = artifactTool;\n",
        "globalThis.codexArtifacts = artifactTool;\n",
        "for (const [name, value] of Object.entries(artifactTool)) {\n",
        "  if (name === \"default\" || Object.prototype.hasOwnProperty.call(globalThis, name)) {\n",
        "    continue;\n",
        "  }\n",
        "  globalThis[name] = value;\n",
        "}\n\n",
        "try {\n",
    )
    .to_string();
    wrapped.push_str(source);
    wrapped.push_str(
        "\n\
} finally {\n\
  const __codexManifest = Buffer.from(\n\
    JSON.stringify({ visuals: __codexVisualArtifacts, document: __codexArtifactDocument ?? null }),\n\
    \"utf8\"\n\
  ).toString(\"base64\");\n\
  process.stdout.write(`\\n",
    );
    wrapped.push_str(VISUAL_ARTIFACTS_MANIFEST_PREFIX);
    wrapped.push_str("${__codexManifest}\\n`);\n}\n");
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
