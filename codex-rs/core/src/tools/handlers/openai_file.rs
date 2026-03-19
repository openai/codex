use async_trait::async_trait;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::openai_files::OPENAI_FILE_DOWNLOAD_LIMIT_BYTES;
use crate::openai_files::download_file_to_managed_temp;
use crate::openai_files::unique_manual_download_scope;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct DownloadOpenAiFileHandler;

#[derive(Deserialize)]
struct DownloadOpenAiFileArgs {
    file_id: String,
}

#[async_trait]
impl ToolHandler for DownloadOpenAiFileHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "download_openai_file handler received unsupported payload".to_string(),
                ));
            }
        };
        let args: DownloadOpenAiFileArgs = parse_arguments(&arguments)?;
        let auth = session.services.auth_manager.auth().await;
        let downloaded = download_file_to_managed_temp(
            turn.config.as_ref(),
            auth.as_ref(),
            turn.cwd.as_path(),
            &args.file_id,
            &unique_manual_download_scope(),
            OPENAI_FILE_DOWNLOAD_LIMIT_BYTES,
        )
        .await
        .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;

        Ok(FunctionToolOutput::from_text(
            serde_json::json!({
                "file_id": downloaded.file_id,
                "uri": downloaded.uri,
                "file_name": downloaded.file_name,
                "mime_type": downloaded.mime_type,
                "destination_path": downloaded.destination_path,
                "bytes_written": downloaded.bytes_written,
            })
            .to_string(),
            Some(true),
        ))
    }
}
