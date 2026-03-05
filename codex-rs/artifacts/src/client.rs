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

#[derive(Clone, Debug)]
pub struct ArtifactsClient {
    runtime_source: RuntimeSource,
}

#[derive(Clone, Debug)]
enum RuntimeSource {
    Managed(ArtifactRuntimeManager),
    Installed(InstalledArtifactRuntime),
}

impl ArtifactsClient {
    pub fn from_runtime_manager(runtime_manager: ArtifactRuntimeManager) -> Self {
        Self {
            runtime_source: RuntimeSource::Managed(runtime_manager),
        }
    }

    pub fn from_installed_runtime(runtime: InstalledArtifactRuntime) -> Self {
        Self {
            runtime_source: RuntimeSource::Installed(runtime),
        }
    }

    pub async fn execute_build(
        &self,
        request: ArtifactBuildRequest,
    ) -> Result<ArtifactCommandOutput, ArtifactsError> {
        let runtime = self.resolve_runtime().await?;
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

        let mut command = Command::new(runtime.node_path());
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
        for (key, value) in &request.env {
            command.env(key, value);
        }

        run_command(
            command,
            request.timeout.unwrap_or(DEFAULT_EXECUTION_TIMEOUT),
        )
        .await
    }

    pub async fn execute_render(
        &self,
        request: ArtifactRenderCommandRequest,
    ) -> Result<ArtifactCommandOutput, ArtifactsError> {
        let runtime = self.resolve_runtime().await?;
        let mut command = Command::new(runtime.node_path());
        command
            .arg(runtime.render_cli_path())
            .args(request.target.to_args())
            .current_dir(&request.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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

#[derive(Clone, Debug, Default)]
pub struct ArtifactBuildRequest {
    pub source: String,
    pub cwd: PathBuf,
    pub timeout: Option<Duration>,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ArtifactRenderCommandRequest {
    pub cwd: PathBuf,
    pub timeout: Option<Duration>,
    pub env: BTreeMap<String, String>,
    pub target: ArtifactRenderTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactRenderTarget {
    Presentation(PresentationRenderTarget),
    Spreadsheet(SpreadsheetRenderTarget),
}

impl ArtifactRenderTarget {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationRenderTarget {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub slide_number: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpreadsheetRenderTarget {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub sheet_name: String,
    pub range: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactCommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ArtifactCommandOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

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
    format!(
        concat!(
            "import {{ pathToFileURL }} from \"node:url\";\n",
            "const artifactTool = await import(pathToFileURL(process.env.CODEX_ARTIFACT_BUILD_ENTRYPOINT).href);\n",
            "globalThis.artifactTool = artifactTool;\n",
            "globalThis.artifacts = artifactTool;\n",
            "globalThis.codexArtifacts = artifactTool;\n",
            "for (const [name, value] of Object.entries(artifactTool)) {{\n",
            "  if (name === \"default\" || Object.prototype.hasOwnProperty.call(globalThis, name)) {{\n",
            "    continue;\n",
            "  }}\n",
            "  globalThis[name] = value;\n",
            "}}\n\n",
            "{}\n"
        ),
        source
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::ArtifactRuntimePlatform;
    #[cfg(unix)]
    use crate::ExtractedRuntimeManifest;
    #[cfg(unix)]
    use crate::RuntimeEntrypoints;
    #[cfg(unix)]
    use crate::RuntimePathEntry;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn wrapped_build_script_exposes_artifact_tool_surface() {
        let wrapped = build_wrapped_script("console.log(Object.keys(artifactTool).length);");
        assert!(wrapped.contains("const artifactTool = await import("));
        assert!(wrapped.contains("globalThis.artifactTool = artifactTool;"));
        assert!(wrapped.contains("globalThis.artifacts = artifactTool;"));
        assert!(wrapped.contains("globalThis.codexArtifacts = artifactTool;"));
        assert!(wrapped.contains("Object.entries(artifactTool)"));
        assert!(wrapped.contains("globalThis[name] = value;"));
    }

    #[test]
    fn presentation_render_target_builds_expected_args() {
        let args = ArtifactRenderTarget::Presentation(PresentationRenderTarget {
            input_path: PathBuf::from("deck.pptx"),
            output_path: PathBuf::from("slide.png"),
            slide_number: 2,
        })
        .to_args();

        assert_eq!(
            args,
            vec![
                "pptx",
                "render",
                "--in",
                "deck.pptx",
                "--slide",
                "2",
                "--out",
                "slide.png"
            ]
        );
    }

    #[test]
    fn spreadsheet_render_target_builds_expected_args() {
        let args = ArtifactRenderTarget::Spreadsheet(SpreadsheetRenderTarget {
            input_path: PathBuf::from("book.xlsx"),
            output_path: PathBuf::from("sheet.png"),
            sheet_name: "Summary".to_string(),
            range: Some("A1:C3".to_string()),
        })
        .to_args();

        assert_eq!(
            args,
            vec![
                "xlsx",
                "render",
                "--in",
                "book.xlsx",
                "--sheet",
                "Summary",
                "--out",
                "sheet.png",
                "--range",
                "A1:C3"
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn execute_build_invokes_runtime_node_with_expected_environment() {
        let temp = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
        let cwd = temp.path().join("cwd");
        fs::create_dir_all(&cwd)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        let log_path = temp.path().join("build.log");
        let fake_node = temp.path().join("fake-node.sh");
        let build_entrypoint = temp.path().join("artifact_tool.mjs");
        let render_entrypoint = temp.path().join("render_cli.mjs");
        fs::write(
            &fake_node,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1\" > \"{}\"\nprintf '%s\\n' \"$CODEX_ARTIFACT_BUILD_ENTRYPOINT\" >> \"{}\"\n",
                log_path.display(),
                log_path.display()
            ),
        )
        .await
        .unwrap_or_else(|error| panic!("{error}"));
        std::fs::set_permissions(&fake_node, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("{error}"));

        let runtime = InstalledArtifactRuntime::new(
            temp.path().join("runtime"),
            "0.1.0".to_string(),
            ArtifactRuntimePlatform::LinuxX64,
            sample_manifest("0.1.0"),
            fake_node.clone(),
            build_entrypoint.clone(),
            render_entrypoint,
        );
        let client = ArtifactsClient::from_installed_runtime(runtime);

        let output = client
            .execute_build(ArtifactBuildRequest {
                source: "console.log('hello');".to_string(),
                cwd: cwd.clone(),
                timeout: Some(Duration::from_secs(5)),
                env: BTreeMap::new(),
            })
            .await
            .unwrap_or_else(|error| panic!("{error}"));

        assert!(output.success());
        let logged = fs::read_to_string(&log_path)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(logged.contains("artifact-build.mjs"));
        assert!(logged.contains(&build_entrypoint.display().to_string()));
    }

    #[cfg(unix)]
    fn sample_manifest(runtime_version: &str) -> ExtractedRuntimeManifest {
        ExtractedRuntimeManifest {
            schema_version: 1,
            runtime_version: runtime_version.to_string(),
            node: RuntimePathEntry {
                relative_path: "node/bin/node".to_string(),
            },
            entrypoints: RuntimeEntrypoints {
                build_js: RuntimePathEntry {
                    relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
                },
                render_cli: RuntimePathEntry {
                    relative_path: "granola-render/dist/cli.mjs".to_string(),
                },
            },
        }
    }
}
