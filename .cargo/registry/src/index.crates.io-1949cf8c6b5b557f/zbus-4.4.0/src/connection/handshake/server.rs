use async_trait::async_trait;
use sha1::{Digest, Sha1};
use std::collections::VecDeque;
use tracing::{instrument, trace};

use crate::names::OwnedUniqueName;

use super::{
    random_ascii, sasl_auth_id, AuthMechanism, Authenticated, BoxedSplit, Command, Common, Cookie,
    CookieContext, Error, Handshake, OwnedGuid, Result,
};

/*
 * Server-side handshake logic
 */
#[derive(Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum ServerHandshakeStep {
    WaitingForAuth,
    WaitingForData(AuthMechanism),
    WaitingForBegin,
    Done,
}

/// A representation of an in-progress handshake, server-side
///
/// This would typically be used to implement a D-Bus broker, or in the context of a P2P connection.
#[derive(Debug)]
pub struct Server<'s> {
    common: Common,
    step: ServerHandshakeStep,
    guid: OwnedGuid,
    #[cfg(unix)]
    client_uid: Option<u32>,
    #[cfg(windows)]
    client_sid: Option<String>,
    cookie_id: Option<usize>,
    cookie_context: CookieContext<'s>,
    unique_name: Option<OwnedUniqueName>,
}

impl<'s> Server<'s> {
    pub fn new(
        socket: BoxedSplit,
        guid: OwnedGuid,
        #[cfg(unix)] client_uid: Option<u32>,
        #[cfg(windows)] client_sid: Option<String>,
        mechanisms: Option<VecDeque<AuthMechanism>>,
        cookie_id: Option<usize>,
        cookie_context: CookieContext<'s>,
        unique_name: Option<OwnedUniqueName>,
    ) -> Result<Server<'s>> {
        let mechanisms = match mechanisms {
            Some(mechanisms) => mechanisms,
            None => {
                let mut mechanisms = VecDeque::new();
                mechanisms.push_back(AuthMechanism::External);

                mechanisms
            }
        };

