use async_trait::async_trait;
use codex_artifacts::ArtifactRuntimeManager;
use codex_artifacts::ArtifactRuntimeManagerConfig;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;

use crate::exec::ExecToolCallOutput;
use crate::exec_policy::ExecApprovalRequest;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::protocol::ExecCommandSource;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::artifacts::ArtifactApprovalKey;
use crate::tools::runtimes::artifacts::ArtifactExecRequest;
use crate::tools::runtimes::artifacts::ArtifactRuntime;
use crate::tools::sandboxing::ToolError;
use codex_protocol::models::FunctionCallOutputBody;

const ARTIFACTS_TOOL_NAME: &str = "artifacts";
const ARTIFACTS_PRAGMA_PREFIXES: [&str; 2] = ["// codex-artifacts:", "// codex-artifact-tool:"];
pub(crate) const PINNED_ARTIFACT_RUNTIME_VERSION: &str = "2.4.0";
const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
const ARTIFACT_BUILD_LAUNCHER_RELATIVE: &str = "runtime-scripts/artifacts/build-launcher.mjs";
const ARTIFACT_BUILD_LAUNCHER_SOURCE: &str = concat!(
    "import { pathToFileURL } from \"node:url\";\n",
    "const [sourcePath] = process.argv.slice(2);\n",
    "if (!sourcePath) {\n",
    "  throw new Error(\"missing artifact source path\");\n",
    "}\n",
    "const artifactTool = await import(pathToFileURL(process.env.CODEX_ARTIFACT_BUILD_ENTRYPOINT).href);\n",
    "globalThis.artifactTool = artifactTool;\n",
    "globalThis.artifacts = artifactTool;\n",
    "globalThis.codexArtifacts = artifactTool;\n",
    "for (const [name, value] of Object.entries(artifactTool)) {\n",
    "  if (name === \"default\" || Object.prototype.hasOwnProperty.call(globalThis, name)) {\n",
    "    continue;\n",
    "  }\n",
    "  globalThis[name] = value;\n",
    "}\n",
    "await import(pathToFileURL(sourcePath).href);\n",
);

pub struct ArtifactsHandler;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactsToolArgs {
    source: String,
    timeout_ms: Option<u64>,
}

struct PreparedArtifactBuild {
    request: ArtifactExecRequest,
    _source_dir: TempDir,
}

#[async_trait]
impl ToolHandler for ArtifactsHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;

        if !session.enabled(Feature::Artifact) {
            return Err(FunctionCallError::RespondToModel(
                "artifacts is disabled by feature flag".to_string(),
            ));
        }

        let args = match payload {
            ToolPayload::Custom { input } => parse_freeform_args(&input)?,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "artifacts expects freeform JavaScript input authored against the preloaded @oai/artifact-tool surface".to_string(),
                ));
            }
        };

        let runtime = default_runtime_manager(turn.config.codex_home.clone())
            .ensure_installed()
            .await;
        let runtime = match runtime {
            Ok(runtime) => runtime,
            Err(error) => {
                return Ok(ToolOutput::Function {
                    body: FunctionCallOutputBody::Text(error.to_string()),
                    success: Some(false),
                });
            }
        };

        let prepared = prepare_artifact_build(
            session.as_ref(),
            turn.as_ref(),
            runtime,
            args.source,
            args.timeout_ms
                .unwrap_or(DEFAULT_EXECUTION_TIMEOUT.as_millis() as u64),
        )
        .await?;

        let emitter = ToolEmitter::shell(
            artifact_display_command(),
            prepared.request.cwd.clone(),
            ExecCommandSource::Agent,
            true,
        );
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, None);
        emitter.begin(event_ctx).await;

        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = ArtifactRuntime;
        let tool_ctx = crate::tools::sandboxing::ToolCtx {
            session: session.clone(),
            turn: turn.clone(),
            call_id: call_id.clone(),
            tool_name: ARTIFACTS_TOOL_NAME.to_string(),
        };
        let result = orchestrator
            .run(
                &mut runtime,
                &prepared.request,
                &tool_ctx,
                &turn,
                turn.approval_policy.value(),
            )
            .await
            .map(|result| result.output);

        Ok(finish_artifact_execution(&emitter, event_ctx, result).await)
    }
}

