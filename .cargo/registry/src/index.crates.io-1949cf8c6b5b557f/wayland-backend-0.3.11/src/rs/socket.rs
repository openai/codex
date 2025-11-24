//! Wayland socket manipulation

use std::collections::VecDeque;
use std::io::{ErrorKind, IoSlice, IoSliceMut, Result as IoResult};
use std::mem::MaybeUninit;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::slice;

use rustix::io::retry_on_intr;
use rustix::net::{
    recvmsg, send, sendmsg, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags,
};

use crate::protocol::{ArgumentType, Message};

use super::wire::{parse_message, write_to_buffers, MessageParseError, MessageWriteError};

/// Maximum number of FD that can be sent in a single socket message
pub const MAX_FDS_OUT: usize = 28;
/// Maximum number of bytes that can be sent in a single socket message
pub const MAX_BYTES_OUT: usize = 4096;

/*
 * Socket
 */

/// A wayland socket
#[derive(Debug)]
pub struct Socket {
    stream: UnixStream,
}

impl Socket {
    /// Send a single message to the socket
    ///
    /// A single socket message can contain several wayland messages
    ///
    /// The `fds` slice should not be longer than `MAX_FDS_OUT`, and the `bytes`
    /// slice should not be longer than `MAX_BYTES_OUT` otherwise the receiving
    /// end may lose some data.
    pub fn send_msg(&self, bytes: &[u8], fds: &[OwnedFd]) -> IoResult<usize> {
        #[cfg(not(target_os = "macos"))]
        let flags = SendFlags::DONTWAIT | SendFlags::NOSIGNAL;
        #[cfg(target_os = "macos")]
        let flags = SendFlags::DONTWAIT;

        if !fds.is_empty() {
            let iov = [IoSlice::new(bytes)];
            let mut cmsg_space =
                vec![MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(fds.len()))];
            let mut cmsg_buffer = SendAncillaryBuffer::new(&mut cmsg_space);
            let fds =
                unsafe { slice::from_raw_parts(fds.as_ptr() as *const BorrowedFd, fds.len()) };
            cmsg_buffer.push(SendAncillaryMessage::ScmRights(fds));
            Ok(retry_on_intr(|| sendmsg(self, &iov, &mut cmsg_buffer, flags))?)
        } else {
            Ok(retry_on_intr(|| send(self, bytes, flags))?)
        }
    }

    /// Receive a single message from the socket
    ///
    /// Return the number of bytes received and the number of Fds received.
    ///
    /// Errors with `WouldBlock` is no message is available.
    ///
    /// A single socket message can contain several wayland messages.
    ///
    /// The `buffer` slice should be at least `MAX_BYTES_OUT` long and the `fds`
    /// slice `MAX_FDS_OUT` long, otherwise some data of the received message may
    /// be lost.
    pub fn rcv_msg(&self, buffer: &mut [u8], fds: &mut VecDeque<OwnedFd>) -> IoResult<usize> {
        #[cfg(not(target_os = "macos"))]
        let flags = RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC;
        #[cfg(target_os = "macos")]
        let flags = RecvFlags::DONTWAIT;

        let mut cmsg_space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS_OUT))];
        let mut cmsg_buffer = RecvAncillaryBuffer::new(&mut cmsg_space);
        let mut iov = [IoSliceMut::new(buffer)];
        let msg = retry_on_intr(|| recvmsg(&self.stream, &mut iov[..], &mut cmsg_buffer, flags))?;

        let received_fds = cmsg_buffer
            .drain()
            .filter_map(|cmsg| match cmsg {
                RecvAncillaryMessage::ScmRights(fds) => Some(fds),
                _ => None,
            })
            .flatten();
        fds.extend(received_fds);
        #[cfg(target_os = "macos")]
        for fd in fds.iter() {
            if let Ok(flags) = rustix::io::fcntl_getfd(fd) {
                let _ = rustix::io::fcntl_setfd(fd, flags | rustix::io::FdFlags::CLOEXEC);
            }
        }
        Ok(msg.bytes)
    }
}

