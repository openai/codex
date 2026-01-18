use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use codex_lsp::DiagnosticEntry;
use codex_lsp::LspError;
use codex_lsp::LspManager;
use codex_lsp::SeverityFilter;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;

use crate::apply_patch;
use crate::apply_patch::InternalApplyPatchInvocation;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::handlers::parse_arguments;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::apply_patch::ApplyPatchRequest;
use crate::tools::runtimes::apply_patch::ApplyPatchRuntime;
use crate::tools::sandboxing::ToolCtx;

pub struct LspHandler;

const DIAGNOSTICS_WAIT: Duration = Duration::from_millis(500);

#[derive(Deserialize)]
struct DiagnosticsArgs {
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    severity: DiagnosticsSeverity,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum DiagnosticsSeverity {
    #[default]
    Errors,
    #[serde(alias = "warnings")]
    ErrorsAndWarnings,
    All,
}

impl DiagnosticsSeverity {
    fn filter(&self) -> SeverityFilter {
        match self {
            DiagnosticsSeverity::Errors => SeverityFilter::Errors,
            DiagnosticsSeverity::ErrorsAndWarnings => SeverityFilter::ErrorsAndWarnings,
            DiagnosticsSeverity::All => SeverityFilter::All,
        }
    }
}

#[derive(Deserialize)]
struct PositionArgs {
    file_path: String,
    line: u32,
    character: u32,
}

#[derive(Deserialize)]
struct ReferencesArgs {
    file_path: String,
    line: u32,
    character: u32,
    #[serde(default)]
    include_declaration: bool,
}

#[derive(Deserialize)]
struct RenameArgs {
    file_path: String,
    line: u32,
    character: u32,
    new_name: String,
}

#[async_trait]
impl ToolHandler for LspHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        invocation.tool_name == "lsp_rename"
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        match invocation.tool_name.as_str() {
            "lsp_diagnostics" => handle_diagnostics(invocation).await,
            "lsp_definition" => handle_definition(invocation).await,
            "lsp_references" => handle_references(invocation).await,
            "lsp_rename" => handle_rename(invocation).await,
            name => Err(FunctionCallError::RespondToModel(format!(
                "unsupported LSP tool {name}"
            ))),
        }
    }
}

async fn handle_diagnostics(invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
    let ToolInvocation {
        payload,
        session,
        turn,
        ..
    } = invocation;
    let args = parse_args::<DiagnosticsArgs>(payload)?;
    let lsp_manager = lsp_manager(&session)?;
    let filter = args.severity.filter();
    let path = args
        .file_path
        .map(|path| resolve_path(&turn, path))
        .map(|path| ensure_file_exists(&path))
        .transpose()?;
    let wait = path.as_ref().map(|_| DIAGNOSTICS_WAIT);
    let diagnostics = lsp_manager
        .diagnostics_for(path.clone(), filter, wait)
        .await
        .map_err(map_lsp_error)?;

    let content = render_diagnostics(&diagnostics);
    Ok(ToolOutput::Function {
        content,
        content_items: None,
        success: Some(true),
    })
}

async fn handle_definition(invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
    let ToolInvocation {
        payload,
        session,
        turn,
        ..
    } = invocation;
    let args = parse_args::<PositionArgs>(payload)?;
    let path = resolve_path(&turn, args.file_path);
    ensure_line_character(args.line, args.character)?;
    ensure_file_exists(&path)?;
    let lsp_manager = lsp_manager(&session)?;
    let locations = lsp_manager
        .definition(&path, args.line, args.character)
        .await
        .map_err(map_lsp_error)?;
    Ok(format_locations_output(locations))
}

async fn handle_references(invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
    let ToolInvocation {
        payload,
        session,
        turn,
        ..
    } = invocation;
    let args = parse_args::<ReferencesArgs>(payload)?;
    let path = resolve_path(&turn, args.file_path);
    ensure_line_character(args.line, args.character)?;
    ensure_file_exists(&path)?;
    let lsp_manager = lsp_manager(&session)?;
    let locations = lsp_manager
        .references(&path, args.line, args.character, args.include_declaration)
        .await
        .map_err(map_lsp_error)?;
    Ok(format_locations_output(locations))
}