fn parse_freeform_args(input: &str) -> Result<ArtifactsToolArgs, FunctionCallError> {
    if input.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "artifacts expects raw JavaScript source text (non-empty) authored against the preloaded @oai/artifact-tool surface. Provide JS only, optionally with first-line `// codex-artifacts: timeout_ms=15000` or `// codex-artifact-tool: timeout_ms=15000`."
                .to_string(),
        ));
    }

    let mut args = ArtifactsToolArgs {
        source: input.to_string(),
        timeout_ms: None,
    };

    let mut lines = input.splitn(2, '\n');
    let first_line = lines.next().unwrap_or_default();
    let rest = lines.next().unwrap_or_default();
    let trimmed = first_line.trim_start();
    let Some(pragma) = parse_pragma_prefix(trimmed) else {
        reject_json_or_quoted_source(&args.source)?;
        return Ok(args);
    };

    let mut timeout_ms = None;
    let directive = pragma.trim();
    if !directive.is_empty() {
        for token in directive.split_whitespace() {
            let (key, value) = token.split_once('=').ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "artifacts pragma expects space-separated key=value pairs (supported keys: timeout_ms); got `{token}`"
                ))
            })?;
            match key {
                "timeout_ms" => {
                    if timeout_ms.is_some() {
                        return Err(FunctionCallError::RespondToModel(
                            "artifacts pragma specifies timeout_ms more than once".to_string(),
                        ));
                    }
                    let parsed = value.parse::<u64>().map_err(|_| {
                        FunctionCallError::RespondToModel(format!(
                            "artifacts pragma timeout_ms must be an integer; got `{value}`"
                        ))
                    })?;
                    timeout_ms = Some(parsed);
                }
                _ => {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "artifacts pragma only supports timeout_ms; got `{key}`"
                    )));
                }
            }
        }
    }

    if rest.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "artifacts pragma must be followed by JavaScript source on subsequent lines"
                .to_string(),
        ));
    }

    reject_json_or_quoted_source(rest)?;
    args.source = rest.to_string();
    args.timeout_ms = timeout_ms;
    Ok(args)
}

fn reject_json_or_quoted_source(code: &str) -> Result<(), FunctionCallError> {
    let trimmed = code.trim();
    if trimmed.starts_with("```") {
        return Err(FunctionCallError::RespondToModel(
            "artifacts expects raw JavaScript source, not markdown code fences. Resend plain JS only (optional first line `// codex-artifacts: ...` or `// codex-artifact-tool: ...`)."
                .to_string(),
        ));
    }
    let Ok(value) = serde_json::from_str::<JsonValue>(trimmed) else {
        return Ok(());
    };
    match value {
        JsonValue::Object(_) | JsonValue::String(_) => Err(FunctionCallError::RespondToModel(
            "artifacts is a freeform tool and expects raw JavaScript source authored against the preloaded @oai/artifact-tool surface. Resend plain JS only (optional first line `// codex-artifacts: ...` or `// codex-artifact-tool: ...`); do not send JSON (`{\"code\":...}`), quoted code, or markdown fences."
                .to_string(),
        )),
        _ => Ok(()),
    }
}

fn parse_pragma_prefix(line: &str) -> Option<&str> {
    ARTIFACTS_PRAGMA_PREFIXES
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix))
}

fn default_runtime_manager(codex_home: std::path::PathBuf) -> ArtifactRuntimeManager {
    ArtifactRuntimeManager::new(ArtifactRuntimeManagerConfig::with_default_release(
        codex_home,
        PINNED_ARTIFACT_RUNTIME_VERSION,
    ))
}