impl From<UnixStream> for Socket {
    fn from(stream: UnixStream) -> Self {
        // macOS doesn't have MSG_NOSIGNAL, but has SO_NOSIGPIPE instead
        #[cfg(target_os = "macos")]
        let _ = rustix::net::sockopt::set_socket_nosigpipe(&stream, true);
        Self { stream }
    }
}

impl AsFd for Socket {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }
}

/*
 * BufferedSocket
 */

/// An adapter around a raw Socket that directly handles buffering and
/// conversion from/to wayland messages
#[derive(Debug)]
pub struct BufferedSocket {
    socket: Socket,
    in_data: Buffer<u8>,
    in_fds: VecDeque<OwnedFd>,
    out_data: Buffer<u8>,
    out_fds: Vec<OwnedFd>,
}

impl BufferedSocket {
    /// Wrap a Socket into a Buffered Socket
    pub fn new(socket: Socket) -> Self {
        Self {
            socket,
            in_data: Buffer::new(2 * MAX_BYTES_OUT), // Incoming buffers are twice as big in order to be
            in_fds: VecDeque::new(),                 // able to store leftover data if needed
            out_data: Buffer::new(MAX_BYTES_OUT),
            out_fds: Vec::new(),
        }
    }

    /// Flush the contents of the outgoing buffer into the socket
    pub fn flush(&mut self) -> IoResult<()> {
        let bytes = self.out_data.get_contents();
        let fds = &self.out_fds;
        let mut written_bytes = 0;
        let mut written_fds = 0;
        let mut ret = Ok(());
        while written_bytes < bytes.len() {
            let mut bytes_to_write = &bytes[written_bytes..];
            let mut fds_to_write = &fds[written_fds..];
            if fds_to_write.len() > MAX_FDS_OUT {
                // While we need to send more than MAX_FDS_OUT fds,
                // send them with separate send_msg calls in MAX_FDS_OUT sized chunks
                // together with 1 byte of normal data.
                // This achieves the same that libwayland does in wl_connection_flush
                // and ensures that all file descriptors are sent
                // before we run out of bytes of normal data to send.
                bytes_to_write = &bytes_to_write[..1];
                fds_to_write = &fds_to_write[..MAX_FDS_OUT];
            }
            if bytes_to_write.len() > MAX_BYTES_OUT {
                // Also ensure the MAX_BYTES_OUT limit, this stays redundant as long
                // as self.out_data has a fixed MAX_BYTES_OUT size and cannot grow.
                bytes_to_write = &bytes_to_write[..MAX_BYTES_OUT];
            }
            match self.socket.send_msg(bytes_to_write, fds_to_write) {
                Ok(0) => {
                    // This branch should be unreachable because
                    // a non-zero sized send or sendmsg should never return 0
                    written_fds += fds_to_write.len();
                    break;
                }
                Ok(count) => {
                    written_bytes += count;
                    written_fds += fds_to_write.len();
                }
                Err(error) => {
                    ret = Err(error);
                    break;
                }
            }
        }
        // Either the flush is done or got an error, so clean up and
        // remove the data and the fds which were sent form the outgoing buffers
        self.out_data.offset(written_bytes);
        self.out_data.move_to_front();
        self.out_fds.drain(..written_fds);
        ret
    }

    // internal method
    //
    // attempts to write a message in the internal out buffers,
    // returns true if successful
    //
    // if false is returned, it means there is not enough space
    // in the buffer
    fn attempt_write_message(&mut self, msg: &Message<u32, RawFd>) -> IoResult<bool> {
        match write_to_buffers(msg, self.out_data.get_writable_storage(), &mut self.out_fds) {
            Ok(bytes_out) => {
                self.out_data.advance(bytes_out);
                Ok(true)
            }
            Err(MessageWriteError::BufferTooSmall) => Ok(false),
            Err(MessageWriteError::DupFdFailed(e)) => Err(e),
        }
    }

