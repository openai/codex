use libc::c_uint;
use serde::Deserialize;
use serde::Serialize;
use socket2::Domain;
use socket2::MaybeUninitSlice;
use socket2::MsgHdr;
use socket2::MsgHdrMut;
use socket2::Socket;
use socket2::Type;
use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;

const MAX_FDS_PER_MESSAGE: usize = 16;
const LENGTH_PREFIX_SIZE: usize = size_of::<u32>();
const MAX_DATAGRAM_SIZE: usize = 8192;

/// Converts a slice of MaybeUninit<T> to a slice of T.
///
/// The caller guarantees that every element of `buf` is initialized.
fn assume_init<T>(buf: &[MaybeUninit<T>]) -> &[T] {
    unsafe { std::slice::from_raw_parts(buf.as_ptr().cast(), buf.len()) }
}

fn assume_init_slice<T, const N: usize>(buf: &[MaybeUninit<T>; N]) -> &[T; N] {
    unsafe { &*(buf as *const [MaybeUninit<T>; N] as *const [T; N]) }
}

fn assume_init_vec<T>(mut buf: Vec<MaybeUninit<T>>) -> Vec<T> {
    unsafe {
        let ptr = buf.as_mut_ptr() as *mut T;
        let len = buf.len();
        let cap = buf.capacity();
        std::mem::forget(buf);
        Vec::from_raw_parts(ptr, len, cap)
    }
}

fn control_space_for_fds(count: usize) -> usize {
    unsafe { libc::CMSG_SPACE((count * size_of::<RawFd>()) as _) as usize }
}

/// Extracts the FDs from a SCM_RIGHTS control message.
fn extract_fds(control: &[u8]) -> Vec<OwnedFd> {
    let mut fds = Vec::new();
    let mut hdr: libc::msghdr = unsafe { std::mem::zeroed() };
    hdr.msg_control = control.as_ptr() as *mut libc::c_void;
    hdr.msg_controllen = control.len() as _;
    let hdr = hdr; // drop mut

    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&hdr) as *const libc::cmsghdr };
    while !cmsg.is_null() {
        let level = unsafe { (*cmsg).cmsg_level };
        let ty = unsafe { (*cmsg).cmsg_type };
        if level == libc::SOL_SOCKET && ty == libc::SCM_RIGHTS {
            let data_ptr = unsafe { libc::CMSG_DATA(cmsg).cast::<RawFd>() };
            let fd_count: usize = {
                let cmsg_data_len =
                    unsafe { (*cmsg).cmsg_len as usize } - unsafe { libc::CMSG_LEN(0) as usize };
                cmsg_data_len / size_of::<RawFd>()
            };
            for i in 0..fd_count {
                let fd = unsafe { data_ptr.add(i).read() };
                fds.push(unsafe { OwnedFd::from_raw_fd(fd) });
            }
        }
        cmsg = unsafe { libc::CMSG_NXTHDR(&hdr, cmsg) };
    }
    fds
}

/// Read a frame from a SOCK_STREAM socket.
///
/// A frame is a message length prefix followed by a payload. FDs may be included in the control
/// message when receiving the frame header.
async fn read_frame(async_socket: &AsyncFd<Socket>) -> std::io::Result<(Vec<u8>, Vec<OwnedFd>)> {
    let (message_len, fds) = read_frame_header(async_socket).await?;
    let payload = read_frame_payload(async_socket, message_len).await?;
    Ok((payload, fds))
}

/// Read the frame header (i.e. length) and any FDs from a SOCK_STREAM socket.
async fn read_frame_header(
    async_socket: &AsyncFd<Socket>,
) -> std::io::Result<(usize, Vec<OwnedFd>)> {
    let mut header = [MaybeUninit::<u8>::uninit(); LENGTH_PREFIX_SIZE];
    let mut filled = 0;
    let mut control = vec![MaybeUninit::<u8>::uninit(); control_space_for_fds(MAX_FDS_PER_MESSAGE)];
    let mut captured_control = false;

    while filled < LENGTH_PREFIX_SIZE {
        let mut guard = async_socket.readable().await?;
        // The first read should come with a control message containing any FDs.
        let result = if !captured_control {
            guard.try_io(|inner| {
                let mut bufs = [MaybeUninitSlice::new(&mut header[filled..])];
                let (read, control_len) = {
                    let mut msg = MsgHdrMut::new()
                        .with_buffers(&mut bufs)
                        .with_control(&mut control);
                    let read = inner.get_ref().recvmsg(&mut msg, 0)?;
                    (read, msg.control_len())
                };
                control.truncate(control_len);
                captured_control = true;
                Ok(read)
            })
        } else {
            guard.try_io(|inner| inner.get_ref().recv(&mut header[filled..]))
        };
        let Ok(result) = result else {
            // Would block, try again.
            continue;
        };

        let read = result?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "socket closed while receiving frame header",
            ));
        }

        filled += read;
        assert!(filled <= LENGTH_PREFIX_SIZE);
        if filled == LENGTH_PREFIX_SIZE {
            let len_bytes = assume_init_slice(&header);
            let payload_len = u32::from_le_bytes(*len_bytes) as usize;
            let fds = extract_fds(assume_init(&control));
            return Ok((payload_len, fds));
        }
    }
    unreachable!("header loop always returns")
}

