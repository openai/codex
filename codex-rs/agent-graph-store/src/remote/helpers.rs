use codex_protocol::ThreadId;

use super::proto;
use crate::AgentGraphStoreError;
use crate::AgentGraphStoreResult;
use crate::ThreadSpawnEdgeStatus;

pub(super) fn proto_status(status: ThreadSpawnEdgeStatus) -> proto::ThreadSpawnEdgeStatus {
    match status {
        ThreadSpawnEdgeStatus::Open => proto::ThreadSpawnEdgeStatus::Open,
        ThreadSpawnEdgeStatus::Closed => proto::ThreadSpawnEdgeStatus::Closed,
    }
}

pub(super) fn proto_status_filter(status: Option<ThreadSpawnEdgeStatus>) -> Option<i32> {
    status.map(proto_status).map(Into::into)
}

pub(super) fn thread_ids_from_proto(
    thread_ids: Vec<String>,
    field_name: &str,
) -> AgentGraphStoreResult<Vec<ThreadId>> {
    thread_ids
        .into_iter()
        .map(|thread_id| {
            ThreadId::from_string(&thread_id).map_err(|err| AgentGraphStoreError::InvalidRequest {
                message: format!("remote agent graph store returned invalid {field_name}: {err}"),
            })
        })
        .collect()
}

pub(super) fn remote_status_to_error(status: tonic::Status) -> AgentGraphStoreError {
    match status.code() {
        tonic::Code::InvalidArgument => AgentGraphStoreError::InvalidRequest {
            message: status.message().to_string(),
        },
        _ => AgentGraphStoreError::Internal {
            message: format!("remote agent graph store request failed: {status}"),
        },
    }
}
