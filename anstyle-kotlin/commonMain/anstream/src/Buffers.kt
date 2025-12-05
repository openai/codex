#![allow(deprecated)]

/// In-memory [`RawStream`][crate::stream::RawStream]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
#[deprecated(since = "0.6.2", note = "Use Vec")]
#[doc(hidden)]
pub struct Buffer(Vec<u8>);

impl Buffer {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for Buffer {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl std::io::Write for Buffer {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.extend(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(all(windows, feature = "wincon"))]
impl anstyle_wincon::WinconStream for Buffer {
    fn write_colored(
        &mut self,
        fg: Option<anstyle::AnsiColor>,
        bg: Option<anstyle::AnsiColor>,
        data: &[u8],
    ) -> std::io::Result<usize> {
        self.0.write_colored(fg, bg, data)
    }
}
