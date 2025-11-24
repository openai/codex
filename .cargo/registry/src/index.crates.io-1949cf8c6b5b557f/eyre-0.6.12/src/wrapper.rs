use crate::StdError;
use core::fmt::{self, Debug, Display};

#[repr(transparent)]
pub(crate) struct DisplayError<M>(pub(crate) M);

#[repr(transparent)]
/// Wraps a Debug + Display type as an error.
///
/// Its Debug and Display impls are the same as the wrapped type.
pub(crate) struct MessageError<M>(pub(crate) M);

pub(crate) struct NoneError;

impl<M> Debug for DisplayError<M>
where
    M: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> Display for DisplayError<M>
where
    M: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> StdError for DisplayError<M> where M: Display + 'static {}

impl<M> Debug for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<M> Display for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> StdError for MessageError<M> where M: Display + Debug + 'static {}

impl Debug for NoneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt("Option was None", f)
    }
}

impl Display for NoneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt("Option was None", f)
    }
}

impl StdError for NoneError {}

#[repr(transparent)]
pub(crate) struct BoxedError(pub(crate) Box<dyn StdError + Send + Sync>);

impl Debug for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl StdError for BoxedError {
    #[cfg(backtrace)]
    fn backtrace(&self) -> Option<&crate::backtrace::Backtrace> {
        self.0.backtrace()
    }

    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}
