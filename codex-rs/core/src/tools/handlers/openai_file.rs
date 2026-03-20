use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

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

#[derive(Serialize)]
struct DownloadOpenAiFileResult {
    file_id: String,
    uri: String,
    file_name: String,
    mime_type: Option<String>,
    destination_path: std::path::PathBuf,
    bytes_written: u64,
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
            &args.file_id,
            &unique_manual_download_scope(),
            OPENAI_FILE_DOWNLOAD_LIMIT_BYTES,
        )
        .await
        .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
        let body = serde_json::to_string(&DownloadOpenAiFileResult {
            file_id: downloaded.file_id,
            uri: downloaded.uri,
            file_name: downloaded.file_name,
            mime_type: downloaded.mime_type,
            destination_path: downloaded.destination_path,
            bytes_written: downloaded.bytes_written,
        })
        .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;

        Ok(FunctionToolOutput::from_text(body, Some(true)))
    }
}
