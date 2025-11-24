use std::collections::VecDeque;
use tracing::{instrument, trace};

use super::{AuthMechanism, BoxedSplit, Command};
use crate::{Error, Result};

// Common code for the client and server side of the handshake.
#[derive(Debug)]
pub(super) struct Common {
    socket: BoxedSplit,
    recv_buffer: Vec<u8>,
    #[cfg(unix)]
    received_fds: Vec<std::os::fd::OwnedFd>,
    cap_unix_fd: bool,
    // the current AUTH mechanism is front, ordered by priority
    mechanisms: VecDeque<AuthMechanism>,
    first_command: bool,
}

impl Common {
    /// Start a handshake on this client socket
    pub fn new(socket: BoxedSplit, mechanisms: VecDeque<AuthMechanism>) -> Self {
        Self {
            socket,
            recv_buffer: Vec::new(),
            #[cfg(unix)]
            received_fds: Vec::new(),
            cap_unix_fd: false,
            mechanisms,
            first_command: true,
        }
    }

    #[cfg(all(unix, feature = "p2p"))]
    pub fn socket(&self) -> &BoxedSplit {
        &self.socket
    }

    pub fn socket_mut(&mut self) -> &mut BoxedSplit {
        &mut self.socket
    }

    pub fn set_cap_unix_fd(&mut self, cap_unix_fd: bool) {
        self.cap_unix_fd = cap_unix_fd;
    }

    #[cfg(feature = "p2p")]
    pub fn mechanisms(&self) -> &VecDeque<AuthMechanism> {
        &self.mechanisms
    }

    pub fn into_components(self) -> IntoComponentsReturn {
        (
            self.socket,
            self.recv_buffer,
            #[cfg(unix)]
            self.received_fds,
            self.cap_unix_fd,
            self.mechanisms,
        )
    }

    #[instrument(skip(self))]
    pub async fn write_command(&mut self, command: Command) -> Result<()> {
        self.write_commands(&[command], None).await
    }

    #[instrument(skip(self))]
    pub async fn write_commands(
        &mut self,
        commands: &[Command],
        extra_bytes: Option<&[u8]>,
    ) -> Result<()> {
        let mut send_buffer =
            commands
                .iter()
                .map(Vec::<u8>::from)
                .fold(vec![], |mut acc, mut c| {
                    if self.first_command {
                        // The first command is sent by the client so we can assume it's the client.
                        self.first_command = false;
                        // leading 0 is sent separately for `freebsd` and `dragonfly`.
                        #[cfg(not(any(target_os = "freebsd", target_os = "dragonfly")))]
                        acc.push(b'\0');
                    }
                    acc.append(&mut c);
                    acc.extend_from_slice(b"\r\n");
                    acc
                });
        if let Some(extra_bytes) = extra_bytes {
            send_buffer.extend_from_slice(extra_bytes);
        }
        while !send_buffer.is_empty() {
            let written = self
                .socket
                .write_mut()
                .sendmsg(
                    &send_buffer,
                    #[cfg(unix)]
                    &[],
                )
                .await?;
            send_buffer.drain(..written);
        }
        trace!("Wrote all commands");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn read_command(&mut self) -> Result<Command> {
        self.read_commands(1)
            .await
            .map(|cmds| cmds.into_iter().next().unwrap())
    }

    #[instrument(skip(self))]
    pub async fn read_commands(&mut self, n_commands: usize) -> Result<Vec<Command>> {
        let mut commands = Vec::with_capacity(n_commands);
        let mut n_received_commands = 0;
        'outer: loop {
            while let Some(lf_index) = self.recv_buffer.iter().position(|b| *b == b'\n') {
                if self.recv_buffer[lf_index - 1] != b'\r' {
                    return Err(Error::Handshake("Invalid line ending in handshake".into()));
                }

                #[allow(unused_mut)]
                let mut start_index = 0;
                if self.first_command {
                    // The first command is sent by the client so we can assume it's the server.
                    self.first_command = false;
                    if self.recv_buffer[0] != b'\0' {
                        return Err(Error::Handshake(
                            "First client byte is not NUL!".to_string(),
                        ));
                    }

                    start_index = 1;
                };

                let line_bytes = self.recv_buffer.drain(..=lf_index);
                let line = std::str::from_utf8(&line_bytes.as_slice()[start_index..])
                    .map_err(|e| Error::Handshake(e.to_string()))?;

                trace!("Reading {line}");
                commands.push(line.parse()?);
                n_received_commands += 1;

                if n_received_commands == n_commands {
                    break 'outer;
                }
            }

            let mut buf = vec![0; 1024];
            let res = self.socket.read_mut().recvmsg(&mut buf).await?;
            let read = {
                #[cfg(unix)]
                {
                    let (read, fds) = res;
                    if !fds.is_empty() {
                        // Most likely belonging to the messages already received.
                        self.received_fds.extend(fds);
                    }
                    read
                }
                #[cfg(not(unix))]
                {
                    res
                }
            };
            if read == 0 {
                return Err(Error::Handshake("Unexpected EOF during handshake".into()));
            }
            self.recv_buffer.extend(&buf[..read]);
        }

        Ok(commands)
    }

    pub fn next_mechanism(&mut self) -> Result<AuthMechanism> {
        self.mechanisms
            .pop_front()
            .ok_or_else(|| Error::Handshake("Exhausted available AUTH mechanisms".into()))
    }
}

#[cfg(unix)]
type IntoComponentsReturn = (
    BoxedSplit,
    Vec<u8>,
    Vec<std::os::fd::OwnedFd>,
    bool,
    VecDeque<AuthMechanism>,
);
#[cfg(not(unix))]
type IntoComponentsReturn = (BoxedSplit, Vec<u8>, bool, VecDeque<AuthMechanism>);
