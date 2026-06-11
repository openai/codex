use super::AppServerSession;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_client::AppServerPath;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_utils_absolute_path::AbsolutePathBuf;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use serde::de::DeserializeOwned;
use serde_json::json;

impl AppServerSession {
    pub(crate) async fn fs_create_directory_all_path(
        &mut self,
        path: &AppServerPath,
    ) -> Result<()> {
        let _: FsCreateDirectoryResponse = self
            .request_fs_path(
                "fs/createDirectory",
                path,
                |request_id, path| ClientRequest::FsCreateDirectory {
                    request_id,
                    params: FsCreateDirectoryParams {
                        path,
                        recursive: Some(true),
                    },
                },
                json!({
                    "path": path.as_str(),
                    "recursive": true,
                }),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn fs_write_file_path(
        &mut self,
        path: &AppServerPath,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let data_base64 = STANDARD.encode(bytes);
        let _: FsWriteFileResponse = self
            .request_fs_path(
                "fs/writeFile",
                path,
                |request_id, path| ClientRequest::FsWriteFile {
                    request_id,
                    params: FsWriteFileParams {
                        path,
                        data_base64: data_base64.clone(),
                    },
                },
                json!({
                    "path": path.as_str(),
                    "dataBase64": data_base64,
                }),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn fs_read_file_path(&mut self, path: &AppServerPath) -> Result<Vec<u8>> {
        let response: FsReadFileResponse = self
            .request_fs_path(
                "fs/readFile",
                path,
                |request_id, path| ClientRequest::FsReadFile {
                    request_id,
                    params: FsReadFileParams { path },
                },
                json!({ "path": path.as_str() }),
            )
            .await?;
        STANDARD
            .decode(response.data_base64)
            .wrap_err("fs/readFile returned invalid base64 data")
    }

    async fn request_fs_path<T>(
        &mut self,
        method: &str,
        path: &AppServerPath,
        local_request: impl FnOnce(RequestId, AbsolutePathBuf) -> ClientRequest,
        remote_params: serde_json::Value,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let request_id = self.next_request_id();
        if self.uses_remote_workspace() {
            let AppServerRequestHandle::Remote(handle) = self.request_handle() else {
                color_eyre::eyre::bail!(
                    "raw JSON-RPC requests are only supported by the remote app-server client"
                );
            };
            let response = handle
                .request_json_rpc(JSONRPCRequest {
                    id: request_id,
                    method: method.to_string(),
                    params: Some(remote_params),
                    trace: None,
                })
                .await
                .wrap_err_with(|| format!("{method} failed in TUI"))?;
            let result = response.map_err(|source| {
                color_eyre::eyre::eyre!("{method} failed in TUI: {}", source.message)
            })?;
            return serde_json::from_value(result)
                .wrap_err_with(|| format!("{method} returned invalid data"));
        }
        let path = AbsolutePathBuf::from_absolute_path_checked(path.as_str())
            .wrap_err_with(|| format!("invalid local app-server fs path {path}"))?;
        self.client
            .request_typed(local_request(request_id, path))
            .await
            .wrap_err_with(|| format!("{method} failed in TUI"))
    }
}
