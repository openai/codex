use std::io;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use codex_sandboxing::ingress::INGRESS_LISTENER_FD_ENV_VAR;

const INGRESS_BRIDGE_READY: u8 = 1;
const TARGET_SERVER_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TARGET_SERVER_CONNECT_RETRY_DELAY: Duration = Duration::from_millis(25);
const CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONCURRENT_CONNECTIONS: usize = 16;

pub(crate) fn take_ingress_listener_fd_from_env() -> io::Result<Option<libc::c_int>> {
    let raw_fd = match std::env::var(INGRESS_LISTENER_FD_ENV_VAR) {
        Ok(raw_fd) => raw_fd,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(io::Error::other(error)),
    };
    let listener_fd = raw_fd.parse::<libc::c_int>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid ingress listener fd `{raw_fd}`: {error}"),
        )
    })?;
    if listener_fd < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid ingress listener fd `{raw_fd}`"),
        ));
    }
    // SAFETY: inner sandbox setup is single-threaded before the final command execs.
    unsafe {
        std::env::remove_var(INGRESS_LISTENER_FD_ENV_VAR);
    }
    Ok(Some(listener_fd))
}

pub(crate) fn activate_ingress(listener_fd: libc::c_int, ingress_port: u16) -> io::Result<()> {
    crate::proxy_routing::ensure_loopback_interface_up()?;
    let (read_fd, write_fd) = create_ready_pipe()?;
    // SAFETY: ingress activation runs before the sandboxed command starts threads.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        let error = io::Error::last_os_error();
        let _ = close_fd(listener_fd);
        let _ = close_fd(read_fd);
        let _ = close_fd(write_fd);
        return Err(error);
    }

    if pid == 0 {
        if close_fd(read_fd).is_err() {
            // SAFETY: this is the forked bridge child; `_exit` avoids unwinding through fork.
            unsafe { libc::_exit(1) };
        }
        if run_ingress_bridge(listener_fd, ingress_port, write_fd).is_err() {
            // SAFETY: this is the forked bridge child; `_exit` avoids unwinding through fork.
            unsafe { libc::_exit(1) };
        }
        // SAFETY: this is the forked bridge child after its bridge loop returns.
        unsafe { libc::_exit(0) };
    }

    if let Err(error) = close_fd(write_fd) {
        let _ = close_fd(listener_fd);
        let _ = close_fd(read_fd);
        return Err(error);
    }
    let readiness_result = wait_for_ingress_bridge_ready(read_fd);
    let close_result = close_fd(listener_fd);
    readiness_result?;
    close_result
}

fn wait_for_ingress_bridge_ready(read_fd: libc::c_int) -> io::Result<()> {
    let mut ready = [0_u8; 1];
    // SAFETY: this parent owns the read end after closing the child write end.
    let mut read_file = unsafe { std::fs::File::from_raw_fd(read_fd) };
    read_file.read_exact(&mut ready).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("ingress bridge failed before acknowledging readiness: {error}"),
        )
    })?;
    if ready[0] != INGRESS_BRIDGE_READY {
        return Err(io::Error::other(
            "ingress bridge did not acknowledge readiness",
        ));
    }
    Ok(())
}

fn run_ingress_bridge(
    listener_fd: libc::c_int,
    ingress_port: u16,
    ready_fd: libc::c_int,
) -> io::Result<()> {
    crate::proxy_routing::harden_bridge_process()?;
    // SAFETY: the launcher transferred ownership of this inherited listener to the bridge child.
    let listener = unsafe { TcpListener::from_raw_fd(listener_fd) };
    // SAFETY: this bridge child owns the write end after closing the parent read end.
    let mut ready_file = unsafe { std::fs::File::from_raw_fd(ready_fd) };
    ready_file.write_all(&[INGRESS_BRIDGE_READY])?;
    drop(ready_file);

    let connection_limiter = Arc::new(ConnectionLimiter::new(MAX_CONCURRENT_CONNECTIONS));
    loop {
        let connection_permit = connection_limiter.acquire();
        let (ingress_stream, _) = listener.accept()?;
        thread::spawn(move || {
            let _connection_permit = connection_permit;
            let target_stream = match connect_target_server_with_retry(ingress_port) {
                Ok(target_stream) => target_stream,
                Err(_) => return,
            };
            if set_connection_timeouts(&ingress_stream)
                .and_then(|()| set_connection_timeouts(&target_stream))
                .is_err()
            {
                return;
            }
            let _ = proxy_bidirectional(ingress_stream, target_stream);
        });
    }
}

