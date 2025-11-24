/// An error that occurred during parsing or compiling a regular expression.
///
/// A parse error occurs when the syntax of the regex pattern is not
/// valid. Otherwise, a regex can still fail to build if it would
/// result in a machine that exceeds the configured size limit, via
/// [`RegexBuilder::size_limit`](crate::RegexBuilder::size_limit).
///
/// This error type provides no introspection capabilities. The only thing you
/// can do with it is convert it to a string as a human readable error message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    msg: &'static str,
}

impl Error {
    pub(crate) fn new(msg: &'static str) -> Error {
        Error { msg }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.msg)
    }
}