    /// Write a message to the outgoing buffer
    ///
    /// This method may flush the internal buffer if necessary (if it is full).
    ///
    /// If the message is too big to fit in the buffer, the error `Error::Sys(E2BIG)`
    /// will be returned.
    pub fn write_message(&mut self, msg: &Message<u32, RawFd>) -> IoResult<()> {
        if !self.attempt_write_message(msg)? {
            // the attempt failed, there is not enough space in the buffer
            // we need to flush it
            if let Err(e) = self.flush() {
                if e.kind() != ErrorKind::WouldBlock {
                    return Err(e);
                }
            }
            if !self.attempt_write_message(msg)? {
                // If this fails again, this means the message is too big
                // to be transmitted at all
                return Err(rustix::io::Errno::TOOBIG.into());
            }
        }
        Ok(())
    }

    /// Try to fill the incoming buffers of this socket, to prepare
    /// a new round of parsing.
    pub fn fill_incoming_buffers(&mut self) -> IoResult<()> {
        // reorganize the buffers
        self.in_data.move_to_front();
        // receive a message
        let in_bytes = {
            let bytes = self.in_data.get_writable_storage();
            self.socket.rcv_msg(bytes, &mut self.in_fds)?
        };
        if in_bytes == 0 {
            // the other end of the socket was closed
            return Err(rustix::io::Errno::PIPE.into());
        }
        // advance the storage
        self.in_data.advance(in_bytes);
        Ok(())
    }

    /// Read and deserialize a single message from the incoming buffers socket
    ///
    /// This method requires one closure that given an object id and an opcode,
    /// must provide the signature of the associated request/event, in the form of
    /// a `&'static [ArgumentType]`.
    pub fn read_one_message<F>(
        &mut self,
        mut signature: F,
    ) -> Result<Message<u32, OwnedFd>, MessageParseError>
    where
        F: FnMut(u32, u16) -> Option<&'static [ArgumentType]>,
    {
        let (msg, read_data) = {
            let data = self.in_data.get_contents();
            if data.len() < 2 * 4 {
                return Err(MessageParseError::MissingData);
            }
            let object_id = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
            let word_2 = u32::from_ne_bytes([data[4], data[5], data[6], data[7]]);
            let opcode = (word_2 & 0x0000_FFFF) as u16;
            if let Some(sig) = signature(object_id, opcode) {
                match parse_message(data, sig, &mut self.in_fds) {
                    Ok((msg, rest_data)) => (msg, data.len() - rest_data.len()),
                    Err(e) => return Err(e),
                }
            } else {
                // no signature found ?
                return Err(MessageParseError::Malformed);
            }
        };

        self.in_data.offset(read_data);

        Ok(msg)
    }
}

impl AsRawFd for BufferedSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

impl AsFd for BufferedSocket {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }
}

/*
 * Buffer
 */
#[derive(Debug)]
struct Buffer<T: Copy> {
    storage: Vec<T>,
    occupied: usize,
    offset: usize,
}

impl<T: Copy + Default> Buffer<T> {
    fn new(size: usize) -> Self {
        Self { storage: vec![T::default(); size], occupied: 0, offset: 0 }
    }

    /// Advance the internal counter of occupied space
    fn advance(&mut self, bytes: usize) {
        self.occupied += bytes;
    }

    /// Advance the read offset of current occupied space
    fn offset(&mut self, bytes: usize) {
        self.offset += bytes;
    }

    /// Clears the contents of the buffer
    ///
    /// This only sets the counter of occupied space back to zero,
    /// allowing previous content to be overwritten.
    #[allow(unused)]
    fn clear(&mut self) {
        self.occupied = 0;
        self.offset = 0;
    }

    /// Get the current contents of the occupied space of the buffer
    fn get_contents(&self) -> &[T] {
        &self.storage[(self.offset)..(self.occupied)]
    }