fn connect_target_server_with_retry(ingress_port: u16) -> io::Result<TcpStream> {
    let deadline = Instant::now() + TARGET_SERVER_CONNECT_TIMEOUT;
    loop {
        match TcpStream::connect((Ipv4Addr::LOCALHOST, ingress_port)) {
            Ok(stream) => return Ok(stream),
            Err(_) if Instant::now() < deadline => {
                thread::sleep(TARGET_SERVER_CONNECT_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

fn proxy_bidirectional(
    mut ingress_stream: TcpStream,
    mut target_stream: TcpStream,
) -> io::Result<()> {
    let mut ingress_reader = ingress_stream.try_clone()?;
    let mut target_writer = target_stream.try_clone()?;
    let ingress_to_target =
        thread::spawn(move || std::io::copy(&mut ingress_reader, &mut target_writer));
    let target_to_ingress = std::io::copy(&mut target_stream, &mut ingress_stream);
    let ingress_to_target = ingress_to_target
        .join()
        .map_err(|_| io::Error::other("ingress bridge thread panicked"))?;
    ingress_to_target?;
    target_to_ingress?;
    Ok(())
}

fn set_connection_timeouts(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(CONNECTION_IDLE_TIMEOUT))?;
    stream.set_write_timeout(Some(CONNECTION_IDLE_TIMEOUT))
}

struct ConnectionLimiter {
    max_connections: usize,
    active_connections: Mutex<usize>,
    connection_available: Condvar,
}

impl ConnectionLimiter {
    fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            active_connections: Mutex::new(0),
            connection_available: Condvar::new(),
        }
    }

    fn acquire(self: &Arc<Self>) -> ConnectionPermit {
        let mut active_connections = self
            .active_connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *active_connections >= self.max_connections {
            active_connections = self
                .connection_available
                .wait(active_connections)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *active_connections += 1;
        ConnectionPermit {
            limiter: Arc::clone(self),
        }
    }
}

struct ConnectionPermit {
    limiter: Arc<ConnectionLimiter>,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        let mut active_connections = self
            .limiter
            .active_connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *active_connections -= 1;
        self.limiter.connection_available.notify_one();
    }
}

fn close_fd(fd: libc::c_int) -> io::Result<()> {
    // SAFETY: callers pass a live inherited file descriptor owned by this process.
    let result = unsafe { libc::close(fd) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn create_ready_pipe() -> io::Result<(libc::c_int, libc::c_int)> {
    let mut pipe_fds = [0; 2];
    // SAFETY: `pipe_fds` points to space for exactly two file descriptors.
    let result = unsafe { libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((pipe_fds[0], pipe_fds[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn listener_fd_is_taken_from_env() {
        // SAFETY: this test mutates the process environment before spawning threads.
        unsafe {
            std::env::set_var(INGRESS_LISTENER_FD_ENV_VAR, "17");
        }

        assert_eq!(
            take_ingress_listener_fd_from_env().expect("take listener fd"),
            Some(17)
        );
        assert_eq!(
            std::env::var(INGRESS_LISTENER_FD_ENV_VAR),
            Err(std::env::VarError::NotPresent)
        );
    }

    #[test]
    fn connection_limiter_waits_for_available_slot() {
        let connection_limiter = Arc::new(ConnectionLimiter::new(/*max_connections*/ 1));
        let first_connection = connection_limiter.acquire();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let waiting_limiter = Arc::clone(&connection_limiter);
        let waiting_connection = thread::spawn(move || {
            let _second_connection = waiting_limiter.acquire();
            acquired_tx.send(()).expect("report acquired connection");
        });

        assert!(
            acquired_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "second connection should wait for the first connection to close"
        );
        drop(first_connection);
        acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second connection should acquire released slot");
        waiting_connection.join().expect("join waiting connection");
    }
}
