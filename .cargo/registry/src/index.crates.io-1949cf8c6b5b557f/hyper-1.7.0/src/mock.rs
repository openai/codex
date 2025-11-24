// FIXME: re-implement tests with `async/await`
/*
#[cfg(feature = "runtime")]
use std::collections::HashMap;
use std::cmp;
use std::io::{self, Read, Write};
#[cfg(feature = "runtime")]
use std::sync::{Arc, Mutex};

use bytes::Buf;
use futures::{Async, Poll};
#[cfg(feature = "runtime")]
use futures::Future;
use futures::task::{self, Task};
use tokio_io::{AsyncRead, AsyncWrite};

#[cfg(feature = "runtime")]
use crate::client::connect::{Connect, Connected, Destination};



#[cfg(feature = "runtime")]
pub struct Duplex {
    inner: Arc<Mutex<DuplexInner>>,
}

#[cfg(feature = "runtime")]
struct DuplexInner {
    handle_read_task: Option<Task>,
    read: AsyncIo<MockCursor>,
    write: AsyncIo<MockCursor>,
}

#[cfg(feature = "runtime")]
impl Duplex {
    pub(crate) fn channel() -> (Duplex, DuplexHandle) {
        let mut inner = DuplexInner {
            handle_read_task: None,
            read: AsyncIo::new_buf(Vec::new(), 0),
            write: AsyncIo::new_buf(Vec::new(), std::usize::MAX),
        };

        inner.read.park_tasks(true);
        inner.write.park_tasks(true);

        let inner = Arc::new(Mutex::new(inner));

        let duplex = Duplex {
            inner: inner.clone(),
        };
        let handle = DuplexHandle {
            inner: inner,
        };

        (duplex, handle)
    }
}

#[cfg(feature = "runtime")]
impl Read for Duplex {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.lock().unwrap().read.read(buf)
    }
}

#[cfg(feature = "runtime")]
impl Write for Duplex {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        let ret = inner.write.write(buf);
        if let Some(task) = inner.handle_read_task.take() {
            trace!("waking DuplexHandle read");
            task.notify();
        }
        ret
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().write.flush()
    }
}

#[cfg(feature = "runtime")]
impl AsyncRead for Duplex {
}

#[cfg(feature = "runtime")]
impl AsyncWrite for Duplex {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        Ok(().into())
    }

    fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.handle_read_task.take() {
            task.notify();
        }
        inner.write.write_buf(buf)
    }
}

#[cfg(feature = "runtime")]
pub struct DuplexHandle {
    inner: Arc<Mutex<DuplexInner>>,
}

#[cfg(feature = "runtime")]
impl DuplexHandle {
    pub fn read(&self, buf: &mut [u8]) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        assert!(buf.len() >= inner.write.inner.len());
        if inner.write.inner.is_empty() {
            trace!("DuplexHandle read parking");
            inner.handle_read_task = Some(task::current());
            return Ok(Async::NotReady);
        }
        inner.write.read(buf).map(Async::Ready)
    }

    pub fn write(&self, bytes: &[u8]) -> Poll<usize, io::Error> {
        let mut inner = self.inner.lock().unwrap();
        assert_eq!(inner.read.inner.pos, 0);
        assert_eq!(inner.read.inner.vec.len(), 0, "write but read isn't empty");
        inner
            .read
            .inner
            .vec
            .extend(bytes);
        inner.read.block_in(bytes.len());
        Ok(Async::Ready(bytes.len()))
    }
}

#[cfg(feature = "runtime")]
impl Drop for DuplexHandle {
    fn drop(&mut self) {
        trace!("mock duplex handle drop");
        if !::std::thread::panicking() {
            let mut inner = self.inner.lock().unwrap();
            inner.read.close();
            inner.write.close();
        }
    }
}

#[cfg(feature = "runtime")]
type BoxedConnectFut = Box<dyn Future<Item=(Duplex, Connected), Error=io::Error> + Send>;

#[cfg(feature = "runtime")]
#[derive(Clone)]
pub struct MockConnector {
    mocks: Arc<Mutex<MockedConnections>>,
}

#[cfg(feature = "runtime")]
struct MockedConnections(HashMap<String, Vec<BoxedConnectFut>>);

#[cfg(feature = "runtime")]
impl MockConnector {
    pub fn new() -> MockConnector {
        MockConnector {
            mocks: Arc::new(Mutex::new(MockedConnections(HashMap::new()))),
        }
    }

    pub fn mock(&mut self, key: &str) -> DuplexHandle {
        use futures::future;
        self.mock_fut(key, future::ok::<_, ()>(()))
    }

    pub fn mock_fut<F>(&mut self, key: &str, fut: F) -> DuplexHandle
    where
        F: Future + Send + 'static,
    {
        self.mock_opts(key, Connected::new(), fut)
    }

    pub fn mock_opts<F>(&mut self, key: &str, connected: Connected, fut: F) -> DuplexHandle
    where
        F: Future + Send + 'static,
    {
        let key = key.to_owned();

        let (duplex, handle) = Duplex::channel();

        let fut = Box::new(fut.then(move |_| {
            trace!("MockConnector mocked fut ready");
            Ok((duplex, connected))
        }));
        self.mocks.lock().unwrap().0.entry(key)
            .or_insert(Vec::new())
            .push(fut);

        handle
    }
}

#[cfg(feature = "runtime")]
impl Connect for MockConnector {
    type Transport = Duplex;
    type Error = io::Error;
    type Future = BoxedConnectFut;

    fn connect(&self, dst: Destination) -> Self::Future {
        trace!("mock connect: {:?}", dst);
        let key = format!("{}://{}{}", dst.scheme(), dst.host(), if let Some(port) = dst.port() {
            format!(":{}", port)
        } else {
            "".to_owned()
        });
        let mut mocks = self.mocks.lock().unwrap();
        let mocks = mocks.0.get_mut(&key)
            .expect(&format!("unknown mocks uri: {}", key));
        assert!(!mocks.is_empty(), "no additional mocks for {}", key);
        mocks.remove(0)
    }
}


#[cfg(feature = "runtime")]
impl Drop for MockedConnections {
    fn drop(&mut self) {
        if !::std::thread::panicking() {
            for (key, mocks) in self.0.iter() {
                assert_eq!(
                    mocks.len(),
                    0,
                    "not all mocked connects for {:?} were used",
                    key,
                );
            }
        }
    }
}
*/