    /// Get mutable access to the unoccupied space of the buffer
    fn get_writable_storage(&mut self) -> &mut [T] {
        &mut self.storage[(self.occupied)..]
    }

    /// Move the unread contents of the buffer to the front, to ensure
    /// maximal write space availability
    fn move_to_front(&mut self) {
        if self.occupied > self.offset {
            self.storage.copy_within((self.offset)..(self.occupied), 0)
        }
        self.occupied -= self.offset;
        self.offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AllowNull, Argument, ArgumentType, Message};

    use std::ffi::CString;
    use std::os::unix::io::IntoRawFd;

    use smallvec::smallvec;

    fn same_file(a: BorrowedFd, b: BorrowedFd) -> bool {
        let stat1 = rustix::fs::fstat(a).unwrap();
        let stat2 = rustix::fs::fstat(b).unwrap();
        stat1.st_dev == stat2.st_dev && stat1.st_ino == stat2.st_ino
    }

    // check if two messages are equal
    //
    // if arguments contain FDs, check that the fd point to
    // the same file, rather than are the same number.
    fn assert_eq_msgs<Fd: AsRawFd + std::fmt::Debug>(
        msg1: &Message<u32, Fd>,
        msg2: &Message<u32, Fd>,
    ) {
        assert_eq!(msg1.sender_id, msg2.sender_id);
        assert_eq!(msg1.opcode, msg2.opcode);
        assert_eq!(msg1.args.len(), msg2.args.len());
        for (arg1, arg2) in msg1.args.iter().zip(msg2.args.iter()) {
            if let (Argument::Fd(fd1), Argument::Fd(fd2)) = (arg1, arg2) {
                let fd1 = unsafe { BorrowedFd::borrow_raw(fd1.as_raw_fd()) };
                let fd2 = unsafe { BorrowedFd::borrow_raw(fd2.as_raw_fd()) };
                assert!(same_file(fd1, fd2));
            } else {
                assert_eq!(arg1, arg2);
            }
        }
    }

    #[test]
    fn write_read_cycle() {
        let msg = Message {
            sender_id: 42,
            opcode: 7,
            args: smallvec![
                Argument::Uint(3),
                Argument::Fixed(-89),
                Argument::Str(Some(Box::new(CString::new(&b"I like trains!"[..]).unwrap()))),
                Argument::Array(vec![1, 2, 3, 4, 5, 6, 7, 8, 9].into()),
                Argument::Object(88),
                Argument::NewId(56),
                Argument::Int(-25),
            ],
        };

        let (client, server) = ::std::os::unix::net::UnixStream::pair().unwrap();
        let mut client = BufferedSocket::new(Socket::from(client));
        let mut server = BufferedSocket::new(Socket::from(server));

        client.write_message(&msg).unwrap();
        client.flush().unwrap();

        static SIGNATURE: &[ArgumentType] = &[
            ArgumentType::Uint,
            ArgumentType::Fixed,
            ArgumentType::Str(AllowNull::No),
            ArgumentType::Array,
            ArgumentType::Object(AllowNull::No),
            ArgumentType::NewId,
            ArgumentType::Int,
        ];

        server.fill_incoming_buffers().unwrap();

        let ret_msg =
            server
                .read_one_message(|sender_id, opcode| {
                    if sender_id == 42 && opcode == 7 {
                        Some(SIGNATURE)
                    } else {
                        None
                    }
                })
                .unwrap();

        assert_eq_msgs(&msg.map_fd(|fd| fd.as_raw_fd()), &ret_msg.map_fd(IntoRawFd::into_raw_fd));
    }

