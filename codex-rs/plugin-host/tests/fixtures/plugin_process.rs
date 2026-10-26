//! Test fixture for plugin process startup, framing, and lifecycle failures.

use codex_plugin_host::framing;
use futures::SinkExt;
use futures::StreamExt;
use futures::executor::block_on;
use futures::io::AllowStdIo;
use std::io::Write;
use tokio_util::codec::FramedRead;
use tokio_util::codec::FramedWrite;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tokio_util::compat::FuturesAsyncWriteCompatExt;

fn crash() -> std::io::Result<()> {
    let mut stderr = std::io::stderr().lock();
    stderr.write_all(&vec![b'x'; 128 * 1024])?;
    stderr.write_all(b"probe failed\n")?;
    std::process::exit(/*code*/ 42);
}

fn main() -> std::io::Result<()> {
    let mut output = FramedWrite::new(
        AllowStdIo::new(std::io::stdout().lock()).compat_write(),
        framing::codec(),
    );
    #[cfg(unix)]
    if let Some(socket) = std::env::args_os()
        .skip(1)
        .find(|arg| std::path::Path::new(arg).is_absolute())
    {
        let _connection = std::os::unix::net::UnixStream::connect(socket)?;
        loop {
            std::thread::park();
        }
    }
    if std::env::args().any(|arg| arg == "--descendant") {
        block_on(output.send(b"descendant-ready".as_slice().into()))?;
        loop {
            std::thread::park();
        }
    }
    let program = std::env::current_exe()?;
    if program.file_stem() == Some(std::ffi::OsStr::new("failing-plugin-probe")) {
        return crash();
    }
    #[cfg(unix)]
    if program.file_stem() == Some(std::ffi::OsStr::new("unready-plugin-probe")) {
        std::os::unix::net::UnixStream::connect(program.with_extension("sock"))?
            .write_all(&std::process::id().to_le_bytes())?;
        loop {
            std::thread::park();
        }
    }
    let mut input = FramedRead::new(
        AllowStdIo::new(std::io::stdin().lock()).compat(),
        framing::codec(),
    );
    block_on(output.send(codex_plugin_host::READY.into()))?;
    #[cfg(unix)]
    {
        // SAFETY: getpgrp reads this process's group identifier.
        let group = unsafe { libc::getpgrp() };
        block_on(output.send(serde_json::to_vec(&(std::process::id(), group))?.into()))?;
    }
    let mut descendants = Vec::new();
    while let Some(message) = block_on(input.next()) {
        let message = message?;
        #[cfg(unix)]
        if message.starts_with(b"/") {
            use std::os::unix::ffi::OsStrExt;
            descendants.push(
                codex_utils_process::background_command(&program)
                    .arg(std::ffi::OsStr::from_bytes(&message))
                    .spawn()?,
            );
            continue;
        }
        if message.as_ref() == b"crash" {
            return crash();
        } else if message.as_ref() == b"spawn-descendant" {
            descendants.push(
                codex_utils_process::background_command(&program)
                    .arg("--descendant")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::null())
                    .spawn()?,
            );
        } else {
            block_on(output.send(message.freeze()))?;
        }
    }
    for mut descendant in descendants {
        descendant.kill()?;
        descendant.wait()?;
    }
    Ok(())
}
