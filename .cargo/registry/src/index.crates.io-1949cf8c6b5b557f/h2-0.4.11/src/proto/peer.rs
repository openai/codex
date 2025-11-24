use crate::error::Reason;
use crate::frame::{Pseudo, StreamId};
use crate::proto::{Error, Open};

use http::{HeaderMap, Request, Response};

use std::fmt;

/// Either a Client or a Server
pub(crate) trait Peer {
    /// Message type polled from the transport
    type Poll: fmt::Debug;
    const NAME: &'static str;

    fn r#dyn() -> Dyn;

    //fn is_server() -> bool;

    fn convert_poll_message(
        pseudo: Pseudo,
        fields: HeaderMap,
        stream_id: StreamId,
    ) -> Result<Self::Poll, Error>;

    /*
    fn is_local_init(id: StreamId) -> bool {
        assert!(!id.is_zero());
        Self::is_server() == id.is_server_initiated()
    }
    */
}

/// A dynamic representation of `Peer`.
///
/// This is used internally to avoid incurring a generic on all internal types.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Dyn {
    Client,
    Server,
}

#[derive(Debug)]
pub enum PollMessage {
    Client(Response<()>),
    Server(Request<()>),
}

// ===== impl Dyn =====

impl Dyn {
    pub fn is_server(&self) -> bool {
        *self == Dyn::Server
    }

    pub fn is_local_init(&self, id: StreamId) -> bool {
        assert!(!id.is_zero());
        self.is_server() == id.is_server_initiated()
    }

    pub fn convert_poll_message(
        &self,
        pseudo: Pseudo,
        fields: HeaderMap,
        stream_id: StreamId,
    ) -> Result<PollMessage, Error> {
        if self.is_server() {
            crate::server::Peer::convert_poll_message(pseudo, fields, stream_id)
                .map(PollMessage::Server)
        } else {
            crate::client::Peer::convert_poll_message(pseudo, fields, stream_id)
                .map(PollMessage::Client)
        }
    }

    /// Returns true if the remote peer can initiate a stream with the given ID.
    pub fn ensure_can_open(&self, id: StreamId, mode: Open) -> Result<(), Error> {
        if self.is_server() {
            // Ensure that the ID is a valid client initiated ID
            if mode.is_push_promise() || !id.is_client_initiated() {
                proto_err!(conn: "cannot open stream {:?} - not client initiated", id);
                return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
            }

            Ok(())
        } else {
            // Ensure that the ID is a valid server initiated ID
            if !mode.is_push_promise() || !id.is_server_initiated() {
                proto_err!(conn: "cannot open stream {:?} - not server initiated", id);
                return Err(Error::library_go_away(Reason::PROTOCOL_ERROR));
            }

            Ok(())
        }
    }
}