async fn handle_rename(invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
    let ToolInvocation {
        payload,
        session,
        turn,
        tracker,
        call_id,
        tool_name,
        ..
    } = invocation;
    let args = parse_args::<RenameArgs>(payload)?;
    if args.new_name.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "new_name must not be empty".to_string(),
        ));
    }
    ensure_line_character(args.line, args.character)?;
    let path = resolve_path(&turn, args.file_path);
    ensure_file_exists(&path)?;
    let lsp_manager = lsp_manager(&session)?;
    let result = lsp_manager
        .rename(&path, args.line, args.character, &args.new_name)
        .await
        .map_err(map_lsp_error)?;
    let changes = convert_apply_patch_to_protocol(&result.action);
    let file_paths = file_paths_for_action(&result.action);

    match apply_patch::apply_patch(turn.as_ref(), result.action).await {
        InternalApplyPatchInvocation::Output(item) => {
            let content = item?;
            Ok(ToolOutput::Function {
                content,
                content_items: None,
                success: Some(true),
            })
        }
        InternalApplyPatchInvocation::DelegateToExec(apply) => {
            let emitter = ToolEmitter::apply_patch(changes.clone(), apply.auto_approved);
            let event_ctx =
                ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, Some(&tracker));
            emitter.begin(event_ctx).await;

            let req = ApplyPatchRequest {
                action: apply.action,
                file_paths: file_paths.clone(),
                changes,
                exec_approval_requirement: apply.exec_approval_requirement,
                timeout_ms: None,
                codex_exe: turn.codex_linux_sandbox_exe.clone(),
            };

            let mut orchestrator = ToolOrchestrator::new();
            let mut runtime = ApplyPatchRuntime::new();
            let tool_ctx = ToolCtx {
                session: session.as_ref(),
                turn: turn.as_ref(),
                call_id: call_id.clone(),
                tool_name: tool_name.to_string(),
            };
            let out = orchestrator
                .run(&mut runtime, &req, &tool_ctx, &turn, turn.approval_policy)
                .await;
            let event_ctx =
                ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, Some(&tracker));
            let content = emitter.finish(event_ctx, out).await?;

            let changed = file_paths
                .iter()
                .map(|path| path.as_path().to_path_buf())
                .collect();
            if let Err(err) = lsp_manager.on_files_changed(changed).await {
                tracing::debug!("lsp update skipped: {err}");
            }

            Ok(ToolOutput::Function {
                content,
                content_items: None,
                success: Some(true),
            })
        }
    }
}

fn parse_args<T>(payload: ToolPayload) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    let arguments = match payload {
        ToolPayload::Function { arguments } => arguments,
        _ => {
            return Err(FunctionCallError::RespondToModel(
                "lsp handler received unsupported payload".to_string(),
            ));
        }
    };
    parse_arguments(&arguments)
}

fn lsp_manager(
    session: &crate::codex::Session,
) -> Result<std::sync::Arc<LspManager>, FunctionCallError> {
    session.services.lsp_manager.clone().ok_or_else(|| {
        FunctionCallError::RespondToModel("LSP is disabled for this session".to_string())
    })
}

fn resolve_path(turn: &crate::codex::TurnContext, path: String) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        turn.cwd.join(candidate)
    }
}

fn ensure_line_character(line: u32, character: u32) -> Result<(), FunctionCallError> {
    if line == 0 || character == 0 {
        return Err(FunctionCallError::RespondToModel(
            "line and character must be 1-indexed".to_string(),
        ));
    }
    Ok(())
}

fn ensure_file_exists(path: &Path) -> Result<PathBuf, FunctionCallError> {
    if path.exists() {
        Ok(path.to_path_buf())
    } else {
        let path_display = path.display();
        Err(FunctionCallError::RespondToModel(format!(
            "file does not exist: {path_display}"
        )))
    }
}

fn render_diagnostics(diagnostics: &[DiagnosticEntry]) -> String {
    if diagnostics.is_empty() {
        return "No diagnostics.".to_string();
    }
    let mut out = String::new();
    for entry in diagnostics {
        let diagnostic = &entry.diagnostic;
        let severity = diagnostic
            .severity
            .map(|severity| format!("{severity:?}").to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());
        let line = diagnostic.range.start.line + 1;
        let character = diagnostic.range.start.character + 1;
        let path_display = entry.path.display();
        let message = diagnostic.message.trim();
        if let Some(source) = diagnostic.source.as_deref() {
            out.push_str(&format!(
                "- {path_display}:{line}:{character} [{severity}] {message} ({source})\n"
            ));
        } else {
            out.push_str(&format!(
                "- {path_display}:{line}:{character} [{severity}] {message}\n"
            ));
        }
    }
    out
}

fn format_locations_output(locations: Vec<codex_lsp::LocationInfo>) -> ToolOutput {
    if locations.is_empty() {
        return ToolOutput::Function {
            content: "No locations found.".to_string(),
            content_items: None,
            success: Some(false),
        };
    }
    let mut out = String::new();
    for location in locations {
        let path_display = location.path.display();
        let line = location.line;
        let character = location.character;
        out.push_str(&format!("{path_display}:{line}:{character}\n"));
    }
    ToolOutput::Function {
        content: out,
        content_items: None,
        success: Some(true),
    }
}

fn map_lsp_error(err: LspError) -> FunctionCallError {
    FunctionCallError::RespondToModel(err.to_string())
}

fn file_paths_for_action(action: &ApplyPatchAction) -> Vec<AbsolutePathBuf> {
    let mut keys = Vec::new();
    let cwd = action.cwd.as_path();

    for (path, change) in action.changes() {
        if let Some(key) = to_abs_path(cwd, path) {
            keys.push(key);
        }

        if let ApplyPatchFileChange::Update { move_path, .. } = change
            && let Some(dest) = move_path
            && let Some(key) = to_abs_path(cwd, dest)
        {
            keys.push(key);
        }
    }

    keys
}

fn to_abs_path(cwd: &Path, path: &Path) -> Option<AbsolutePathBuf> {
    AbsolutePathBuf::resolve_path_against_base(path, cwd).ok()
}
