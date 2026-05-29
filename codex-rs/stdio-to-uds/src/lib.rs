#![deny(clippy::print_stdout)]

use std::io;
use std::path::Path;

use anyhow::Context;
use codex_uds::UnixStream;
use tokio::io::AsyncWriteExt;

/// Connects to the Unix Domain Socket at `socket_path` and relays data between
/// standard input/output and the socket.
pub async fn run(socket_path: &Path) -> anyhow::Result<()> {
    let stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to socket at {}", socket_path.display()))?;
    let (mut socket_reader, mut socket_writer) = tokio::io::split(stream);

    let copy_socket_to_stdout = async {
        let mut stdout = tokio::io::stdout();
        tokio::io::copy(&mut socket_reader, &mut stdout).await?;
        stdout.flush().await?;
        Ok(())
    };
    let copy_stdin_to_socket = async {
        let mut stdin = tokio::io::stdin();
        tokio::io::copy(&mut stdin, &mut socket_writer)
            .await
            .context("failed to copy data from stdin to socket")?;

        // The peer can close immediately after sending its response; in that
        // race, half-closing our write side can report NotConnected on some
        // platforms.
        if let Err(err) = socket_writer.shutdown().await
            && err.kind() != io::ErrorKind::NotConnected
        {
            return Err(err).context("failed to shutdown socket writer");
        }

        anyhow::Ok(())
    };

    tokio::try_join!(copy_stdin_to_socket, copy_socket_to_stdout)
        .context("failed to relay data between stdio and socket")?;

    Ok(())
}
