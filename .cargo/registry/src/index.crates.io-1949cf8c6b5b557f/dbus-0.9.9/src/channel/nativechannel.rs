use crate::{Error, Message};
use std::time::Duration;
use super::{BusType, Watch};
use futures_util::lock::Mutex as FMutex;
use std::sync::Mutex;
use futures_executor::block_on;
use dbus_native_channel::address;
use std::error::Error as stdError;
use futures_util::io as fio;
use std::pin::Pin;
use std::os::unix::net::UnixStream;
use std::collections::VecDeque;
use std::os::unix::io::{RawFd, AsRawFd};

/// Low-level connection - handles read/write to the socket
///
/// You probably do not need to worry about this as you would typically
/// use the various blocking and non-blocking "Connection" structs instead.
///
/// This version avoids any dependency on the C dbus library, making it possible
/// to use with async rust code etc.
//#[derive(Debug)]
pub struct Channel {
    unique_name: Option<crate::strings::BusName<'static>>,
    out_queue: Mutex<(u32, VecDeque<Message>)>,
    in_queue: Mutex<VecDeque<Message>>,
    reader: FMutex<Pin<Box<dyn fio::AsyncBufRead + Send>>>,
    writer: FMutex<Pin<Box<dyn fio::AsyncWrite + Send>>>,
    raw_fd: RawFd,
    unix_fd: bool,
}

async fn do_auth<W: fio::AsyncWrite + std::marker::Unpin, R: fio::AsyncBufRead + std::marker::Unpin>(r: &mut R, w: &mut W) -> Result<bool, Box<dyn stdError>> {
    use dbus_native_channel::authentication::Authentication;
    use fio::{AsyncWriteExt, AsyncBufReadExt};
    let (mut auth, s) = Authentication::new(true);
    w.write_all(s.as_bytes()).await?;
    loop {
        let mut v = vec!();
        r.read_until(b'\n', &mut v).await?;
        let s = auth.handle(&v)?;
        w.write_all(s.as_bytes()).await?;
        if let Authentication::Begin(unixfd) = &auth {
            return Ok(*unixfd)
        }
    }
}

impl Channel {

    /// Creates a new D-Bus connection.
    ///
    /// Blocking: until the connection is up and running.
    pub fn get_private(bus: BusType) -> Result<Channel, Error> {
        block_on(async {
            Self::get_private_async(bus, |s| {
                // s.set_nonblocking(true).unwrap();
                let s2 = s.try_clone().unwrap();
                let r = fio::AllowStdIo::new(std::io::BufReader::new(s));
                let w = fio::AllowStdIo::new(s2);
                (r, w)
            }).await.map_err(|x| Error::new_failed(&x.to_string()))
        })
    }

