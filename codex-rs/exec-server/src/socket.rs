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

const MAX_MESSAGE_SIZE: usize = 64 * 1024;
const MAX_FDS_PER_MESSAGE: usize = 16;

#[derive(Debug)]
struct ReceivedMessage {
    data: Vec<u8>,
    #[allow(dead_code)]
    fds: Vec<OwnedFd>,
}

fn assume_init(buf: &[MaybeUninit<u8>]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(buf.as_ptr().cast(), buf.len()) }
}
fn control_space_for_fds(count: usize) -> usize {
    unsafe { libc::CMSG_SPACE((count * size_of::<RawFd>()) as _) as usize }
}
fn extract_fds(control: &mut [MaybeUninit<u8>], len: usize) -> std::io::Result<Vec<OwnedFd>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut fds = Vec::new();
    let mut hdr: libc::msghdr = unsafe { std::mem::zeroed() };
    hdr.msg_control = control.as_mut_ptr().cast();
    hdr.msg_controllen = len as _;

    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&hdr) };
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
    Ok(fds)
}

fn receive_message(socket: &Socket) -> std::io::Result<ReceivedMessage> {
    let mut data = [MaybeUninit::<u8>::uninit(); MAX_MESSAGE_SIZE];
    let mut control = vec![MaybeUninit::<u8>::uninit(); control_space_for_fds(MAX_FDS_PER_MESSAGE)];
    let (received, control_len) = {
        let mut bufs = [MaybeUninitSlice::new(&mut data)];
        let mut msg = MsgHdrMut::new()
            .with_buffers(&mut bufs)
            .with_control(&mut control);
        let received = socket.recvmsg(&mut msg, 0)?;
        (received, msg.control_len())
    };

    let message = assume_init(&data[..received]).to_vec();
    let fds = extract_fds(&mut control, control_len)?;
    Ok(ReceivedMessage { data: message, fds })
}
pub(crate) fn receive_json_message<T: for<'de> Deserialize<'de>>(
    socket: &Socket,
) -> std::io::Result<(T, Vec<OwnedFd>)> {
    let ReceivedMessage { data, fds } = receive_message(socket)?;
    let message: T = serde_json::from_slice(&data)?;
    Ok((message, fds))
}
fn send_message_bytes(socket: &Socket, data: &[u8], fds: &[OwnedFd]) -> std::io::Result<()> {
    if fds.len() > MAX_FDS_PER_MESSAGE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("too many fds: {}", fds.len()),
        ));
    }
    let mut control = vec![0u8; control_space_for_fds(fds.len())];
    unsafe {
        let cmsg = control.as_mut_ptr().cast::<libc::cmsghdr>();
        (*cmsg).cmsg_len = libc::CMSG_LEN(size_of::<RawFd>() as c_uint * fds.len() as c_uint) as _;
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        let data_ptr = libc::CMSG_DATA(cmsg).cast::<RawFd>();
        for (i, fd) in fds.iter().enumerate() {
            data_ptr.add(i).write(fd.as_raw_fd());
        }
    }

    let payload = [IoSlice::new(data)];
    let msg = MsgHdr::new().with_buffers(&payload).with_control(&control);
    socket.sendmsg(&msg, 0)?;
    Ok(())
}

pub(crate) fn send_json_message<T: Serialize>(
    socket: &Socket,
    msg: T,
    fds: &[OwnedFd],
) -> std::io::Result<()> {
    let data = serde_json::to_vec(&msg)?;
    send_message_bytes(socket, &data, fds)
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

    pub unsafe fn from_raw_fd(fd: RawFd) -> std::io::Result<AsyncSocket> {
        AsyncSocket::new(unsafe { Socket::from_raw_fd(fd) })
    }

    pub fn from_fd(fd: OwnedFd) -> std::io::Result<AsyncSocket> {
        AsyncSocket::new(Socket::from(fd))
    }

    pub fn pair() -> std::io::Result<(AsyncSocket, AsyncSocket)> {
        let (server, client) = Socket::pair(Domain::UNIX, Type::DGRAM, None)?;
        Ok((AsyncSocket::new(server)?, AsyncSocket::new(client)?))
    }

    pub async fn send_with_fds<T: Serialize>(
        &self,
        msg: T,
        fds: &[OwnedFd],
    ) -> std::io::Result<()> {
        self.inner
            .async_io(Interest::WRITABLE, |socket| {
                send_json_message(socket, &msg, fds)
            })
            .await
    }

    pub async fn receive_with_fds<T: for<'de> Deserialize<'de>>(
        &self,
    ) -> std::io::Result<(T, Vec<OwnedFd>)> {
        self.inner
            .async_io(Interest::READABLE, |socket| receive_json_message(socket))
            .await
    }

    pub async fn send<T>(&self, msg: T) -> std::io::Result<()>
    where
        T: Serialize,
    {
        self.send_with_fds(&msg, &[]).await
    }

    pub async fn receive<T: for<'de> Deserialize<'de>>(&self) -> std::io::Result<T> {
        let (msg, _) = self.receive_with_fds().await?;
        Ok(msg)
    }

    pub fn into_inner(self) -> Socket {
        self.inner.into_inner()
    }
}
