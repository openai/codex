use crate::error::Error;
use crate::pool::PoolReturner;
use crate::stream::{remote_addr_for_test, ReadOnlyStream, Stream};
use crate::unit::Unit;
use crate::ReadWrite;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Cursor, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

mod agent_test;
mod body_read;
mod body_send;
mod query_string;
mod range;
mod redirect;
mod simple;
mod timeout;

type RequestHandler = dyn Fn(&Unit) -> Result<Stream, Error> + Send + 'static;

#[allow(clippy::type_complexity)]
pub(crate) static TEST_HANDLERS: Lazy<Arc<Mutex<HashMap<String, Box<RequestHandler>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

pub(crate) fn set_handler<H>(path: &str, handler: H)
where
    H: Fn(&Unit) -> Result<Stream, Error> + Send + 'static,
{
    let path = path.to_string();
    let handler = Box::new(handler);
    // See `resolve_handler` below for why poisoning isn't necessary.
    let mut handlers = match TEST_HANDLERS.lock() {
        Ok(h) => h,
        Err(poison) => poison.into_inner(),
    };
    handlers.insert(path, handler);
}

#[allow(clippy::write_with_newline)]
pub(crate) fn make_response(
    status: u16,
    status_text: &str,
    headers: Vec<&str>,
    mut body: Vec<u8>,
) -> Result<Stream, Error> {
    let mut buf: Vec<u8> = vec![];
    write!(&mut buf, "HTTP/1.1 {} {}\r\n", status, status_text).ok();
    for hstr in headers.iter() {
        write!(&mut buf, "{}\r\n", hstr).ok();
    }
    write!(&mut buf, "\r\n").ok();
    buf.append(&mut body);
    Ok(Stream::new(
        ReadOnlyStream::new(buf),
        remote_addr_for_test(),
        PoolReturner::none(),
    ))
}

pub(crate) fn resolve_handler(unit: &Unit) -> Result<Stream, Error> {
    let path = unit.url.path();
    // The only way this can panic is if
    // 1. `remove(path).unwrap()` panics, in which case the HANDLERS haven't been modified.
    // 2. `make_hash` for `handlers.insert` panics (in `set_handler`), in which case the HANDLERS haven't been modified.
    // In all cases, another test will fail as a result, so it's ok to continue other tests in parallel.
    let mut handlers = match TEST_HANDLERS.lock() {
        Ok(h) => h,
        Err(poison) => poison.into_inner(),
    };
    let handler = handlers.remove(path)
        .unwrap_or_else(|| panic!("call make_response(\"{}\") before fetching it in tests (or if you did make it, avoid fetching it more than once)", path));
    drop(handlers);
    handler(unit)
}

#[derive(Default, Clone)]
pub(crate) struct Recorder {
    contents: Arc<Mutex<Vec<u8>>>,
}

impl Recorder {
    fn register(path: &str) -> Self {
        let recorder = Recorder::default();
        let recorder2 = recorder.clone();
        set_handler(path, move |_unit| Ok(recorder.stream()));
        recorder2
    }

    #[cfg(feature = "charset")]
    fn to_vec(self) -> Vec<u8> {
        self.contents.lock().unwrap().clone()
    }

    fn contains(&self, s: &str) -> bool {
        std::str::from_utf8(&self.contents.lock().unwrap())
            .unwrap()
            .contains(s)
    }

    fn stream(&self) -> Stream {
        let cursor = Cursor::new(b"HTTP/1.1 200 OK\r\n\r\n");
        Stream::new(
            TestStream::new(cursor, self.clone()),
            remote_addr_for_test(),
            PoolReturner::none(),
        )
    }
}

impl Write for Recorder {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.contents.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) struct TestStream(Box<dyn Read + Send + Sync>, Box<dyn Write + Send + Sync>);

impl TestStream {
    #[cfg(test)]
    pub(crate) fn new(
        response: impl Read + Send + Sync + 'static,
        recorder: impl Write + Send + Sync + 'static,
    ) -> Self {
        Self(Box::new(response), Box::new(recorder))
    }
}

impl ReadWrite for TestStream {
    fn socket(&self) -> Option<&TcpStream> {
        None
    }
}

impl Read for TestStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for TestStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.1.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Debug for TestStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TestStream").finish()
    }
}
