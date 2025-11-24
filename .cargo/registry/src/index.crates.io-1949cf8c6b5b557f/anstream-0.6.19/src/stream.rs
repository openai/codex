//! Higher-level traits to describe writeable streams

/// Required functionality for underlying [`std::io::Write`] for adaptation
#[cfg(not(all(windows, feature = "wincon")))]
pub trait RawStream: std::io::Write + IsTerminal + private::Sealed {}

/// Required functionality for underlying [`std::io::Write`] for adaptation
#[cfg(all(windows, feature = "wincon"))]
pub trait RawStream:
    std::io::Write + IsTerminal + anstyle_wincon::WinconStream + private::Sealed
{
}

impl<T: RawStream + ?Sized> RawStream for &mut T {}
impl<T: RawStream + ?Sized> RawStream for Box<T> {}

impl RawStream for std::io::Stdout {}

impl RawStream for std::io::StdoutLock<'_> {}

impl RawStream for std::io::Stderr {}

impl RawStream for std::io::StderrLock<'_> {}

impl RawStream for dyn std::io::Write {}
impl RawStream for dyn std::io::Write + Send {}
impl RawStream for dyn std::io::Write + Send + Sync {}

impl RawStream for Vec<u8> {}

impl RawStream for std::fs::File {}

#[allow(deprecated)]
impl RawStream for crate::Buffer {}

/// Trait to determine if a descriptor/handle refers to a terminal/tty.
pub trait IsTerminal: private::Sealed {
    /// Returns `true` if the descriptor/handle refers to a terminal/tty.
    fn is_terminal(&self) -> bool;
}

impl<T: IsTerminal + ?Sized> IsTerminal for &T {
    #[inline]
    fn is_terminal(&self) -> bool {
        (**self).is_terminal()
    }
}

impl<T: IsTerminal + ?Sized> IsTerminal for &mut T {
    #[inline]
    fn is_terminal(&self) -> bool {
        (**self).is_terminal()
    }
}

impl<T: IsTerminal + ?Sized> IsTerminal for Box<T> {
    #[inline]
    fn is_terminal(&self) -> bool {
        (**self).is_terminal()
    }
}

impl IsTerminal for std::io::Stdout {
    #[inline]
    fn is_terminal(&self) -> bool {
        is_terminal_polyfill::IsTerminal::is_terminal(self)
    }
}

impl IsTerminal for std::io::StdoutLock<'_> {
    #[inline]
    fn is_terminal(&self) -> bool {
        is_terminal_polyfill::IsTerminal::is_terminal(self)
    }
}

impl IsTerminal for std::io::Stderr {
    #[inline]
    fn is_terminal(&self) -> bool {
        is_terminal_polyfill::IsTerminal::is_terminal(self)
    }
}

impl IsTerminal for std::io::StderrLock<'_> {
    #[inline]
    fn is_terminal(&self) -> bool {
        is_terminal_polyfill::IsTerminal::is_terminal(self)
    }
}

impl IsTerminal for dyn std::io::Write {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

impl IsTerminal for dyn std::io::Write + Send {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

impl IsTerminal for dyn std::io::Write + Send + Sync {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

impl IsTerminal for Vec<u8> {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

impl IsTerminal for std::fs::File {
    #[inline]
    fn is_terminal(&self) -> bool {
        is_terminal_polyfill::IsTerminal::is_terminal(self)
    }
}

#[allow(deprecated)]
impl IsTerminal for crate::Buffer {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

/// Lock a stream
pub trait AsLockedWrite: private::Sealed {
    /// Locked writer type
    type Write<'w>: RawStream + 'w
    where
        Self: 'w;

    /// Lock a stream
    fn as_locked_write(&mut self) -> Self::Write<'_>;
}

impl<T: AsLockedWrite + ?Sized> AsLockedWrite for &mut T {
    type Write<'w>
        = T::Write<'w>
    where
        Self: 'w;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        (**self).as_locked_write()
    }
}

impl<T: AsLockedWrite + ?Sized> AsLockedWrite for Box<T> {
    type Write<'w>
        = T::Write<'w>
    where
        Self: 'w;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        (**self).as_locked_write()
    }
}

impl AsLockedWrite for std::io::Stdout {
    type Write<'w> = std::io::StdoutLock<'w>;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self.lock()
    }
}

impl AsLockedWrite for std::io::StdoutLock<'static> {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

impl AsLockedWrite for std::io::Stderr {
    type Write<'w> = std::io::StderrLock<'w>;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self.lock()
    }
}

impl AsLockedWrite for std::io::StderrLock<'static> {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

impl AsLockedWrite for dyn std::io::Write {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

impl AsLockedWrite for dyn std::io::Write + Send {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

impl AsLockedWrite for dyn std::io::Write + Send + Sync {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

impl AsLockedWrite for Vec<u8> {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

impl AsLockedWrite for std::fs::File {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

#[allow(deprecated)]
impl AsLockedWrite for crate::Buffer {
    type Write<'w> = &'w mut Self;

    #[inline]
    fn as_locked_write(&mut self) -> Self::Write<'_> {
        self
    }
}

mod private {
    #[allow(unnameable_types)]
    pub trait Sealed {}

    impl<T: Sealed + ?Sized> Sealed for &T {}
    impl<T: Sealed + ?Sized> Sealed for &mut T {}
    impl<T: Sealed + ?Sized> Sealed for Box<T> {}

    impl Sealed for std::io::Stdout {}

    impl Sealed for std::io::StdoutLock<'_> {}

    impl Sealed for std::io::Stderr {}

    impl Sealed for std::io::StderrLock<'_> {}

    impl Sealed for dyn std::io::Write {}
    impl Sealed for dyn std::io::Write + Send {}
    impl Sealed for dyn std::io::Write + Send + Sync {}

    impl Sealed for Vec<u8> {}

    impl Sealed for std::fs::File {}

    #[allow(deprecated)]
    impl Sealed for crate::Buffer {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_raw_stream<T: RawStream>()
    where
        crate::AutoStream<T>: std::io::Write,
    {
    }

    #[test]
    fn test() {
        assert_raw_stream::<Box<dyn std::io::Write>>();
        assert_raw_stream::<Box<dyn std::io::Write + 'static>>();
        assert_raw_stream::<Box<dyn std::io::Write + Send>>();
        assert_raw_stream::<Box<dyn std::io::Write + Send + Sync>>();

        assert_raw_stream::<&mut (dyn std::io::Write)>();
        assert_raw_stream::<&mut (dyn std::io::Write + 'static)>();
        assert_raw_stream::<&mut (dyn std::io::Write + Send)>();
        assert_raw_stream::<&mut (dyn std::io::Write + Send + Sync)>();

        assert_raw_stream::<Vec<u8>>();
        assert_raw_stream::<&mut Vec<u8>>();

        assert_raw_stream::<std::fs::File>();
        assert_raw_stream::<&mut std::fs::File>();
    }
}
