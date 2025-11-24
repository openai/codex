use std::cmp;
use std::fmt;
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::rt::{Read, ReadBuf, Write};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_core::ready;

use super::{Http1Transaction, ParseContext, ParsedMessage};
use crate::common::buf::BufList;

/// The initial buffer size allocated before trying to read from IO.
pub(crate) const INIT_BUFFER_SIZE: usize = 8192;

/// The minimum value that can be set to max buffer size.
pub(crate) const MINIMUM_MAX_BUFFER_SIZE: usize = INIT_BUFFER_SIZE;

/// The default maximum read buffer size. If the buffer gets this big and
/// a message is still not complete, a `TooLarge` error is triggered.
// Note: if this changes, update server::conn::Http::max_buf_size docs.
pub(crate) const DEFAULT_MAX_BUFFER_SIZE: usize = 8192 + 4096 * 100;

/// The maximum number of distinct `Buf`s to hold in a list before requiring
/// a flush. Only affects when the buffer strategy is to queue buffers.
///
/// Note that a flush can happen before reaching the maximum. This simply
/// forces a flush if the queue gets this big.
const MAX_BUF_LIST_BUFFERS: usize = 16;

pub(crate) struct Buffered<T, B> {
    flush_pipeline: bool,
    io: T,
    partial_len: Option<usize>,
    read_blocked: bool,
    read_buf: BytesMut,
    read_buf_strategy: ReadStrategy,
    write_buf: WriteBuf<B>,
}

impl<T, B> fmt::Debug for Buffered<T, B>
where
    B: Buf,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("read_buf", &self.read_buf)
            .field("write_buf", &self.write_buf)
            .finish()
    }
}

