use std::sync::Arc;

use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_RESOLVE_APPROVAL_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::ExecParams;
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_READ_DIRECTORY_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::ReadParams;
use crate::protocol::ResolveExecApprovalParams;
use crate::protocol::TerminateParams;
use crate::protocol::WriteParams;
use crate::rpc::RpcRouter;
use crate::server::ExecServerHandler;
use codex_app_server_protocol::FsCopyParams;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsGetMetadataParams;
use codex_app_server_protocol::FsReadDirectoryParams;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsWriteFileParams;

pub(crate) fn build_router() -> RpcRouter<ExecServerHandler> {
    let mut router = RpcRouter::new();
    router.request(
        INITIALIZE_METHOD,
        |handler: Arc<ExecServerHandler>, _params: InitializeParams| async move {
            handler.initialize()
        },
    );
    router.notification(
        INITIALIZED_METHOD,
        |handler: Arc<ExecServerHandler>, _params: serde_json::Value| async move {
            handler.initialized()
        },
    );
    router.request(
        EXEC_METHOD,
        |handler: Arc<ExecServerHandler>, params: ExecParams| async move { handler.exec(params).await },
    );
    router.request(
        EXEC_READ_METHOD,
        |handler: Arc<ExecServerHandler>, params: ReadParams| async move {
            handler.exec_read(params).await
        },
    );
    router.request(
        EXEC_RESOLVE_APPROVAL_METHOD,
        |handler: Arc<ExecServerHandler>, params: ResolveExecApprovalParams| async move {
            handler.resolve_exec_approval(params).await
        },
    );
    router.request(
        EXEC_WRITE_METHOD,
        |handler: Arc<ExecServerHandler>, params: WriteParams| async move {
            handler.exec_write(params).await
        },
    );
    router.request(
        EXEC_TERMINATE_METHOD,
        |handler: Arc<ExecServerHandler>, params: TerminateParams| async move {
            handler.terminate(params).await
        },
    );
    router.request(
        FS_READ_FILE_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsReadFileParams| async move {
            handler.fs_read_file(params).await
        },
    );
    router.request(
        FS_WRITE_FILE_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsWriteFileParams| async move {
            handler.fs_write_file(params).await
        },
    );
    router.request(
        FS_CREATE_DIRECTORY_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsCreateDirectoryParams| async move {
            handler.fs_create_directory(params).await
        },
    );
    router.request(
        FS_GET_METADATA_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsGetMetadataParams| async move {
            handler.fs_get_metadata(params).await
        },
    );
    router.request(
        FS_READ_DIRECTORY_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsReadDirectoryParams| async move {
            handler.fs_read_directory(params).await
        },
    );
    router.request(
        FS_REMOVE_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsRemoveParams| async move {
            handler.fs_remove(params).await
        },
    );
    router.request(
        FS_COPY_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsCopyParams| async move {
            handler.fs_copy(params).await
        },
    );
    router
}
