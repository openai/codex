//! Filesystem RPCs on the app-server host, usable by sessions and background requests.

use super::AppServerSession;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_client::AppServerPath;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsRemoveResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use serde::de::DeserializeOwned;
use serde_json::json;
use uuid::Uuid;

impl AppServerSession {
    pub(crate) async fn resolve_thread_file(
        &self,
        source: ThreadId,
        path: &AppServerPath,
    ) -> Result<PathUri> {
        let cwd = self.preview_workspace(source).await?;
        resolve_preview_path(path, &cwd)
    }

    async fn preview_workspace(&self, source: ThreadId) -> Result<PathUri> {
        // Read only the environment metadata. Other paths in thread/read may be native
        // to a different operating system from the TUI.
        #[derive(serde::Deserialize)]
        struct Workspace {
            environments: Option<Vec<codex_app_server_protocol::ThreadEnvironment>>,
        }
        #[derive(serde::Deserialize)]
        struct Response {
            thread: Workspace,
        }
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response: Response = self
            .request_handle()
            .request_typed(ClientRequest::ThreadRead {
                request_id,
                params: codex_app_server_protocol::ThreadReadParams {
                    thread_id: source.to_string(),
                    include_turns: false,
                },
            })
            .await?;
        let environments = response.thread.environments.as_deref();
        environments
            .filter(|_| {
                crate::windows_sandbox::host_from_environments(environments)
                    == crate::app::WindowsSandboxHost::Local
            })
            .and_then(|envs| envs.first())
            .and_then(|primary| primary.cwd.to_inferred_path_uri())
            .ok_or_else(|| {
                color_eyre::eyre::eyre!("File previews require a workspace on the app-server host")
            })
    }

    pub(crate) async fn read_document(
        &self,
        source: ThreadId,
        original_path: &AppServerPath,
        resolved_path: &PathUri,
    ) -> Result<String> {
        let bytes = self
            .read_preview_file(source, original_path, resolved_path)
            .await?;
        Self::preview_text(bytes)
    }

    pub(crate) fn preview_text(bytes: Vec<u8>) -> Result<String> {
        let text = String::from_utf8(bytes).wrap_err("Not a UTF-8 text file")?;
        color_eyre::eyre::ensure!(
            !text
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\t' | '\n' | '\r')),
            "Not a text file"
        );
        Ok(text)
    }

    pub(crate) async fn read_preview_file(
        &self,
        source: ThreadId,
        original_path: &AppServerPath,
        resolved_path: &PathUri,
    ) -> Result<Vec<u8>> {
        color_eyre::eyre::ensure!(
            resolved_path.infer_path_convention() == Some(PathConvention::Posix),
            "File previews on Windows hosts are unavailable"
        );
        let workspace = self.preview_workspace(source).await?;
        color_eyre::eyre::ensure!(
            resolve_preview_path(original_path, &workspace)? == *resolved_path,
            "Workspace changed. Reopen the file to try again."
        );
        // fs/readFile follows symlinks. Only a server-side open can enforce no-follow.
        let path = AppServerPath::from_app_server(resolved_path.inferred_native_path_string());
        let bytes = self.file_system().fs_read_file_path(&path).await?;
        color_eyre::eyre::ensure!(
            self.preview_workspace(source).await? == workspace,
            "Workspace changed. Reopen the file to try again."
        );
        Ok(bytes)
    }

    pub(crate) fn file_system(&self) -> AppServerFileSystem {
        AppServerFileSystem {
            request_handle: self.request_handle(),
        }
    }
}

fn resolve_preview_path(path: &AppServerPath, cwd: &PathUri) -> Result<PathUri> {
    let path = LegacyAppPathString::from_string(path.as_str())
        .resolve_against(cwd, /*user_home_dir*/ None)?;
    color_eyre::eyre::ensure!(
        !path.to_url().has_host(),
        "File previews require a path on the app-server host"
    );
    Ok(path)
}

pub(crate) struct AppServerFileSystem {
    pub(crate) request_handle: AppServerRequestHandle,
}

impl AppServerFileSystem {
    pub(crate) async fn fs_create_directory_all_path(&self, path: &AppServerPath) -> Result<()> {
        self.request_fs_path::<FsCreateDirectoryResponse>(
            "fs/createDirectory",
            path,
            |request_id, path| ClientRequest::FsCreateDirectory {
                request_id,
                params: FsCreateDirectoryParams {
                    path,
                    recursive: Some(true),
                },
            },
            || json!({ "path": path.as_str(), "recursive": true }),
        )
        .await
        .map(drop)
    }