async fn prepare_artifact_build(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    installed_runtime: codex_artifacts::InstalledArtifactRuntime,
    source: String,
    timeout_ms: u64,
) -> Result<PreparedArtifactBuild, FunctionCallError> {
    let launcher_path = ensure_artifact_build_launcher(turn.config.codex_home.as_path()).await?;
    let source_dir = TempDir::new().map_err(|error| {
        FunctionCallError::RespondToModel(format!(
            "failed to create artifact source staging directory: {error}"
        ))
    })?;
    let source_path = source_dir.path().join("artifact-source.mjs");
    fs::write(&source_path, source).await.map_err(|error| {
        FunctionCallError::RespondToModel(format!(
            "failed to write artifact source at `{}`: {error}",
            source_path.display()
        ))
    })?;

    let js_runtime = installed_runtime
        .resolve_js_runtime()
        .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
    let command =
        build_artifact_build_command(js_runtime.executable_path(), &launcher_path, &source_path);
    let approval_key = ArtifactApprovalKey {
        command_prefix: artifact_prefix_rule(&command),
        cwd: turn.cwd.clone(),
        staged_script: source_path.clone(),
    };
    let initial_approval_requirement = session
        .services
        .exec_policy
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: turn.approval_policy.value(),
            sandbox_policy: turn.sandbox_policy.get(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        })
        .await;
    let escalation_approval_requirement = session
        .services
        .exec_policy
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: turn.approval_policy.value(),
            sandbox_policy: turn.sandbox_policy.get(),
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            prefix_rule: None,
        })
        .await;
    let initial_approval_requirement =
        clear_proposed_execpolicy_amendment(initial_approval_requirement);
    let escalation_approval_requirement =
        clear_proposed_execpolicy_amendment(escalation_approval_requirement);

    let env = build_artifact_env(
        &installed_runtime,
        js_runtime.executable_path(),
        js_runtime.requires_electron_run_as_node(),
        codex_artifacts::system_node_path().as_deref(),
    );

    Ok(PreparedArtifactBuild {
        request: ArtifactExecRequest {
            command,
            cwd: turn.cwd.clone(),
            timeout_ms: Some(timeout_ms),
            env,
            approval_key,
            initial_approval_requirement,
            escalation_approval_requirement,
        },
        _source_dir: source_dir,
    })
}

fn clear_proposed_execpolicy_amendment(
    requirement: crate::tools::sandboxing::ExecApprovalRequirement,
) -> crate::tools::sandboxing::ExecApprovalRequirement {
    match requirement {
        crate::tools::sandboxing::ExecApprovalRequirement::Skip { bypass_sandbox, .. } => {
            crate::tools::sandboxing::ExecApprovalRequirement::Skip {
                bypass_sandbox,
                proposed_execpolicy_amendment: None,
            }
        }
        crate::tools::sandboxing::ExecApprovalRequirement::NeedsApproval { reason, .. } => {
            crate::tools::sandboxing::ExecApprovalRequirement::NeedsApproval {
                reason,
                proposed_execpolicy_amendment: None,
            }
        }
        crate::tools::sandboxing::ExecApprovalRequirement::Forbidden { reason } => {
            crate::tools::sandboxing::ExecApprovalRequirement::Forbidden { reason }
        }
    }
}

async fn ensure_artifact_build_launcher(codex_home: &Path) -> Result<PathBuf, FunctionCallError> {
    let launcher_path = artifact_build_launcher_path(codex_home);
    match fs::read_to_string(&launcher_path).await {
        Ok(existing) if existing == ARTIFACT_BUILD_LAUNCHER_SOURCE => return Ok(launcher_path),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(FunctionCallError::RespondToModel(format!(
                "failed to read artifact launcher `{}`: {error}",
                launcher_path.display()
            )));
        }
    }

    if let Some(parent) = launcher_path.parent() {
        fs::create_dir_all(parent).await.map_err(|error| {
            FunctionCallError::RespondToModel(format!(
                "failed to create artifact launcher directory `{}`: {error}",
                parent.display()
            ))
        })?;
    }
    fs::write(&launcher_path, ARTIFACT_BUILD_LAUNCHER_SOURCE)
        .await
        .map_err(|error| {
            FunctionCallError::RespondToModel(format!(
                "failed to write artifact launcher `{}`: {error}",
                launcher_path.display()
            ))
        })?;

    Ok(launcher_path)
}

fn artifact_build_launcher_path(codex_home: &Path) -> PathBuf {
    codex_home.join(ARTIFACT_BUILD_LAUNCHER_RELATIVE)
}

fn build_artifact_build_command(
    executable_path: &Path,
    launcher_path: &Path,
    source_path: &Path,
) -> Vec<String> {
    vec![
        executable_path.display().to_string(),
        launcher_path.display().to_string(),
        source_path.display().to_string(),
    ]
}

fn artifact_prefix_rule(command: &[String]) -> Vec<String> {
    command.iter().take(2).cloned().collect()
}

