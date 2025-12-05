/// A shim which allows a [`std::io::Write`] to be implemented in terms of a [`std::fmt::Write`]
///
/// This saves off I/O errors. instead of discarding them
pub(crate) struct Adapter<W>
where
    W: FnMut(&[u8]) -> std::io::Result<()>,
{
    writer: W,
    error: std::io::Result<()>,
}

impl<W> Adapter<W>
where
    W: FnMut(&[u8]) -> std::io::Result<()>,
{
    pub(crate) fn new(writer: W) -> Self {
        Adapter {
            writer,
            error: Ok(()),
        }
    }

    pub(crate) fn write_fmt(mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        match std::fmt::write(&mut self, fmt) {
            Ok(()) => Ok(()),
            Err(..) => {
                // check if the error came from the underlying `Write` or not
                if self.error.is_err() {
                    self.error
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "formatter error",
                    ))
                }
            }
        }
    }
}

impl<W> std::fmt::Write for Adapter<W>
where
    W: FnMut(&[u8]) -> std::io::Result<()>,
{
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        match (self.writer)(s.as_bytes()) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.error = Err(e);
                Err(std::fmt::Error)
            }
        }
    }
}
