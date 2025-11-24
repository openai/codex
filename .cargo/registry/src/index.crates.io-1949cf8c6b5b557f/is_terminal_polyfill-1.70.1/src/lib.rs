//! > Polyfill for `is_terminal` stdlib feature for use with older MSRVs

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

/// Trait to determine if a descriptor/handle refers to a terminal/tty.
pub trait IsTerminal: sealed::Sealed {
    /// Returns `true` if the descriptor/handle refers to a terminal/tty.
    ///
    /// On platforms where Rust does not know how to detect a terminal yet, this will return
    /// `false`. This will also return `false` if an unexpected error occurred, such as from
    /// passing an invalid file descriptor.
    ///
    /// # Platform-specific behavior
    ///
    /// On Windows, in addition to detecting consoles, this currently uses some heuristics to
    /// detect older msys/cygwin/mingw pseudo-terminals based on device name: devices with names
    /// starting with `msys-` or `cygwin-` and ending in `-pty` will be considered terminals.
    /// Note that this [may change in the future][changes].
    ///
    /// [changes]: std::io#platform-specific-behavior
    fn is_terminal(&self) -> bool;
}

mod sealed {
    pub trait Sealed {}
}

macro_rules! impl_is_terminal {
    ($($t:ty),*$(,)?) => {$(
        impl sealed::Sealed for $t {}

        impl IsTerminal for $t {
            #[inline]
            fn is_terminal(&self) -> bool {
                std::io::IsTerminal::is_terminal(self)
            }
        }
    )*}
}

impl_is_terminal!(
    std::fs::File,
    std::io::Stdin,
    std::io::StdinLock<'_>,
    std::io::Stdout,
    std::io::StdoutLock<'_>,
    std::io::Stderr,
    std::io::StderrLock<'_>
);
