//! Server-side rust implementation of a Wayland protocol backend

use std::os::unix::io::OwnedFd;
use std::{fmt, sync::Arc};

use crate::protocol::{same_interface, Interface, Message};

mod client;
mod common_poll;
mod handle;
mod registry;

pub use crate::types::server::Credentials;
pub use common_poll::InnerBackend;
pub use handle::{InnerHandle, WeakInnerHandle};

use super::server::*;

#[derive(Clone)]
pub struct InnerObjectId {
    id: u32,
    serial: u32,
    client_id: InnerClientId,
    interface: &'static Interface,
}

impl InnerObjectId {
    pub fn is_null(&self) -> bool {
        self.id == 0
    }

    pub fn interface(&self) -> &'static Interface {
        self.interface
    }

    pub fn same_client_as(&self, other: &Self) -> bool {
        self.client_id == other.client_id
    }

    pub fn protocol_id(&self) -> u32 {
        self.id
    }
}

impl fmt::Display for InnerObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}[{}]", self.interface.name, self.id, self.client_id.id)
    }
}

impl fmt::Debug for InnerObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({}, {})", self, self.serial)
    }
}

impl PartialEq for InnerObjectId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.serial == other.serial
            && self.client_id == other.client_id
            && same_interface(self.interface, other.interface)
    }
}

impl std::cmp::Eq for InnerObjectId {}

impl std::hash::Hash for InnerObjectId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.serial.hash(state);
        self.client_id.hash(state);
    }
}

/// An id of a client connected to the server.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InnerClientId {
    id: u32,
    serial: u32,
}

impl InnerClientId {
    fn as_u64(&self) -> u64 {
        ((self.id as u64) << 32) + self.serial as u64
    }

    fn from_u64(t: u64) -> Self {
        Self { id: (t >> 32) as u32, serial: t as u32 }
    }
}

/// The ID of a global
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InnerGlobalId {
    id: u32,
    serial: u32,
}

#[derive(Debug)]
pub(crate) struct Data<D: 'static> {
    user_data: Arc<dyn ObjectData<D>>,
    serial: u32,
}

impl<D> Clone for Data<D> {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn clone(&self) -> Self {
        Self { user_data: self.user_data.clone(), serial: self.serial }
    }
}

struct UninitObjectData;

impl<D> ObjectData<D> for UninitObjectData {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn request(
        self: Arc<Self>,
        _: &Handle,
        _: &mut D,
        _: ClientId,
        msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData<D>>> {
        panic!("Received a message on an uninitialized object: {msg:?}");
    }

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn destroyed(self: Arc<Self>, _: &Handle, _: &mut D, _: ClientId, _: ObjectId) {}

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UninitObjectData").finish()
    }
}
