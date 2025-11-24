//! Insert line breaks between written buffers when they would overflow the line length.
use std::io;

// The pnm standard says to insert line breaks after 70 characters. Assumes that no line breaks
// are actually written. We have to be careful to fully commit buffers or not commit them at all,
// otherwise we might insert a newline in the middle of a token.
pub(crate) struct AutoBreak<W: io::Write> {
    wrapped: W,
    line_capacity: usize,
    line: Vec<u8>,
    has_newline: bool,
    panicked: bool, // see https://github.com/rust-lang/rust/issues/30888
}

impl<W: io::Write> AutoBreak<W> {
    pub(crate) fn new(writer: W, line_capacity: usize) -> io::Result<Self> {
        let mut line = Vec::new();
        line.try_reserve_exact(line_capacity + 1)?;
        Ok(AutoBreak {
            wrapped: writer,
            line_capacity,
            line,
            has_newline: false,
            panicked: false,
        })
    }

    fn flush_buf(&mut self) -> io::Result<()> {
        // from BufWriter
        let mut written = 0;
        let len = self.line.len();
        let mut ret = Ok(());
        while written < len {
            self.panicked = true;
            let r = self.wrapped.write(&self.line[written..]);
            self.panicked = false;
            match r {
                Ok(0) => {
                    ret = Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write the buffered data",
                    ));
                    break;
                }
                Ok(n) => written += n,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    ret = Err(e);
                    break;
                }
            }
        }
        if written > 0 {
            self.line.drain(..written);
        }
        ret
    }
}

impl<W: io::Write> io::Write for AutoBreak<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.has_newline {
            self.flush()?;
            self.has_newline = false;
        }

        if !self.line.is_empty() && self.line.len() + buffer.len() > self.line_capacity {
            self.line.push(b'\n');
            self.has_newline = true;
            self.flush()?;
            self.has_newline = false;
        }

        self.line.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf()?;
        self.wrapped.flush()
    }
}

impl<W: io::Write> Drop for AutoBreak<W> {
    fn drop(&mut self) {
        if !self.panicked {
            let _r = self.flush_buf();
            // internal writer flushed automatically by Drop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_aligned_writes() {
        let mut output = Vec::new();

        {
            let mut writer = AutoBreak::new(&mut output, 10).unwrap();
            writer.write_all(b"0123456789").unwrap();
            writer.write_all(b"0123456789").unwrap();
        }

        assert_eq!(output.as_slice(), b"0123456789\n0123456789");
    }

    #[test]
    fn test_greater_writes() {
        let mut output = Vec::new();

        {
            let mut writer = AutoBreak::new(&mut output, 10).unwrap();
            writer.write_all(b"012").unwrap();
            writer.write_all(b"345").unwrap();
            writer.write_all(b"0123456789").unwrap();
            writer.write_all(b"012345678910").unwrap();
            writer.write_all(b"_").unwrap();
        }

        assert_eq!(output.as_slice(), b"012345\n0123456789\n012345678910\n_");
    }
}
