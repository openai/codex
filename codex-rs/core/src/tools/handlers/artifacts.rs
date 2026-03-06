use async_trait::async_trait;
use codex_artifacts::ArtifactBuildRequest;
use codex_artifacts::ArtifactCommandOutput;
use codex_artifacts::ArtifactRuntimeManager;
use codex_artifacts::ArtifactRuntimeManagerConfig;
use codex_artifacts::ArtifactsClient;
use codex_artifacts::ArtifactsError;
use serde_json::Value as JsonValue;
use std::time::Duration;
use std::time::Instant;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecToolCallOutput;
use crate::exec::StreamOutput;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::protocol::ExecCommandSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputBody;

const ARTIFACTS_TOOL_NAME: &str = "artifacts";
const ARTIFACTS_PRAGMA_PREFIXES: [&str; 2] = ["// codex-artifacts:", "// codex-artifact-tool:"];
pub(crate) const PINNED_ARTIFACT_RUNTIME_VERSION: &str = "2.4.0";
const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ArtifactsHandler;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactsToolArgs {
    source: String,
    timeout_ms: Option<u64>,
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

        let client = ArtifactsClient::from_runtime_manager(default_runtime_manager(
            turn.config.codex_home.clone(),
        ));

        let started_at = Instant::now();
        emit_exec_begin(session.as_ref(), turn.as_ref(), &call_id).await;

        let result = client
            .execute_build(ArtifactBuildRequest {
                source: args.source,
                cwd: turn.cwd.clone(),
                timeout: Some(Duration::from_millis(
                    args.timeout_ms
                        .unwrap_or(DEFAULT_EXECUTION_TIMEOUT.as_millis() as u64),
                )),
                env: Default::default(),
            })
            .await;

        let (success, output) = match result {
            Ok(output) => (output.success(), output),
            Err(error) => (false, error_output(&error)),
        };

        emit_exec_end(
            session.as_ref(),
            turn.as_ref(),
            &call_id,
            &output,
            started_at.elapsed(),
            success,
        )
        .await;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(format_artifact_output(&output)),
            success: Some(success),
        })
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

async fn emit_exec_begin(session: &Session, turn: &TurnContext, call_id: &str) {
    let emitter = ToolEmitter::shell(
        vec![ARTIFACTS_TOOL_NAME.to_string()],
        turn.cwd.clone(),
        ExecCommandSource::Agent,
        true,
    );
    let ctx = ToolEventCtx::new(session, turn, call_id, None);
    emitter.emit(ctx, ToolEventStage::Begin).await;
}

async fn emit_exec_end(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    output: &ArtifactCommandOutput,
    duration: Duration,
    success: bool,
) {
    let exec_output = ExecToolCallOutput {
        exit_code: output.exit_code.unwrap_or(1),
        stdout: StreamOutput::new(output.stdout.clone()),
        stderr: StreamOutput::new(output.stderr.clone()),
        aggregated_output: StreamOutput::new(format_artifact_output(output)),
        duration,
        timed_out: false,
    };
    let emitter = ToolEmitter::shell(
        vec![ARTIFACTS_TOOL_NAME.to_string()],
        turn.cwd.clone(),
        ExecCommandSource::Agent,
        true,
    );
    let ctx = ToolEventCtx::new(session, turn, call_id, None);
    let stage = if success {
        ToolEventStage::Success(exec_output)
    } else {
        ToolEventStage::Failure(ToolEventFailure::Output(exec_output))
    };
    emitter.emit(ctx, stage).await;
}

fn format_artifact_output(output: &ArtifactCommandOutput) -> String {
    let stdout = output.stdout.trim();
    let stderr = output.stderr.trim();
    let mut sections = vec![format!(
        "exit_code: {}",
        output
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "null".to_string())
    )];
    if !stdout.is_empty() {
        sections.push(format!("stdout:\n{stdout}"));
    }
    if !stderr.is_empty() {
        sections.push(format!("stderr:\n{stderr}"));
    }
    if stdout.is_empty() && stderr.is_empty() && output.success() {
        sections.push("artifact JS completed successfully.".to_string());
    }
    sections.join("\n\n")
}

fn error_output(error: &ArtifactsError) -> ArtifactCommandOutput {
    ArtifactCommandOutput {
        exit_code: Some(1),
        stdout: String::new(),
        stderr: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_artifacts::RuntimeEntrypoints;
    use codex_artifacts::RuntimePathEntry;
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
        let formatted = format_artifact_output(&ArtifactCommandOutput {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });
        assert!(formatted.contains("artifact JS completed successfully."));
    }
}
