macro_rules! err {
    ($($tt:tt)*) => {{
        crate::shared::util::error::Error::from_args(format_args!($($tt)*))
    }}
}

pub(crate) use err;

/// An error that can be returned when parsing.
#[derive(Clone, Debug)]
pub struct Error {
    #[cfg(feature = "alloc")]
    message: alloc::boxed::Box<str>,
    // only-jiff-start
    #[cfg(not(feature = "alloc"))]
    message: &'static str,
    // only-jiff-end
}

impl Error {
    pub(crate) fn from_args<'a>(message: core::fmt::Arguments<'a>) -> Error {
        #[cfg(feature = "alloc")]
        {
            use alloc::string::ToString;

            let message = message.to_string().into_boxed_str();
            Error { message }
        }
        // only-jiff-start
        #[cfg(not(feature = "alloc"))]
        {
            let message = message.as_str().unwrap_or(
                "unknown Jiff error (better error messages require \
                 enabling the `alloc` feature for the `jiff` crate)",
            );
            Error { message }
        }
        // only-jiff-end
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.message, f)
    }
}
