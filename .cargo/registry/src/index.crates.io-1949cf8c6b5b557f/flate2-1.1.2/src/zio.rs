use std::io;
use std::io::prelude::*;
use std::mem;

use crate::{
    Compress, CompressError, Decompress, DecompressError, FlushCompress, FlushDecompress, Status,
};

#[derive(Debug)]
pub struct Writer<W: Write, D: Ops> {
    obj: Option<W>,
    pub data: D,
    buf: Vec<u8>,
}

pub trait Ops {
    type Error: Into<io::Error>;
    type Flush: Flush;
    fn total_in(&self) -> u64;
    fn total_out(&self) -> u64;
    fn run(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: Self::Flush,
    ) -> Result<Status, Self::Error>;
    fn run_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: Self::Flush,
    ) -> Result<Status, Self::Error>;
}

impl Ops for Compress {
    type Error = CompressError;
    type Flush = FlushCompress;
    fn total_in(&self) -> u64 {
        self.total_in()
    }
    fn total_out(&self) -> u64 {
        self.total_out()
    }
    fn run(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        self.compress(input, output, flush)
    }
    fn run_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        self.compress_vec(input, output, flush)
    }
}

impl Ops for Decompress {
    type Error = DecompressError;
    type Flush = FlushDecompress;
    fn total_in(&self) -> u64 {
        self.total_in()
    }
    fn total_out(&self) -> u64 {
        self.total_out()
    }
    fn run(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        self.decompress(input, output, flush)
    }
    fn run_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        self.decompress_vec(input, output, flush)
    }
}

pub trait Flush {
    fn none() -> Self;
    fn sync() -> Self;
    fn finish() -> Self;
}

impl Flush for FlushCompress {
    fn none() -> Self {
        FlushCompress::None
    }

    fn sync() -> Self {
        FlushCompress::Sync
    }

    fn finish() -> Self {
        FlushCompress::Finish
    }
}

impl Flush for FlushDecompress {
    fn none() -> Self {
        FlushDecompress::None
    }

    fn sync() -> Self {
        FlushDecompress::Sync
    }

    fn finish() -> Self {
        FlushDecompress::Finish
    }
}

pub fn read<R, D>(obj: &mut R, data: &mut D, dst: &mut [u8]) -> io::Result<usize>
where
    R: BufRead,
    D: Ops,
{
    loop {
        let (read, consumed, ret, eof);
        {
            let input = obj.fill_buf()?;
            eof = input.is_empty();
            let before_out = data.total_out();
            let before_in = data.total_in();
            let flush = if eof {
                D::Flush::finish()
            } else {
                D::Flush::none()
            };
            ret = data.run(input, dst, flush);
            read = (data.total_out() - before_out) as usize;
            consumed = (data.total_in() - before_in) as usize;
        }
        obj.consume(consumed);

        match ret {
            // If we haven't ready any data and we haven't hit EOF yet,
            // then we need to keep asking for more data because if we
            // return that 0 bytes of data have been read then it will
            // be interpreted as EOF.
            Ok(Status::Ok | Status::BufError) if read == 0 && !eof && !dst.is_empty() => continue,
            Ok(Status::Ok | Status::BufError | Status::StreamEnd) => return Ok(read),

            Err(..) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "corrupt deflate stream",
                ))
            }
        }
    }
}

impl<W: Write, D: Ops> Writer<W, D> {
    pub fn new(w: W, d: D) -> Writer<W, D> {
        Writer {
            obj: Some(w),
            data: d,
            buf: Vec::with_capacity(32 * 1024),
        }
    }

    pub fn finish(&mut self) -> io::Result<()> {
        loop {
            self.dump()?;

            let before = self.data.total_out();
            self.data
                .run_vec(&[], &mut self.buf, Flush::finish())
                .map_err(Into::into)?;
            if before == self.data.total_out() {
                return Ok(());
            }
        }
    }

    pub fn replace(&mut self, w: W) -> W {
        self.buf.truncate(0);
        mem::replace(self.get_mut(), w)
    }

    pub fn get_ref(&self) -> &W {
        self.obj.as_ref().unwrap()
    }

    pub fn get_mut(&mut self) -> &mut W {
        self.obj.as_mut().unwrap()
    }

    // Note that this should only be called if the outer object is just about
    // to be consumed!
    //
    // (e.g. an implementation of `into_inner`)
    pub fn take_inner(&mut self) -> W {
        self.obj.take().unwrap()
    }

    pub fn is_present(&self) -> bool {
        self.obj.is_some()
    }

    // Returns total written bytes and status of underlying codec
    pub(crate) fn write_with_status(&mut self, buf: &[u8]) -> io::Result<(usize, Status)> {
        // miniz isn't guaranteed to actually write any of the buffer provided,
        // it may be in a flushing mode where it's just giving us data before
        // we're actually giving it any data. We don't want to spuriously return
        // `Ok(0)` when possible as it will cause calls to write_all() to fail.
        // As a result we execute this in a loop to ensure that we try our
        // darndest to write the data.
        loop {
            self.dump()?;

            let before_in = self.data.total_in();
            let ret = self.data.run_vec(buf, &mut self.buf, D::Flush::none());
            let written = (self.data.total_in() - before_in) as usize;
            let is_stream_end = matches!(ret, Ok(Status::StreamEnd));

            if !buf.is_empty() && written == 0 && ret.is_ok() && !is_stream_end {
                continue;
            }
            return match ret {
                Ok(st) => match st {
                    Status::Ok | Status::BufError | Status::StreamEnd => Ok((written, st)),
                },
                Err(..) => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "corrupt deflate stream",
                )),
            };
        }
    }

    fn dump(&mut self) -> io::Result<()> {
        // TODO: should manage this buffer not with `drain` but probably more of
        // a deque-like strategy.
        while !self.buf.is_empty() {
            let n = self.obj.as_mut().unwrap().write(&self.buf)?;
            if n == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            self.buf.drain(..n);
        }
        Ok(())
    }
}

impl<W: Write, D: Ops> Write for Writer<W, D> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_with_status(buf).map(|res| res.0)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.data
            .run_vec(&[], &mut self.buf, Flush::sync())
            .map_err(Into::into)?;

        // Unfortunately miniz doesn't actually tell us when we're done with
        // pulling out all the data from the internal stream. To remedy this we
        // have to continually ask the stream for more memory until it doesn't
        // give us a chunk of memory the same size as our own internal buffer,
        // at which point we assume it's reached the end.
        loop {
            self.dump()?;
            let before = self.data.total_out();
            self.data
                .run_vec(&[], &mut self.buf, Flush::none())
                .map_err(Into::into)?;
            if before == self.data.total_out() {
                break;
            }
        }

        self.obj.as_mut().unwrap().flush()
    }
}

impl<W: Write, D: Ops> Drop for Writer<W, D> {
    fn drop(&mut self) {
        if self.obj.is_some() {
            let _ = self.finish();
        }
    }
}
