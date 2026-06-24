use std::sync::Arc;

use crate::protocol::ENVIRONMENT_INFO_METHOD;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_SIGNAL_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::ExecParams;
use crate::protocol::FS_CANONICALIZE_METHOD;
use crate::protocol::FS_CLOSE_METHOD;
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_OPEN_METHOD;
use crate::protocol::FS_READ_BLOCK_METHOD;
use crate::protocol::FS_READ_DIRECTORY_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WALK_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCanonicalizeParams;
use crate::protocol::FsCloseParams;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsOpenParams;
use crate::protocol::FsReadBlockParams;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsWalkParams;
use crate::protocol::FsWriteFileParams;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpRequestParams;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::ReadParams;
use crate::protocol::SignalParams;
use crate::protocol::TerminateParams;
use crate::protocol::WriteParams;
use crate::rpc::RpcRouter;
use crate::server::ExecServerHandler;

pub(crate) fn build_router() -> RpcRouter<ExecServerHandler> {
    let mut router = RpcRouter::new();
    router.notification(
        INITIALIZED_METHOD,
        |handler: Arc<ExecServerHandler>, _params: serde_json::Value| async move {
            handler.initialized()
        },
    );
    router.request(
        INITIALIZE_METHOD,
        |handler: Arc<ExecServerHandler>, params: InitializeParams| async move {
            handler.initialize(params).await
        },
    );
    router.request_with_id(
        HTTP_REQUEST_METHOD,
        |handler: Arc<ExecServerHandler>, request_id, params: HttpRequestParams| async move {
            handler.http_request(request_id, params).await
        },
    );
    router.request(
        EXEC_METHOD,
        |handler: Arc<ExecServerHandler>, params: ExecParams| async move { handler.exec(params).await },
    );
    router.request(
        ENVIRONMENT_INFO_METHOD,
        |handler: Arc<ExecServerHandler>, _params: ()| async move { handler.environment_info() },
    );
    router.request(
        EXEC_READ_METHOD,
        |handler: Arc<ExecServerHandler>, params: ReadParams| async move {
            handler.exec_read(params).await
        },
    );
    router.request(
        EXEC_WRITE_METHOD,
        |handler: Arc<ExecServerHandler>, params: WriteParams| async move {
            handler.exec_write(params).await
        },
    );
    router.request(
        EXEC_SIGNAL_METHOD,
        |handler: Arc<ExecServerHandler>, params: SignalParams| async move {
            handler.signal(params).await
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
        FS_OPEN_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsOpenParams| async move {
            handler.fs_open(params).await
        },
    );
    router.request(
        FS_READ_BLOCK_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsReadBlockParams| async move {
            handler.fs_read_block(params).await
        },
    );
    router.request(
        FS_CLOSE_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsCloseParams| async move {
            handler.fs_close(params).await
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
        FS_CANONICALIZE_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsCanonicalizeParams| async move {
            handler.fs_canonicalize(params).await
        },
    );
    router.request(
        FS_READ_DIRECTORY_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsReadDirectoryParams| async move {
            handler.fs_read_directory(params).await
        },
    );
    router.request(
        FS_WALK_METHOD,
        |handler: Arc<ExecServerHandler>, params: FsWalkParams| async move {
            handler.fs_walk(params).await
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