/// Read `message_len` bytes from a SOCK_STREAM socket.
async fn read_frame_payload(
    async_socket: &AsyncFd<Socket>,
    message_len: usize,
) -> std::io::Result<Vec<u8>> {
    if message_len == 0 {
        return Ok(Vec::new());
    }
    let mut payload = vec![MaybeUninit::<u8>::uninit(); message_len];
    let mut filled = 0;
    while filled < message_len {
        let mut guard = async_socket.readable().await?;
        let result = guard.try_io(|inner| inner.get_ref().recv(&mut payload[filled..]));
        let Ok(result) = result else {
            // Would block, try again.
            continue;
        };
        let read = result?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "socket closed while receiving frame payload",
            ));
        }
        filled += read;
        assert!(filled <= message_len);
        if filled == message_len {
            return Ok(assume_init_vec(payload));
        }
    }
    unreachable!("loop exits only after returning payload")
}

fn send_datagram_bytes(socket: &Socket, data: &[u8], fds: &[OwnedFd]) -> std::io::Result<()> {
    let control = make_control_message(fds)?;
    let payload = [IoSlice::new(data)];
    let msg = if control.is_empty() {
        MsgHdr::new().with_buffers(&payload)
    } else {
        MsgHdr::new().with_buffers(&payload).with_control(&control)
    };
    let written = socket.sendmsg(&msg, 0)?;
    if written != data.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::WriteZero,
            format!(
                "short datagram write: wrote {written} bytes out of {}",
                data.len()
            ),
        ));
    }
    Ok(())
}

fn encode_length(len: usize) -> std::io::Result<[u8; LENGTH_PREFIX_SIZE]> {
    let len_u32 = u32::try_from(len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("message too large: {len}"),
        )
    })?;
    Ok(len_u32.to_le_bytes())
}

fn make_control_message(fds: &[OwnedFd]) -> std::io::Result<Vec<u8>> {
    if fds.len() > MAX_FDS_PER_MESSAGE {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("too many fds: {}", fds.len()),
        ))
    } else if fds.is_empty() {
        Ok(Vec::new())
    } else {
        let mut control = vec![0u8; control_space_for_fds(fds.len())];
        unsafe {
            let cmsg = control.as_mut_ptr().cast::<libc::cmsghdr>();
            (*cmsg).cmsg_len =
                libc::CMSG_LEN(size_of::<RawFd>() as c_uint * fds.len() as c_uint) as _;
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            let data_ptr = libc::CMSG_DATA(cmsg).cast::<RawFd>();
            for (i, fd) in fds.iter().enumerate() {
                data_ptr.add(i).write(fd.as_raw_fd());
            }
        }
        Ok(control)
    }
}

fn receive_datagram_bytes(socket: &Socket) -> std::io::Result<(Vec<u8>, Vec<OwnedFd>)> {
    let mut buffer = vec![MaybeUninit::<u8>::uninit(); MAX_DATAGRAM_SIZE];
    let mut control = vec![MaybeUninit::<u8>::uninit(); control_space_for_fds(MAX_FDS_PER_MESSAGE)];
    let (read, control_len) = {
        let mut bufs = [MaybeUninitSlice::new(&mut buffer)];
        let mut msg = MsgHdrMut::new()
            .with_buffers(&mut bufs)
            .with_control(&mut control);
        let read = socket.recvmsg(&mut msg, 0)?;
        (read, msg.control_len())
    };
    let data = assume_init(&buffer[..read]).to_vec();
    let fds = extract_fds(assume_init(&control[..control_len]));
    Ok((data, fds))
}

