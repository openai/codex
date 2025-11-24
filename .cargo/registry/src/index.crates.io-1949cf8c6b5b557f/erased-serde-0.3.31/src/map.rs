pub(crate) trait ResultExt<T, E> {
    unsafe fn unsafe_map<U>(self, op: unsafe fn(T) -> U) -> Result<U, E>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    unsafe fn unsafe_map<U>(self, op: unsafe fn(T) -> U) -> Result<U, E> {
        match self {
            Ok(t) => Ok(unsafe { op(t) }),
            Err(e) => Err(e),
        }
    }
}

pub(crate) trait OptionExt<T> {
    unsafe fn unsafe_map<U>(self, op: unsafe fn(T) -> U) -> Option<U>;
}

impl<T> OptionExt<T> for Option<T> {
    unsafe fn unsafe_map<U>(self, op: unsafe fn(T) -> U) -> Option<U> {
        match self {
            Some(t) => Some(unsafe { op(t) }),
            None => None,
        }
    }
}
