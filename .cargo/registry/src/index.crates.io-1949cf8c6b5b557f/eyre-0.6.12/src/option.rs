use crate::OptionExt;
use core::fmt::{Debug, Display};

impl<T> OptionExt<T> for Option<T> {
    #[track_caller]
    fn ok_or_eyre<M>(self, message: M) -> crate::Result<T>
    where
        M: Debug + Display + Send + Sync + 'static,
    {
        match self {
            Some(ok) => Ok(ok),
            None => Err(crate::Report::msg(message)),
        }
    }
}
