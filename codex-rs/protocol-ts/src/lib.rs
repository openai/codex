use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use ts_rs::TS;

const HEADER: &str = "// GENERATED CODE! DO NOT MODIFY BY HAND!\n\n";

pub fn generate_ts(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    ensure_dir(out_dir)?;

    // Generate TS bindings
    codex_protocol::mcp_protocol::ConversationId::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::InputItem::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::ClientRequest::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::ServerRequest::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::NewConversationParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::NewConversationResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::AddConversationListenerParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::AddConversationSubscriptionResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::RemoveConversationListenerParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::RemoveConversationSubscriptionResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::SendUserMessageParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::SendUserMessageResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::SendUserTurnParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::SendUserTurnResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::InterruptConversationParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::InterruptConversationResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::GitDiffToRemoteParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::GitDiffToRemoteResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::LoginChatGptResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::LoginChatGptCompleteNotification::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::CancelLoginChatGptParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::CancelLoginChatGptResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::LogoutChatGptParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::LogoutChatGptResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::GetAuthStatusParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::GetAuthStatusResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::ApplyPatchApprovalParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::ApplyPatchApprovalResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::ExecCommandApprovalParams::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::ExecCommandApprovalResponse::export_all_to(out_dir)?;
    codex_protocol::mcp_protocol::ServerNotification::export_all_to(out_dir)?;

    // Prepend header to each generated .ts file
    let ts_files = ts_files_in(out_dir)?;
    for file in &ts_files {
        prepend_header_if_missing(file)?;
    }

    // Format with Prettier by passing individual files (no shell globbing)
    if let Some(prettier_bin) = prettier
        && !ts_files.is_empty()
    {
        let status = Command::new(prettier_bin)
            .arg("--write")
            .args(ts_files.iter().map(|p| p.as_os_str()))
            .status()
            .with_context(|| format!("Failed to invoke Prettier at {}", prettier_bin.display()))?;
        if !status.success() {
            return Err(anyhow!("Prettier failed with status {}", status));
        }
    }

    Ok(())
}

fn ensure_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create output directory {}", dir.display()))
}

fn prepend_header_if_missing(path: &Path) -> Result<()> {
    let mut content = String::new();
    {
        let mut f = fs::File::open(path)
            .with_context(|| format!("Failed to open {} for reading", path.display()))?;
        f.read_to_string(&mut content)
            .with_context(|| format!("Failed to read {}", path.display()))?;
    }

    if content.starts_with(HEADER) {
        return Ok(());
    }

    let mut f = fs::File::create(path)
        .with_context(|| format!("Failed to open {} for writing", path.display()))?;
    f.write_all(HEADER.as_bytes())
        .with_context(|| format!("Failed to write header to {}", path.display()))?;
    f.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write content to {}", path.display()))?;
    Ok(())
}

fn ts_files_in(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("Failed to read dir {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension() == Some(OsStr::new("ts")) {
            files.push(path);
        }
    }
    Ok(files)
}
