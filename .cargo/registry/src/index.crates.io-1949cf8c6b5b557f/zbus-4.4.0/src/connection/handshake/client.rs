use async_trait::async_trait;
use std::collections::VecDeque;
use tracing::{debug, instrument, trace, warn};

use sha1::{Digest, Sha1};

use crate::{conn::socket::ReadHalf, is_flatpak, names::OwnedUniqueName, Message};

use super::{
    random_ascii, sasl_auth_id, AuthMechanism, Authenticated, BoxedSplit, Command, Common, Cookie,
    Error, Handshake, OwnedGuid, Result, Str,
};

/// A representation of an in-progress handshake, client-side
///
/// This struct is an async-compatible representation of the initial handshake that must be
/// performed before a D-Bus connection can be used.
#[derive(Debug)]
pub struct Client {
    common: Common,
    server_guid: Option<OwnedGuid>,
    bus: bool,
}

impl Client {
    /// Start a handshake on this client socket
    pub fn new(
        socket: BoxedSplit,
        mechanisms: Option<VecDeque<AuthMechanism>>,
        server_guid: Option<OwnedGuid>,
        bus: bool,
    ) -> Client {
        let mechanisms = mechanisms.unwrap_or_else(|| {
            let mut mechanisms = VecDeque::new();
            mechanisms.push_back(AuthMechanism::External);
            mechanisms.push_back(AuthMechanism::Cookie);
            mechanisms.push_back(AuthMechanism::Anonymous);
            mechanisms
        });

        Client {
            common: Common::new(socket, mechanisms),
            server_guid,
            bus,
        }
    }

    /// Respond to a cookie authentication challenge from the server.
    ///
    /// Returns the next command to send to the server.
    async fn handle_cookie_challenge(&mut self, data: Vec<u8>) -> Result<Command> {
        let context = std::str::from_utf8(&data)
            .map_err(|_| Error::Handshake("Cookie context was not valid UTF-8".into()))?;
        let mut split = context.split_ascii_whitespace();
        let context = split
            .next()
            .ok_or_else(|| Error::Handshake("Missing cookie context name".into()))?;
        let context = Str::from(context).try_into()?;
        let id = split
            .next()
            .ok_or_else(|| Error::Handshake("Missing cookie ID".into()))?;
        let id = id
            .parse()
            .map_err(|e| Error::Handshake(format!("Invalid cookie ID `{id}`: {e}")))?;
        let server_challenge = split
            .next()
            .ok_or_else(|| Error::Handshake("Missing cookie challenge".into()))?;

        let cookie = Cookie::lookup(&context, id).await?;
        let cookie = cookie.cookie();
        let client_challenge = random_ascii(16);
        let sec = format!("{server_challenge}:{client_challenge}:{cookie}");
        let sha1 = hex::encode(Sha1::digest(sec));
        let data = format!("{client_challenge} {sha1}").into_bytes();

        Ok(Command::Data(Some(data)))
    }

    fn set_guid(&mut self, guid: OwnedGuid) -> Result<()> {
        match &self.server_guid {
            Some(server_guid) if *server_guid != guid => {
                return Err(Error::Handshake(format!(
                    "Server GUID mismatch: expected {server_guid}, got {guid}",
                )));
            }
            Some(_) => (),
            None => self.server_guid = Some(guid),
        }

        Ok(())
    }

    // The dbus daemon on some platforms requires sending the zero byte as a
    // separate message with SCM_CREDS.
    #[instrument(skip(self))]
    #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
    async fn send_zero_byte(&mut self) -> Result<()> {
        let write = self.common.socket_mut().write_mut();

        let written = match write.send_zero_byte().await.map_err(|e| {
            Error::Handshake(format!("Could not send zero byte with credentials: {}", e))
        })? {
            // This likely means that the socket type is unable to send SCM_CREDS.
            // Let's try to send the 0 byte as a regular message.
            None => write.sendmsg(&[0], &[]).await?,
            Some(n) => n,
        };

        if written != 1 {
            return Err(Error::Handshake(
                "Could not send zero byte with credentials".to_string(),
            ));
        }

        Ok(())
    }

    /// Perform the authentication handshake with the server.
    ///
    /// In case of cookie auth, it returns the challenge response to send to the server, so it can
    /// be batched with rest of the commands.
    #[instrument(skip(self))]
    async fn authenticate(&mut self) -> Result<Option<Command>> {
        loop {
            let mechanism = self.common.next_mechanism()?;
            trace!("Trying {mechanism} mechanism");
            let auth_cmd = match mechanism {
                AuthMechanism::Anonymous => Command::Auth(Some(mechanism), Some("zbus".into())),
                AuthMechanism::External => {
                    Command::Auth(Some(mechanism), Some(sasl_auth_id()?.into_bytes()))
                }
                AuthMechanism::Cookie => Command::Auth(
                    Some(AuthMechanism::Cookie),
                    Some(sasl_auth_id()?.into_bytes()),
                ),
            };
            self.common.write_command(auth_cmd).await?;

            match self.common.read_command().await? {
                Command::Ok(guid) => {
                    trace!("Received OK from server");
                    self.set_guid(guid)?;

                    return Ok(None);
                }
                Command::Data(data) if mechanism == AuthMechanism::Cookie => {
                    let data = data.ok_or_else(|| {
                        Error::Handshake("Received DATA with no data from server".into())
                    })?;
                    trace!("Received cookie challenge from server");
                    let response = self.handle_cookie_challenge(data).await?;

                    return Ok(Some(response));
                }
                Command::Rejected(_) => debug!("{mechanism} rejected by the server"),
                Command::Error(e) => debug!("Received error from server: {e}"),
                cmd => {
                    return Err(Error::Handshake(format!(
                        "Unexpected command from server: {cmd}"
                    )))
                }
            }
        }
    }