        Ok(Server {
            common: Common::new(socket, mechanisms),
            step: ServerHandshakeStep::WaitingForAuth,
            #[cfg(unix)]
            client_uid,
            #[cfg(windows)]
            client_sid,
            cookie_id,
            cookie_context,
            guid,
            unique_name,
        })
    }

    #[instrument(skip(self))]
    async fn auth_ok(&mut self) -> Result<()> {
        let guid = self.guid.clone();
        let cmd = Command::Ok(guid);
        trace!("Sending authentication OK");
        self.common.write_command(cmd).await?;
        self.step = ServerHandshakeStep::WaitingForBegin;

        Ok(())
    }

    async fn check_external_auth(&mut self, sasl_id: &[u8]) -> Result<()> {
        let auth_ok = {
            let id = std::str::from_utf8(sasl_id)
                .map_err(|e| Error::Handshake(format!("Invalid ID: {e}")))?;
            #[cfg(unix)]
            {
                let uid = id
                    .parse::<u32>()
                    .map_err(|e| Error::Handshake(format!("Invalid UID: {e}")))?;
                self.client_uid.map(|u| u == uid).unwrap_or(false)
            }
            #[cfg(windows)]
            {
                self.client_sid.as_ref().map(|u| u == id).unwrap_or(false)
            }
        };

        if auth_ok {
            self.auth_ok().await
        } else {
            self.rejected_error().await
        }
    }

    #[instrument(skip(self))]
    async fn check_cookie_auth(&mut self, sasl_id: &[u8]) -> Result<()> {
        let cookie = match self.cookie_id {
            Some(cookie_id) => Cookie::lookup(&self.cookie_context, cookie_id).await?,
            None => Cookie::first(&self.cookie_context).await?,
        };
        let id = std::str::from_utf8(sasl_id)
            .map_err(|e| Error::Handshake(format!("Invalid ID: {e}")))?;
        if sasl_auth_id()? != id {
            // While the spec will make you believe that DBUS_COOKIE_SHA1 can be used to
            // authenticate any user, it is not even possible (or correct) for the server to manage
            // contents in random users' home directories.
            //
            // The dbus reference implementation also has the same limitation/behavior.
            self.rejected_error().await?;
            return Ok(());
        }
        let server_challenge = random_ascii(16);
        let data = format!("{} {} {server_challenge}", self.cookie_context, cookie.id());
        let cmd = Command::Data(Some(data.into_bytes()));
        trace!("Sending DBUS_COOKIE_SHA1 authentication challenge");
        self.common.write_command(cmd).await?;

        let auth_data = match self.common.read_command().await? {
            Command::Data(data) => data,
            _ => None,
        };
        let auth_data = auth_data.ok_or_else(|| {
            Error::Handshake("Expected DBUS_COOKIE_SHA1 authentication challenge response".into())
        })?;
        let client_auth = std::str::from_utf8(&auth_data)
            .map_err(|e| Error::Handshake(format!("Invalid COOKIE authentication data: {e}")))?;
        let mut split = client_auth.split_ascii_whitespace();
        let client_challenge = split
            .next()
            .ok_or_else(|| Error::Handshake("Missing cookie challenge".into()))?;
        let client_sha1 = split
            .next()
            .ok_or_else(|| Error::Handshake("Missing client cookie data".into()))?;
        let sec = format!("{server_challenge}:{client_challenge}:{}", cookie.cookie());
        let sha1 = hex::encode(Sha1::digest(sec));

        if sha1 == client_sha1 {
            self.auth_ok().await
        } else {
            self.rejected_error().await
        }
    }

    #[instrument(skip(self))]
    async fn unsupported_command_error(&mut self) -> Result<()> {
        let cmd = Command::Error("Unsupported or misplaced command".to_string());
        self.common.write_command(cmd).await?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn rejected_error(&mut self) -> Result<()> {
        let mechanisms = self.common.mechanisms().iter().cloned().collect();
        let cmd = Command::Rejected(mechanisms);
        trace!("Sending authentication error");
        self.common.write_command(cmd).await?;
        self.step = ServerHandshakeStep::WaitingForAuth;

        Ok(())
    }

    /// Perform the next step in the handshake.
    #[instrument(skip(self))]
    async fn next_step(&mut self) -> Result<bool> {
        match self.step {
            ServerHandshakeStep::WaitingForAuth => self.handle_auth().await?,
            ServerHandshakeStep::WaitingForData(mech) => self.handle_auth_data(mech).await?,
            ServerHandshakeStep::WaitingForBegin => self.finalize().await?,
            ServerHandshakeStep::Done => return Ok(true),
        }

        Ok(false)
    }

    /// Handle the authentication step of the handshake.
    #[instrument(skip(self))]
    async fn handle_auth(&mut self) -> Result<()> {
        assert_eq!(self.step, ServerHandshakeStep::WaitingForAuth);

        trace!("Waiting for authentication");
        let reply = self.common.read_command().await?;
        match reply {
            Command::Auth(mech, resp) => {
                let mech = mech.filter(|m| self.common.mechanisms().contains(m));

                match (mech, &resp) {
                    (Some(mech), None) => {
                        trace!("Sending data request");
                        self.common.write_command(Command::Data(None)).await?;
                        self.step = ServerHandshakeStep::WaitingForData(mech);
                    }
                    (Some(AuthMechanism::Anonymous), Some(_)) => {
                        self.auth_ok().await?;
                    }
                    (Some(AuthMechanism::External), Some(sasl_id)) => {
                        self.check_external_auth(sasl_id).await?;
                    }
                    (Some(AuthMechanism::Cookie), Some(sasl_id)) => {
                        self.check_cookie_auth(sasl_id).await?;
                    }
                    _ => self.rejected_error().await?,
                }
            }
            Command::Cancel | Command::Error(_) => {
                trace!("Received CANCEL or ERROR command from the client");
                self.rejected_error().await?;
            }
            _ => self.unsupported_command_error().await?,
        }

        Ok(())
    }

    /// Handle the authentication data receiving step of the handshake.
    #[instrument(skip(self))]
    async fn handle_auth_data(&mut self, mech: AuthMechanism) -> Result<()> {
        assert!(matches!(self.step, ServerHandshakeStep::WaitingForData(_)));

        trace!("Waiting for authentication data");
        let reply = self.common.read_command().await?;
        match (mech, reply) {
            (AuthMechanism::External, Command::Data(None)) => self.auth_ok().await?,
            (AuthMechanism::External, Command::Data(Some(data))) => {
                self.check_external_auth(&data).await?;
            }
            (AuthMechanism::Anonymous, Command::Data(_)) => self.auth_ok().await?,
            (_, Command::Data(_)) => self.rejected_error().await?,
            (_, _) => self.unsupported_command_error().await?,
        }
        Ok(())
    }

    /// Finalize the handshake.
    #[instrument(skip(self))]
    async fn finalize(&mut self) -> Result<()> {
        assert_eq!(self.step, ServerHandshakeStep::WaitingForBegin);

        trace!("Waiting for Begin command from the client");
        let reply = self.common.read_command().await?;
        match reply {
            Command::Begin => {
                trace!("Received Begin command from the client");
                self.step = ServerHandshakeStep::Done;
            }
            Command::Cancel | Command::Error(_) => {
                trace!("Received CANCEL or ERROR command from the client");
                self.rejected_error().await?;
            }
            #[cfg(unix)]
            Command::NegotiateUnixFD => {
                trace!("Received NEGOTIATE_UNIX_FD command from the client");
                if self.common.socket().read().can_pass_unix_fd() {
                    self.common.set_cap_unix_fd(true);
                    trace!("Sending AGREE_UNIX_FD to the client");
                    self.common.write_command(Command::AgreeUnixFD).await?;
                } else {
                    trace!("FD transmission not possible on this socket type. Rejecting..");
                    let cmd =
                        Command::Error("FD-passing not possible on this socket type".to_string());
                    self.common.write_command(cmd).await?;
                }
            }
            _ => self.unsupported_command_error().await?,
        }

        Ok(())
    }
}

#[async_trait]
impl Handshake for Server<'_> {
    #[instrument(skip(self))]
    async fn perform(mut self) -> Result<Authenticated> {
        while !self.next_step().await? {}

        trace!("Handshake done");
        #[cfg(unix)]
        let (socket, recv_buffer, received_fds, cap_unix_fd, _) = self.common.into_components();
        #[cfg(not(unix))]
        let (socket, recv_buffer, _, _) = self.common.into_components();
        let (read, write) = socket.take();
        Ok(Authenticated {
            socket_write: write,
            socket_read: Some(read),
            server_guid: self.guid,
            #[cfg(unix)]
            cap_unix_fd,
            already_received_bytes: recv_buffer,
            #[cfg(unix)]
            already_received_fds: received_fds,
            unique_name: self.unique_name,
        })
    }
}
