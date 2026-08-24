use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::RawFd;
use std::sync::Arc;

use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub(crate) enum StdinCloseBehavior {
    SendEof,
    NoEof,
}

pub(crate) struct PtyIo {
    fd: Arc<AsyncFd<File>>,
}

impl PtyIo {
    pub(crate) fn new(master_fd: RawFd) -> io::Result<Self> {
        // The PTY owner remains alive while its descriptor is duplicated.
        let fd = unsafe { BorrowedFd::borrow_raw(master_fd) }.try_clone_to_owned()?;
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags == -1
            || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
        {
            return Err(io::Error::last_os_error());
        }
        // Duplicates share O_NONBLOCK, so both reads and writes must use readiness.
        // Async tasks can be aborted even when a detached child keeps the PTY open
        // or the output channel is full; blocking read/send tasks cannot.
        // AsyncFd panics if the current runtime has no I/O driver.
        let fd = std::panic::catch_unwind(move || AsyncFd::new(File::from(fd)))
            .map_err(|_| io::Error::other("PTY I/O requires a Tokio runtime with I/O enabled"))??;
        Ok(Self { fd: Arc::new(fd) })
    }

    pub(crate) fn spawn(
        self,
        stdout_tx: mpsc::Sender<Vec<u8>>,
        mut writer_rx: mpsc::Receiver<Vec<u8>>,
        stdin_close: StdinCloseBehavior,
    ) -> (JoinHandle<()>, JoinHandle<()>) {
        let fd = self.fd;
        let reader = Arc::clone(&fd);
        let reader_handle = tokio::spawn(async move {
            let mut buf = [0u8; 8_192];
            loop {
                match reader
                    .async_io(Interest::READABLE, |mut file| file.read(&mut buf))
                    .await
                {
                    Ok(0) => break,
                    Ok(count) => {
                        // Output caps may close the receiver before the child exits.
                        let _ = stdout_tx.send(buf[..count].to_vec()).await;
                    }
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    // Unix PTYs may report EIO rather than EOF when the slave closes.
                    Err(_) => break,
                }
            }
        });
        let writer_handle = tokio::spawn(async move {
            while let Some(bytes) = writer_rx.recv().await {
                if write_all(&fd, &bytes).await.is_err() {
                    return;
                }
            }
            match stdin_close {
                StdinCloseBehavior::SendEof => {
                    // Preserve portable-pty's newline + current VEOF on stdin close,
                    // including when the terminal input queue requires waiting.
                    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
                    if unsafe { libc::tcgetattr(fd.get_ref().as_raw_fd(), termios.as_mut_ptr()) }
                        == 0
                    {
                        let eof = unsafe { termios.assume_init() }.c_cc[libc::VEOF];
                        if eof != 0 {
                            let _ = write_all(&fd, &[b'\n', eof]).await;
                        }
                    }
                }
                StdinCloseBehavior::NoEof => {}
            }
        });
        (reader_handle, writer_handle)
    }
}

async fn write_all(fd: &AsyncFd<File>, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        match fd
            .async_io(Interest::WRITABLE, |mut file| file.write(bytes))
            .await
        {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => bytes = &bytes[count..],
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}
