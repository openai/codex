mod auth_mechanism;
mod client;
mod command;
mod common;
mod cookies;
#[cfg(feature = "p2p")]
mod server;

use async_trait::async_trait;
#[cfg(unix)]
use nix::unistd::Uid;
use std::{collections::VecDeque, fmt::Debug};
use zbus_names::OwnedUniqueName;
use zvariant::Str;

#[cfg(windows)]
use crate::win32;
use crate::{Error, OwnedGuid, Result};

use super::socket::{BoxedSplit, ReadHalf, WriteHalf};

pub use auth_mechanism::AuthMechanism;
use client::Client;
use command::Command;
use common::Common;
use cookies::Cookie;
pub(crate) use cookies::CookieContext;
#[cfg(feature = "p2p")]
use server::Server;

/// The result of a finalized handshake
///
/// The result of a finalized [`ClientHandshake`] or [`ServerHandshake`].
///
/// [`ClientHandshake`]: struct.ClientHandshake.html
/// [`ServerHandshake`]: struct.ServerHandshake.html
#[derive(Debug)]
pub struct Authenticated {
    pub(crate) socket_write: Box<dyn WriteHalf>,
    /// The server Guid
    pub(crate) server_guid: OwnedGuid,
    /// Whether file descriptor passing has been accepted by both sides
    #[cfg(unix)]
    pub(crate) cap_unix_fd: bool,

    pub(crate) socket_read: Option<Box<dyn ReadHalf>>,
    pub(crate) already_received_bytes: Vec<u8>,
    #[cfg(unix)]
    pub(crate) already_received_fds: Vec<std::os::fd::OwnedFd>,
    pub(crate) unique_name: Option<OwnedUniqueName>,
}

impl Authenticated {
    /// Create a client-side `Authenticated` for the given `socket`.
    pub async fn client(
        socket: BoxedSplit,
        server_guid: Option<OwnedGuid>,
        mechanisms: Option<VecDeque<AuthMechanism>>,
        bus: bool,
    ) -> Result<Self> {
        Client::new(socket, mechanisms, server_guid, bus)
            .perform()
            .await
    }

    /// Create a server-side `Authenticated` for the given `socket`.
    ///
    /// The function takes `client_uid` on Unix only. On Windows, it takes `client_sid` instead.
    #[cfg(feature = "p2p")]
    pub async fn server(
        socket: BoxedSplit,
        guid: OwnedGuid,
        #[cfg(unix)] client_uid: Option<u32>,
        #[cfg(windows)] client_sid: Option<String>,
        auth_mechanisms: Option<VecDeque<AuthMechanism>>,
        cookie_id: Option<usize>,
        cookie_context: CookieContext<'_>,
        unique_name: Option<OwnedUniqueName>,
    ) -> Result<Self> {
        Server::new(
            socket,
            guid,
            #[cfg(unix)]
            client_uid,
            #[cfg(windows)]
            client_sid,
            auth_mechanisms,
            cookie_id,
            cookie_context,
            unique_name,
        )?
        .perform()
        .await
    }
}

#[async_trait]
pub trait Handshake {
    /// Perform the handshake.
    ///
    /// On a successful handshake, you get an `Authenticated`. If you need to send a Bus Hello,
    /// this remains to be done.
    async fn perform(mut self) -> Result<Authenticated>;
}

fn random_ascii(len: usize) -> String {
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    use std::iter;

    let mut rng = thread_rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(len)
        .collect()
}

fn sasl_auth_id() -> Result<String> {
    let id = {
        #[cfg(unix)]
        {
            Uid::effective().to_string()
        }

        #[cfg(windows)]
        {
            win32::ProcessToken::open(None)?.sid()?
        }
    };

    Ok(id)
}

#[cfg(feature = "p2p")]
#[cfg(unix)]
#[cfg(test)]
mod tests {
    use futures_util::future::join;
    #[cfg(not(feature = "tokio"))]
    use futures_util::io::{AsyncWrite, AsyncWriteExt};
    use ntest::timeout;
    #[cfg(not(feature = "tokio"))]
    use std::os::unix::net::UnixStream;
    use test_log::test;
    #[cfg(feature = "tokio")]
    use tokio::{
        io::{AsyncWrite, AsyncWriteExt},
        net::UnixStream,
    };

    use super::*;

    use crate::{Guid, Socket};