    /// Sends out all commands after authentication.
    ///
    /// This includes the challenge response for cookie auth, if any and returns the number of
    /// responses expected from the server.
    #[instrument(skip(self))]
    async fn send_secondary_commands(
        &mut self,
        challenge_response: Option<Command>,
    ) -> Result<usize> {
        let mut commands = Vec::with_capacity(4);
        if let Some(response) = challenge_response {
            commands.push(response);
        }

        let can_pass_fd = self.common.socket_mut().read_mut().can_pass_unix_fd();
        if can_pass_fd {
            // xdg-dbus-proxy can't handle pipelining, hence this special handling.
            // FIXME: Remove this as soon as flatpak is fixed and fix is available in major distros.
            // See https://github.com/flatpak/xdg-dbus-proxy/issues/21
            if is_flatpak() {
                self.common.write_command(Command::NegotiateUnixFD).await?;
                match self.common.read_command().await? {
                    Command::AgreeUnixFD => self.common.set_cap_unix_fd(true),
                    Command::Error(e) => warn!("UNIX file descriptor passing rejected: {e}"),
                    cmd => {
                        return Err(Error::Handshake(format!(
                            "Unexpected command from server: {cmd}"
                        )))
                    }
                }
            } else {
                commands.push(Command::NegotiateUnixFD);
            }
        };
        commands.push(Command::Begin);
        let hello_method = if self.bus {
            Some(create_hello_method_call())
        } else {
            None
        };

        self.common
            .write_commands(&commands, hello_method.as_ref().map(|m| &**m.data()))
            .await?;

        // Server replies to all commands except `BEGIN`.
        Ok(commands.len() - 1)
    }

    #[instrument(skip(self))]
    async fn receive_secondary_responses(&mut self, expected_n_responses: usize) -> Result<()> {
        for response in self.common.read_commands(expected_n_responses).await? {
            match response {
                Command::Ok(guid) => {
                    trace!("Received OK from server");
                    self.set_guid(guid)?;
                }
                Command::AgreeUnixFD => self.common.set_cap_unix_fd(true),
                Command::Error(e) => warn!("UNIX file descriptor passing rejected: {e}"),
                // This also covers "REJECTED", which would mean that the server has rejected the
                // authentication challenge response (likely cookie) since it already agreed to the
                // mechanism. Theoretically we should be just trying the next auth mechanism but
                // this most likely means something is very wrong and we're already too deep into
                // the handshake to recover.
                cmd => {
                    return Err(Error::Handshake(format!(
                        "Unexpected command from server: {cmd}"
                    )))
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Handshake for Client {
    #[instrument(skip(self))]
    async fn perform(mut self) -> Result<Authenticated> {
        trace!("Initializing");

        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        self.send_zero_byte().await?;

        let challenge_response = self.authenticate().await?;
        let expected_n_responses = self.send_secondary_commands(challenge_response).await?;

        if expected_n_responses > 0 {
            self.receive_secondary_responses(expected_n_responses)
                .await?;
        }

        trace!("Handshake done");
        #[cfg(unix)]
        let (socket, mut recv_buffer, received_fds, cap_unix_fd, _) = self.common.into_components();
        #[cfg(not(unix))]
        let (socket, mut recv_buffer, _, _) = self.common.into_components();
        let (mut read, write) = socket.take();

        // If we're a bus connection, we need to read the unique name from `Hello` response.
        let unique_name = if self.bus {
            let unique_name = receive_hello_response(&mut read, &mut recv_buffer).await?;

            Some(unique_name)
        } else {
            None
        };

        Ok(Authenticated {
            socket_write: write,
            socket_read: Some(read),
            server_guid: self.server_guid.unwrap(),
            #[cfg(unix)]
            cap_unix_fd,
            already_received_bytes: recv_buffer,
            #[cfg(unix)]
            already_received_fds: received_fds,
            unique_name,
        })
    }
}

fn create_hello_method_call() -> Message {
    Message::method("/org/freedesktop/DBus", "Hello")
        .unwrap()
        .destination("org.freedesktop.DBus")
        .unwrap()
        .interface("org.freedesktop.DBus")
        .unwrap()
        .build(&())
        .unwrap()
}

async fn receive_hello_response(
    read: &mut Box<dyn ReadHalf>,
    recv_buffer: &mut Vec<u8>,
) -> Result<OwnedUniqueName> {
    use crate::message::Type;

    let reply = read
        .receive_message(
            0,
            recv_buffer,
            #[cfg(unix)]
            &mut vec![],
        )
        .await?;
    match reply.message_type() {
        Type::MethodReturn => reply.body().deserialize(),
        Type::Error => Err(Error::from(reply)),
        m => Err(Error::Handshake(format!("Unexpected messgage `{m:?}`"))),
    }
}