    #[test]
    fn write_read_cycle_fd() {
        let msg = Message {
            sender_id: 42,
            opcode: 7,
            args: smallvec![
                Argument::Fd(1), // stdin
                Argument::Fd(0), // stdout
            ],
        };

        let (client, server) = ::std::os::unix::net::UnixStream::pair().unwrap();
        let mut client = BufferedSocket::new(Socket::from(client));
        let mut server = BufferedSocket::new(Socket::from(server));

        client.write_message(&msg).unwrap();
        client.flush().unwrap();

        static SIGNATURE: &[ArgumentType] = &[ArgumentType::Fd, ArgumentType::Fd];

        server.fill_incoming_buffers().unwrap();

        let ret_msg =
            server
                .read_one_message(|sender_id, opcode| {
                    if sender_id == 42 && opcode == 7 {
                        Some(SIGNATURE)
                    } else {
                        None
                    }
                })
                .unwrap();
        assert_eq_msgs(&msg.map_fd(|fd| fd.as_raw_fd()), &ret_msg.map_fd(IntoRawFd::into_raw_fd));
    }

    #[test]
    fn write_read_cycle_multiple() {
        let messages = vec![
            Message {
                sender_id: 42,
                opcode: 0,
                args: smallvec![
                    Argument::Int(42),
                    Argument::Str(Some(Box::new(CString::new(&b"I like trains"[..]).unwrap()))),
                ],
            },
            Message {
                sender_id: 42,
                opcode: 1,
                args: smallvec![
                    Argument::Fd(1), // stdin
                    Argument::Fd(0), // stdout
                ],
            },
            Message {
                sender_id: 42,
                opcode: 2,
                args: smallvec![
                    Argument::Uint(3),
                    Argument::Fd(2), // stderr
                ],
            },
        ];

        static SIGNATURES: &[&[ArgumentType]] = &[
            &[ArgumentType::Int, ArgumentType::Str(AllowNull::No)],
            &[ArgumentType::Fd, ArgumentType::Fd],
            &[ArgumentType::Uint, ArgumentType::Fd],
        ];

        let (client, server) = ::std::os::unix::net::UnixStream::pair().unwrap();
        let mut client = BufferedSocket::new(Socket::from(client));
        let mut server = BufferedSocket::new(Socket::from(server));

        for msg in &messages {
            client.write_message(msg).unwrap();
        }
        client.flush().unwrap();

        server.fill_incoming_buffers().unwrap();

        let mut recv_msgs = Vec::new();
        while let Ok(message) = server.read_one_message(|sender_id, opcode| {
            if sender_id == 42 {
                Some(SIGNATURES[opcode as usize])
            } else {
                None
            }
        }) {
            recv_msgs.push(message);
        }
        assert_eq!(recv_msgs.len(), 3);
        for (msg1, msg2) in messages.into_iter().zip(recv_msgs.into_iter()) {
            assert_eq_msgs(&msg1.map_fd(|fd| fd.as_raw_fd()), &msg2.map_fd(IntoRawFd::into_raw_fd));
        }
    }

    #[test]
    fn parse_with_string_len_multiple_of_4() {
        let msg = Message {
            sender_id: 2,
            opcode: 0,
            args: smallvec![
                Argument::Uint(18),
                Argument::Str(Some(Box::new(CString::new(&b"wl_shell"[..]).unwrap()))),
                Argument::Uint(1),
            ],
        };

        let (client, server) = ::std::os::unix::net::UnixStream::pair().unwrap();
        let mut client = BufferedSocket::new(Socket::from(client));
        let mut server = BufferedSocket::new(Socket::from(server));

        client.write_message(&msg).unwrap();
        client.flush().unwrap();

        static SIGNATURE: &[ArgumentType] =
            &[ArgumentType::Uint, ArgumentType::Str(AllowNull::No), ArgumentType::Uint];

        server.fill_incoming_buffers().unwrap();

        let ret_msg =
            server
                .read_one_message(|sender_id, opcode| {
                    if sender_id == 2 && opcode == 0 {
                        Some(SIGNATURE)
                    } else {
                        None
                    }
                })
                .unwrap();

        assert_eq_msgs(&msg.map_fd(|fd| fd.as_raw_fd()), &ret_msg.map_fd(IntoRawFd::into_raw_fd));
    }
}
