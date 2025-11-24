/// Unwrap an `Option<T>` in a `const` context.
///
/// If it fails, panics with the given message.
macro_rules! unwrap {
    ($val:expr, $msg:expr$(,)?) => {
        match $val {
            Some(val) => val,
            None => panic!($msg),
        }
    };
}

pub(crate) use unwrap;
