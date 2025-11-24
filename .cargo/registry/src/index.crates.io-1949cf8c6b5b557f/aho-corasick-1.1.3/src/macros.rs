#![allow(unused_macros)]

macro_rules! log {
    ($($tt:tt)*) => {
        #[cfg(feature = "logging")]
        {
            $($tt)*
        }
    }
}

macro_rules! debug {
    ($($tt:tt)*) => { log!(log::debug!($($tt)*)) }
}

macro_rules! trace {
    ($($tt:tt)*) => { log!(log::trace!($($tt)*)) }
}
