use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use codex_bridge::BridgeClient;
use codex_bridge::BridgeRequest;
use codex_bridge::BridgeResponse;
use codex_bridge::BridgeTransport;
use codex_bridge::OpaqueFrame;
#[cfg(unix)]
use codex_bridge::UnixSocketBridgeTransport;
use codex_bridge::decode_opaque_msgpack;
use codex_bridge::encode_opaque_msgpack;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;

mod recorder;
mod schema;
mod types;

use recorder::RemoteThreadRecorder;
pub use schema::thread_store_bridge_schema;
use types::AppendThreadRequest;
use types::ArchiveThreadRequest;
use types::CreateThreadRequest;
use types::DynamicToolsRequest;
use types::DynamicToolsResponse;
use types::FindThreadByNameRequest;
use types::FindThreadByNameResponse;
use types::FindThreadSpawnByPathRequest;
use types::FindThreadSpawnByPathResponse;
use types::ListThreadSpawnEdgesRequest;
use types::ListThreadSpawnEdgesResponse;
use types::ListThreadsRequest;
use types::ListThreadsResponse;
use types::LoadThreadHistoryRequest;
use types::LoadThreadHistoryResponse;
use types::MemoryModeRequest;
use types::MemoryModeResponse;
use types::ReadThreadRequest;
use types::ReadThreadResponse;
use types::ResumeThreadRecorderRequest;
use types::SetMemoryModeRequest;
use types::SetThreadNameRequest;
use types::StoredThreadPayload;
use types::ThreadSpawnEdgeRecord;
use types::UpdateThreadMetadataRequest;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::DynamicToolsParams;
use crate::FindThreadByNameParams;
use crate::FindThreadSpawnByPathParams;
use crate::ListThreadSpawnEdgesParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::ResolveLegacyPathParams;
use crate::ResumeThreadRecorderParams;
use crate::SetThreadMemoryModeParams;
use crate::SetThreadNameParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadMemoryModeParams;
use crate::ThreadPage;
use crate::ThreadRecorder;
use crate::ThreadSpawnEdge;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

const CREATE_THREAD: &str = "thread_store/create_thread";
const RESUME_THREAD_RECORDER: &str = "thread_store/resume_thread_recorder";
const APPEND_THREAD_ITEMS: &str = "thread_store/append_thread_items";
const LOAD_THREAD_HISTORY: &str = "thread_store/load_thread_history";
const READ_THREAD: &str = "thread_store/read_thread";
const LIST_THREADS: &str = "thread_store/list_threads";
const FIND_THREAD_BY_NAME: &str = "thread_store/find_thread_by_name";
const SET_THREAD_NAME: &str = "thread_store/set_thread_name";
const UPDATE_THREAD_METADATA: &str = "thread_store/update_thread_metadata";
const ARCHIVE_THREAD: &str = "thread_store/archive_thread";
const UNARCHIVE_THREAD: &str = "thread_store/unarchive_thread";
const DYNAMIC_TOOLS: &str = "thread_store/dynamic_tools";
const MEMORY_MODE: &str = "thread_store/memory_mode";
const SET_MEMORY_MODE: &str = "thread_store/set_memory_mode";
const MARK_MEMORY_MODE_POLLUTED: &str = "thread_store/mark_memory_mode_polluted";
const UPSERT_THREAD_SPAWN_EDGE: &str = "thread_store/upsert_thread_spawn_edge";
const LIST_THREAD_SPAWN_EDGES: &str = "thread_store/list_thread_spawn_edges";
const FIND_THREAD_SPAWN_BY_PATH: &str = "thread_store/find_thread_spawn_by_path";

const ROLLOUT_ITEMS_CODEC: &str = "codex.rollout_items.msgpack.v1";
const STORED_THREAD_HISTORY_CODEC: &str = "codex.stored_thread_history.msgpack.v1";

/// Remote [`ThreadStore`] implementation backed by a Python bridge service.
///
/// This adapter keeps cloud-specific storage details behind the bridge. Python receives typed,
/// bounded metadata DTOs plus opaque MsgPack payload frames for rollout history.
pub struct RemoteThreadStore<T> {
    client: Arc<BridgeClient<T>>,
}