fn artifact_display_command() -> Vec<String> {
    vec![ARTIFACTS_TOOL_NAME.to_string()]
}

fn build_artifact_env(
    installed_runtime: &codex_artifacts::InstalledArtifactRuntime,
    selected_runtime_path: &Path,
    requires_electron_run_as_node: bool,
    host_node_path: Option<&Path>,
) -> HashMap<String, String> {
    let mut env = HashMap::from([
        (
            "CODEX_ARTIFACT_BUILD_ENTRYPOINT".to_string(),
            installed_runtime.build_js_path().display().to_string(),
        ),
        (
            "CODEX_ARTIFACT_RENDER_ENTRYPOINT".to_string(),
            installed_runtime.render_cli_path().display().to_string(),
        ),
    ]);
    if requires_electron_run_as_node {
        env.insert("ELECTRON_RUN_AS_NODE".to_string(), "1".to_string());
    }
    if selected_runtime_path == installed_runtime.node_path()
        && let Some(host_node_path) = host_node_path
    {
        env.insert(
            "CODEX_ARTIFACT_NODE_PATH".to_string(),
            host_node_path.display().to_string(),
        );
    }
    env
}

async fn finish_artifact_execution(
    emitter: &ToolEmitter,
    event_ctx: ToolEventCtx<'_>,
    result: Result<ExecToolCallOutput, ToolError>,
) -> ToolOutput {
    let (body, success, stage) = match result {
        Ok(output) => {
            let success = output.exit_code == 0;
            let body = format_artifact_output(&output);
            let stage = if success {
                ToolEventStage::Success(output)
            } else {
                ToolEventStage::Failure(ToolEventFailure::Output(output))
            };
            (body, success, stage)
        }
        Err(ToolError::Codex(crate::error::CodexErr::Sandbox(
            crate::error::SandboxErr::Timeout { output },
        )))
        | Err(ToolError::Codex(crate::error::CodexErr::Sandbox(
            crate::error::SandboxErr::Denied { output, .. },
        ))) => {
            let output = *output;
            let body = format_artifact_output(&output);
            (
                body,
                false,
                ToolEventStage::Failure(ToolEventFailure::Output(output)),
            )
        }
        Err(ToolError::Codex(error)) => {
            let message = format!("execution error: {error:?}");
            (
                message.clone(),
                false,
                ToolEventStage::Failure(ToolEventFailure::Message(message)),
            )
        }
        Err(ToolError::Rejected(message)) => {
            let normalized = if message == "rejected by user" {
                "artifact command rejected by user".to_string()
            } else {
                message
            };
            (
                normalized.clone(),
                false,
                ToolEventStage::Failure(ToolEventFailure::Rejected(normalized)),
            )
        }
    };
    emitter.emit(event_ctx, stage).await;

    ToolOutput::Function {
        body: FunctionCallOutputBody::Text(body),
        success: Some(success),
    }
}

fn format_artifact_output(output: &ExecToolCallOutput) -> String {
    let stdout = output.stdout.text.trim();
    let stderr = format_artifact_stderr(output);
    let mut sections = vec![format!("exit_code: {}", output.exit_code)];
    if !stdout.is_empty() {
        sections.push(format!("stdout:\n{stdout}"));
    }
    if !stderr.is_empty() {
        sections.push(format!("stderr:\n{stderr}"));
    }
    if stdout.is_empty() && stderr.is_empty() && output.exit_code == 0 {
        sections.push("artifact JS completed successfully.".to_string());
    }
    sections.join("\n\n")
}