    /// Creates a new D-Bus connection without blocking.
    pub async fn get_private_async<R, W, F>(bus: BusType, f: F) -> Result<Channel, Box<dyn stdError>>
    where
        R: fio::AsyncBufRead + 'static + Send,
        W: fio::AsyncWrite + 'static + Send,
        F: FnOnce(UnixStream) -> (R, W) {
        let addr = match bus {
            BusType::Starter => address::read_starter_address(),
            BusType::Session => address::read_session_address(),
            BusType::System => address::read_system_address(),
        }?;
        // Tokio does not do this any less blocking than we do here.
        let stream = address::connect_blocking(&addr)?;
        let raw_fd = stream.as_raw_fd();
        let (r, w) = f(stream);
        let (mut r, mut w) = (Box::pin(r), Box::pin(w));
        let unix_fd = do_auth(&mut r, &mut w).await?;
        // dbg!(&unix_fd);
        let mut c = Channel {
            unique_name: None,
            raw_fd,
            unix_fd,
            in_queue: Default::default(),
            out_queue: Default::default(),
            reader: FMutex::new(r),
            writer: FMutex::new(w),
        };
        let msg = Message::new_method_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "Hello")?;
        let r = c.send_with_reply_async(msg).await?;
        let s: String = r.read1()?;
        c.unique_name = Some(crate::strings::BusName::new(s)?);
        Ok(c)
    }

    /// Puts a message into the out queue, without trying to send it.
    ///
    /// Returns a serial number than can be used to match against a reply.
    ///
    /// Note:
    /// Call "flush" to flush the out queue.
    pub fn send(&self, mut msg: Message) -> Result<u32, ()> {
        let mut q = self.out_queue.lock().unwrap();
        q.0 += 1;
        let serial = q.0;
        msg.set_serial(serial);
        q.1.push_back(msg);
        Ok(serial)
    }

    async fn write_message(&self, msg: Message) -> Result<(), fio::Error> {
        use fio::AsyncWriteExt;
        let mut v = vec!();
        let _: Result<(), ()> = msg.marshal(|b| {
            // At some point we might be able to skip this copy. For now let's just try to get
            // things up and running.
            v.extend_from_slice(b);
            Ok(())
        });
        let mut writer = self.writer.lock().await;
        writer.write_all(&v).await
    }

    async fn read_message(&self) -> Result<Message, Error> {
        use fio::AsyncReadExt;
        let mut v = vec![0; 16];
        let mut reader = self.reader.lock().await;
        reader.read_exact(&mut v).await.map_err(|e| Error::new_failed(&e.to_string()))?;
        let count = Message::demarshal_bytes_needed(&v).map_err(|_| Error::new_failed("Protocol error"))?;
        // dbg!(&v, &count);
        v.resize(count, 0);
        reader.read_exact(&mut v[16..]).await.map_err(|e| Error::new_failed(&e.to_string()))?;
        // dbg!(&v, &count);
        Message::demarshal(&v)
    }

    /// Note: Calling this from more than one thread in parallel might result in reordering
    /// as well as mutex blocking.
    pub async fn flush_async(&self) -> Result<(), Error> {
        loop {
            let msg = {
                let mut q = self.out_queue.lock().unwrap();
                if let Some(msg) = q.1.pop_front() { msg } else { return Ok(()) }
            };
            self.write_message(msg).await.map_err(|e| Error::new_failed(&e.to_string()))?;
        }
    }

    /// Flush the queue of outgoing messages.
    ///
    /// Blocking: until the outgoing queue is empty.
    pub fn flush(&self) {
        block_on(async {
            self.flush_async().await.unwrap();
        });
    }

    /// Removes a message from the incoming queue, or waits until timeout if the queue is empty.
    ///
    pub fn blocking_pop_message(&self, _timeout: Duration) -> Result<Option<Message>, Error> {
        if let Some(msg) = self.pop_message() { return Ok(Some(msg)) }
        // TODO: Timeout
        block_on(async {
            let msg = self.read_message().await?;
            Ok(Some(msg))
        })
    }

    /// This function does nothing. Provided for compatibility
    pub fn set_watch_enabled(&mut self, _enable: bool) {
    }

    /// Gets the file descriptor to listen for read/write.
    pub fn watch(&self) -> Watch {
        Watch {
            fd: self.as_raw_fd(),
            read: true,
            write: false,
        }
    }

    /// Gets whether the connection is currently open.
    pub fn is_connected(&self) -> bool {
        // TODO
        true
    }

    /// Removes a message from the incoming queue, or returns None if the queue is empty.
    pub fn pop_message(&self) -> Option<Message> {
        let mut q = self.in_queue.lock().unwrap();
        q.pop_front()
    }

    /// Read and write to the connection.
    ///
    /// Incoming messages are put in the internal queue, outgoing messages are written.
    ///
    /// Blocking: If there are no messages, for up to timeout, or forever if timeout is None.
    /// For non-blocking behaviour, set timeout to Some(0).
    pub fn read_write(&self, _timeout: Option<Duration>) -> Result<(), ()> {
        block_on(async {
            // TODO: Timeout
            self.flush_async().await.map_err(|_| ())?;
            let msg = self.read_message().await.map_err(|_| ())?;
            let mut q = self.in_queue.lock().unwrap();
            q.push_back(msg);
            Ok(())
        })
    }

    /// Sends a message over the D-Bus and waits for a reply. This is used for method calls.
    ///
    /// Blocking: until a reply is received or the timeout expires.
    ///
    /// Note: In case of an error reply, this is returned as an Err(), not as a Ok(Message) with the error type.
    ///
    /// Note: In case pop_message and send_with_reply_and_block is called in parallel from different threads,
    /// they might race to retrieve the reply message from the internal queue.
    pub fn send_with_reply_and_block(&self, msg: Message, _timeout: Duration) -> Result<Message, Error> {
        block_on(async {
            // TODO: Timeout
            self.send_with_reply_async(msg).await
        })
    }

    pub async fn send_with_reply_async(&self, msg: Message) -> Result<Message, Error> {
        let serial = self.send(msg).map_err(|_| Error::new_failed("Failed to send message"))?;
        self.flush_async().await?;
        loop {
            let msg = self.read_message().await?;
            if msg.get_reply_serial() == Some(serial) {
                return Ok(msg);
            }
            let mut q = self.in_queue.lock().unwrap();
            q.push_back(msg)
        }
    }

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> Option<&str> {
        self.unique_name.as_ref().map(|x| &**x)
    }
}

impl AsRawFd for Channel {
    fn as_raw_fd(&self) -> RawFd { self.raw_fd }
}
