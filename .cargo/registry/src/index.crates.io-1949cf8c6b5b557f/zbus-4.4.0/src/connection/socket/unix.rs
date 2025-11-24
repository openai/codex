#[cfg(not(feature = "tokio"))]
use async_io::Async;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, BorrowedFd, FromRawFd, RawFd};
#[cfg(all(unix, not(feature = "tokio")))]
use std::os::unix::net::UnixStream;
#[cfg(not(feature = "tokio"))]
use std::sync::Arc;
#[cfg(unix)]
use std::{
    future::poll_fn,
    io::{self, IoSlice, IoSliceMut},
    os::fd::OwnedFd,
    task::Poll,
};
#[cfg(all(windows, not(feature = "tokio")))]
use uds_windows::UnixStream;

#[cfg(unix)]
use nix::{
    cmsg_space,
    sys::socket::{recvmsg, sendmsg, ControlMessage, ControlMessageOwned, MsgFlags, UnixAddr},
};

#[cfg(unix)]
use crate::utils::FDS_MAX;

#[cfg(all(unix, not(feature = "tokio")))]
#[async_trait::async_trait]
impl super::ReadHalf for Arc<Async<UnixStream>> {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> super::RecvmsgResult {
        poll_fn(|cx| {
            let (len, fds) = loop {
                match fd_recvmsg(self.as_raw_fd(), buf) {
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => match self.poll_readable(cx)
                    {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(res) => res?,
                    },
                    v => break v?,
                }
            };
            Poll::Ready(Ok((len, fds)))
        })
        .await
    }

    /// Supports passing file descriptors.
    fn can_pass_unix_fd(&self) -> bool {
        true
    }

    async fn peer_credentials(&mut self) -> io::Result<crate::fdo::ConnectionCredentials> {
        get_unix_peer_creds(self).await
    }
}

#[cfg(all(unix, not(feature = "tokio")))]
#[async_trait::async_trait]
impl super::WriteHalf for Arc<Async<UnixStream>> {
    async fn sendmsg(
        &mut self,
        buffer: &[u8],
        #[cfg(unix)] fds: &[BorrowedFd<'_>],
    ) -> io::Result<usize> {
        poll_fn(|cx| loop {
            match fd_sendmsg(
                self.as_raw_fd(),
                buffer,
                #[cfg(unix)]
                fds,
            ) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => match self.poll_writable(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(res) => res?,
                },
                v => return Poll::Ready(v),
            }
        })
        .await
    }

    async fn close(&mut self) -> io::Result<()> {
        let stream = self.clone();
        crate::Task::spawn_blocking(
            move || stream.get_ref().shutdown(std::net::Shutdown::Both),
            "close socket",
        )
        .await
    }

    #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
    async fn send_zero_byte(&mut self) -> io::Result<Option<usize>> {
        send_zero_byte(self).await.map(Some)
    }

    /// Supports passing file descriptors.
    fn can_pass_unix_fd(&self) -> bool {
        true
    }

    async fn peer_credentials(&mut self) -> io::Result<crate::fdo::ConnectionCredentials> {
        get_unix_peer_creds(self).await
    }
}

#[cfg(all(unix, feature = "tokio"))]
impl super::Socket for tokio::net::UnixStream {
    type ReadHalf = tokio::net::unix::OwnedReadHalf;
    type WriteHalf = tokio::net::unix::OwnedWriteHalf;

    fn split(self) -> super::Split<Self::ReadHalf, Self::WriteHalf> {
        let (read, write) = self.into_split();

        super::Split { read, write }
    }
}

