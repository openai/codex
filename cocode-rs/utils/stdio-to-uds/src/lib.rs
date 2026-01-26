#![deny(clippy::print_stdout)]

use std::io;
use std::io::Write;
use std::net::Shutdown;
use std::path::Path;
use std::thread;

use anyhow::Context;
use anyhow::anyhow;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(windows)]
use uds_windows::UnixStream;

/// Connects to the Unix Domain Socket at `socket_path` and relays data between
/// standard input/output and the socket.
pub fn run(socket_path: &Path) -> anyhow::Result<()> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("failed to connect to socket at {}", socket_path.display()))?;

    let mut reader = stream
        .try_clone()
        .context("failed to clone socket for reading")?;

    let stdout_thread = thread::spawn(move || -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        io::copy(&mut reader, &mut handle)?;
        handle.flush()?;
        Ok(())
    });

    let stdin = io::stdin();
    {
        let mut handle = stdin.lock();
        io::copy(&mut handle, &mut stream).context("failed to copy data from stdin to socket")?;
    }

    stream
        .shutdown(Shutdown::Write)
        .context("failed to shutdown socket writer")?;

    let stdout_result = stdout_thread
        .join()
        .map_err(|_| anyhow!("thread panicked while copying socket data to stdout"))?;
    stdout_result.context("failed to copy data from socket to stdout")?;

    Ok(())
}
