use std::io::Result as IoResult;
use std::io::{Read, Write};

pub struct CustomStream<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> CustomStream<R, W>
where
    R: Read,
    W: Write,
{
    pub fn new(reader: R, writer: W) -> CustomStream<R, W> {
        CustomStream { reader, writer }
    }
}

impl<R, W> Read for CustomStream<R, W>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.reader.read(buf)
    }
}

impl<R, W> Write for CustomStream<R, W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}