#[cfg(all(unix, feature = "tokio"))]
#[async_trait::async_trait]
impl super::ReadHalf for tokio::net::unix::OwnedReadHalf {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> super::RecvmsgResult {
        let stream = self.as_ref();
        poll_fn(|cx| {
            loop {
                match stream.try_io(tokio::io::Interest::READABLE, || {
                    // We use own custom function for reading because we need to receive file
                    // descriptors too.
                    fd_recvmsg(stream.as_raw_fd(), buf)
                }) {
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        match stream.poll_read_ready(cx) {
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(res) => res?,
                        }
                    }
                    v => return Poll::Ready(v),
                }
            }
        })
        .await
    }

    /// Supports passing file descriptors.
    fn can_pass_unix_fd(&self) -> bool {
        true
    }

    async fn peer_credentials(&mut self) -> io::Result<crate::fdo::ConnectionCredentials> {
        get_unix_peer_creds(self.as_ref()).await
    }
}

#[cfg(all(unix, feature = "tokio"))]
#[async_trait::async_trait]
impl super::WriteHalf for tokio::net::unix::OwnedWriteHalf {
    async fn sendmsg(
        &mut self,
        buffer: &[u8],
        #[cfg(unix)] fds: &[BorrowedFd<'_>],
    ) -> io::Result<usize> {
        let stream = self.as_ref();
        poll_fn(|cx| loop {
            match stream.try_io(tokio::io::Interest::WRITABLE, || {
                fd_sendmsg(
                    stream.as_raw_fd(),
                    buffer,
                    #[cfg(unix)]
                    fds,
                )
            }) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    match stream.poll_write_ready(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(res) => res?,
                    }
                }
                v => return Poll::Ready(v),
            }
        })
        .await
    }

    async fn close(&mut self) -> io::Result<()> {
        tokio::io::AsyncWriteExt::shutdown(self).await
    }

    #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
    async fn send_zero_byte(&mut self) -> io::Result<Option<usize>> {
        send_zero_byte(self.as_ref()).await.map(Some)
    }

    /// Supports passing file descriptors.
    fn can_pass_unix_fd(&self) -> bool {
        true
    }

    async fn peer_credentials(&mut self) -> io::Result<crate::fdo::ConnectionCredentials> {
        get_unix_peer_creds(self.as_ref()).await
    }
}

#[cfg(all(windows, not(feature = "tokio")))]
#[async_trait::async_trait]
impl super::ReadHalf for Arc<Async<UnixStream>> {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> super::RecvmsgResult {
        match futures_util::AsyncReadExt::read(&mut self.as_ref(), buf).await {
            Err(e) => Err(e),
            Ok(len) => {
                #[cfg(unix)]
                let ret = (len, vec![]);
                #[cfg(not(unix))]
                let ret = len;
                Ok(ret)
            }
        }
    }

    async fn peer_credentials(&mut self) -> std::io::Result<crate::fdo::ConnectionCredentials> {
        let stream = self.clone();
        crate::Task::spawn_blocking(
            move || {
                use crate::win32::{unix_stream_get_peer_pid, ProcessToken};

                let pid = unix_stream_get_peer_pid(stream.get_ref())? as _;
                let sid = ProcessToken::open(if pid != 0 { Some(pid as _) } else { None })
                    .and_then(|process_token| process_token.sid())?;
                Ok(crate::fdo::ConnectionCredentials::default()
                    .set_process_id(pid)
                    .set_windows_sid(sid))
            },
            "peer credentials",
        )
        .await
    }
}

#[cfg(all(windows, not(feature = "tokio")))]
#[async_trait::async_trait]
impl super::WriteHalf for Arc<Async<UnixStream>> {
    async fn sendmsg(
        &mut self,
        buf: &[u8],
        #[cfg(unix)] _fds: &[BorrowedFd<'_>],
    ) -> std::io::Result<usize> {
        futures_util::AsyncWriteExt::write(&mut self.as_ref(), buf).await
    }

    async fn close(&mut self) -> std::io::Result<()> {
        let stream = self.clone();
        crate::Task::spawn_blocking(
            move || stream.get_ref().shutdown(std::net::Shutdown::Both),
            "close socket",
        )
        .await
    }