fn format_artifact_stderr(output: &ExecToolCallOutput) -> String {
    let stderr = output.stderr.text.trim();
    if output.timed_out {
        let timeout_message = format!(
            "command timed out after {} milliseconds",
            output.duration.as_millis()
        );
        if stderr.is_empty() {
            timeout_message
        } else {
            format!("{timeout_message}\n{stderr}")
        }
    } else {
        stderr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::config::Constrained;
    use crate::exec::StreamOutput;
    use crate::protocol::AskForApproval;
    use crate::protocol::SandboxPolicy;
    use codex_artifacts::RuntimeEntrypoints;
    use codex_artifacts::RuntimePathEntry;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn parse_freeform_args_without_pragma() {
        let args = parse_freeform_args("console.log('ok');").expect("parse args");
        assert_eq!(args.source, "console.log('ok');");
        assert_eq!(args.timeout_ms, None);
    }

    #[test]
    fn parse_freeform_args_with_pragma() {
        let args = parse_freeform_args("// codex-artifacts: timeout_ms=45000\nconsole.log('ok');")
            .expect("parse args");
        assert_eq!(args.source, "console.log('ok');");
        assert_eq!(args.timeout_ms, Some(45_000));
    }

    #[test]
    fn parse_freeform_args_with_artifact_tool_pragma() {
        let args =
            parse_freeform_args("// codex-artifact-tool: timeout_ms=45000\nconsole.log('ok');")
                .expect("parse args");
        assert_eq!(args.source, "console.log('ok');");
        assert_eq!(args.timeout_ms, Some(45_000));
    }

    #[test]
    fn parse_freeform_args_rejects_json_wrapped_code() {
        let err =
            parse_freeform_args("{\"code\":\"console.log('ok')\"}").expect_err("expected error");
        assert!(
            err.to_string()
                .contains("artifacts is a freeform tool and expects raw JavaScript source")
        );
    }

    #[test]
    fn default_runtime_manager_uses_openai_codex_release_base() {
        let codex_home = TempDir::new().expect("create temp codex home");
        let manager = default_runtime_manager(codex_home.path().to_path_buf());

        assert_eq!(
            manager.config().release().base_url().as_str(),
            "https://github.com/openai/codex/releases/download/"
        );
        assert_eq!(
            manager.config().release().runtime_version(),
            PINNED_ARTIFACT_RUNTIME_VERSION
        );
    }

    #[test]
    fn load_cached_runtime_reads_pinned_cache_path() {
        let codex_home = TempDir::new().expect("create temp codex home");
        let platform =
            codex_artifacts::ArtifactRuntimePlatform::detect_current().expect("detect platform");
        let install_dir = codex_home
            .path()
            .join("packages")
            .join("artifacts")
            .join(PINNED_ARTIFACT_RUNTIME_VERSION)
            .join(platform.as_str());
        std::fs::create_dir_all(&install_dir).expect("create install dir");
        std::fs::write(
            install_dir.join("manifest.json"),
            serde_json::json!({
                "schema_version": 1,
                "runtime_version": PINNED_ARTIFACT_RUNTIME_VERSION,
                "node": { "relative_path": "node/bin/node" },
                "entrypoints": {
                    "build_js": { "relative_path": "artifact-tool/dist/artifact_tool.mjs" },
                    "render_cli": { "relative_path": "granola-render/dist/render_cli.mjs" }
                }
            })
            .to_string(),
        )
        .expect("write manifest");
        std::fs::create_dir_all(install_dir.join("artifact-tool/dist"))
            .expect("create build entrypoint dir");
        std::fs::create_dir_all(install_dir.join("granola-render/dist"))
            .expect("create render entrypoint dir");
        std::fs::write(
            install_dir.join("artifact-tool/dist/artifact_tool.mjs"),
            "export const ok = true;\n",
        )
        .expect("write build entrypoint");
        std::fs::write(
            install_dir.join("granola-render/dist/render_cli.mjs"),
            "export const ok = true;\n",
        )
        .expect("write render entrypoint");

        let runtime = codex_artifacts::load_cached_runtime(
            &codex_home
                .path()
                .join(codex_artifacts::DEFAULT_CACHE_ROOT_RELATIVE),
            PINNED_ARTIFACT_RUNTIME_VERSION,
        )
        .expect("resolve runtime");
        assert_eq!(runtime.runtime_version(), PINNED_ARTIFACT_RUNTIME_VERSION);
        assert_eq!(
            runtime.manifest().entrypoints,
            RuntimeEntrypoints {
                build_js: RuntimePathEntry {
                    relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
                },
                render_cli: RuntimePathEntry {
                    relative_path: "granola-render/dist/render_cli.mjs".to_string(),
                },
            }
        );
    }

    #[test]
    fn format_artifact_output_includes_success_message_when_silent() {
        let formatted = format_artifact_output(&ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::ZERO,
            timed_out: false,
        });
        assert!(formatted.contains("artifact JS completed successfully."));
    }

    #[test]
    fn format_artifact_output_includes_timeout_message() {
        let formatted = format_artifact_output(&ExecToolCallOutput {
            exit_code: 124,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new("render hung".to_string()),
            aggregated_output: StreamOutput::new("render hung".to_string()),
            duration: Duration::from_millis(1_500),
            timed_out: true,
        });

        assert!(formatted.contains("command timed out after 1500 milliseconds"));
        assert!(formatted.contains("render hung"));
    }

    #[test]
    fn artifact_prefix_rule_uses_stable_launcher_prefix() {
        let command = build_artifact_build_command(
            Path::new("/runtime/node"),
            Path::new("/codex/home/runtime-scripts/artifacts/build-launcher.mjs"),
            Path::new("/tmp/artifact-source.mjs"),
        );

        assert_eq!(
            artifact_prefix_rule(&command),
            vec![
                "/runtime/node".to_string(),
                "/codex/home/runtime-scripts/artifacts/build-launcher.mjs".to_string(),
            ]
        );
    }

    #[test]
    fn artifact_display_command_is_user_facing() {
        assert_eq!(artifact_display_command(), vec!["artifacts".to_string()]);
    }

    #[test]
    fn build_artifact_env_includes_host_node_override_for_bundled_wrapper() {
        let runtime = codex_artifacts::InstalledArtifactRuntime::new(
            PathBuf::from("/runtime"),
            PINNED_ARTIFACT_RUNTIME_VERSION.to_string(),
            codex_artifacts::ArtifactRuntimePlatform::detect_current().expect("detect platform"),
            codex_artifacts::ExtractedRuntimeManifest {
                schema_version: 1,
                runtime_version: PINNED_ARTIFACT_RUNTIME_VERSION.to_string(),
                node: RuntimePathEntry {
                    relative_path: "node/bin/node".to_string(),
                },
                entrypoints: RuntimeEntrypoints {
                    build_js: RuntimePathEntry {
                        relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
                    },
                    render_cli: RuntimePathEntry {
                        relative_path: "granola-render/dist/render_cli.mjs".to_string(),
                    },
                },
            },
            PathBuf::from("/runtime/node/bin/node"),
            PathBuf::from("/runtime/artifact-tool/dist/artifact_tool.mjs"),
            PathBuf::from("/runtime/granola-render/dist/render_cli.mjs"),
        );

        let env = build_artifact_env(
            &runtime,
            Path::new("/runtime/node/bin/node"),
            false,
            Some(Path::new("/opt/homebrew/bin/node")),
        );

        assert_eq!(
            env.get("CODEX_ARTIFACT_NODE_PATH"),
            Some(&"/opt/homebrew/bin/node".to_string())
        );
    }

    #[test]
    fn build_artifact_env_skips_host_node_override_for_machine_runtime() {
        let runtime = codex_artifacts::InstalledArtifactRuntime::new(
            PathBuf::from("/runtime"),
            PINNED_ARTIFACT_RUNTIME_VERSION.to_string(),
            codex_artifacts::ArtifactRuntimePlatform::detect_current().expect("detect platform"),
            codex_artifacts::ExtractedRuntimeManifest {
                schema_version: 1,
                runtime_version: PINNED_ARTIFACT_RUNTIME_VERSION.to_string(),
                node: RuntimePathEntry {
                    relative_path: "node/bin/node".to_string(),
                },
                entrypoints: RuntimeEntrypoints {
                    build_js: RuntimePathEntry {
                        relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
                    },
                    render_cli: RuntimePathEntry {
                        relative_path: "granola-render/dist/render_cli.mjs".to_string(),
                    },
                },
            },
            PathBuf::from("/runtime/node/bin/node"),
            PathBuf::from("/runtime/artifact-tool/dist/artifact_tool.mjs"),
            PathBuf::from("/runtime/granola-render/dist/render_cli.mjs"),
        );

        let env = build_artifact_env(
            &runtime,
            Path::new("/opt/homebrew/bin/node"),
            false,
            Some(Path::new("/opt/homebrew/bin/node")),
        );

        assert!(!env.contains_key("CODEX_ARTIFACT_NODE_PATH"));
    }

    #[tokio::test]
    async fn ensure_artifact_build_launcher_writes_expected_source() {
        let codex_home = TempDir::new().expect("create temp codex home");

        let launcher_path = ensure_artifact_build_launcher(codex_home.path())
            .await
            .expect("write launcher");

        assert_eq!(
            launcher_path,
            codex_home.path().join(ARTIFACT_BUILD_LAUNCHER_RELATIVE)
        );
        let launcher_source =
            std::fs::read_to_string(&launcher_path).expect("read artifact launcher source");
        assert!(launcher_source.contains("globalThis.artifacts = artifactTool;"));
        assert!(launcher_source.contains("await import(pathToFileURL(sourcePath).href);"));
    }

    #[tokio::test]
    async fn prepare_artifact_build_uses_script_specific_approval_key_without_execpolicy_rule() {
        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        turn.sandbox_policy = Constrained::allow_any(SandboxPolicy::new_read_only_policy());
        let runtime = codex_artifacts::InstalledArtifactRuntime::new(
            PathBuf::from("/runtime"),
            PINNED_ARTIFACT_RUNTIME_VERSION.to_string(),
            codex_artifacts::ArtifactRuntimePlatform::detect_current().expect("detect platform"),
            codex_artifacts::ExtractedRuntimeManifest {
                schema_version: 1,
                runtime_version: PINNED_ARTIFACT_RUNTIME_VERSION.to_string(),
                node: RuntimePathEntry {
                    relative_path: "node/bin/node".to_string(),
                },
                entrypoints: RuntimeEntrypoints {
                    build_js: RuntimePathEntry {
                        relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
                    },
                    render_cli: RuntimePathEntry {
                        relative_path: "granola-render/dist/render_cli.mjs".to_string(),
                    },
                },
            },
            PathBuf::from("/runtime/node/bin/node"),
            PathBuf::from("/runtime/artifact-tool/dist/artifact_tool.mjs"),
            PathBuf::from("/runtime/granola-render/dist/render_cli.mjs"),
        );

        let prepared = prepare_artifact_build(
            &session,
            &turn,
            runtime,
            "console.log('ok');".to_string(),
            5_000,
        )
        .await
        .expect("prepare artifact build");

        assert_eq!(
            prepared.request.approval_key.command_prefix,
            vec![
                prepared.request.command[0].clone(),
                turn.config
                    .codex_home
                    .join(ARTIFACT_BUILD_LAUNCHER_RELATIVE)
                    .display()
                    .to_string(),
            ]
        );
        assert_eq!(
            prepared.request.approval_key.staged_script,
            PathBuf::from(&prepared.request.command[2])
        );
        assert!(
            prepared
                .request
                .escalation_approval_requirement
                .proposed_execpolicy_amendment()
                .is_none()
        );
        match prepared.request.initial_approval_requirement {
            crate::tools::sandboxing::ExecApprovalRequirement::NeedsApproval {
                reason,
                proposed_execpolicy_amendment,
            } => {
                assert_eq!(reason, None);
                assert_eq!(proposed_execpolicy_amendment, None);
            }
            crate::tools::sandboxing::ExecApprovalRequirement::Skip {
                bypass_sandbox,
                proposed_execpolicy_amendment,
            } => {
                assert!(!bypass_sandbox);
                assert_eq!(proposed_execpolicy_amendment, None);
            }
            crate::tools::sandboxing::ExecApprovalRequirement::Forbidden { reason } => {
                panic!("unexpected forbidden initial requirement: {reason}");
            }
        }
        match prepared.request.escalation_approval_requirement {
            crate::tools::sandboxing::ExecApprovalRequirement::NeedsApproval {
                reason,
                proposed_execpolicy_amendment,
            } => {
                assert_eq!(reason, None);
                assert_eq!(proposed_execpolicy_amendment, None);
            }
            crate::tools::sandboxing::ExecApprovalRequirement::Skip {
                bypass_sandbox,
                proposed_execpolicy_amendment,
            } => {
                assert!(!bypass_sandbox);
                assert_eq!(proposed_execpolicy_amendment, None);
            }
            crate::tools::sandboxing::ExecApprovalRequirement::Forbidden { reason } => {
                panic!("unexpected forbidden escalation requirement: {reason}");
            }
        }
    }
}
