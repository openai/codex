//! Wrapper around tracing so that tracing can be an optional dependency.

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[cfg(feature = "tracing")]
pub(crate) mod implementation {
    impl super::Level {
        pub(crate) const fn to_tracing(self) -> tracing::Level {
            match self {
                Self::Error => tracing::Level::ERROR,
                Self::Warn => tracing::Level::WARN,
                Self::Info => tracing::Level::INFO,
                Self::Debug => tracing::Level::DEBUG,
                Self::Trace => tracing::Level::TRACE,
            }
        }
    }

    macro_rules! event {
        ( $lvl:expr, $($arg:tt)+ ) => {
            tracing::event!($crate::tracing::Level::to_tracing($lvl), $($arg)+)
        }
    }

    macro_rules! span {
        ( $lvl:expr, $name:expr, $($fields:tt)* ) => {
            tracing::span!($crate::tracing::Level::to_tracing($lvl), $name, $($fields)*)
        }
    }

    pub(crate) use event;
    pub(crate) use span;
}

#[cfg(not(feature = "tracing"))]
pub(crate) mod implementation {
    macro_rules! event {
        ( $lvl:expr, { $($fields:tt)+ }, $($arg:tt)+ ) => {
            let _ = format_args!($($arg)+);
        };
        ( $lvl:expr, $($arg:tt)+ ) => {
            let _ = format_args!($($arg)+);
        };
    }

    pub(crate) struct Span;
    pub(crate) struct EnteredSpan;

    impl Span {
        pub(crate) fn entered(&self) -> EnteredSpan {
            EnteredSpan
        }
    }

    macro_rules! span {
        ( $lvl:expr, $name:expr, $($fields:tt)* ) => {
            $crate::tracing::implementation::Span
        };
    }

    pub(crate) use event;
    pub(crate) use span;
}

macro_rules! error {
    ( $($arg:tt)+ ) => { $crate::tracing::implementation::event!($crate::tracing::Level::Error, $($arg)+) };
}
macro_rules! warning {
    ( $($arg:tt)+ ) => { $crate::tracing::implementation::event!($crate::tracing::Level::Warn, $($arg)+) };
}
macro_rules! info {
    ( $($arg:tt)+ ) => { $crate::tracing::implementation::event!($crate::tracing::Level::Info, $($arg)+) };
}
macro_rules! debug {
    ( $($arg:tt)+ ) => { $crate::tracing::implementation::event!($crate::tracing::Level::Debug, $($arg)+) };
}
macro_rules! trace {
    ( $($arg:tt)+ ) => { $crate::tracing::implementation::event!($crate::tracing::Level::Trace, $($arg)+) };
}

pub(crate) use debug;
pub(crate) use error;
pub(crate) use info;
pub(crate) use trace;
pub(crate) use warning;

#[allow(unused_macros)]
macro_rules! error_span {
    ( $name:expr ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Error, $name, ) };
    ( $name:expr, $($fields:tt)* ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Error, $name, $($fields)*) };
}
#[allow(unused_macros)]
macro_rules! warning_span {
    ( $name:expr ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Warn, $name, ) };
    ( $name:expr, $($fields:tt)* ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Warn, $name, $($fields)*) };
}
#[allow(unused_macro_rules)]
macro_rules! info_span {
    ( $name:expr ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Info, $name, ) };
    ( $name:expr, $($fields:tt)* ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Info, $name, $($fields)*) };
}
macro_rules! debug_span {
    ( $name:expr ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Debug, $name, ) };
    ( $name:expr, $($fields:tt)* ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Debug, $name, $($fields)*) };
}
#[allow(unused_macro_rules)]
macro_rules! trace_span {
    ( $name:expr ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Trace, $name, ) };
    ( $name:expr, $($fields:tt)* ) => { $crate::tracing::implementation::span!($crate::tracing::Level::Trace, $name, $($fields)*) };
}

pub(crate) use debug_span;
#[allow(unused_imports)]
pub(crate) use error_span;
pub(crate) use info_span;
pub(crate) use trace_span;
#[allow(unused_imports)]
pub(crate) use warning_span;