impl<T> Clone for RemoteThreadStore<T> {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
        }
    }
}

impl<T> RemoteThreadStore<T> {
    /// Create a remote store from a bridge transport.
    pub fn new(transport: T) -> Self {
        Self {
            client: Arc::new(BridgeClient::new(transport)),
        }
    }
}

#[cfg(unix)]
impl RemoteThreadStore<UnixSocketBridgeTransport> {
    /// Create a remote store that talks to a Python bridge over a Unix domain socket.
    pub fn from_unix_socket(path: PathBuf) -> Self {
        Self::new(UnixSocketBridgeTransport::new(path))
    }
}

impl<T> RemoteThreadStore<T>
where
    T: BridgeTransport,
{
    async fn call<Req, Resp>(
        &self,
        method: &'static str,
        request: BridgeRequest<Req>,
    ) -> ThreadStoreResult<BridgeResponse<Resp>>
    where
        Req: serde::Serialize + Send + Sync,
        Resp: serde::de::DeserializeOwned,
    {
        self.client
            .call(method, request)
            .await
            .map_err(remote_error)
    }

    async fn call_empty<Req>(&self, method: &'static str, request: Req) -> ThreadStoreResult<()>
    where
        Req: serde::Serialize + Send + Sync,
    {
        let _: BridgeResponse<()> = self.call(method, BridgeRequest::new(request)).await?;
        Ok(())
    }

    async fn append_items_inner(
        &self,
        params: AppendThreadItemsParams,
        event_persistence_mode: Option<String>,
    ) -> ThreadStoreResult<()> {
        let items_frame = encode_opaque_msgpack("items", ROLLOUT_ITEMS_CODEC, &params.items)
            .map_err(remote_error)?;
        let request = AppendThreadRequest::from_params(params, event_persistence_mode);
        let _: BridgeResponse<()> = self
            .call(
                APPEND_THREAD_ITEMS,
                BridgeRequest::with_opaque_frames(request, vec![items_frame]),
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl<T> ThreadStore for RemoteThreadStore<T>
where
    T: BridgeTransport + 'static,
{
    async fn create_thread(
        &self,
        params: CreateThreadParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        let initial_items = initial_rollout_items(&params);
        let initial_items_frame =
            encode_opaque_msgpack("initialItems", ROLLOUT_ITEMS_CODEC, &initial_items)
                .map_err(remote_error)?;
        let thread_id = params.thread_id;
        let owner = params.owner.clone();
        let event_persistence_mode = types::event_persistence_mode(params.event_persistence_mode);
        let request = CreateThreadRequest::from_params(params);
        let _: BridgeResponse<()> = self
            .call(
                CREATE_THREAD,
                BridgeRequest::with_opaque_frames(request, vec![initial_items_frame]),
            )
            .await?;
        Ok(Box::new(RemoteThreadRecorder::new(
            thread_id,
            owner,
            event_persistence_mode,
            self.clone(),
        )))
    }

    async fn resume_thread_recorder(
        &self,
        params: ResumeThreadRecorderParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        let thread_id = params.thread_id;
        let owner = params.owner.clone();
        let event_persistence_mode = types::event_persistence_mode(params.event_persistence_mode);
        let _: BridgeResponse<()> = self
            .call(
                RESUME_THREAD_RECORDER,
                BridgeRequest::new(ResumeThreadRecorderRequest::from_params(params)),
            )
            .await?;
        Ok(Box::new(RemoteThreadRecorder::new(
            thread_id,
            owner,
            event_persistence_mode,
            self.clone(),
        )))
    }

    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        self.append_items_inner(params, None).await
    }

    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let response: BridgeResponse<LoadThreadHistoryResponse> = self
            .call(
                LOAD_THREAD_HISTORY,
                BridgeRequest::new(LoadThreadHistoryRequest::from_params(params)),
            )
            .await?;
        decode_history_from_response(response)
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        let include_history = params.include_history;
        let response: BridgeResponse<ReadThreadResponse> = self
            .call(
                READ_THREAD,
                BridgeRequest::new(ReadThreadRequest::from_params(params)),
            )
            .await?;
        let history = if include_history {
            Some(decode_history_frame(response.opaque_frames.as_slice())?)
        } else {
            None
        };
        Ok(response.body.thread.into_stored_thread(history)?)
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        let response: BridgeResponse<ListThreadsResponse> = self
            .call(
                LIST_THREADS,
                BridgeRequest::new(ListThreadsRequest::from_params(params)),
            )
            .await?;
        let mut items = Vec::with_capacity(response.body.items.len());
        for item in response.body.items {
            items.push(item.into_stored_thread(None)?);
        }
        Ok(ThreadPage {
            items,
            next_cursor: response.body.next_cursor,
            scanned: response.body.scanned,
        })
    }

    async fn find_thread_by_name(
        &self,
        params: FindThreadByNameParams,
    ) -> ThreadStoreResult<Option<StoredThread>> {
        let response: BridgeResponse<FindThreadByNameResponse> = self
            .call(
                FIND_THREAD_BY_NAME,
                BridgeRequest::new(FindThreadByNameRequest::from_params(params)),
            )
            .await?;
        response
            .body
            .thread
            .map(|thread| thread.into_stored_thread(None))
            .transpose()
    }

    async fn set_thread_name(&self, params: SetThreadNameParams) -> ThreadStoreResult<()> {
        self.call_empty(SET_THREAD_NAME, SetThreadNameRequest::from_params(params))
            .await
    }

    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        let response: BridgeResponse<ReadThreadResponse> = self
            .call(
                UPDATE_THREAD_METADATA,
                BridgeRequest::new(UpdateThreadMetadataRequest::from_params(params)),
            )
            .await?;
        response.body.thread.into_stored_thread(None)
    }

    async fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        self.call_empty(ARCHIVE_THREAD, ArchiveThreadRequest::from_params(params))
            .await
    }

    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        let response: BridgeResponse<ReadThreadResponse> = self
            .call(
                UNARCHIVE_THREAD,
                BridgeRequest::new(ArchiveThreadRequest::from_params(params)),
            )
            .await?;
        response.body.thread.into_stored_thread(None)
    }

    async fn resolve_legacy_path(
        &self,
        _params: ResolveLegacyPathParams,
    ) -> ThreadStoreResult<Option<ThreadId>> {
        Ok(None)
    }

    async fn dynamic_tools(
        &self,
        params: DynamicToolsParams,
    ) -> ThreadStoreResult<Option<Vec<DynamicToolSpec>>> {
        let response: BridgeResponse<DynamicToolsResponse> = self
            .call(
                DYNAMIC_TOOLS,
                BridgeRequest::new(DynamicToolsRequest::from_params(params)),
            )
            .await?;
        Ok(response.body.dynamic_tools)
    }

    async fn memory_mode(
        &self,
        params: ThreadMemoryModeParams,
    ) -> ThreadStoreResult<Option<String>> {
        let response: BridgeResponse<MemoryModeResponse> = self
            .call(
                MEMORY_MODE,
                BridgeRequest::new(MemoryModeRequest::from_params(params)),
            )
            .await?;
        Ok(response.body.memory_mode)
    }

    async fn set_memory_mode(&self, params: SetThreadMemoryModeParams) -> ThreadStoreResult<()> {
        self.call_empty(SET_MEMORY_MODE, SetMemoryModeRequest::from_params(params))
            .await
    }

    async fn mark_memory_mode_polluted(
        &self,
        params: ThreadMemoryModeParams,
    ) -> ThreadStoreResult<()> {
        self.call_empty(
            MARK_MEMORY_MODE_POLLUTED,
            MemoryModeRequest::from_params(params),
        )
        .await
    }

    async fn upsert_thread_spawn_edge(&self, edge: ThreadSpawnEdge) -> ThreadStoreResult<()> {
        self.call_empty(
            UPSERT_THREAD_SPAWN_EDGE,
            ThreadSpawnEdgeRecord::from_thread_spawn_edge(edge),
        )
        .await
    }

    async fn list_thread_spawn_edges(
        &self,
        params: ListThreadSpawnEdgesParams,
    ) -> ThreadStoreResult<Vec<ThreadSpawnEdge>> {
        let response: BridgeResponse<ListThreadSpawnEdgesResponse> = self
            .call(
                LIST_THREAD_SPAWN_EDGES,
                BridgeRequest::new(ListThreadSpawnEdgesRequest::from_params(params)),
            )
            .await?;
        response
            .body
            .edges
            .into_iter()
            .map(ThreadSpawnEdgeRecord::into_thread_spawn_edge)
            .collect()
    }

    async fn find_thread_spawn_by_path(
        &self,
        params: FindThreadSpawnByPathParams,
    ) -> ThreadStoreResult<Option<ThreadId>> {
        let response: BridgeResponse<FindThreadSpawnByPathResponse> = self
            .call(
                FIND_THREAD_SPAWN_BY_PATH,
                BridgeRequest::new(FindThreadSpawnByPathRequest::from_params(params)),
            )
            .await?;
        response.body.thread_id.map(parse_thread_id).transpose()
    }

    fn supports_legacy_path(&self, _path: &Path) -> bool {
        false
    }
}

