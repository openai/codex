use async_trait::async_trait;
use base64::Engine;
use codex_artifacts::ArtifactBuildRequest;
use codex_artifacts::ArtifactCommandOutput;
use codex_artifacts::ArtifactRuntimeManager;
use codex_artifacts::ArtifactRuntimeManagerConfig;
use codex_artifacts::ArtifactsClient;
use codex_artifacts::ArtifactsError;
use codex_protocol::items::MarkdownDocumentState;
use codex_protocol::items::TurnItem;
use codex_protocol::items::VisualArtifact as TurnVisualArtifact;
use codex_protocol::items::VisualArtifactItem;
use codex_protocol::items::VisualArtifactStatus;
use serde::Deserialize;
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
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

const ARTIFACTS_TOOL_NAME: &str = "artifacts";
const ARTIFACTS_PRAGMA_PREFIXES: [&str; 2] = ["// codex-artifacts:", "// codex-artifact-tool:"];
const VISUAL_ARTIFACTS_MANIFEST_PREFIX: &str = "__CODEX_VISUAL_ARTIFACTS__";
pub(crate) const PINNED_ARTIFACT_RUNTIME_VERSION: &str = "2.4.0";
const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ArtifactsHandler;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactsToolArgs {
    source: String,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct VisualArtifactsManifest {
    visuals: Vec<VisualArtifactManifestEntry>,
    document: Option<MarkdownDocumentManifest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisualArtifactManifestEntry {
    title: String,
    html: String,
    summary: Option<String>,
    height_px: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkdownDocumentManifest {
    title: String,
    markdown: String,
}

#[derive(Debug)]
struct ParsedArtifactOutput {
    output: ArtifactCommandOutput,
    visuals: Vec<VisualArtifactManifestEntry>,
    document: Option<MarkdownDocumentManifest>,
}

#[async_trait]
impl ToolHandler for ArtifactsHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
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

        let parsed = parse_artifact_output(output);
        let visual_count = parsed.visuals.len();
        let has_document = parsed.document.is_some();

        emit_exec_end(
            session.as_ref(),
            turn.as_ref(),
            &call_id,
            &parsed.output,
            started_at.elapsed(),
            success,
            visual_count,
            has_document,
        )
        .await;

        if success && (!parsed.visuals.is_empty() || parsed.document.is_some()) {
            emit_visual_artifact_item(
                session.as_ref(),
                turn.as_ref(),
                &call_id,
                parsed.visuals,
                parsed.document,
            )
            .await;
        }

        Ok(FunctionToolOutput::from_text(
            format_artifact_output(&parsed.output, visual_count, has_document),
            Some(success),
        ))
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
        /*freeform*/ true,
    );
    let ctx = ToolEventCtx::new(session, turn, call_id, /*turn_diff_tracker*/ None);
    emitter.emit(ctx, ToolEventStage::Begin).await;
}

async fn emit_exec_end(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    output: &ArtifactCommandOutput,
    duration: Duration,
    success: bool,
    visual_count: usize,
    has_document: bool,
) {
    let exec_output = ExecToolCallOutput {
        exit_code: output.exit_code.unwrap_or(1),
        stdout: StreamOutput::new(output.stdout.clone()),
        stderr: StreamOutput::new(output.stderr.clone()),
        aggregated_output: StreamOutput::new(format_artifact_output(
            output,
            visual_count,
            has_document,
        )),
        duration,
        timed_out: false,
    };
    let emitter = ToolEmitter::shell(
        vec![ARTIFACTS_TOOL_NAME.to_string()],
        turn.cwd.clone(),
        ExecCommandSource::Agent,
        /*freeform*/ true,
    );
    let ctx = ToolEventCtx::new(session, turn, call_id, /*turn_diff_tracker*/ None);
    let stage = if success {
        ToolEventStage::Success(exec_output)
    } else {
        ToolEventStage::Failure(ToolEventFailure::Output(exec_output))
    };
    emitter.emit(ctx, stage).await;
}

async fn emit_visual_artifact_item(
    session: &Session,
    turn: &TurnContext,
    call_id: &str,
    visuals: Vec<VisualArtifactManifestEntry>,
    document: Option<MarkdownDocumentManifest>,
) {
    let started = TurnItem::VisualArtifact(VisualArtifactItem::in_progress(call_id.to_string()));
    session.emit_turn_item_started(turn, &started).await;

    let completed = TurnItem::VisualArtifact(VisualArtifactItem {
        id: call_id.to_string(),
        status: VisualArtifactStatus::Completed,
        visuals: visuals
            .into_iter()
            .map(|visual| TurnVisualArtifact {
                title: visual.title,
                html: visual.html,
                summary: visual.summary,
                height_px: visual.height_px,
            })
            .collect(),
        document: document.map(|document| MarkdownDocumentState {
            title: document.title,
            markdown: document.markdown,
        }),
        error: None,
    });
    session.emit_turn_item_completed(turn, completed).await;
}

fn parse_artifact_output(mut output: ArtifactCommandOutput) -> ParsedArtifactOutput {
    let mut visuals = Vec::new();
    let mut document = None;
    let mut cleaned_lines = Vec::new();
    let mut manifest_error = None;

    for line in output.stdout.lines() {
        let Some(encoded_manifest) = line.strip_prefix(VISUAL_ARTIFACTS_MANIFEST_PREFIX) else {
            cleaned_lines.push(line);
            continue;
        };

        match decode_visual_manifest(encoded_manifest) {
            Ok(mut manifest) => {
                visuals.append(&mut manifest.visuals);
                document = manifest.document;
            }
            Err(err) => manifest_error = Some(err),
        }
    }

    output.stdout = cleaned_lines.join("\n");
    if let Some(err) = manifest_error {
        if !output.stderr.trim().is_empty() {
            output.stderr.push('\n');
        }
        output
            .stderr
            .push_str(&format!("visual artifact manifest error: {err}"));
    }

    ParsedArtifactOutput {
        output,
        visuals,
        document,
    }
}

fn decode_visual_manifest(encoded_manifest: &str) -> Result<VisualArtifactsManifest, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_manifest)
        .map_err(|err| format!("invalid base64 payload: {err}"))?;
    let mut manifest = serde_json::from_slice::<VisualArtifactsManifest>(&bytes)
        .map_err(|err| format!("invalid manifest JSON: {err}"))?;
    manifest
        .visuals
        .retain(|visual| !visual.html.trim().is_empty());
    if let Some(document_manifest) = &manifest.document
        && document_manifest.markdown.trim().is_empty()
    {
        manifest.document = None;
    }
    Ok(manifest)
}

fn format_artifact_output(
    output: &ArtifactCommandOutput,
    visual_count: usize,
    has_document: bool,
) -> String {
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
    if output.success() && visual_count > 0 {
        sections.push(format!(
            "generated {visual_count} interactive visual artifact{}.",
            if visual_count == 1 { "" } else { "s" }
        ));
    }
    if output.success() && has_document {
        sections.push("generated a live markdown workspace.".to_string());
    }
    if stdout.is_empty()
        && stderr.is_empty()
        && output.success()
        && visual_count == 0
        && !has_document
    {
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
#[path = "artifacts_tests.rs"]
mod tests;
