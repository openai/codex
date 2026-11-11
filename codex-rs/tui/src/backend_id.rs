//! Logical backend identity, independent of a transport connection or native thread ID.
//! Reconnecting and auxiliary picker connections reuse the launch owner; a different
//! backend gets a fresh identity. These process-local IDs never appear on the wire.

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct BackendId(uuid::Uuid);

impl BackendId {
    pub(crate) fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