pub(crate) async fn append_items_via_bridge<T>(
    store: &RemoteThreadStore<T>,
    thread_id: ThreadId,
    owner: crate::ThreadOwner,
    event_persistence_mode: String,
    items: &[RolloutItem],
) -> ThreadStoreResult<()>
where
    T: BridgeTransport,
{
    store
        .append_items_inner(
            AppendThreadItemsParams {
                thread_id,
                owner,
                items: items.to_vec(),
                idempotency_key: None,
                updated_at: None,
                new_thread_memory_mode: None,
            },
            Some(event_persistence_mode),
        )
        .await
}

fn initial_rollout_items(params: &CreateThreadParams) -> Vec<RolloutItem> {
    vec![RolloutItem::SessionMeta(SessionMetaLine {
        meta: SessionMeta {
            id: params.thread_id,
            forked_from_id: params.forked_from_id,
            timestamp: Utc::now().to_rfc3339(),
            cwd: params.cwd.clone(),
            originator: params.originator.clone(),
            cli_version: params.cli_version.clone(),
            source: params.source.clone(),
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            model_provider: Some(params.model_provider.clone()),
            base_instructions: Some(params.base_instructions.clone()),
            dynamic_tools: Some(params.dynamic_tools.clone()),
            memory_mode: params.memory_mode.clone(),
        },
        git: params.git_info.clone(),
    })]
}