impl<T, B> Buffered<T, B>
where
    T: Read + Write + Unpin,
    B: Buf,
{
    pub(crate) fn new(io: T) -> Buffered<T, B> {
        let strategy = if io.is_write_vectored() {
            WriteStrategy::Queue
        } else {
            WriteStrategy::Flatten
        };
        let write_buf = WriteBuf::new(strategy);
        Buffered {
            flush_pipeline: false,
            io,
            partial_len: None,
            read_blocked: false,
            read_buf: BytesMut::with_capacity(0),
            read_buf_strategy: ReadStrategy::default(),
            write_buf,
        }
    }

    #[cfg(feature = "server")]
    pub(crate) fn set_flush_pipeline(&mut self, enabled: bool) {
        debug_assert!(!self.write_buf.has_remaining());
        self.flush_pipeline = enabled;
        if enabled {
            self.set_write_strategy_flatten();
        }
    }

    pub(crate) fn set_max_buf_size(&mut self, max: usize) {
        assert!(
            max >= MINIMUM_MAX_BUFFER_SIZE,
            "The max_buf_size cannot be smaller than {}.",
            MINIMUM_MAX_BUFFER_SIZE,
        );
        self.read_buf_strategy = ReadStrategy::with_max(max);
        self.write_buf.max_buf_size = max;
    }

    #[cfg(feature = "client")]
    pub(crate) fn set_read_buf_exact_size(&mut self, sz: usize) {
        self.read_buf_strategy = ReadStrategy::Exact(sz);
    }

    pub(crate) fn set_write_strategy_flatten(&mut self) {
        // this should always be called only at construction time,
        // so this assert is here to catch myself
        debug_assert!(self.write_buf.queue.bufs_cnt() == 0);
        self.write_buf.set_strategy(WriteStrategy::Flatten);
    }

    pub(crate) fn set_write_strategy_queue(&mut self) {
        // this should always be called only at construction time,
        // so this assert is here to catch myself
        debug_assert!(self.write_buf.queue.bufs_cnt() == 0);
        self.write_buf.set_strategy(WriteStrategy::Queue);
    }

    pub(crate) fn read_buf(&self) -> &[u8] {
        self.read_buf.as_ref()
    }

    #[cfg(test)]
    #[cfg(feature = "nightly")]
    pub(super) fn read_buf_mut(&mut self) -> &mut BytesMut {
        &mut self.read_buf
    }

    /// Return the "allocated" available space, not the potential space
    /// that could be allocated in the future.
    fn read_buf_remaining_mut(&self) -> usize {
        self.read_buf.capacity() - self.read_buf.len()
    }

    /// Return whether we can append to the headers buffer.
    ///
    /// Reasons we can't:
    /// - The write buf is in queue mode, and some of the past body is still
    ///   needing to be flushed.
    pub(crate) fn can_headers_buf(&self) -> bool {
        !self.write_buf.queue.has_remaining()
    }

    pub(crate) fn headers_buf(&mut self) -> &mut Vec<u8> {
        let buf = self.write_buf.headers_mut();
        &mut buf.bytes
    }

    pub(super) fn write_buf(&mut self) -> &mut WriteBuf<B> {
        &mut self.write_buf
    }

    pub(crate) fn buffer<BB: Buf + Into<B>>(&mut self, buf: BB) {
        self.write_buf.buffer(buf)
    }

    pub(crate) fn can_buffer(&self) -> bool {
        self.flush_pipeline || self.write_buf.can_buffer()
    }

    pub(crate) fn consume_leading_lines(&mut self) {
        if !self.read_buf.is_empty() {
            let mut i = 0;
            while i < self.read_buf.len() {
                match self.read_buf[i] {
                    b'\r' | b'\n' => i += 1,
                    _ => break,
                }
            }
            self.read_buf.advance(i);
        }
    }

    pub(super) fn parse<S>(
        &mut self,
        cx: &mut Context<'_>,
        parse_ctx: ParseContext<'_>,
    ) -> Poll<crate::Result<ParsedMessage<S::Incoming>>>
    where
        S: Http1Transaction,
    {
        loop {
            match super::role::parse_headers::<S>(
                &mut self.read_buf,
                self.partial_len,
                ParseContext {
                    cached_headers: parse_ctx.cached_headers,
                    req_method: parse_ctx.req_method,
                    h1_parser_config: parse_ctx.h1_parser_config.clone(),
                    h1_max_headers: parse_ctx.h1_max_headers,
                    preserve_header_case: parse_ctx.preserve_header_case,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: parse_ctx.preserve_header_order,
                    h09_responses: parse_ctx.h09_responses,
                    #[cfg(feature = "client")]
                    on_informational: parse_ctx.on_informational,
                },
            )? {
                Some(msg) => {
                    debug!("parsed {} headers", msg.head.headers.len());
                    self.partial_len = None;
                    return Poll::Ready(Ok(msg));
                }
                None => {
                    let max = self.read_buf_strategy.max();
                    let curr_len = self.read_buf.len();
                    if curr_len >= max {
                        debug!("max_buf_size ({}) reached, closing", max);
                        return Poll::Ready(Err(crate::Error::new_too_large()));
                    }
                    if curr_len > 0 {
                        trace!("partial headers; {} bytes so far", curr_len);
                        self.partial_len = Some(curr_len);
                    } else {
                        // 1xx gobled some bytes
                        self.partial_len = None;
                    }
                }
            }
            if ready!(self.poll_read_from_io(cx)).map_err(crate::Error::new_io)? == 0 {
                trace!("parse eof");
                return Poll::Ready(Err(crate::Error::new_incomplete()));
            }
        }
    }

    pub(crate) fn poll_read_from_io(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        self.read_blocked = false;
        let next = self.read_buf_strategy.next();
        if self.read_buf_remaining_mut() < next {
            self.read_buf.reserve(next);
        }

        // SAFETY: ReadBuf and poll_read promise not to set any uninitialized
        // bytes onto `dst`.
        let dst = unsafe { self.read_buf.chunk_mut().as_uninit_slice_mut() };
        let mut buf = ReadBuf::uninit(dst);
        match Pin::new(&mut self.io).poll_read(cx, buf.unfilled()) {
            Poll::Ready(Ok(_)) => {
                let n = buf.filled().len();
                trace!("received {} bytes", n);
                unsafe {
                    // Safety: we just read that many bytes into the
                    // uninitialized part of the buffer, so this is okay.
                    // @tokio pls give me back `poll_read_buf` thanks
                    self.read_buf.advance_mut(n);
                }
                self.read_buf_strategy.record(n);
                Poll::Ready(Ok(n))
            }
            Poll::Pending => {
                self.read_blocked = true;
                Poll::Pending
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    pub(crate) fn into_inner(self) -> (T, Bytes) {
        (self.io, self.read_buf.freeze())
    }

    pub(crate) fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    pub(crate) fn is_read_blocked(&self) -> bool {
        self.read_blocked
    }

    pub(crate) fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.flush_pipeline && !self.read_buf.is_empty() {
            Poll::Ready(Ok(()))
        } else if self.write_buf.remaining() == 0 {
            Pin::new(&mut self.io).poll_flush(cx)
        } else {
            if let WriteStrategy::Flatten = self.write_buf.strategy {
                return self.poll_flush_flattened(cx);
            }

            const MAX_WRITEV_BUFS: usize = 64;
            loop {
                let n = {
                    let mut iovs = [IoSlice::new(&[]); MAX_WRITEV_BUFS];
                    let len = self.write_buf.chunks_vectored(&mut iovs);
                    ready!(Pin::new(&mut self.io).poll_write_vectored(cx, &iovs[..len]))?
                };
                // TODO(eliza): we have to do this manually because
                // `poll_write_buf` doesn't exist in Tokio 0.3 yet...when
                // `poll_write_buf` comes back, the manual advance will need to leave!
                self.write_buf.advance(n);
                debug!("flushed {} bytes", n);
                if self.write_buf.remaining() == 0 {
                    break;
                } else if n == 0 {
                    trace!(
                        "write returned zero, but {} bytes remaining",
                        self.write_buf.remaining()
                    );
                    return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
                }
            }
            Pin::new(&mut self.io).poll_flush(cx)
        }
    }

    /// Specialized version of `flush` when strategy is Flatten.
    ///
    /// Since all buffered bytes are flattened into the single headers buffer,
    /// that skips some bookkeeping around using multiple buffers.
    fn poll_flush_flattened(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let n = ready!(Pin::new(&mut self.io).poll_write(cx, self.write_buf.headers.chunk()))?;
            debug!("flushed {} bytes", n);
            self.write_buf.headers.advance(n);
            if self.write_buf.headers.remaining() == 0 {
                self.write_buf.headers.reset();
                break;
            } else if n == 0 {
                trace!(
                    "write returned zero, but {} bytes remaining",
                    self.write_buf.remaining()
                );
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }
        Pin::new(&mut self.io).poll_flush(cx)
    }

    #[cfg(test)]
    fn flush(&mut self) -> impl std::future::Future<Output = io::Result<()>> + '_ {
        futures_util::future::poll_fn(move |cx| self.poll_flush(cx))
    }
}

// The `B` is a `Buf`, we never project a pin to it
impl<T: Unpin, B> Unpin for Buffered<T, B> {}

// TODO: This trait is old... at least rename to PollBytes or something...
pub(crate) trait MemRead {
    fn read_mem(&mut self, cx: &mut Context<'_>, len: usize) -> Poll<io::Result<Bytes>>;
}

impl<T, B> MemRead for Buffered<T, B>
where
    T: Read + Write + Unpin,
    B: Buf,
{
    fn read_mem(&mut self, cx: &mut Context<'_>, len: usize) -> Poll<io::Result<Bytes>> {
        if !self.read_buf.is_empty() {
            let n = std::cmp::min(len, self.read_buf.len());
            Poll::Ready(Ok(self.read_buf.split_to(n).freeze()))
        } else {
            let n = ready!(self.poll_read_from_io(cx))?;
            Poll::Ready(Ok(self.read_buf.split_to(::std::cmp::min(len, n)).freeze()))
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ReadStrategy {
    Adaptive {
        decrease_now: bool,
        next: usize,
        max: usize,
    },
    #[cfg(feature = "client")]
    Exact(usize),
}

impl ReadStrategy {
    fn with_max(max: usize) -> ReadStrategy {
        ReadStrategy::Adaptive {
            decrease_now: false,
            next: INIT_BUFFER_SIZE,
            max,
        }
    }

    fn next(&self) -> usize {
        match *self {
            ReadStrategy::Adaptive { next, .. } => next,
            #[cfg(feature = "client")]
            ReadStrategy::Exact(exact) => exact,
        }
    }

    fn max(&self) -> usize {
        match *self {
            ReadStrategy::Adaptive { max, .. } => max,
            #[cfg(feature = "client")]
            ReadStrategy::Exact(exact) => exact,
        }
    }

    fn record(&mut self, bytes_read: usize) {
        match *self {
            ReadStrategy::Adaptive {
                ref mut decrease_now,
                ref mut next,
                max,
                ..
            } => {
                if bytes_read >= *next {
                    *next = cmp::min(incr_power_of_two(*next), max);
                    *decrease_now = false;
                } else {
                    let decr_to = prev_power_of_two(*next);
                    if bytes_read < decr_to {
                        if *decrease_now {
                            *next = cmp::max(decr_to, INIT_BUFFER_SIZE);
                            *decrease_now = false;
                        } else {
                            // Decreasing is a two "record" process.
                            *decrease_now = true;
                        }
                    } else {
                        // A read within the current range should cancel
                        // a potential decrease, since we just saw proof
                        // that we still need this size.
                        *decrease_now = false;
                    }
                }
            }
            #[cfg(feature = "client")]
            ReadStrategy::Exact(_) => (),
        }
    }
}

fn incr_power_of_two(n: usize) -> usize {
    n.saturating_mul(2)
}

fn prev_power_of_two(n: usize) -> usize {
    // Only way this shift can underflow is if n is less than 4.
    // (Which would means `usize::MAX >> 64` and underflowed!)
    debug_assert!(n >= 4);
    (usize::MAX >> (n.leading_zeros() + 2)) + 1
}

impl Default for ReadStrategy {
    fn default() -> ReadStrategy {
        ReadStrategy::with_max(DEFAULT_MAX_BUFFER_SIZE)
    }
}

#[derive(Clone)]
pub(crate) struct Cursor<T> {
    bytes: T,
    pos: usize,
}

impl<T: AsRef<[u8]>> Cursor<T> {
    #[inline]
    pub(crate) fn new(bytes: T) -> Cursor<T> {
        Cursor { bytes, pos: 0 }
    }
}

impl Cursor<Vec<u8>> {
    /// If we've advanced the position a bit in this cursor, and wish to
    /// extend the underlying vector, we may wish to unshift the "read" bytes
    /// off, and move everything else over.
    fn maybe_unshift(&mut self, additional: usize) {
        if self.pos == 0 {
            // nothing to do
            return;
        }

        if self.bytes.capacity() - self.bytes.len() >= additional {
            // there's room!
            return;
        }

        self.bytes.drain(0..self.pos);
        self.pos = 0;
    }

    fn reset(&mut self) {
        self.pos = 0;
        self.bytes.clear();
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for Cursor<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cursor")
            .field("pos", &self.pos)
            .field("len", &self.bytes.as_ref().len())
            .finish()
    }
}

impl<T: AsRef<[u8]>> Buf for Cursor<T> {
    #[inline]
    fn remaining(&self) -> usize {
        self.bytes.as_ref().len() - self.pos
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        &self.bytes.as_ref()[self.pos..]
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        debug_assert!(self.pos + cnt <= self.bytes.as_ref().len());
        self.pos += cnt;
    }
}

// an internal buffer to collect writes before flushes
pub(super) struct WriteBuf<B> {
    /// Re-usable buffer that holds message headers
    headers: Cursor<Vec<u8>>,
    max_buf_size: usize,
    /// Deque of user buffers if strategy is Queue
    queue: BufList<B>,
    strategy: WriteStrategy,
}

impl<B: Buf> WriteBuf<B> {
    fn new(strategy: WriteStrategy) -> WriteBuf<B> {
        WriteBuf {
            headers: Cursor::new(Vec::with_capacity(INIT_BUFFER_SIZE)),
            max_buf_size: DEFAULT_MAX_BUFFER_SIZE,
            queue: BufList::new(),
            strategy,
        }
    }
}

impl<B> WriteBuf<B>
where
    B: Buf,
{
    fn set_strategy(&mut self, strategy: WriteStrategy) {
        self.strategy = strategy;
    }

    pub(super) fn buffer<BB: Buf + Into<B>>(&mut self, mut buf: BB) {
        debug_assert!(buf.has_remaining());
        match self.strategy {
            WriteStrategy::Flatten => {
                let head = self.headers_mut();

                head.maybe_unshift(buf.remaining());
                trace!(
                    self.len = head.remaining(),
                    buf.len = buf.remaining(),
                    "buffer.flatten"
                );
                //perf: This is a little faster than <Vec as BufMut>>::put,
                //but accomplishes the same result.
                loop {
                    let adv = {
                        let slice = buf.chunk();
                        if slice.is_empty() {
                            return;
                        }
                        head.bytes.extend_from_slice(slice);
                        slice.len()
                    };
                    buf.advance(adv);
                }
            }
            WriteStrategy::Queue => {
                trace!(
                    self.len = self.remaining(),
                    buf.len = buf.remaining(),
                    "buffer.queue"
                );
                self.queue.push(buf.into());
            }
        }
    }

    fn can_buffer(&self) -> bool {
        match self.strategy {
            WriteStrategy::Flatten => self.remaining() < self.max_buf_size,
            WriteStrategy::Queue => {
                self.queue.bufs_cnt() < MAX_BUF_LIST_BUFFERS && self.remaining() < self.max_buf_size
            }
        }
    }

    fn headers_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        debug_assert!(!self.queue.has_remaining());
        &mut self.headers
    }
}

impl<B: Buf> fmt::Debug for WriteBuf<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteBuf")
            .field("remaining", &self.remaining())
            .field("strategy", &self.strategy)
            .finish()
    }
}

impl<B: Buf> Buf for WriteBuf<B> {
    #[inline]
    fn remaining(&self) -> usize {
        self.headers.remaining() + self.queue.remaining()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        let headers = self.headers.chunk();
        if !headers.is_empty() {
            headers
        } else {
            self.queue.chunk()
        }
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        let hrem = self.headers.remaining();

        match hrem.cmp(&cnt) {
            cmp::Ordering::Equal => self.headers.reset(),
            cmp::Ordering::Greater => self.headers.advance(cnt),
            cmp::Ordering::Less => {
                let qcnt = cnt - hrem;
                self.headers.reset();
                self.queue.advance(qcnt);
            }
        }
    }

    #[inline]
    fn chunks_vectored<'t>(&'t self, dst: &mut [IoSlice<'t>]) -> usize {
        let n = self.headers.chunks_vectored(dst);
        self.queue.chunks_vectored(&mut dst[n..]) + n
    }
}

#[derive(Debug)]
enum WriteStrategy {
    Flatten,
    Queue,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::io::Compat;
    use std::time::Duration;

    use tokio_test::io::Builder as Mock;

    // #[cfg(feature = "nightly")]
    // use test::Bencher;

    /*
    impl<T: Read> MemRead for AsyncIo<T> {
        fn read_mem(&mut self, len: usize) -> Poll<Bytes, io::Error> {
            let mut v = vec![0; len];
            let n = try_nb!(self.read(v.as_mut_slice()));
            Ok(Async::Ready(BytesMut::from(&v[..n]).freeze()))
        }
    }
    */

    #[tokio::test]
    #[ignore]
    async fn iobuf_write_empty_slice() {
        // TODO(eliza): can i have writev back pls T_T
        // // First, let's just check that the Mock would normally return an
        // // error on an unexpected write, even if the buffer is empty...
        // let mut mock = Mock::new().build();
        // futures_util::future::poll_fn(|cx| {
        //     Pin::new(&mut mock).poll_write_buf(cx, &mut Cursor::new(&[]))
        // })
        // .await
        // .expect_err("should be a broken pipe");

        // // underlying io will return the logic error upon write,
        // // so we are testing that the io_buf does not trigger a write
        // // when there is nothing to flush
        // let mock = Mock::new().build();
        // let mut io_buf = Buffered::<_, Cursor<Vec<u8>>>::new(mock);
        // io_buf.flush().await.expect("should short-circuit flush");
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn parse_reads_until_blocked() {
        use crate::proto::h1::ClientTransaction;

        let _ = pretty_env_logger::try_init();
        let mock = Mock::new()
            // Split over multiple reads will read all of it
            .read(b"HTTP/1.1 200 OK\r\n")
            .read(b"Server: hyper\r\n")
            // missing last line ending
            .wait(Duration::from_secs(1))
            .build();

        let mut buffered = Buffered::<_, Cursor<Vec<u8>>>::new(Compat::new(mock));

        // We expect a `parse` to be not ready, and so can't await it directly.
        // Rather, this `poll_fn` will wrap the `Poll` result.
        futures_util::future::poll_fn(|cx| {
            let parse_ctx = ParseContext {
                cached_headers: &mut None,
                req_method: &mut None,
                h1_parser_config: Default::default(),
                h1_max_headers: None,
                preserve_header_case: false,
                #[cfg(feature = "ffi")]
                preserve_header_order: false,
                h09_responses: false,
                #[cfg(feature = "client")]
                on_informational: &mut None,
            };
            assert!(buffered
                .parse::<ClientTransaction>(cx, parse_ctx)
                .is_pending());
            Poll::Ready(())
        })
        .await;

        assert_eq!(
            buffered.read_buf,
            b"HTTP/1.1 200 OK\r\nServer: hyper\r\n"[..]
        );
    }

    #[test]
    fn read_strategy_adaptive_increments() {
        let mut strategy = ReadStrategy::default();
        assert_eq!(strategy.next(), 8192);

        // Grows if record == next
        strategy.record(8192);
        assert_eq!(strategy.next(), 16384);

        strategy.record(16384);
        assert_eq!(strategy.next(), 32768);

        // Enormous records still increment at same rate
        strategy.record(usize::MAX);
        assert_eq!(strategy.next(), 65536);

        let max = strategy.max();
        while strategy.next() < max {
            strategy.record(max);
        }

        assert_eq!(strategy.next(), max, "never goes over max");
        strategy.record(max + 1);
        assert_eq!(strategy.next(), max, "never goes over max");
    }

    #[test]
    fn read_strategy_adaptive_decrements() {
        let mut strategy = ReadStrategy::default();
        strategy.record(8192);
        assert_eq!(strategy.next(), 16384);

        strategy.record(1);
        assert_eq!(
            strategy.next(),
            16384,
            "first smaller record doesn't decrement yet"
        );
        strategy.record(8192);
        assert_eq!(strategy.next(), 16384, "record was with range");

        strategy.record(1);
        assert_eq!(
            strategy.next(),
            16384,
            "in-range record should make this the 'first' again"
        );

        strategy.record(1);
        assert_eq!(strategy.next(), 8192, "second smaller record decrements");

        strategy.record(1);
        assert_eq!(strategy.next(), 8192, "first doesn't decrement");
        strategy.record(1);
        assert_eq!(strategy.next(), 8192, "doesn't decrement under minimum");
    }

    #[test]
    fn read_strategy_adaptive_stays_the_same() {
        let mut strategy = ReadStrategy::default();
        strategy.record(8192);
        assert_eq!(strategy.next(), 16384);

        strategy.record(8193);
        assert_eq!(
            strategy.next(),
            16384,
            "first smaller record doesn't decrement yet"
        );

        strategy.record(8193);
        assert_eq!(
            strategy.next(),
            16384,
            "with current step does not decrement"
        );
    }

    #[test]
    fn read_strategy_adaptive_max_fuzz() {
        fn fuzz(max: usize) {
            let mut strategy = ReadStrategy::with_max(max);
            while strategy.next() < max {
                strategy.record(usize::MAX);
            }
            let mut next = strategy.next();
            while next > 8192 {
                strategy.record(1);
                strategy.record(1);
                next = strategy.next();
                assert!(
                    next.is_power_of_two(),
                    "decrement should be powers of two: {} (max = {})",
                    next,
                    max,
                );
            }
        }

        let mut max = 8192;
        while max < std::usize::MAX {
            fuzz(max);
            max = (max / 2).saturating_mul(3);
        }
        fuzz(usize::MAX);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)] // needs to trigger a debug_assert
    fn write_buf_requires_non_empty_bufs() {
        let mock = Mock::new().build();
        let mut buffered = Buffered::<_, Cursor<Vec<u8>>>::new(Compat::new(mock));

        buffered.buffer(Cursor::new(Vec::new()));
    }

    /*
    TODO: needs tokio_test::io to allow configure write_buf calls
    #[test]
    fn write_buf_queue() {
        let _ = pretty_env_logger::try_init();

        let mock = AsyncIo::new_buf(vec![], 1024);
        let mut buffered = Buffered::<_, Cursor<Vec<u8>>>::new(mock);


        buffered.headers_buf().extend(b"hello ");
        buffered.buffer(Cursor::new(b"world, ".to_vec()));
        buffered.buffer(Cursor::new(b"it's ".to_vec()));
        buffered.buffer(Cursor::new(b"hyper!".to_vec()));
        assert_eq!(buffered.write_buf.queue.bufs_cnt(), 3);
        buffered.flush().unwrap();

        assert_eq!(buffered.io, b"hello world, it's hyper!");
        assert_eq!(buffered.io.num_writes(), 1);
        assert_eq!(buffered.write_buf.queue.bufs_cnt(), 0);
    }
    */

    #[cfg(not(miri))]
    #[tokio::test]
    async fn write_buf_flatten() {
        let _ = pretty_env_logger::try_init();

        let mock = Mock::new().write(b"hello world, it's hyper!").build();

        let mut buffered = Buffered::<_, Cursor<Vec<u8>>>::new(Compat::new(mock));
        buffered.write_buf.set_strategy(WriteStrategy::Flatten);

        buffered.headers_buf().extend(b"hello ");
        buffered.buffer(Cursor::new(b"world, ".to_vec()));
        buffered.buffer(Cursor::new(b"it's ".to_vec()));
        buffered.buffer(Cursor::new(b"hyper!".to_vec()));
        assert_eq!(buffered.write_buf.queue.bufs_cnt(), 0);

        buffered.flush().await.expect("flush");
    }

    #[test]
    fn write_buf_flatten_partially_flushed() {
        let _ = pretty_env_logger::try_init();

        let b = |s: &str| Cursor::new(s.as_bytes().to_vec());

        let mut write_buf = WriteBuf::<Cursor<Vec<u8>>>::new(WriteStrategy::Flatten);

        write_buf.buffer(b("hello "));
        write_buf.buffer(b("world, "));

        assert_eq!(write_buf.chunk(), b"hello world, ");

        // advance most of the way, but not all
        write_buf.advance(11);

        assert_eq!(write_buf.chunk(), b", ");
        assert_eq!(write_buf.headers.pos, 11);
        assert_eq!(write_buf.headers.bytes.capacity(), INIT_BUFFER_SIZE);

        // there's still room in the headers buffer, so just push on the end
        write_buf.buffer(b("it's hyper!"));

        assert_eq!(write_buf.chunk(), b", it's hyper!");
        assert_eq!(write_buf.headers.pos, 11);

        let rem1 = write_buf.remaining();
        let cap = write_buf.headers.bytes.capacity();

        // but when this would go over capacity, don't copy the old bytes
        write_buf.buffer(Cursor::new(vec![b'X'; cap]));
        assert_eq!(write_buf.remaining(), cap + rem1);
        assert_eq!(write_buf.headers.pos, 0);
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn write_buf_queue_disable_auto() {
        let _ = pretty_env_logger::try_init();

        let mock = Mock::new()
            .write(b"hello ")
            .write(b"world, ")
            .write(b"it's ")
            .write(b"hyper!")
            .build();

        let mut buffered = Buffered::<_, Cursor<Vec<u8>>>::new(Compat::new(mock));
        buffered.write_buf.set_strategy(WriteStrategy::Queue);

        // we have 4 buffers, and vec IO disabled, but explicitly said
        // don't try to auto detect (via setting strategy above)

        buffered.headers_buf().extend(b"hello ");
        buffered.buffer(Cursor::new(b"world, ".to_vec()));
        buffered.buffer(Cursor::new(b"it's ".to_vec()));
        buffered.buffer(Cursor::new(b"hyper!".to_vec()));
        assert_eq!(buffered.write_buf.queue.bufs_cnt(), 3);

        buffered.flush().await.expect("flush");

        assert_eq!(buffered.write_buf.queue.bufs_cnt(), 0);
    }

    // #[cfg(feature = "nightly")]
    // #[bench]
    // fn bench_write_buf_flatten_buffer_chunk(b: &mut Bencher) {
    //     let s = "Hello, World!";
    //     b.bytes = s.len() as u64;

    //     let mut write_buf = WriteBuf::<bytes::Bytes>::new();
    //     write_buf.set_strategy(WriteStrategy::Flatten);
    //     b.iter(|| {
    //         let chunk = bytes::Bytes::from(s);
    //         write_buf.buffer(chunk);
    //         ::test::black_box(&write_buf);
    //         write_buf.headers.bytes.clear();
    //     })
    // }
}
