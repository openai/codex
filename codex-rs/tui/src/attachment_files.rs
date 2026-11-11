//! Stores TUI attachments on the app-server host so later turns can reference them.

use crate::app_server_session::AppServerSession;
use anyhow::Context;
use anyhow::Result;
use codex_app_server_client::AppServerPath;
use uuid::Uuid;

pub(crate) const ATTACHMENT_DIR: &str = "attachments";

pub(crate) async fn create_attachment_dir(
    app_server: &mut AppServerSession,
    codex_home: Option<&AppServerPath>,
) -> Result<AppServerPath> {
    let codex_home =
        codex_home.context("App server did not report $CODEX_HOME; cannot create attachment")?;
    let path = codex_home
        .join(ATTACHMENT_DIR)
        .join(Uuid::new_v4().to_string());
    app_server
        .file_system()
        .fs_create_directory_all_path(&path)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not create attachment directory {path}"))?;
    Ok(path)
}

pub(crate) async fn write_attachment(
    app_server: &mut AppServerSession,
    path: AppServerPath,
    bytes: Vec<u8>,
) -> Result<()> {
    app_server
        .file_system()
        .fs_write_file_path(&path, bytes)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not write attachment {path}"))
}

pub(crate) async fn write_approved_plan(
    app_server: &mut AppServerSession,
    codex_home: Option<&AppServerPath>,
    plan: String,
) -> Result<AppServerPath> {
    let dir = create_attachment_dir(app_server, codex_home).await?;
    let path = dir.join("approved-plan.md");
    if let Err(err) = write_attachment(app_server, path.clone(), plan.into_bytes()).await {
        let _ = app_server.file_system().fs_remove_path(&dir).await;
        return Err(err);
    }
    Ok(path)
}
