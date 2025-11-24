use crate::alloc::{String, ToString};
use core::fmt::{self, Display};

/// Error when a `Serializer` or `Deserializer` trait object fails.
#[derive(Debug)]
pub struct Error {
    msg: String,
}

/// Result type alias where the error is `erased_serde::Error`.
pub type Result<T> = core::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.msg.fmt(formatter)
    }
}

impl serde::ser::StdError for Error {}

impl serde::ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error {
            msg: msg.to_string(),
        }
    }
}

impl serde::de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error {
            msg: msg.to_string(),
        }
    }
}