fn decode_history_from_response(
    response: BridgeResponse<LoadThreadHistoryResponse>,
) -> ThreadStoreResult<StoredThreadHistory> {
    let mut history = decode_history_frame(response.opaque_frames.as_slice())?;
    if history.thread_id.to_string() != response.body.thread_id {
        history.thread_id = parse_thread_id(response.body.thread_id)?;
    }
    Ok(history)
}

fn decode_history_frame(frames: &[OpaqueFrame]) -> ThreadStoreResult<StoredThreadHistory> {
    let payload: StoredThreadPayload =
        decode_opaque_msgpack(frames, "history", STORED_THREAD_HISTORY_CODEC)
            .map_err(remote_error)?;
    payload.into_stored_thread_history()
}

fn parse_thread_id(thread_id: String) -> ThreadStoreResult<ThreadId> {
    ThreadId::from_string(thread_id.as_str()).map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("invalid remote thread id `{thread_id}`: {err}"),
    })
}

fn remote_error(err: codex_bridge::BridgeError) -> ThreadStoreError {
    match err {
        codex_bridge::BridgeError::Remote { code, message, .. } => match code.as_str() {
            "thread_not_found" => ThreadStoreError::InvalidRequest { message },
            "invalid_request" => ThreadStoreError::InvalidRequest { message },
            "conflict" => ThreadStoreError::Conflict { message },
            "unavailable" => ThreadStoreError::Unavailable { message },
            _ => ThreadStoreError::Internal { message },
        },
        codex_bridge::BridgeError::Transport { message } => {
            ThreadStoreError::Unavailable { message }
        }
        codex_bridge::BridgeError::Codec { message }
        | codex_bridge::BridgeError::InvalidResponse { message } => {
            ThreadStoreError::Internal { message }
        }
    }
}
