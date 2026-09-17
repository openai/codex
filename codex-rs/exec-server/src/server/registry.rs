use codex_file_system::FileSystemSandboxContext;
use codex_file_system::WireFileSystemSandboxContext;
use codex_utils_path_uri::PathUri;
use opentelemetry::trace::TraceContextExt;
use std::sync::Arc;

use crate::local_file_system::resolve_existing_path;
use crate::protocol::CAPABILITY_ROOTS_DISCOVER_METHOD;
use crate::protocol::ENVIRONMENT_CONFIG_READ_METHOD;
use crate::protocol::ENVIRONMENT_INFO_METHOD;
use crate::protocol::ENVIRONMENT_STATUS_METHOD;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_SIGNAL_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::EnvironmentConfigReadParams;
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
use crate::protocol::FsCloseParams;
use crate::protocol::FsReadBlockParams;
use crate::protocol::HTTP_REQUEST_METHOD;
use crate::protocol::HttpRequestParams;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::JSONRPCErrorError;
use crate::protocol::ReadParams;
use crate::protocol::SignalParams;
use crate::protocol::TerminateParams;
use crate::protocol::WireCapabilityRootsDiscoverParams;
use crate::protocol::WireExecParams;
use crate::protocol::WireFsCanonicalizeParams;
use crate::protocol::WireFsCopyParams;
use crate::protocol::WireFsCreateDirectoryParams;
use crate::protocol::WireFsGetMetadataParams;
use crate::protocol::WireFsOpenParams;
use crate::protocol::WireFsReadDirectoryParams;
use crate::protocol::WireFsReadFileParams;
use crate::protocol::WireFsRemoveParams;
use crate::protocol::WireFsWalkParams;
use crate::protocol::WireFsWriteFileParams;
use crate::protocol::WriteParams;
use crate::rpc::RpcRouter;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;
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
    router.request_with_trace(
        EXEC_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireExecParams, trace| async move {
            let launch_context = trace
                .as_ref()
                .and_then(codex_otel::context_from_w3c_trace_context)
                .map(|context| context.span().span_context().clone());
            handler.exec(params.into(), launch_context).await
        },
    );
    router.request(
        ENVIRONMENT_INFO_METHOD,
        |handler: Arc<ExecServerHandler>, _params: ()| async move { handler.environment_info() },
    );
    router.request(
        ENVIRONMENT_CONFIG_READ_METHOD,
        |handler: Arc<ExecServerHandler>, params: EnvironmentConfigReadParams| async move {
            handler.environment_config_read(params).await
        },
    );
    router.request(
        ENVIRONMENT_STATUS_METHOD,
        |handler: Arc<ExecServerHandler>, _params: ()| async move { handler.environment_status() },
    );
    router.request(
        CAPABILITY_ROOTS_DISCOVER_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireCapabilityRootsDiscoverParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.discover_capability_roots(params).await
        },
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
        |handler: Arc<ExecServerHandler>, params: WireFsReadFileParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_read_file(params).await
        },
    );
    router.request(
        FS_OPEN_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsOpenParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
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
        |handler: Arc<ExecServerHandler>, params: WireFsWriteFileParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_write_file(params).await
        },
    );
    router.request(
        FS_CREATE_DIRECTORY_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsCreateDirectoryParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_create_directory(params).await
        },
    );
    router.request(
        FS_GET_METADATA_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsGetMetadataParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_get_metadata(params).await
        },
    );
    router.request(
        FS_CANONICALIZE_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsCanonicalizeParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_canonicalize(params).await
        },
    );
    router.request(
        FS_READ_DIRECTORY_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsReadDirectoryParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_read_directory(params).await
        },
    );
    router.request(
        FS_WALK_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsWalkParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_walk(params).await
        },
    );
    router.request(
        FS_REMOVE_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsRemoveParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_remove(params).await
        },
    );
    router.request(
        FS_COPY_METHOD,
        |handler: Arc<ExecServerHandler>, params: WireFsCopyParams| async move {
            let params = params.try_into_request(resolve_filesystem_sandbox)?;
            handler.fs_copy(params).await
        },
    );
    router
}

fn resolve_filesystem_sandbox(
    sandbox: WireFileSystemSandboxContext,
) -> Result<FileSystemSandboxContext, JSONRPCErrorError> {
    let cwd = if let Some(cwd) = sandbox.cwd() {
        cwd.clone()
    } else {
        if sandbox.requires_cwd() {
            return Err(invalid_params(
                "file system sandbox context with dynamic permissions requires cwd".to_string(),
            ));
        }
        // Legacy filesystem clients omitted cwd only for these policies. Preserve the executor's
        // own default (and Windows drive/share), rather than using the requested path.
        std::env::current_dir()
            .and_then(|cwd| resolve_existing_path(&cwd))
            .and_then(PathUri::from_host_native_path)
            .map_err(|err| internal_error(err.to_string()))?
    };
    Ok(sandbox.into_context(cwd))
}