    fn create_async_socket_pair() -> (impl AsyncWrite + Socket, impl AsyncWrite + Socket) {
        // Tokio needs us to call the sync function from async context. :shrug:
        let (p0, p1) = crate::utils::block_on(async { UnixStream::pair().unwrap() });

        // initialize both handshakes
        #[cfg(not(feature = "tokio"))]
        let (p0, p1) = {
            p0.set_nonblocking(true).unwrap();
            p1.set_nonblocking(true).unwrap();

            (
                async_io::Async::new(p0).unwrap(),
                async_io::Async::new(p1).unwrap(),
            )
        };

        (p0, p1)
    }

    #[test]
    #[timeout(15000)]
    fn handshake() {
        let (p0, p1) = create_async_socket_pair();

        let guid = OwnedGuid::from(Guid::generate());
        let client = Client::new(p0.into(), None, Some(guid.clone()), false);
        let server = Server::new(
            p1.into(),
            guid,
            Some(Uid::effective().into()),
            None,
            None,
            CookieContext::default(),
            None,
        )
        .unwrap();

        // proceed to the handshakes
        let (client, server) = crate::utils::block_on(join(
            async move { client.perform().await.unwrap() },
            async move { server.perform().await.unwrap() },
        ));

        assert_eq!(client.server_guid, server.server_guid);
        assert_eq!(client.cap_unix_fd, server.cap_unix_fd);
    }

    #[test]
    #[timeout(15000)]
    fn pipelined_handshake() {
        let (mut p0, p1) = create_async_socket_pair();
        let server = Server::new(
            p1.into(),
            Guid::generate().into(),
            Some(Uid::effective().into()),
            None,
            None,
            CookieContext::default(),
            None,
        )
        .unwrap();

        crate::utils::block_on(
            p0.write_all(
                format!(
                    "\0AUTH EXTERNAL {}\r\nNEGOTIATE_UNIX_FD\r\nBEGIN\r\n",
                    hex::encode(sasl_auth_id().unwrap())
                )
                .as_bytes(),
            ),
        )
        .unwrap();
        let server = crate::utils::block_on(server.perform()).unwrap();

        assert!(server.cap_unix_fd);
    }

    #[test]
    #[timeout(15000)]
    fn separate_external_data() {
        let (mut p0, p1) = create_async_socket_pair();
        let server = Server::new(
            p1.into(),
            Guid::generate().into(),
            Some(Uid::effective().into()),
            None,
            None,
            CookieContext::default(),
            None,
        )
        .unwrap();

        crate::utils::block_on(
            p0.write_all(
                format!(
                    "\0AUTH EXTERNAL\r\nDATA {}\r\nBEGIN\r\n",
                    hex::encode(sasl_auth_id().unwrap())
                )
                .as_bytes(),
            ),
        )
        .unwrap();
        crate::utils::block_on(server.perform()).unwrap();
    }

    #[test]
    #[timeout(15000)]
    fn missing_external_data() {
        let (mut p0, p1) = create_async_socket_pair();
        let server = Server::new(
            p1.into(),
            Guid::generate().into(),
            Some(Uid::effective().into()),
            None,
            None,
            CookieContext::default(),
            None,
        )
        .unwrap();

        crate::utils::block_on(p0.write_all(b"\0AUTH EXTERNAL\r\nDATA\r\nBEGIN\r\n")).unwrap();
        crate::utils::block_on(server.perform()).unwrap();
    }

    #[test]
    #[timeout(15000)]
    fn anonymous_handshake() {
        let (mut p0, p1) = create_async_socket_pair();
        let server = Server::new(
            p1.into(),
            Guid::generate().into(),
            Some(Uid::effective().into()),
            Some(vec![AuthMechanism::Anonymous].into()),
            None,
            CookieContext::default(),
            None,
        )
        .unwrap();

        crate::utils::block_on(p0.write_all(b"\0AUTH ANONYMOUS abcd\r\nBEGIN\r\n")).unwrap();
        crate::utils::block_on(server.perform()).unwrap();
    }

    #[test]
    #[timeout(15000)]
    fn separate_anonymous_data() {
        let (mut p0, p1) = create_async_socket_pair();
        let server = Server::new(
            p1.into(),
            Guid::generate().into(),
            Some(Uid::effective().into()),
            Some(vec![AuthMechanism::Anonymous].into()),
            None,
            CookieContext::default(),
            None,
        )
        .unwrap();

        crate::utils::block_on(p0.write_all(b"\0AUTH ANONYMOUS\r\nDATA abcd\r\nBEGIN\r\n"))
            .unwrap();
        crate::utils::block_on(server.perform()).unwrap();
    }
}