    async fn peer_credentials(&mut self) -> std::io::Result<crate::fdo::ConnectionCredentials> {
        super::ReadHalf::peer_credentials(self).await
    }
}

#[cfg(unix)]
fn fd_recvmsg(fd: RawFd, buffer: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
    let mut iov = [IoSliceMut::new(buffer)];
    let mut cmsgspace = cmsg_space!([RawFd; FDS_MAX]);

    let msg = recvmsg::<UnixAddr>(fd, &mut iov, Some(&mut cmsgspace), MsgFlags::empty())?;
    if msg.bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "failed to read from socket",
        ));
    }
    let mut fds = vec![];
    for cmsg in msg.cmsgs()? {
        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        if let ControlMessageOwned::ScmCreds(_) = cmsg {
            continue;
        }
        if let ControlMessageOwned::ScmRights(fd) = cmsg {
            fds.extend(fd.iter().map(|&f| unsafe { OwnedFd::from_raw_fd(f) }));
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected CMSG kind",
            ));
        }
    }
    Ok((msg.bytes, fds))
}

#[cfg(unix)]
fn fd_sendmsg(fd: RawFd, buffer: &[u8], fds: &[BorrowedFd<'_>]) -> io::Result<usize> {
    // FIXME: Remove this conversion once nix supports BorrowedFd here.
    //
    // Tracking issue: https://github.com/nix-rust/nix/issues/1750
    let fds: Vec<_> = fds.iter().map(|f| f.as_raw_fd()).collect();
    let cmsg = if !fds.is_empty() {
        vec![ControlMessage::ScmRights(&fds)]
    } else {
        vec![]
    };
    let iov = [IoSlice::new(buffer)];
    match sendmsg::<UnixAddr>(fd, &iov, &cmsg, MsgFlags::empty(), None) {
        // can it really happen?
        Ok(0) => Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "failed to write to buffer",
        )),
        Ok(n) => Ok(n),
        Err(e) => Err(e.into()),
    }
}

#[cfg(unix)]
async fn get_unix_peer_creds(fd: &impl AsRawFd) -> io::Result<crate::fdo::ConnectionCredentials> {
    let fd = fd.as_raw_fd();
    // FIXME: Is it likely enough for sending of 1 byte to block, to justify a task (possibly
    // launching a thread in turn)?
    crate::Task::spawn_blocking(move || get_unix_peer_creds_blocking(fd), "peer credentials").await
}

#[cfg(unix)]
fn get_unix_peer_creds_blocking(fd: RawFd) -> io::Result<crate::fdo::ConnectionCredentials> {
    // TODO: get this BorrowedFd directly from get_unix_peer_creds(), but this requires a
    // 'static lifetime due to the Task.
    let fd = unsafe { BorrowedFd::borrow_raw(fd) };

    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};

        getsockopt(&fd, PeerCredentials)
            .map(|creds| {
                crate::fdo::ConnectionCredentials::default()
                    .set_process_id(creds.pid() as _)
                    .set_unix_user_id(creds.uid())
            })
            .map_err(|e| e.into())
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    {
        let uid = nix::unistd::getpeereid(fd).map(|(uid, _)| uid.into())?;
        // FIXME: Handle pid fetching too.
        Ok(crate::fdo::ConnectionCredentials::default().set_unix_user_id(uid))
    }
}

// Send 0 byte as a separate SCM_CREDS message.
#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
async fn send_zero_byte(fd: &impl AsRawFd) -> io::Result<usize> {
    let fd = fd.as_raw_fd();
    crate::Task::spawn_blocking(move || send_zero_byte_blocking(fd), "send zero byte").await
}

#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
fn send_zero_byte_blocking(fd: RawFd) -> io::Result<usize> {
    let iov = [std::io::IoSlice::new(b"\0")];
    sendmsg::<()>(
        fd,
        &iov,
        &[ControlMessage::ScmCreds],
        MsgFlags::empty(),
        None,
    )
    .map_err(|e| e.into())
}
