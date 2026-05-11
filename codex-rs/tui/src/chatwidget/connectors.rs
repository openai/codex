//! Connector list cache state for `ChatWidget`.

use crate::app_event::ConnectorsSnapshot;

#[derive(Debug, Clone, Default)]
pub(super) enum ConnectorsCacheState {
    #[default]
    Uninitialized,
    Loading,
    Ready(ConnectorsSnapshot),
    Failed(String),
}

#[derive(Debug, Default)]
pub(super) struct ConnectorsState {
    pub(super) cache: ConnectorsCacheState,
    pub(super) partial_snapshot: Option<ConnectorsSnapshot>,
    pub(super) prefetch_in_flight: bool,
    pub(super) force_refetch_pending: bool,
}
