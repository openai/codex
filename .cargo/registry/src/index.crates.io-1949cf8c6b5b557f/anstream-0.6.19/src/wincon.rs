use crate::adapter::WinconBytes;
use crate::stream::AsLockedWrite;
use crate::stream::IsTerminal;

/// Only pass printable data to the inner `Write`
#[cfg(feature = "wincon")] // here mostly for documentation purposes
#[derive(Debug)]
pub struct WinconStream<S>
where
    S: anstyle_wincon::WinconStream,
{
    raw: S,
    // `WinconBytes` is especially large compared to other variants of `AutoStream`, so boxing it
    // here so `AutoStream` doesn't have to discard one allocation and create another one when
    // calling `AutoStream::lock`
    state: Box<WinconBytes>,
}

impl<S> WinconStream<S>
where
    S: anstyle_wincon::WinconStream,
{
    /// Only pass printable data to the inner `Write`
    #[inline]
    pub fn new(raw: S) -> Self {
        Self {
            raw,
            state: Default::default(),
        }
    }

    /// Get the wrapped [`anstyle_wincon::WinconStream`]
    #[inline]
    pub fn into_inner(self) -> S {
        self.raw
    }
}

impl<S> WinconStream<S>
where
    S: anstyle_wincon::WinconStream,
    S: IsTerminal,
{
    /// Returns `true` if the descriptor/handle refers to a terminal/tty.
    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.raw.is_terminal()
    }
}

impl WinconStream<std::io::Stdout> {
    /// Get exclusive access to the `WinconStream`
    ///
    /// Why?
    /// - Faster performance when writing in a loop
    /// - Avoid other threads interleaving output with the current thread
    #[inline]
    pub fn lock(self) -> WinconStream<std::io::StdoutLock<'static>> {
        WinconStream {
            raw: self.raw.lock(),
            state: self.state,
        }
    }
}

impl WinconStream<std::io::Stderr> {
    /// Get exclusive access to the `WinconStream`
    ///
    /// Why?
    /// - Faster performance when writing in a loop
    /// - Avoid other threads interleaving output with the current thread
    #[inline]
    pub fn lock(self) -> WinconStream<std::io::StderrLock<'static>> {
        WinconStream {
            raw: self.raw.lock(),
            state: self.state,
        }
    }
}

impl<S> std::io::Write for WinconStream<S>
where
    S: anstyle_wincon::WinconStream,
    S: AsLockedWrite,
{
    // Must forward all calls to ensure locking happens appropriately
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        write(&mut self.raw.as_locked_write(), &mut self.state, buf)
    }
    #[inline]
    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        let buf = bufs
            .iter()
            .find(|b| !b.is_empty())
            .map(|b| &**b)
            .unwrap_or(&[][..]);
        self.write(buf)
    }
    // is_write_vectored: nightly only
    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.raw.as_locked_write().flush()
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        write_all(&mut self.raw.as_locked_write(), &mut self.state, buf)
    }
    // write_all_vectored: nightly only
    #[inline]
    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        write_fmt(&mut self.raw.as_locked_write(), &mut self.state, args)
    }
}

fn write(
    raw: &mut dyn anstyle_wincon::WinconStream,
    state: &mut WinconBytes,
    buf: &[u8],
) -> std::io::Result<usize> {
    for (style, printable) in state.extract_next(buf) {
        let fg = style.get_fg_color().and_then(cap_wincon_color);
        let bg = style.get_bg_color().and_then(cap_wincon_color);
        let written = raw.write_colored(fg, bg, printable.as_bytes())?;
        let possible = printable.len();
        if possible != written {
            // HACK: Unsupported atm
            break;
        }
    }
    Ok(buf.len())
}

fn write_all(
    raw: &mut dyn anstyle_wincon::WinconStream,
    state: &mut WinconBytes,
    buf: &[u8],
) -> std::io::Result<()> {
    for (style, printable) in state.extract_next(buf) {
        let mut buf = printable.as_bytes();
        let fg = style.get_fg_color().and_then(cap_wincon_color);
        let bg = style.get_bg_color().and_then(cap_wincon_color);
        while !buf.is_empty() {
            match raw.write_colored(fg, bg, buf) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ));
                }
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

fn write_fmt(
    raw: &mut dyn anstyle_wincon::WinconStream,
    state: &mut WinconBytes,
    args: std::fmt::Arguments<'_>,
) -> std::io::Result<()> {
    let write_all = |buf: &[u8]| write_all(raw, state, buf);
    crate::fmt::Adapter::new(write_all).write_fmt(args)
}

fn cap_wincon_color(color: anstyle::Color) -> Option<anstyle::AnsiColor> {
    match color {
        anstyle::Color::Ansi(c) => Some(c),
        anstyle::Color::Ansi256(c) => c.into_ansi(),
        anstyle::Color::Rgb(_) => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;
    use std::io::Write as _;

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn write_all_no_escapes(s in "\\PC*") {
            let buffer = Vec::new();
            let mut stream = WinconStream::new(buffer);
            stream.write_all(s.as_bytes()).unwrap();
            let buffer = stream.into_inner();
            let actual = std::str::from_utf8(buffer.as_ref()).unwrap();
            assert_eq!(s, actual);
        }

        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn write_byte_no_escapes(s in "\\PC*") {
            let buffer = Vec::new();
            let mut stream = WinconStream::new(buffer);
            for byte in s.as_bytes() {
                stream.write_all(&[*byte]).unwrap();
            }
            let buffer = stream.into_inner();
            let actual = std::str::from_utf8(buffer.as_ref()).unwrap();
            assert_eq!(s, actual);
        }

        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn write_all_random(s in any::<Vec<u8>>()) {
            let buffer = Vec::new();
            let mut stream = WinconStream::new(buffer);
            stream.write_all(s.as_slice()).unwrap();
        }

        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn write_byte_random(s in any::<Vec<u8>>()) {
            let buffer = Vec::new();
            let mut stream = WinconStream::new(buffer);
            for byte in s.as_slice() {
                stream.write_all(&[*byte]).unwrap();
            }
        }
    }
}
