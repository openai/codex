use std::collections::HashMap;
use std::io;
use std::net::Ipv4Addr;
use std::net::TcpListener;
use std::os::fd::AsRawFd;

/// Environment variable carrying the inherited ingress listener fd.
pub const INGRESS_LISTENER_FD_ENV_VAR: &str = "CODEX_INGRESS_LISTENER_FD";

/// Parent-owned TCP ingress for one sandboxed process.
pub struct IngressListener {
    listener: TcpListener,
}

/// Error returned while preparing process ingress.
#[derive(Debug)]
pub enum IngressListenerError {
    InvalidPort,
    PortInUse(u16),
    Io(io::Error),
}

impl IngressListener {
    /// Bind the requested parent-visible TCP port when ingress is enabled.
    pub fn prepare(ingress: Option<u16>) -> Result<Option<Self>, IngressListenerError> {
        if ingress == Some(0) {
            return Err(IngressListenerError::InvalidPort);
        }
        ingress.map(Self::bind).transpose()
    }

    /// Add the inherited listener fd consumed by `codex-linux-sandbox`.
    pub fn add_to_child_env(&self, env: &mut HashMap<String, String>) {
        env.insert(
            INGRESS_LISTENER_FD_ENV_VAR.to_string(),
            self.listener.as_raw_fd().to_string(),
        );
    }

    /// Return the listener fd that the spawned sandbox helper must preserve.
    pub fn inherited_fd(&self) -> i32 {
        self.listener.as_raw_fd()
    }

    fn bind(port: u16) -> Result<Self, IngressListenerError> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).map_err(|error| {
            if error.kind() == io::ErrorKind::AddrInUse {
                IngressListenerError::PortInUse(port)
            } else {
                IngressListenerError::Io(error)
            }
        })?;
        set_close_on_exec(listener.as_raw_fd())?;
        Ok(Self { listener })
    }
}

impl From<io::Error> for IngressListenerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn set_close_on_exec(fd: libc::c_int) -> io::Result<()> {
    // SAFETY: `fd` comes from this process's live `TcpListener`.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: `fd` comes from this process's live `TcpListener`.
    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_exports_inherited_fd_to_child_env() {
        let listener = IngressListener::bind(/*port*/ 0).expect("bind ingress listener");
        let mut env = HashMap::new();

        listener.add_to_child_env(&mut env);

        assert_eq!(
            env.get(INGRESS_LISTENER_FD_ENV_VAR),
            Some(&listener.inherited_fd().to_string())
        );
    }

    #[test]
    fn listener_keeps_close_on_exec_in_parent() {
        let listener = IngressListener::bind(/*port*/ 0).expect("bind ingress listener");

        assert_ne!(fd_flags(listener.inherited_fd()) & libc::FD_CLOEXEC, 0);
    }

    #[test]
    fn listener_rejects_ephemeral_ingress_port() {
        assert!(matches!(
            IngressListener::prepare(Some(0)),
            Err(IngressListenerError::InvalidPort)
        ));
    }

    fn fd_flags(fd: libc::c_int) -> libc::c_int {
        // SAFETY: `fd` comes from this test's live `TcpListener`.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(flags >= 0, "read fd flags: {}", io::Error::last_os_error());
        flags
    }
}