pub(crate) struct AsyncSocket {
    inner: AsyncFd<Socket>,
}

impl AsyncSocket {
    fn new(socket: Socket) -> std::io::Result<AsyncSocket> {
        socket.set_nonblocking(true)?;
        let async_socket = AsyncFd::new(socket)?;
        Ok(AsyncSocket {
            inner: async_socket,
        })
    }

    pub fn from_fd(fd: OwnedFd) -> std::io::Result<AsyncSocket> {
        AsyncSocket::new(Socket::from(fd))
    }

    pub fn pair() -> std::io::Result<(AsyncSocket, AsyncSocket)> {
        let (server, client) = Socket::pair(Domain::UNIX, Type::STREAM, None)?;
        Ok((AsyncSocket::new(server)?, AsyncSocket::new(client)?))
    }

    pub async fn send_with_fds<T: Serialize>(
        &self,
        msg: T,
        fds: &[OwnedFd],
    ) -> std::io::Result<()> {
        let payload = serde_json::to_vec(&msg)?;
        let mut frame = Vec::with_capacity(LENGTH_PREFIX_SIZE + payload.len());
        frame.extend_from_slice(&encode_length(payload.len())?);
        frame.extend_from_slice(&payload);
        send_stream_frame(&self.inner, &frame, fds).await
    }

    pub async fn receive_with_fds<T: for<'de> Deserialize<'de>>(
        &self,
    ) -> std::io::Result<(T, Vec<OwnedFd>)> {
        let (payload, fds) = read_frame(&self.inner).await?;
        let message: T = serde_json::from_slice(&payload)?;
        Ok((message, fds))
    }

    pub async fn send<T>(&self, msg: T) -> std::io::Result<()>
    where
        T: Serialize,
    {
        self.send_with_fds(&msg, &[]).await
    }

    pub async fn receive<T: for<'de> Deserialize<'de>>(&self) -> std::io::Result<T> {
        let (msg, fds) = self.receive_with_fds().await?;
        if !fds.is_empty() {
            tracing::warn!("unexpected fds in receive: {}", fds.len());
        }
        Ok(msg)
    }

    pub fn into_inner(self) -> Socket {
        self.inner.into_inner()
    }
}

async fn send_stream_frame(
    socket: &AsyncFd<Socket>,
    frame: &[u8],
    fds: &[OwnedFd],
) -> std::io::Result<()> {
    let mut written = 0;
    let mut include_fds = !fds.is_empty();
    while written < frame.len() {
        let mut guard = socket.writable().await?;
        let result = guard.try_io(|inner| {
            send_stream_chunk(inner.get_ref(), &frame[written..], fds, include_fds)
        });
        let bytes_written = match result {
            Ok(bytes_written) => bytes_written?,
            Err(_would_block) => continue,
        };
        if bytes_written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "socket closed while sending frame payload",
            ));
        }
        written += bytes_written;
        include_fds = false;
    }
    Ok(())
}

fn send_stream_chunk(
    socket: &Socket,
    frame: &[u8],
    fds: &[OwnedFd],
    include_fds: bool,
) -> std::io::Result<usize> {
    let control = if include_fds {
        make_control_message(fds)?
    } else {
        Vec::new()
    };
    let payload = [IoSlice::new(frame)];
    let msg = if control.is_empty() {
        MsgHdr::new().with_buffers(&payload)
    } else {
        MsgHdr::new().with_buffers(&payload).with_control(&control)
    };
    socket.sendmsg(&msg, 0)
}

pub(crate) struct AsyncDatagramSocket {
    inner: AsyncFd<Socket>,
}

