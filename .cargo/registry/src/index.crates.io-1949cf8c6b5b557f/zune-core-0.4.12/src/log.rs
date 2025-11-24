/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

// #[macro_export] is required to make macros works across crates
// but it always put the macro in the crate root.
// #[doc(hidden)] + "pub use" is a workaround to namespace a macro.
pub use crate::{
    __debug as debug, __error as error, __info as info, __log_enabled as log_enabled,
    __trace as trace, __warn as warn
};

#[repr(usize)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace
}

#[doc(hidden)]
#[macro_export]
macro_rules! __log_enabled {
    ($lvl:expr) => {{
        let _ = $lvl;
        false
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __error {
    ($($arg:tt)+) => {
        #[cfg(feature = "std")]
        {
            //eprintln!($($arg)+);
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __warn {
    ($($arg:tt)+) => {
        #[cfg(feature = "std")]
        {
            //eprintln!($($arg)+);
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __info {
    ($($arg:tt)+) => {};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __debug {
    ($($arg:tt)+) => {};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __trace {
    ($($arg:tt)+) => {};
}
