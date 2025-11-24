use std::sync::Arc;

use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    ClientMessageEvent, ConnectionExt as _, EventMask, CLIENT_MESSAGE_EVENT,
};

// Regression test for https://github.com/psychon/x11rb/issues/231
#[test]
fn multithread_test() {
    let conn = fake_stream::connect().unwrap();
    let conn = Arc::new(conn);

    // Auxiliary thread: send requests and wait for replies
    let conn1 = conn.clone();
    let join = std::thread::spawn(move || {
        // Bug #231 sometimes caused `reply` to hang forever.
        // Send a huge amount of requests and wait for the reply
        // to check if it hangs at some point.
        for i in 1..=1_000_000 {
            let cookie = conn1.get_input_focus().unwrap();
            cookie.reply().unwrap();

            if (i % 50_000) == 0 {
                eprintln!("{}", i);
            }
        }
        eprintln!("all replies received successfully");

        // Just anything, we don't care
        let event = ClientMessageEvent::new(32, 0, 1u32, [0, 0, 0, 0, 0]);

        conn1
            .send_event(false, 0u32, EventMask::NO_EVENT, event)
            .unwrap();
        conn1.flush().unwrap();
    });

    // Main thread: wait for events until finished
    loop {
        let event = conn.wait_for_raw_event().unwrap();
        if event[0] == CLIENT_MESSAGE_EVENT {
            break;
        }
    }

    join.join().unwrap();
}

/// Implementations of `Read` and `Write` that do enough for the test to work.
mod fake_stream {
    use std::io::{Error, ErrorKind};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Condvar, Mutex};

    use x11rb::errors::ConnectError;
    use x11rb::protocol::xproto::{
        ImageOrder, Setup, CLIENT_MESSAGE_EVENT, GET_INPUT_FOCUS_REQUEST, SEND_EVENT_REQUEST,
    };
    use x11rb::rust_connection::{PollMode, RustConnection, Stream};
    use x11rb::utils::RawFdContainer;

    use x11rb_protocol::SequenceNumber;

    /// Create a new `RustConnection` connected to a fake stream
    pub(crate) fn connect() -> Result<RustConnection<FakeStream>, ConnectError> {
        let setup = Setup {
            status: 0,
            protocol_major_version: 0,
            protocol_minor_version: 0,
            length: 0,
            release_number: 0,
            resource_id_base: 0,
            resource_id_mask: 0xff,
            motion_buffer_size: 0,
            maximum_request_length: 0,
            image_byte_order: ImageOrder::LSB_FIRST,
            bitmap_format_bit_order: ImageOrder::LSB_FIRST,
            bitmap_format_scanline_unit: 0,
            bitmap_format_scanline_pad: 0,
            min_keycode: 0,
            max_keycode: 0,
            vendor: Vec::new(),
            pixmap_formats: Vec::new(),
            roots: Vec::new(),
        };
        let stream = fake_stream();
        RustConnection::for_connected_stream(stream, setup)
    }

    /// Get a pair of fake streams that are connected to each other
    fn fake_stream() -> FakeStream {
        let (send, recv) = channel();
        let pending = Vec::new();
        FakeStream {
            inner: Mutex::new(FakeStreamInner {
                read: FakeStreamRead { recv, pending },
                write: FakeStreamWrite {
                    send,
                    seqno: 0,
                    skip: 0,
                },
            }),
            condvar: Condvar::new(),
        }
    }

    /// A packet that still needs to be read from FakeStreamRead
    #[derive(Debug)]
    enum Packet {
        GetInputFocusReply(SequenceNumber),
        Event,
    }

    impl Packet {
        fn to_raw(&self) -> Vec<u8> {
            match self {
                Packet::GetInputFocusReply(seqno) => {
                    let seqno = (*seqno as u16).to_ne_bytes();
                    let mut reply = vec![0; 32];
                    reply[0] = 1; // This is a reply
                    reply[2..4].copy_from_slice(&seqno);
                    reply
                }
                Packet::Event => {
                    let mut reply = vec![0; 32];
                    reply[0] = CLIENT_MESSAGE_EVENT;
                    reply
                }
            }
        }
    }

    #[derive(Debug)]
    pub(crate) struct FakeStream {
        inner: Mutex<FakeStreamInner>,
        condvar: Condvar,
    }

    #[derive(Debug)]
    struct FakeStreamInner {
        read: FakeStreamRead,
        write: FakeStreamWrite,
    }

    #[derive(Debug)]
    struct FakeStreamRead {
        recv: Receiver<Packet>,
        pending: Vec<u8>,
    }

    #[derive(Debug)]
    pub(crate) struct FakeStreamWrite {
        send: Sender<Packet>,
        seqno: SequenceNumber,
        skip: usize,
    }

    impl Stream for FakeStream {
        fn poll(&self, mode: PollMode) -> std::io::Result<()> {
            if mode.writable() {
                Ok(())
            } else {
                let mut inner = self.inner.lock().unwrap();
                loop {
                    if inner.read.pending.is_empty() {
                        let recv_result = inner.read.recv.try_recv();
                        match recv_result {
                            Ok(packet) => {
                                inner.read.pending.extend(packet.to_raw());
                                return Ok(());
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                inner = self.condvar.wait(inner).unwrap();
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => unreachable!(),
                        }
                    } else {
                        return Ok(());
                    }
                }
            }
        }

        fn read(
            &self,
            buf: &mut [u8],
            _fd_storage: &mut Vec<RawFdContainer>,
        ) -> std::io::Result<usize> {
            let mut inner = self.inner.lock().unwrap();
            if inner.read.pending.is_empty() {
                let recv_result = inner.read.recv.try_recv();
                match recv_result {
                    Ok(packet) => inner.read.pending.extend(packet.to_raw()),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        return Err(Error::new(ErrorKind::WouldBlock, "Would block"));
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => unreachable!(),
                }
            }

            let len = inner.read.pending.len().min(buf.len());
            buf[..len].copy_from_slice(&inner.read.pending[..len]);
            inner.read.pending.drain(..len);
            Ok(len)
        }

        fn write(&self, buf: &[u8], fds: &mut Vec<RawFdContainer>) -> std::io::Result<usize> {
            assert!(fds.is_empty());

            let mut inner = self.inner.lock().unwrap();

            if inner.write.skip > 0 {
                assert_eq!(inner.write.skip, buf.len());
                inner.write.skip = 0;
                return Ok(buf.len());
            }

            inner.write.seqno += 1;
            match buf[0] {
                GET_INPUT_FOCUS_REQUEST => inner
                    .write
                    .send
                    .send(Packet::GetInputFocusReply(inner.write.seqno))
                    .unwrap(),
                SEND_EVENT_REQUEST => inner.write.send.send(Packet::Event).unwrap(),
                _ => unimplemented!(),
            }
            // Compute how much of the package was not yet received
            inner.write.skip = usize::from(u16::from_ne_bytes([buf[2], buf[3]])) * 4 - buf.len();

            self.condvar.notify_all();

            Ok(buf.len())
        }
    }
}
