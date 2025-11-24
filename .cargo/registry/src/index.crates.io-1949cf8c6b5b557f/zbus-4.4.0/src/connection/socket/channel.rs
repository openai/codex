use std::io;

use async_broadcast::{broadcast, Receiver, Sender};

use crate::{fdo::ConnectionCredentials, Message};

/// An in-process channel-based socket.
///
/// This is a pair of two cross-wired channels. Since all communication happens in-process, there is
/// no need for any authentication.
///
/// This type is only available when `p2p` feature is enabled.
#[derive(Debug)]
pub struct Channel {
    writer: Writer,
    reader: Reader,
}

impl Channel {
    /// Create a pair of cross-wired channels.
    ///
    /// Use [`crate::connection::Builder::authenticated_socket`] to create `Connection` instances
    /// from each channel.
    pub fn pair() -> (Self, Self) {
        let (tx1, rx1) = broadcast(CHANNEL_CAPACITY);
        let (tx2, rx2) = broadcast(CHANNEL_CAPACITY);

        (
            Self {
                writer: Writer(tx1),
                reader: Reader(rx2),
            },
            Self {
                writer: Writer(tx2),
                reader: Reader(rx1),
            },
        )
    }
}

impl super::Socket for Channel {
    type ReadHalf = Reader;
    type WriteHalf = Writer;

    fn split(self) -> super::Split<Self::ReadHalf, Self::WriteHalf> {
        super::Split {
            read: self.reader,
            write: self.writer,
        }
    }
}

/// The reader half of a [`Channel`].
///
/// This type is only available when `p2p` feature is enabled.
#[derive(Debug)]
pub struct Reader(Receiver<Message>);

#[async_trait::async_trait]
impl super::ReadHalf for Reader {
    async fn receive_message(
        &mut self,
        _seq: u64,
        _already_received_bytes: &mut Vec<u8>,
        #[cfg(unix)] _already_received_fds: &mut Vec<std::os::fd::OwnedFd>,
    ) -> crate::Result<Message> {
        self.0.recv().await.map_err(|e| {
            crate::Error::InputOutput(io::Error::new(io::ErrorKind::BrokenPipe, e).into())
        })
    }

    async fn peer_credentials(&mut self) -> io::Result<ConnectionCredentials> {
        self_credentials().await
    }
}

/// The writer half of a [`Channel`].
///
/// This type is only available when `p2p` feature is enabled.
#[derive(Debug)]
pub struct Writer(Sender<Message>);

#[async_trait::async_trait]
impl super::WriteHalf for Writer {
    async fn send_message(&mut self, msg: &Message) -> crate::Result<()> {
        self.0
            .broadcast_direct(msg.clone())
            .await
            .map_err(|e| {
                crate::Error::InputOutput(io::Error::new(io::ErrorKind::BrokenPipe, e).into())
            })
            .map(|removed| {
                // We don't enable `overflow` mode so items should never be removed.
                assert!(removed.is_none());
            })
    }

    async fn close(&mut self) -> io::Result<()> {
        self.0.close();

        Ok(())
    }

    async fn peer_credentials(&mut self) -> io::Result<ConnectionCredentials> {
        self_credentials().await
    }
}

/// Return the credentials of the current process.
async fn self_credentials() -> io::Result<ConnectionCredentials> {
    let mut creds = ConnectionCredentials::default().set_process_id(std::process::id());

    #[cfg(unix)]
    {
        use nix::unistd::{Gid, Uid};

        creds = creds
            .set_unix_user_id(Uid::effective().into())
            .add_unix_group_id(Gid::effective().into());
    }
    #[cfg(windows)]
    {
        let sid = crate::win32::ProcessToken::open(None)?.sid()?;
        creds = creds.set_windows_sid(sid);
    }

    Ok(creds)
}

const CHANNEL_CAPACITY: usize = 32;