impl AsyncDatagramSocket {
    fn new(socket: Socket) -> std::io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            inner: AsyncFd::new(socket)?,
        })
    }

    pub unsafe fn from_raw_fd(fd: RawFd) -> std::io::Result<Self> {
        Self::new(unsafe { Socket::from_raw_fd(fd) })
    }

    pub fn pair() -> std::io::Result<(Self, Self)> {
        let (server, client) = Socket::pair(Domain::UNIX, Type::DGRAM, None)?;
        Ok((Self::new(server)?, Self::new(client)?))
    }

    pub async fn send_with_fds(&self, data: &[u8], fds: &[OwnedFd]) -> std::io::Result<()> {
        self.inner
            .async_io(Interest::WRITABLE, |socket| {
                send_datagram_bytes(socket, data, fds)
            })
            .await
    }

    pub async fn receive_with_fds(&self) -> std::io::Result<(Vec<u8>, Vec<OwnedFd>)> {
        self.inner
            .async_io(Interest::READABLE, receive_datagram_bytes)
            .await
    }

    pub fn into_inner(self) -> Socket {
        self.inner.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use serde::Serialize;
    use std::os::fd::AsFd;
    use std::os::fd::AsRawFd;
    use tempfile::NamedTempFile;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    struct TestPayload {
        id: i32,
        label: String,
    }

    fn fd_list(count: usize) -> std::io::Result<Vec<OwnedFd>> {
        let file = NamedTempFile::new()?;
        let mut fds = Vec::new();
        for _ in 0..count {
            fds.push(file.as_fd().try_clone_to_owned()?);
        }
        Ok(fds)
    }

    #[tokio::test]
    async fn async_socket_round_trips_payload_and_fds() -> std::io::Result<()> {
        let (server, client) = AsyncSocket::pair()?;
        let payload = TestPayload {
            id: 7,
            label: "round-trip".to_string(),
        };
        let send_fds = fd_list(1)?;

        let receive_task =
            tokio::spawn(async move { server.receive_with_fds::<TestPayload>().await });
        client.send_with_fds(payload.clone(), &send_fds).await?;
        drop(send_fds);

        let (received_payload, received_fds) = receive_task.await.unwrap()?;
        assert_eq!(payload, received_payload);
        assert_eq!(1, received_fds.len());
        let fd_status = unsafe { libc::fcntl(received_fds[0].as_raw_fd(), libc::F_GETFD) };
        assert!(
            fd_status >= 0,
            "expected received file descriptor to be valid, but got {fd_status}",
        );
        Ok(())
    }

    #[tokio::test]
    async fn async_socket_handles_large_payload() -> std::io::Result<()> {
        let (server, client) = AsyncSocket::pair()?;
        let payload = vec![b'A'; 10_000];
        let receive_task = tokio::spawn(async move { server.receive::<Vec<u8>>().await });
        client.send(payload.clone()).await?;
        let received_payload = receive_task.await.unwrap()?;
        assert_eq!(payload, received_payload);
        Ok(())
    }

    #[tokio::test]
    async fn async_datagram_sockets_round_trip_messages() -> std::io::Result<()> {
        let (server, client) = AsyncDatagramSocket::pair()?;
        let data = b"datagram payload".to_vec();
        let send_fds = fd_list(1)?;
        let receive_task = tokio::spawn(async move { server.receive_with_fds().await });

        client.send_with_fds(&data, &send_fds).await?;
        drop(send_fds);

        let (received_bytes, received_fds) = receive_task.await.unwrap()?;
        assert_eq!(data, received_bytes);
        assert_eq!(1, received_fds.len());
        Ok(())
    }

    #[test]
    fn send_datagram_bytes_rejects_excessive_fd_counts() -> std::io::Result<()> {
        let (socket, _peer) = Socket::pair(Domain::UNIX, Type::DGRAM, None)?;
        let fds = fd_list(MAX_FDS_PER_MESSAGE + 1)?;
        let err = send_datagram_bytes(&socket, b"hi", &fds).unwrap_err();
        assert_eq!(std::io::ErrorKind::InvalidInput, err.kind());
        Ok(())
    }

    #[test]
    fn send_stream_chunk_rejects_excessive_fd_counts() -> std::io::Result<()> {
        let (socket, _peer) = Socket::pair(Domain::UNIX, Type::STREAM, None)?;
        let fds = fd_list(MAX_FDS_PER_MESSAGE + 1)?;
        let err = send_stream_chunk(&socket, b"hello", &fds, true).unwrap_err();
        assert_eq!(std::io::ErrorKind::InvalidInput, err.kind());
        Ok(())
    }

    #[test]
    fn encode_length_errors_for_oversized_messages() {
        let err = encode_length(usize::MAX).unwrap_err();
        assert_eq!(std::io::ErrorKind::InvalidInput, err.kind());
    }

    #[tokio::test]
    async fn receive_fails_when_peer_closes_before_header() {
        let (server, client) = AsyncSocket::pair().expect("failed to create socket pair");
        drop(client);
        let err = server
            .receive::<serde_json::Value>()
            .await
            .expect_err("expected read failure");
        assert_eq!(std::io::ErrorKind::UnexpectedEof, err.kind());
    }
}