    pub(crate) async fn fs_write_file_path(
        &self,
        path: &AppServerPath,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let data_base64 = STANDARD.encode(bytes);
        self.request_fs_path::<FsWriteFileResponse>(
            "fs/writeFile",
            path,
            |request_id, path| ClientRequest::FsWriteFile {
                request_id,
                params: FsWriteFileParams {
                    path,
                    data_base64: data_base64.clone(),
                },
            },
            || json!({ "path": path.as_str(), "dataBase64": data_base64 }),
        )
        .await
        .map(drop)
    }

    pub(crate) async fn fs_read_file_path(&self, path: &AppServerPath) -> Result<Vec<u8>> {
        let response: FsReadFileResponse = self
            .request_fs_path(
                "fs/readFile",
                path,
                |request_id, path| ClientRequest::FsReadFile {
                    request_id,
                    params: FsReadFileParams { path },
                },
                || json!({ "path": path.as_str() }),
            )
            .await?;
        STANDARD
            .decode(response.data_base64)
            .wrap_err("fs/readFile returned invalid base64 data")
    }

    pub(crate) async fn fs_remove_path(&self, path: &AppServerPath) -> Result<()> {
        self.request_fs_path::<FsRemoveResponse>(
            "fs/remove",
            path,
            |request_id, path| ClientRequest::FsRemove {
                request_id,
                params: FsRemoveParams {
                    path,
                    recursive: None,
                    force: None,
                },
            },
            || json!({ "path": path.as_str() }),
        )
        .await
        .map(drop)
    }

    pub(crate) async fn watch_document(&self, path: &AppServerPath, id: Uuid) -> Result<()> {
        // fs/changed notifications use client-native absolute paths. If we can't represent
        // this directory, don't register a watch whose notifications we can't decode.
        let path = AbsolutePathBuf::from_absolute_path_checked(path.as_str())?;
        let _: codex_app_server_protocol::FsWatchResponse = self
            .request_handle
            .request_typed(ClientRequest::FsWatch {
                request_id: RequestId::String(format!("tui-fs-{}", Uuid::new_v4())),
                params: codex_app_server_protocol::FsWatchParams {
                    watch_id: id.to_string(),
                    path,
                },
            })
            .await?;
        Ok(())
    }

    pub(crate) async fn unwatch_document(&self, id: Uuid) {
        let _: Result<codex_app_server_protocol::FsUnwatchResponse, _> = self
            .request_handle
            .request_typed(ClientRequest::FsUnwatch {
                request_id: RequestId::String(format!("tui-fs-{}", Uuid::new_v4())),
                params: codex_app_server_protocol::FsUnwatchParams {
                    watch_id: id.to_string(),
                },
            })
            .await;
    }

    async fn request_fs_path<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &AppServerPath,
        local_request: impl FnOnce(RequestId, AbsolutePathBuf) -> ClientRequest,
        remote_params: impl FnOnce() -> serde_json::Value,
    ) -> Result<T> {
        let request_id = RequestId::String(format!("tui-fs-{}", Uuid::new_v4()));
        match &self.request_handle {
            AppServerRequestHandle::Cloud(_) => Err(color_eyre::eyre::eyre!(
                "{method} is unavailable for this backend"
            )),
            AppServerRequestHandle::Remote(handle) => {
                let response = handle
                    .request_json_rpc(JSONRPCRequest {
                        id: request_id,
                        method: method.to_string(),
                        params: Some(remote_params()),
                        trace: None,
                    })
                    .await
                    .wrap_err_with(|| format!("{method} failed in TUI"))?;
                serde_json::from_value(response.map_err(|mut source| {
                    // Callers log these errors. Keep raw server diagnostics out of the logs.
                    source.data = None;
                    TypedRequestError::Server {
                        method: method.to_string(),
                        source,
                    }
                })?)
                .wrap_err_with(|| format!("{method} returned invalid data"))
            }
            AppServerRequestHandle::InProcess(_) => {
                let path = AbsolutePathBuf::from_absolute_path_checked(path.as_str())
                    .wrap_err_with(|| format!("invalid local app-server fs path {path}"))?;
                self.request_handle
                    .request_typed(local_request(request_id, path))
                    .await
                    .wrap_err_with(|| format!("{method} failed in TUI"))
            }
        }
    }
}

#[cfg(test)]
#[path = "fs_preview_tests.rs"]
mod preview_tests;
