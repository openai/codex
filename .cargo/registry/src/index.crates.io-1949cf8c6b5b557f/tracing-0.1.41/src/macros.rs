/// Constructs a new span.
///
/// See [the top-level documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [lib]: crate#using-the-macros
///
/// # Examples
///
/// Creating a new span:
/// ```
/// # use tracing::{span, Level};
/// # fn main() {
/// let span = span!(Level::TRACE, "my span");
/// let _enter = span.enter();
/// // do work inside the span...
/// # }
/// ```
#[macro_export]
macro_rules! span {
    (target: $target:expr, parent: $parent:expr, $lvl:expr, $name:expr) => {
        $crate::span!(target: $target, parent: $parent, $lvl, $name,)
    };
    (target: $target:expr, parent: $parent:expr, $lvl:expr, $name:expr, $($fields:tt)*) => {
        {
            use $crate::__macro_support::Callsite as _;
            static __CALLSITE: $crate::__macro_support::MacroCallsite = $crate::callsite2! {
                name: $name,
                kind: $crate::metadata::Kind::SPAN,
                target: $target,
                level: $lvl,
                fields: $($fields)*
            };
            let mut interest = $crate::subscriber::Interest::never();
            if $crate::level_enabled!($lvl)
                && { interest = __CALLSITE.interest(); !interest.is_never() }
                && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest)
            {
                let meta = __CALLSITE.metadata();
                // span with explicit parent
                $crate::Span::child_of(
                    $parent,
                    meta,
                    &$crate::valueset!(meta.fields(), $($fields)*),
                )
            } else {
                let span = $crate::__macro_support::__disabled_span(__CALLSITE.metadata());
                $crate::if_log_enabled! { $lvl, {
                    span.record_all(&$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
                }};
                span
            }
        }
    };
    (target: $target:expr, $lvl:expr, $name:expr, $($fields:tt)*) => {
        {
            use $crate::__macro_support::Callsite as _;
            static __CALLSITE: $crate::callsite::DefaultCallsite = $crate::callsite2! {
                name: $name,
                kind: $crate::metadata::Kind::SPAN,
                target: $target,
                level: $lvl,
                fields: $($fields)*
            };
            let mut interest = $crate::subscriber::Interest::never();
            if $crate::level_enabled!($lvl)
                && { interest = __CALLSITE.interest(); !interest.is_never() }
                && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest)
            {
                let meta = __CALLSITE.metadata();
                // span with contextual parent
                $crate::Span::new(
                    meta,
                    &$crate::valueset!(meta.fields(), $($fields)*),
                )
            } else {
                let span = $crate::__macro_support::__disabled_span(__CALLSITE.metadata());
                $crate::if_log_enabled! { $lvl, {
                    span.record_all(&$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
                }};
                span
            }
        }
    };
    (target: $target:expr, parent: $parent:expr, $lvl:expr, $name:expr) => {
        $crate::span!(target: $target, parent: $parent, $lvl, $name,)
    };
    (parent: $parent:expr, $lvl:expr, $name:expr, $($fields:tt)*) => {
        $crate::span!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            $name,
            $($fields)*
        )
    };
    (parent: $parent:expr, $lvl:expr, $name:expr) => {
        $crate::span!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            $name,
        )
    };
    (target: $target:expr, $lvl:expr, $name:expr, $($fields:tt)*) => {
        $crate::span!(
            target: $target,
            $lvl,
            $name,
            $($fields)*
        )
    };
    (target: $target:expr, $lvl:expr, $name:expr) => {
        $crate::span!(target: $target, $lvl, $name,)
    };
    ($lvl:expr, $name:expr, $($fields:tt)*) => {
        $crate::span!(
            target: module_path!(),
            $lvl,
            $name,
            $($fields)*
        )
    };
    ($lvl:expr, $name:expr) => {
        $crate::span!(
            target: module_path!(),
            $lvl,
            $name,
        )
    };
}

/// Constructs a span at the trace level.
///
/// [Fields] and [attributes] are set using the same syntax as the [`span!`]
/// macro.
///
/// See [the top-level documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [lib]: crate#using-the-macros
/// [attributes]: crate#configuring-attributes
/// [Fields]: crate#recording-fields
/// [`span!`]: crate::span!
///
/// # Examples
///
/// ```rust
/// # use tracing::{trace_span, span, Level};
/// # fn main() {
/// trace_span!("my_span");
/// // is equivalent to:
/// span!(Level::TRACE, "my_span");
/// # }
/// ```
///
/// ```rust
/// # use tracing::{trace_span, span, Level};
/// # fn main() {
/// let span = trace_span!("my span");
/// span.in_scope(|| {
///     // do work inside the span...
/// });
/// # }
/// ```
#[macro_export]
macro_rules! trace_span {
    (target: $target:expr, parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            parent: $parent,
            $crate::Level::TRACE,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, parent: $parent:expr, $name:expr) => {
        $crate::trace_span!(target: $target, parent: $parent, $name,)
    };
    (parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            $name,
            $($field)*
        )
    };
    (parent: $parent:expr, $name:expr) => {
        $crate::trace_span!(parent: $parent, $name,)
    };
    (target: $target:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            $crate::Level::TRACE,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, $name:expr) => {
        $crate::trace_span!(target: $target, $name,)
    };
    ($name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            $crate::Level::TRACE,
            $name,
            $($field)*
        )
    };
    ($name:expr) => { $crate::trace_span!($name,) };
}

/// Constructs a span at the debug level.
///
/// [Fields] and [attributes] are set using the same syntax as the [`span!`]
/// macro.
///
/// See [the top-level documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [lib]: crate#using-the-macros
/// [attributes]: crate#configuring-attributes
/// [Fields]: crate#recording-fields
/// [`span!`]: crate::span!
///
/// # Examples
///
/// ```rust
/// # use tracing::{debug_span, span, Level};
/// # fn main() {
/// debug_span!("my_span");
/// // is equivalent to:
/// span!(Level::DEBUG, "my_span");
/// # }
/// ```
///
/// ```rust
/// # use tracing::debug_span;
/// # fn main() {
/// let span = debug_span!("my span");
/// span.in_scope(|| {
///     // do work inside the span...
/// });
/// # }
/// ```
#[macro_export]
macro_rules! debug_span {
    (target: $target:expr, parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            parent: $parent,
            $crate::Level::DEBUG,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, parent: $parent:expr, $name:expr) => {
        $crate::debug_span!(target: $target, parent: $parent, $name,)
    };
    (parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            $name,
            $($field)*
        )
    };
    (parent: $parent:expr, $name:expr) => {
        $crate::debug_span!(parent: $parent, $name,)
    };
    (target: $target:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            $crate::Level::DEBUG,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, $name:expr) => {
        $crate::debug_span!(target: $target, $name,)
    };
    ($name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            $crate::Level::DEBUG,
            $name,
            $($field)*
        )
    };
    ($name:expr) => {$crate::debug_span!($name,)};
}

/// Constructs a span at the info level.
///
/// [Fields] and [attributes] are set using the same syntax as the [`span!`]
/// macro.
///
/// See [the top-level documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [lib]: crate#using-the-macros
/// [attributes]: crate#configuring-attributes
/// [Fields]: crate#recording-fields
/// [`span!`]: crate::span!
///
/// # Examples
///
/// ```rust
/// # use tracing::{span, info_span, Level};
/// # fn main() {
/// info_span!("my_span");
/// // is equivalent to:
/// span!(Level::INFO, "my_span");
/// # }
/// ```
///
/// ```rust
/// # use tracing::info_span;
/// # fn main() {
/// let span = info_span!("my span");
/// span.in_scope(|| {
///     // do work inside the span...
/// });
/// # }
/// ```
#[macro_export]
macro_rules! info_span {
    (target: $target:expr, parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            parent: $parent,
            $crate::Level::INFO,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, parent: $parent:expr, $name:expr) => {
        $crate::info_span!(target: $target, parent: $parent, $name,)
    };
    (parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            $name,
            $($field)*
        )
    };
    (parent: $parent:expr, $name:expr) => {
        $crate::info_span!(parent: $parent, $name,)
    };
    (target: $target:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            $crate::Level::INFO,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, $name:expr) => {
        $crate::info_span!(target: $target, $name,)
    };
    ($name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            $crate::Level::INFO,
            $name,
            $($field)*
        )
    };
    ($name:expr) => {$crate::info_span!($name,)};
}

/// Constructs a span at the warn level.
///
/// [Fields] and [attributes] are set using the same syntax as the [`span!`]
/// macro.
///
/// See [the top-level documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [lib]: crate#using-the-macros
/// [attributes]: crate#configuring-attributes
/// [Fields]: crate#recording-fields
/// [`span!`]: crate::span!
///
/// # Examples
///
/// ```rust
/// # use tracing::{warn_span, span, Level};
/// # fn main() {
/// warn_span!("my_span");
/// // is equivalent to:
/// span!(Level::WARN, "my_span");
/// # }
/// ```
///
/// ```rust
/// use tracing::warn_span;
/// # fn main() {
/// let span = warn_span!("my span");
/// span.in_scope(|| {
///     // do work inside the span...
/// });
/// # }
/// ```
#[macro_export]
macro_rules! warn_span {
    (target: $target:expr, parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            parent: $parent,
            $crate::Level::WARN,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, parent: $parent:expr, $name:expr) => {
        $crate::warn_span!(target: $target, parent: $parent, $name,)
    };
    (parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            $name,
            $($field)*
        )
    };
    (parent: $parent:expr, $name:expr) => {
        $crate::warn_span!(parent: $parent, $name,)
    };
    (target: $target:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            $crate::Level::WARN,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, $name:expr) => {
        $crate::warn_span!(target: $target, $name,)
    };
    ($name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            $crate::Level::WARN,
            $name,
            $($field)*
        )
    };
    ($name:expr) => {$crate::warn_span!($name,)};
}
/// Constructs a span at the error level.
///
/// [Fields] and [attributes] are set using the same syntax as the [`span!`]
/// macro.
///
/// See [the top-level documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [lib]: crate#using-the-macros
/// [attributes]: crate#configuring-attributes
/// [Fields]: crate#recording-fields
/// [`span!`]: crate::span!
///
/// # Examples
///
/// ```rust
/// # use tracing::{span, error_span, Level};
/// # fn main() {
/// error_span!("my_span");
/// // is equivalent to:
/// span!(Level::ERROR, "my_span");
/// # }
/// ```
///
/// ```rust
/// # use tracing::error_span;
/// # fn main() {
/// let span = error_span!("my span");
/// span.in_scope(|| {
///     // do work inside the span...
/// });
/// # }
/// ```
#[macro_export]
macro_rules! error_span {
    (target: $target:expr, parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            parent: $parent,
            $crate::Level::ERROR,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, parent: $parent:expr, $name:expr) => {
        $crate::error_span!(target: $target, parent: $parent, $name,)
    };
    (parent: $parent:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            $name,
            $($field)*
        )
    };
    (parent: $parent:expr, $name:expr) => {
        $crate::error_span!(parent: $parent, $name,)
    };
    (target: $target:expr, $name:expr, $($field:tt)*) => {
        $crate::span!(
            target: $target,
            $crate::Level::ERROR,
            $name,
            $($field)*
        )
    };
    (target: $target:expr, $name:expr) => {
        $crate::error_span!(target: $target, $name,)
    };
    ($name:expr, $($field:tt)*) => {
        $crate::span!(
            target: module_path!(),
            $crate::Level::ERROR,
            $name,
            $($field)*
        )
    };
    ($name:expr) => {$crate::error_span!($name,)};
}

/// Constructs a new `Event`.
///
/// The event macro is invoked with a `Level` and up to 32 key-value fields.
/// Optionally, a format string and arguments may follow the fields; this will
/// be used to construct an implicit field named "message".
///
/// See [the top-level documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [lib]: crate#using-the-macros
///
/// # Examples
///
/// ```rust
/// use tracing::{event, Level};
///
/// # fn main() {
/// let data = (42, "forty-two");
/// let private_data = "private";
/// let error = "a bad error";
///
/// event!(Level::ERROR, %error, "Received error");
/// event!(
///     target: "app_events",
///     Level::WARN,
///     private_data,
///     ?data,
///     "App warning: {}",
///     error
/// );
/// event!(name: "answer", Level::INFO, the_answer = data.0);
/// event!(Level::INFO, the_answer = data.0);
/// # }
/// ```
///
// /// Note that *unlike `span!`*, `event!` requires a value for all fields. As
// /// events are recorded immediately when the macro is invoked, there is no
// /// opportunity for fields to be recorded later. A trailing comma on the final
// /// field is valid.
// ///
// /// For example, the following does not compile:
// /// ```rust,compile_fail
// /// # use tracing::{Level, event};
// /// # fn main() {
// /// event!(Level::INFO, foo = 5, bad_field, bar = "hello")
// /// #}
// /// ```
#[macro_export]
macro_rules! event {
    // Name / target / parent.
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $lvl:expr, { $($fields:tt)* } )=> ({
        use $crate::__macro_support::Callsite as _;
        static __CALLSITE: $crate::__macro_support::MacroCallsite = $crate::callsite2! {
            name: $name,
            kind: $crate::metadata::Kind::EVENT,
            target: $target,
            level: $lvl,
            fields: $($fields)*
        };

        let enabled = $crate::level_enabled!($lvl) && {
            let interest = __CALLSITE.interest();
            !interest.is_never() && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest)
        };
        if enabled {
            (|value_set: $crate::field::ValueSet| {
                $crate::__tracing_log!(
                    $lvl,
                    __CALLSITE,
                    &value_set
                );
                let meta = __CALLSITE.metadata();
                // event with explicit parent
                $crate::Event::child_of(
                    $parent,
                    meta,
                    &value_set
                );
            })($crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
        } else {
            $crate::__tracing_log!(
                $lvl,
                __CALLSITE,
                &$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*)
            );
        }
    });
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            name: $name,
            target: $target,
            parent: $parent,
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $lvl:expr, $($k:ident).+ = $($fields:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $lvl, { $($k).+ = $($fields)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $lvl:expr, $($arg:tt)+) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $lvl, { $($arg)+ })
    );

    // Name / target.
    (name: $name:expr, target: $target:expr, $lvl:expr, { $($fields:tt)* } )=> ({
        use $crate::__macro_support::Callsite as _;
        static __CALLSITE: $crate::__macro_support::MacroCallsite = $crate::callsite2! {
            name: $name,
            kind: $crate::metadata::Kind::EVENT,
            target: $target,
            level: $lvl,
            fields: $($fields)*
        };
        let enabled = $crate::level_enabled!($lvl) && {
            let interest = __CALLSITE.interest();
            !interest.is_never() && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest)
        };
        if enabled {
            (|value_set: $crate::field::ValueSet| {
                let meta = __CALLSITE.metadata();
                // event with contextual parent
                $crate::Event::dispatch(
                    meta,
                    &value_set
                );
                $crate::__tracing_log!(
                    $lvl,
                    __CALLSITE,
                    &value_set
                );
            })($crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
        } else {
            $crate::__tracing_log!(
                $lvl,
                __CALLSITE,
                &$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*)
            );
        }
    });
    (name: $name:expr, target: $target:expr, $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            name: $name,
            target: $target,
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    (name: $name:expr, target: $target:expr, $lvl:expr, $($k:ident).+ = $($fields:tt)* ) => (
        $crate::event!(name: $name, target: $target, $lvl, { $($k).+ = $($fields)* })
    );
    (name: $name:expr, target: $target:expr, $lvl:expr, $($arg:tt)+) => (
        $crate::event!(name: $name, target: $target, $lvl, { $($arg)+ })
    );

    // Target / parent.
    (target: $target:expr, parent: $parent:expr, $lvl:expr, { $($fields:tt)* } )=> ({
        use $crate::__macro_support::Callsite as _;
        static __CALLSITE: $crate::callsite::DefaultCallsite = $crate::callsite2! {
            name: $crate::__macro_support::concat!(
                "event ",
                $crate::__macro_support::file!(),
                ":",
                $crate::__macro_support::line!()
            ),
            kind: $crate::metadata::Kind::EVENT,
            target: $target,
            level: $lvl,
            fields: $($fields)*
        };

        let enabled = $crate::level_enabled!($lvl) && {
            let interest = __CALLSITE.interest();
            !interest.is_never() && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest)
        };
        if enabled {
            (|value_set: $crate::field::ValueSet| {
                $crate::__tracing_log!(
                    $lvl,
                    __CALLSITE,
                    &value_set
                );
                let meta = __CALLSITE.metadata();
                // event with explicit parent
                $crate::Event::child_of(
                    $parent,
                    meta,
                    &value_set
                );
            })($crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
        } else {
            $crate::__tracing_log!(
                $lvl,
                __CALLSITE,
                &$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*)
            );
        }
    });
    (target: $target:expr, parent: $parent:expr, $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            target: $target,
            parent: $parent,
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    (target: $target:expr, parent: $parent:expr, $lvl:expr, $($k:ident).+ = $($fields:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $lvl, { $($k).+ = $($fields)* })
    );
    (target: $target:expr, parent: $parent:expr, $lvl:expr, $($arg:tt)+) => (
        $crate::event!(target: $target, parent: $parent, $lvl, { $($arg)+ })
    );

    // Name / parent.
    (name: $name:expr, parent: $parent:expr, $lvl:expr, { $($fields:tt)* } )=> ({
        use $crate::__macro_support::Callsite as _;
        static __CALLSITE: $crate::__macro_support::MacroCallsite = $crate::callsite2! {
            name: $name,
            kind: $crate::metadata::Kind::EVENT,
            target: module_path!(),
            level: $lvl,
            fields: $($fields)*
        };

        let enabled = $crate::level_enabled!($lvl) && {
            let interest = __CALLSITE.interest();
            !interest.is_never() && __CALLSITE.is_enabled(interest)
        };
        if enabled {
            (|value_set: $crate::field::ValueSet| {
                $crate::__tracing_log!(
                    $lvl,
                    __CALLSITE,
                    &value_set
                );
                let meta = __CALLSITE.metadata();
                // event with explicit parent
                $crate::Event::child_of(
                    $parent,
                    meta,
                    &value_set
                );
            })($crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
        } else {
            $crate::__tracing_log!(
                $lvl,
                __CALLSITE,
                &$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*)
            );
        }
    });
    (name: $name:expr, parent: $parent:expr, $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            name: $name,
            parent: $parent,
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    (name: $name:expr, parent: $parent:expr, $lvl:expr, $($k:ident).+ = $($fields:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $lvl, { $($k).+ = $($fields)* })
    );
    (name: $name:expr, parent: $parent:expr, $lvl:expr, $($arg:tt)+) => (
        $crate::event!(name: $name, parent: $parent, $lvl, { $($arg)+ })
    );

    // Name.
    (name: $name:expr, $lvl:expr, { $($fields:tt)* } )=> ({
        use $crate::__macro_support::Callsite as _;
        static __CALLSITE: $crate::__macro_support::MacroCallsite = $crate::callsite2! {
            name: $name,
            kind: $crate::metadata::Kind::EVENT,
            target: module_path!(),
            level: $lvl,
            fields: $($fields)*
        };
        let enabled = $crate::level_enabled!($lvl) && {
            let interest = __CALLSITE.interest();
            !interest.is_never() && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest)
        };
        if enabled {
            (|value_set: $crate::field::ValueSet| {
                let meta = __CALLSITE.metadata();
                // event with contextual parent
                $crate::Event::dispatch(
                    meta,
                    &value_set
                );
                $crate::__tracing_log!(
                    $lvl,
                    __CALLSITE,
                    &value_set
                );
            })($crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
        } else {
            $crate::__tracing_log!(
                $lvl,
                __CALLSITE,
                &$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*)
            );
        }
    });
    (name: $name:expr, $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            name: $name,
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    (name: $name:expr, $lvl:expr, $($k:ident).+ = $($fields:tt)* ) => (
        $crate::event!(name: $name, $lvl, { $($k).+ = $($fields)* })
    );
    (name: $name:expr, $lvl:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, $lvl, { $($arg)+ })
    );

    // Target.
    (target: $target:expr, $lvl:expr, { $($fields:tt)* } )=> ({
        use $crate::__macro_support::Callsite as _;
        static __CALLSITE: $crate::callsite::DefaultCallsite = $crate::callsite2! {
            name: $crate::__macro_support::concat!(
                "event ",
                $crate::__macro_support::file!(),
                ":",
                $crate::__macro_support::line!()
            ),
            kind: $crate::metadata::Kind::EVENT,
            target: $target,
            level: $lvl,
            fields: $($fields)*
        };
        let enabled = $crate::level_enabled!($lvl) && {
            let interest = __CALLSITE.interest();
            !interest.is_never() && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest)
        };
        if enabled {
            (|value_set: $crate::field::ValueSet| {
                let meta = __CALLSITE.metadata();
                // event with contextual parent
                $crate::Event::dispatch(
                    meta,
                    &value_set
                );
                $crate::__tracing_log!(
                    $lvl,
                    __CALLSITE,
                    &value_set
                );
            })($crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*));
        } else {
            $crate::__tracing_log!(
                $lvl,
                __CALLSITE,
                &$crate::valueset!(__CALLSITE.metadata().fields(), $($fields)*)
            );
        }
    });
    (target: $target:expr, $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            target: $target,
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    (target: $target:expr, $lvl:expr, $($k:ident).+ = $($fields:tt)* ) => (
        $crate::event!(target: $target, $lvl, { $($k).+ = $($fields)* })
    );
    (target: $target:expr, $lvl:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, $lvl, { $($arg)+ })
    );

    // Parent.
    (parent: $parent:expr, $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    (parent: $parent:expr, $lvl:expr, $($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            { $($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $lvl:expr, ?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            { ?$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $lvl:expr, %$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            { %$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $lvl:expr, $($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            { $($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $lvl:expr, %$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            { %$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $lvl:expr, ?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $lvl,
            { ?$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $lvl:expr, $($arg:tt)+ ) => (
        $crate::event!(target: module_path!(), parent: $parent, $lvl, { $($arg)+ })
    );

    // ...
    ( $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            $lvl,
            { message = $crate::__macro_support::format_args!($($arg)+), $($fields)* }
        )
    );
    ( $lvl:expr, { $($fields:tt)* }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            $lvl,
            { message = format_args!($($arg)+), $($fields)* }
        )
    );
    ($lvl:expr, $($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $lvl,
            { $($k).+ = $($field)*}
        )
    );
    ($lvl:expr, $($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $lvl,
            { $($k).+, $($field)*}
        )
    );
    ($lvl:expr, ?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $lvl,
            { ?$($k).+, $($field)*}
        )
    );
    ($lvl:expr, %$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $lvl,
            { %$($k).+, $($field)*}
        )
    );
    ($lvl:expr, ?$($k:ident).+) => (
        $crate::event!($lvl, ?$($k).+,)
    );
    ($lvl:expr, %$($k:ident).+) => (
        $crate::event!($lvl, %$($k).+,)
    );
    ($lvl:expr, $($k:ident).+) => (
        $crate::event!($lvl, $($k).+,)
    );
    ( $lvl:expr, $($arg:tt)+ ) => (
        $crate::event!(target: module_path!(), $lvl, { $($arg)+ })
    );
}

/// Tests whether an event with the specified level and target would be enabled.
///
/// This is similar to [`enabled!`], but queries the current subscriber specifically for
/// an event, whereas [`enabled!`] queries for an event _or_ span.
///
/// See the documentation for [`enabled!]` for more details on using this macro.
/// See also [`span_enabled!`].
///
/// # Examples
///
/// ```rust
/// # use tracing::{event_enabled, Level};
/// if event_enabled!(target: "my_crate", Level::DEBUG) {
///     // some expensive work...
/// }
/// // simpler
/// if event_enabled!(Level::DEBUG) {
///     // some expensive work...
/// }
/// // with fields
/// if event_enabled!(Level::DEBUG, foo_field) {
///     // some expensive work...
/// }
/// ```
///
/// [`enabled!`]: crate::enabled
/// [`span_enabled!`]: crate::span_enabled
#[macro_export]
macro_rules! event_enabled {
    ($($rest:tt)*)=> (
        $crate::enabled!(kind: $crate::metadata::Kind::EVENT, $($rest)*)
    )
}

/// Tests whether a span with the specified level and target would be enabled.
///
/// This is similar to [`enabled!`], but queries the current subscriber specifically for
/// an event, whereas [`enabled!`] queries for an event _or_ span.
///
/// See the documentation for [`enabled!]` for more details on using this macro.
/// See also [`span_enabled!`].
///
/// # Examples
///
/// ```rust
/// # use tracing::{span_enabled, Level};
/// if span_enabled!(target: "my_crate", Level::DEBUG) {
///     // some expensive work...
/// }
/// // simpler
/// if span_enabled!(Level::DEBUG) {
///     // some expensive work...
/// }
/// // with fields
/// if span_enabled!(Level::DEBUG, foo_field) {
///     // some expensive work...
/// }
/// ```
///
/// [`enabled!`]: crate::enabled
/// [`span_enabled!`]: crate::span_enabled
#[macro_export]
macro_rules! span_enabled {
    ($($rest:tt)*)=> (
        $crate::enabled!(kind: $crate::metadata::Kind::SPAN, $($rest)*)
    )
}

/// Checks whether a span or event is [enabled] based on the provided [metadata].
///
/// [enabled]: crate::Subscriber::enabled
/// [metadata]: crate::Metadata
///
/// This macro is a specialized tool: it is intended to be used prior
/// to an expensive computation required *just* for that event, but
/// *cannot* be done as part of an argument to that event, such as
/// when multiple events are emitted (e.g., iterating over a collection
/// and emitting an event for each item).
///
/// # Usage
///
/// [Subscribers] can make filtering decisions based all the data included in a
/// span or event's [`Metadata`]. This means that it is possible for `enabled!`
/// to return a _false positive_ (indicating that something would be enabled
/// when it actually would not be) or a _false negative_ (indicating that
/// something would be disabled when it would actually be enabled).
///
/// [Subscribers]: crate::subscriber::Subscriber
/// [`Metadata`]: crate::metadata::Metadata
///
/// This occurs when a subscriber is using a _more specific_ filter than the
/// metadata provided to the `enabled!` macro. Some situations that can result
/// in false positives or false negatives include:
///
/// - If a subscriber is using a filter which may enable a span or event based
///   on field names, but `enabled!` is invoked without listing field names,
///   `enabled!` may return a false negative if a specific field name would
///   cause the subscriber to enable something that would otherwise be disabled.
/// - If a subscriber is using a filter which enables or disables specific events by
///   file path and line number,  a particular event may be enabled/disabled
///   even if an `enabled!` invocation with the same level, target, and fields
///   indicated otherwise.
/// - The subscriber can choose to enable _only_ spans or _only_ events, which `enabled`
///   will not reflect.
///
/// `enabled!()` requires a [level](crate::Level) argument, an optional `target:`
/// argument, and an optional set of field names. If the fields are not provided,
/// they are considered to be unknown. `enabled!` attempts to match the
/// syntax of `event!()` as closely as possible, which can be seen in the
/// examples below.
///
/// # Examples
///
/// If the current subscriber is interested in recording `DEBUG`-level spans and
/// events in the current file and module path, this will evaluate to true:
/// ```rust
/// use tracing::{enabled, Level};
///
/// if enabled!(Level::DEBUG) {
///     // some expensive work...
/// }
/// ```
///
/// If the current subscriber is interested in recording spans and events
/// in the current file and module path, with the target "my_crate", and at the
/// level  `DEBUG`, this will evaluate to true:
/// ```rust
/// # use tracing::{enabled, Level};
/// if enabled!(target: "my_crate", Level::DEBUG) {
///     // some expensive work...
/// }
/// ```
///
/// If the current subscriber is interested in recording spans and events
/// in the current file and module path, with the target "my_crate", at
/// the level `DEBUG`, and with a field named "hello", this will evaluate
/// to true:
///
/// ```rust
/// # use tracing::{enabled, Level};
/// if enabled!(target: "my_crate", Level::DEBUG, hello) {
///     // some expensive work...
/// }
/// ```
///
/// # Alternatives
///
/// `enabled!` queries subscribers with [`Metadata`] where
/// [`is_event`] and [`is_span`] both return `false`. Alternatively,
/// use [`event_enabled!`] or [`span_enabled!`] to ensure one of these
/// returns true.
///
///
/// [`Metadata`]: crate::Metadata
/// [`is_event`]: crate::Metadata::is_event
/// [`is_span`]: crate::Metadata::is_span
/// [`enabled!`]: crate::enabled
/// [`span_enabled!`]: crate::span_enabled
#[macro_export]
macro_rules! enabled {
    (kind: $kind:expr, target: $target:expr, $lvl:expr, { $($fields:tt)* } )=> ({
        if $crate::level_enabled!($lvl) {
            use $crate::__macro_support::Callsite as _;
            static __CALLSITE: $crate::callsite::DefaultCallsite = $crate::callsite2! {
                name: $crate::__macro_support::concat!(
                    "enabled ",
                    $crate::__macro_support::file!(),
                    ":",
                    $crate::__macro_support::line!()
                ),
                kind: $kind.hint(),
                target: $target,
                level: $lvl,
                fields: $($fields)*
            };
            let interest = __CALLSITE.interest();
            if !interest.is_never() && $crate::__macro_support::__is_enabled(__CALLSITE.metadata(), interest) {
                let meta = __CALLSITE.metadata();
                $crate::dispatcher::get_default(|current| current.enabled(meta))
            } else {
                false
            }
        } else {
            false
        }
    });
    // Just target and level
    (kind: $kind:expr, target: $target:expr, $lvl:expr ) => (
        $crate::enabled!(kind: $kind, target: $target, $lvl, { })
    );
    (target: $target:expr, $lvl:expr ) => (
        $crate::enabled!(kind: $crate::metadata::Kind::HINT, target: $target, $lvl, { })
    );

    // These four cases handle fields with no values
    (kind: $kind:expr, target: $target:expr, $lvl:expr, $($field:tt)*) => (
        $crate::enabled!(
            kind: $kind,
            target: $target,
            $lvl,
            { $($field)*}
        )
    );
    (target: $target:expr, $lvl:expr, $($field:tt)*) => (
        $crate::enabled!(
            kind: $crate::metadata::Kind::HINT,
            target: $target,
            $lvl,
            { $($field)*}
        )
    );

    // Level and field case
    (kind: $kind:expr, $lvl:expr, $($field:tt)*) => (
        $crate::enabled!(
            kind: $kind,
            target: module_path!(),
            $lvl,
            { $($field)*}
        )
    );

    // Simplest `enabled!` case
    (kind: $kind:expr, $lvl:expr) => (
        $crate::enabled!(kind: $kind, target: module_path!(), $lvl, { })
    );
    ($lvl:expr) => (
        $crate::enabled!(kind: $crate::metadata::Kind::HINT, target: module_path!(), $lvl, { })
    );

    // Fallthrough from above
    ($lvl:expr, $($field:tt)*) => (
        $crate::enabled!(
            kind: $crate::metadata::Kind::HINT,
            target: module_path!(),
            $lvl,
            { $($field)*}
        )
    );
}

/// Constructs an event at the trace level.
///
/// This functions similarly to the [`event!`] macro. See [the top-level
/// documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [`event!`]: crate::event!
/// [lib]: crate#using-the-macros
///
/// # Examples
///
/// ```rust
/// use tracing::trace;
/// # #[derive(Debug, Copy, Clone)] struct Position { x: f32, y: f32 }
/// # impl Position {
/// # const ORIGIN: Self = Self { x: 0.0, y: 0.0 };
/// # fn dist(&self, other: Position) -> f32 {
/// #    let x = (other.x - self.x).exp2(); let y = (self.y - other.y).exp2();
/// #    (x + y).sqrt()
/// # }
/// # }
/// # fn main() {
/// let pos = Position { x: 3.234, y: -1.223 };
/// let origin_dist = pos.dist(Position::ORIGIN);
///
/// trace!(position = ?pos, ?origin_dist);
/// trace!(
///     target: "app_events",
///     position = ?pos,
///     "x is {} and y is {}",
///     if pos.x >= 0.0 { "positive" } else { "negative" },
///     if pos.y >= 0.0 { "positive" } else { "negative" }
/// );
/// trace!(name: "completed", position = ?pos);
/// # }
/// ```
#[macro_export]
macro_rules! trace {
    // Name / target / parent.
    (name: $name:expr, target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::TRACE, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::TRACE, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::TRACE, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::TRACE, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::TRACE, {}, $($arg)+)
    );

    // Name / target.
    (name: $name:expr, target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::TRACE, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::TRACE, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::TRACE, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::TRACE, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::TRACE, {}, $($arg)+)
    );

    // Target / parent.
    (target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::TRACE, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::TRACE, { $($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::TRACE, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::TRACE, { %$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::TRACE, {}, $($arg)+)
    );

    // Name / parent.
    (name: $name:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::TRACE, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::TRACE, { $($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::TRACE, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::TRACE, { %$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::TRACE, {}, $($arg)+)
    );

    // Name.
    (name: $name:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::TRACE, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::TRACE, { $($k).+ $($field)* })
    );
    (name: $name:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::TRACE, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::TRACE, { %$($k).+ $($field)* })
    );
    (name: $name:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, $crate::Level::TRACE, {}, $($arg)+)
    );

    // Target.
    (target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::TRACE, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::TRACE, { $($k).+ $($field)* })
    );
    (target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::TRACE, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::TRACE, { %$($k).+ $($field)* })
    );
    (target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, $crate::Level::TRACE, {}, $($arg)+)
    );

    // Parent.
    (parent: $parent:expr, { $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            { $($field)+ },
            $($arg)+
        )
    );
    (parent: $parent:expr, $($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            { $($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            { ?$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            { %$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            { $($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            { ?$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            { %$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::TRACE,
            {},
            $($arg)+
        )
    );

    // ...
    ({ $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { $($field)+ },
            $($arg)+
        )
    );
    ($($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { $($k).+ = $($field)*}
        )
    );
    (?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { ?$($k).+ = $($field)*}
        )
    );
    (%$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { %$($k).+ = $($field)*}
        )
    );
    ($($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { $($k).+, $($field)*}
        )
    );
    (?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { ?$($k).+, $($field)*}
        )
    );
    (%$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { %$($k).+, $($field)*}
        )
    );
    (?$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { ?$($k).+ }
        )
    );
    (%$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { %$($k).+ }
        )
    );
    ($($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            { $($k).+ }
        )
    );
    ($($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::TRACE,
            $($arg)+
        )
    );
}

/// Constructs an event at the debug level.
///
/// This functions similarly to the [`event!`] macro. See [the top-level
/// documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [`event!`]: crate::event!
/// [lib]: crate#using-the-macros
///
/// # Examples
///
/// ```rust
/// use tracing::debug;
/// # fn main() {
/// # #[derive(Debug)] struct Position { x: f32, y: f32 }
///
/// let pos = Position { x: 3.234, y: -1.223 };
///
/// debug!(?pos.x, ?pos.y);
/// debug!(target: "app_events", position = ?pos, "New position");
/// debug!(name: "completed", position = ?pos);
/// # }
/// ```
#[macro_export]
macro_rules! debug {
    // Name / target / parent.
    (name: $name:expr, target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::DEBUG, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::DEBUG, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::DEBUG, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::DEBUG, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::DEBUG, {}, $($arg)+)
    );

    // Name / target.
    (name: $name:expr, target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::DEBUG, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::DEBUG, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::DEBUG, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::DEBUG, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::DEBUG, {}, $($arg)+)
    );

    // Target / parent.
    (target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::DEBUG, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::DEBUG, { $($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::DEBUG, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::DEBUG, { %$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::DEBUG, {}, $($arg)+)
    );

    // Name / parent.
    (name: $name:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::DEBUG, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::DEBUG, { $($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::DEBUG, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::DEBUG, { %$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::DEBUG, {}, $($arg)+)
    );

    // Name.
    (name: $name:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::DEBUG, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::DEBUG, { $($k).+ $($field)* })
    );
    (name: $name:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::DEBUG, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::DEBUG, { %$($k).+ $($field)* })
    );
    (name: $name:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, $crate::Level::DEBUG, {}, $($arg)+)
    );

    // Target.
    (target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::DEBUG, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::DEBUG, { $($k).+ $($field)* })
    );
    (target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::DEBUG, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::DEBUG, { %$($k).+ $($field)* })
    );
    (target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, $crate::Level::DEBUG, {}, $($arg)+)
    );

    // Parent.
    (parent: $parent:expr, { $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            { $($field)+ },
            $($arg)+
        )
    );
    (parent: $parent:expr, $($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            { $($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            { ?$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            { %$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            { $($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            { ?$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            { %$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::DEBUG,
            {},
            $($arg)+
        )
    );

    // ...
    ({ $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { $($field)+ },
            $($arg)+
        )
    );
    ($($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { $($k).+ = $($field)*}
        )
    );
    (?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { ?$($k).+ = $($field)*}
        )
    );
    (%$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { %$($k).+ = $($field)*}
        )
    );
    ($($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { $($k).+, $($field)*}
        )
    );
    (?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { ?$($k).+, $($field)*}
        )
    );
    (%$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { %$($k).+, $($field)*}
        )
    );
    (?$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { ?$($k).+ }
        )
    );
    (%$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { %$($k).+ }
        )
    );
    ($($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            { $($k).+ }
        )
    );
    ($($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::DEBUG,
            $($arg)+
        )
    );
}

/// Constructs an event at the info level.
///
/// This functions similarly to the [`event!`] macro. See [the top-level
/// documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [`event!`]: crate::event!
/// [lib]: crate#using-the-macros
///
/// # Examples
///
/// ```rust
/// use tracing::info;
/// # // this is so the test will still work in no-std mode
/// # #[derive(Debug)]
/// # pub struct Ipv4Addr;
/// # impl Ipv4Addr { fn new(o1: u8, o2: u8, o3: u8, o4: u8) -> Self { Self } }
/// # fn main() {
/// # struct Connection { port: u32, speed: f32 }
/// use tracing::field;
///
/// let addr = Ipv4Addr::new(127, 0, 0, 1);
/// let conn = Connection { port: 40, speed: 3.20 };
///
/// info!(conn.port, "connected to {:?}", addr);
/// info!(
///     target: "connection_events",
///     ip = ?addr,
///     conn.port,
///     ?conn.speed,
/// );
/// info!(name: "completed", "completed connection to {:?}", addr);
/// # }
/// ```
#[macro_export]
macro_rules! info {
    // Name / target / parent.
    (name: $name:expr, target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::INFO, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::INFO, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::INFO, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::INFO, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::INFO, {}, $($arg)+)
    );

    // Name / target.
    (name: $name:expr, target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::INFO, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::INFO, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::INFO, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::INFO, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::INFO, {}, $($arg)+)
    );

    // Target / parent.
    (target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::INFO, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::INFO, { $($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::INFO, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::INFO, { %$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::INFO, {}, $($arg)+)
    );

    // Name / parent.
    (name: $name:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::INFO, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::INFO, { $($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::INFO, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::INFO, { %$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::INFO, {}, $($arg)+)
    );

    // Name.
    (name: $name:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::INFO, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::INFO, { $($k).+ $($field)* })
    );
    (name: $name:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::INFO, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::INFO, { %$($k).+ $($field)* })
    );
    (name: $name:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, $crate::Level::INFO, {}, $($arg)+)
    );

    // Target.
    (target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::INFO, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::INFO, { $($k).+ $($field)* })
    );
    (target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::INFO, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::INFO, { %$($k).+ $($field)* })
    );
    (target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, $crate::Level::INFO, {}, $($arg)+)
    );

    // Parent.
    (parent: $parent:expr, { $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            { $($field)+ },
            $($arg)+
        )
    );
    (parent: $parent:expr, $($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            { $($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            { ?$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            { %$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            { $($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            { ?$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            { %$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::INFO,
            {},
            $($arg)+
        )
    );

    // ...
    ({ $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { $($field)+ },
            $($arg)+
        )
    );
    ($($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { $($k).+ = $($field)*}
        )
    );
    (?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { ?$($k).+ = $($field)*}
        )
    );
    (%$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { %$($k).+ = $($field)*}
        )
    );
    ($($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { $($k).+, $($field)*}
        )
    );
    (?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { ?$($k).+, $($field)*}
        )
    );
    (%$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { %$($k).+, $($field)*}
        )
    );
    (?$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { ?$($k).+ }
        )
    );
    (%$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { %$($k).+ }
        )
    );
    ($($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            { $($k).+ }
        )
    );
    ($($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::INFO,
            $($arg)+
        )
    );
}

/// Constructs an event at the warn level.
///
/// This functions similarly to the [`event!`] macro. See [the top-level
/// documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [`event!`]: crate::event!
/// [lib]: crate#using-the-macros
///
/// # Examples
///
/// ```rust
/// use tracing::warn;
/// # fn main() {
///
/// let warn_description = "Invalid Input";
/// let input = &[0x27, 0x45];
///
/// warn!(?input, warning = warn_description);
/// warn!(
///     target: "input_events",
///     warning = warn_description,
///     "Received warning for input: {:?}", input,
/// );
/// warn!(name: "invalid", ?input);
/// # }
/// ```
#[macro_export]
macro_rules! warn {
    // Name / target / parent.
    (name: $name:expr, target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::WARN, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::WARN, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::WARN, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::WARN, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::WARN, {}, $($arg)+)
    );

    // Name / target.
    (name: $name:expr, target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::WARN, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::WARN, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::WARN, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::WARN, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::WARN, {}, $($arg)+)
    );

    // Target / parent.
    (target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::WARN, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::WARN, { $($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::WARN, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::WARN, { %$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::WARN, {}, $($arg)+)
    );

    // Name / parent.
    (name: $name:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::WARN, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::WARN, { $($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::WARN, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::WARN, { %$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::WARN, {}, $($arg)+)
    );

    // Name.
    (name: $name:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::WARN, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::WARN, { $($k).+ $($field)* })
    );
    (name: $name:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::WARN, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::WARN, { %$($k).+ $($field)* })
    );
    (name: $name:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, $crate::Level::WARN, {}, $($arg)+)
    );

    // Target.
    (target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::WARN, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::WARN, { $($k).+ $($field)* })
    );
    (target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::WARN, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::WARN, { %$($k).+ $($field)* })
    );
    (target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, $crate::Level::WARN, {}, $($arg)+)
    );

    // Parent.
    (parent: $parent:expr, { $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            { $($field)+ },
            $($arg)+
        )
    );
    (parent: $parent:expr, $($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            { $($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            { ?$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            { %$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            { $($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            { ?$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            { %$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::WARN,
            {},
            $($arg)+
        )
    );

    // ...
    ({ $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { $($field)+ },
            $($arg)+
        )
    );
    ($($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { $($k).+ = $($field)*}
        )
    );
    (?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { ?$($k).+ = $($field)*}
        )
    );
    (%$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { %$($k).+ = $($field)*}
        )
    );
    ($($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { $($k).+, $($field)*}
        )
    );
    (?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { ?$($k).+, $($field)*}
        )
    );
    (%$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { %$($k).+, $($field)*}
        )
    );
    (?$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { ?$($k).+ }
        )
    );
    (%$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { %$($k).+ }
        )
    );
    ($($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            { $($k).+ }
        )
    );
    ($($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::WARN,
            $($arg)+
        )
    );
}

/// Constructs an event at the error level.
///
/// This functions similarly to the [`event!`] macro. See [the top-level
/// documentation][lib] for details on the syntax accepted by
/// this macro.
///
/// [`event!`]: crate::event!
/// [lib]: crate#using-the-macros
///
/// # Examples
///
/// ```rust
/// use tracing::error;
/// # fn main() {
///
/// let (err_info, port) = ("No connection", 22);
///
/// error!(port, error = %err_info);
/// error!(target: "app_events", "App Error: {}", err_info);
/// error!({ info = err_info }, "error on port: {}", port);
/// error!(name: "invalid_input", "Invalid input: {}", err_info);
/// # }
/// ```
#[macro_export]
macro_rules! error {
    // Name / target / parent.
    (name: $name:expr, target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::ERROR, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::ERROR, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::ERROR, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::ERROR, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, parent: $parent, $crate::Level::ERROR, {}, $($arg)+)
    );

    // Name / target.
    (name: $name:expr, target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::ERROR, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::ERROR, { $($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::ERROR, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::ERROR, { %$($k).+ $($field)* })
    );
    (name: $name:expr, target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, target: $target, $crate::Level::ERROR, {}, $($arg)+)
    );

    // Target / parent.
    (target: $target:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::ERROR, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::ERROR, { $($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::ERROR, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::ERROR, { %$($k).+ $($field)* })
    );
    (target: $target:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, parent: $parent, $crate::Level::ERROR, {}, $($arg)+)
    );

    // Name / parent.
    (name: $name:expr, parent: $parent:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::ERROR, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, parent: $parent:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::ERROR, { $($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::ERROR, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::ERROR, { %$($k).+ $($field)* })
    );
    (name: $name:expr, parent: $parent:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, parent: $parent, $crate::Level::ERROR, {}, $($arg)+)
    );

    // Name.
    (name: $name:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::ERROR, { $($field)* }, $($arg)*)
    );
    (name: $name:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::ERROR, { $($k).+ $($field)* })
    );
    (name: $name:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::ERROR, { ?$($k).+ $($field)* })
    );
    (name: $name:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(name: $name, $crate::Level::ERROR, { %$($k).+ $($field)* })
    );
    (name: $name:expr, $($arg:tt)+ ) => (
        $crate::event!(name: $name, $crate::Level::ERROR, {}, $($arg)+)
    );

    // Target.
    (target: $target:expr, { $($field:tt)* }, $($arg:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::ERROR, { $($field)* }, $($arg)*)
    );
    (target: $target:expr, $($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::ERROR, { $($k).+ $($field)* })
    );
    (target: $target:expr, ?$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::ERROR, { ?$($k).+ $($field)* })
    );
    (target: $target:expr, %$($k:ident).+ $($field:tt)* ) => (
        $crate::event!(target: $target, $crate::Level::ERROR, { %$($k).+ $($field)* })
    );
    (target: $target:expr, $($arg:tt)+ ) => (
        $crate::event!(target: $target, $crate::Level::ERROR, {}, $($arg)+)
    );

    // Parent.
    (parent: $parent:expr, { $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            { $($field)+ },
            $($arg)+
        )
    );
    (parent: $parent:expr, $($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            { $($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            { ?$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            { %$($k).+ = $($field)*}
        )
    );
    (parent: $parent:expr, $($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            { $($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, ?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            { ?$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, %$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            { %$($k).+, $($field)*}
        )
    );
    (parent: $parent:expr, $($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            parent: $parent,
            $crate::Level::ERROR,
            {},
            $($arg)+
        )
    );

    // ...
    ({ $($field:tt)+ }, $($arg:tt)+ ) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { $($field)+ },
            $($arg)+
        )
    );
    ($($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { $($k).+ = $($field)*}
        )
    );
    (?$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { ?$($k).+ = $($field)*}
        )
    );
    (%$($k:ident).+ = $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { %$($k).+ = $($field)*}
        )
    );
    ($($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { $($k).+, $($field)*}
        )
    );
    (?$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { ?$($k).+, $($field)*}
        )
    );
    (%$($k:ident).+, $($field:tt)*) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { %$($k).+, $($field)*}
        )
    );
    (?$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { ?$($k).+ }
        )
    );
    (%$($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { %$($k).+ }
        )
    );
    ($($k:ident).+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            { $($k).+ }
        )
    );
    ($($arg:tt)+) => (
        $crate::event!(
            target: module_path!(),
            $crate::Level::ERROR,
            $($arg)+
        )
    );
}

/// Constructs a new static callsite for a span or event.
#[doc(hidden)]
#[macro_export]
macro_rules! callsite {
    (name: $name:expr, kind: $kind:expr, fields: $($fields:tt)*) => {{
        $crate::callsite! {
            name: $name,
            kind: $kind,
            target: module_path!(),
            level: $crate::Level::TRACE,
            fields: $($fields)*
        }
    }};
    (
        name: $name:expr,
        kind: $kind:expr,
        level: $lvl:expr,
        fields: $($fields:tt)*
    ) => {{
        $crate::callsite! {
            name: $name,
            kind: $kind,
            target: module_path!(),
            level: $lvl,
            fields: $($fields)*
        }
    }};
    (
        name: $name:expr,
        kind: $kind:expr,
        target: $target:expr,
        level: $lvl:expr,
        fields: $($fields:tt)*
    ) => {{
        static META: $crate::Metadata<'static> = {
            $crate::metadata! {
                name: $name,
                target: $target,
                level: $lvl,
                fields: $crate::fieldset!( $($fields)* ),
                callsite: &__CALLSITE,
                kind: $kind,
            }
        };
        static __CALLSITE: $crate::callsite::DefaultCallsite = $crate::callsite::DefaultCallsite::new(&META);
        __CALLSITE.register();
        &__CALLSITE
    }};
}

/// Constructs a new static callsite for a span or event.
#[doc(hidden)]
#[macro_export]
macro_rules! callsite2 {
    (name: $name:expr, kind: $kind:expr, fields: $($fields:tt)*) => {{
        $crate::callsite2! {
            name: $name,
            kind: $kind,
            target: module_path!(),
            level: $crate::Level::TRACE,
            fields: $($fields)*
        }
    }};
    (
        name: $name:expr,
        kind: $kind:expr,
        level: $lvl:expr,
        fields: $($fields:tt)*
    ) => {{
        $crate::callsite2! {
            name: $name,
            kind: $kind,
            target: module_path!(),
            level: $lvl,
            fields: $($fields)*
        }
    }};
    (
        name: $name:expr,
        kind: $kind:expr,
        target: $target:expr,
        level: $lvl:expr,
        fields: $($fields:tt)*
    ) => {{
        static META: $crate::Metadata<'static> = {
            $crate::metadata! {
                name: $name,
                target: $target,
                level: $lvl,
                fields: $crate::fieldset!( $($fields)* ),
                callsite: &__CALLSITE,
                kind: $kind,
            }
        };
        $crate::callsite::DefaultCallsite::new(&META)
    }};
}

#[macro_export]
// TODO: determine if this ought to be public API?`
#[doc(hidden)]
macro_rules! level_enabled {
    ($lvl:expr) => {
        $lvl <= $crate::level_filters::STATIC_MAX_LEVEL
            && $lvl <= $crate::level_filters::LevelFilter::current()
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! valueset {

    // === base case ===
    (@ { $(,)* $($val:expr),* $(,)* }, $next:expr $(,)*) => {
        &[ $($val),* ]
    };

    // === recursive case (more tts) ===

    // TODO(#1138): determine a new syntax for uninitialized span fields, and
    // re-enable this.
    // (@{ $(,)* $($out:expr),* }, $next:expr, $($k:ident).+ = _, $($rest:tt)*) => {
    //     $crate::valueset!(@ { $($out),*, (&$next, None) }, $next, $($rest)*)
    // };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+ = ?$val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&debug(&$val) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+ = %$val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&display(&$val) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+ = $val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&$val as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&$($k).+ as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, ?$($k:ident).+, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&debug(&$($k).+) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, %$($k:ident).+, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&display(&$($k).+) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+ = ?$val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&debug(&$val) as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+ = %$val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&display(&$val) as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+ = $val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&$val as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $($k:ident).+) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&$($k).+ as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, ?$($k:ident).+) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&debug(&$($k).+) as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, %$($k:ident).+) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&display(&$($k).+) as &dyn Value)) },
            $next,
        )
    };

    // Handle literal names
    (@ { $(,)* $($out:expr),* }, $next:expr, $k:literal = ?$val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&debug(&$val) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $k:literal = %$val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&display(&$val) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $k:literal = $val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&$val as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $k:literal = ?$val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&debug(&$val) as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $k:literal = %$val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&display(&$val) as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, $k:literal = $val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, $crate::__macro_support::Option::Some(&$val as &dyn Value)) },
            $next,
        )
    };

    // Handle constant names
    (@ { $(,)* $($out:expr),* }, $next:expr, { $k:expr } = ?$val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, Some(&debug(&$val) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, { $k:expr } = %$val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, Some(&display(&$val) as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, { $k:expr } = $val:expr, $($rest:tt)*) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, Some(&$val as &dyn Value)) },
            $next,
            $($rest)*
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, { $k:expr } = ?$val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, Some(&debug(&$val) as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, { $k:expr } = %$val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, Some(&display(&$val) as &dyn Value)) },
            $next,
        )
    };
    (@ { $(,)* $($out:expr),* }, $next:expr, { $k:expr } = $val:expr) => {
        $crate::valueset!(
            @ { $($out),*, (&$next, Some(&$val as &dyn Value)) },
            $next,
        )
    };

    // Remainder is unparsable, but exists --- must be format args!
    (@ { $(,)* $($out:expr),* }, $next:expr, $($rest:tt)+) => {
        $crate::valueset!(@ { (&$next, $crate::__macro_support::Option::Some(&$crate::__macro_support::format_args!($($rest)+) as &dyn Value)), $($out),* }, $next, )
    };

    // === entry ===
    ($fields:expr, $($kvs:tt)+) => {
        {
            #[allow(unused_imports)]
            use $crate::field::{debug, display, Value};
            let mut iter = $fields.iter();
            $fields.value_set($crate::valueset!(
                @ { },
                $crate::__macro_support::Iterator::next(&mut iter).expect("FieldSet corrupted (this is a bug)"),
                $($kvs)+
            ))
        }
    };
    ($fields:expr,) => {
        {
            $fields.value_set(&[])
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! fieldset {
    // == base case ==
    (@ { $(,)* $($out:expr),* $(,)* } $(,)*) => {
        &[ $($out),* ]
    };

    // == recursive cases (more tts) ==
    (@ { $(,)* $($out:expr),* } $($k:ident).+ = ?$val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $crate::__tracing_stringify!($($k).+) } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } $($k:ident).+ = %$val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $crate::__tracing_stringify!($($k).+) } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } $($k:ident).+ = $val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $crate::__tracing_stringify!($($k).+) } $($rest)*)
    };
    // TODO(#1138): determine a new syntax for uninitialized span fields, and
    // re-enable this.
    // (@ { $($out:expr),* } $($k:ident).+ = _, $($rest:tt)*) => {
    //     $crate::fieldset!(@ { $($out),*, $crate::__tracing_stringify!($($k).+) } $($rest)*)
    // };
    (@ { $(,)* $($out:expr),* } ?$($k:ident).+, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $crate::__tracing_stringify!($($k).+) } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } %$($k:ident).+, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $crate::__tracing_stringify!($($k).+) } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } $($k:ident).+, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $crate::__tracing_stringify!($($k).+) } $($rest)*)
    };

    // Handle literal names
    (@ { $(,)* $($out:expr),* } $k:literal = ?$val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $k } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } $k:literal = %$val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $k } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } $k:literal = $val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $k } $($rest)*)
    };

    // Handle constant names
    (@ { $(,)* $($out:expr),* } { $k:expr } = ?$val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $k } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } { $k:expr } = %$val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $k } $($rest)*)
    };
    (@ { $(,)* $($out:expr),* } { $k:expr } = $val:expr, $($rest:tt)*) => {
        $crate::fieldset!(@ { $($out),*, $k } $($rest)*)
    };

    // Remainder is unparsable, but exists --- must be format args!
    (@ { $(,)* $($out:expr),* } $($rest:tt)+) => {
        $crate::fieldset!(@ { "message", $($out),*, })
    };

    // == entry ==
    ($($args:tt)*) => {
        $crate::fieldset!(@ { } $($args)*,)
    };

}

#[cfg(feature = "log")]
#[doc(hidden)]
#[macro_export]
macro_rules! level_to_log {
    ($level:expr) => {
        match $level {
            $crate::Level::ERROR => $crate::log::Level::Error,
            $crate::Level::WARN => $crate::log::Level::Warn,
            $crate::Level::INFO => $crate::log::Level::Info,
            $crate::Level::DEBUG => $crate::log::Level::Debug,
            _ => $crate::log::Level::Trace,
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __tracing_stringify {
    ($($t:tt)*) => {
        stringify!($($t)*)
    };
}

#[cfg(not(feature = "log"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __tracing_log {
    ($level:expr, $callsite:expr, $value_set:expr) => {};
}

#[cfg(feature = "log")]
#[doc(hidden)]
#[macro_export]
macro_rules! __tracing_log {
    ($level:expr, $callsite:expr, $value_set:expr) => {
        $crate::if_log_enabled! { $level, {
            use $crate::log;
            let level = $crate::level_to_log!($level);
            if level <= log::max_level() {
                let meta = $callsite.metadata();
                let log_meta = log::Metadata::builder()
                    .level(level)
                    .target(meta.target())
                    .build();
                let logger = log::logger();
                if logger.enabled(&log_meta) {
                    $crate::__macro_support::__tracing_log(meta, logger, log_meta, $value_set)
                }
            }
        }}
    };
}

#[cfg(not(feature = "log"))]
#[doc(hidden)]
#[macro_export]
macro_rules! if_log_enabled {
    ($lvl:expr, $e:expr;) => {
        $crate::if_log_enabled! { $lvl, $e }
    };
    ($lvl:expr, $if_log:block) => {
        $crate::if_log_enabled! { $lvl, $if_log else {} }
    };
    ($lvl:expr, $if_log:block else $else_block:block) => {
        $else_block
    };
}

#[cfg(all(feature = "log", not(feature = "log-always")))]
#[doc(hidden)]
#[macro_export]
macro_rules! if_log_enabled {
    ($lvl:expr, $e:expr;) => {
        $crate::if_log_enabled! { $lvl, $e }
    };
    ($lvl:expr, $if_log:block) => {
        $crate::if_log_enabled! { $lvl, $if_log else {} }
    };
    ($lvl:expr, $if_log:block else $else_block:block) => {
        if $crate::level_to_log!($lvl) <= $crate::log::STATIC_MAX_LEVEL {
            if !$crate::dispatcher::has_been_set() {
                $if_log
            } else {
                $else_block
            }
        } else {
            $else_block
        }
    };
}

#[cfg(all(feature = "log", feature = "log-always"))]
#[doc(hidden)]
#[macro_export]
macro_rules! if_log_enabled {
    ($lvl:expr, $e:expr;) => {
        $crate::if_log_enabled! { $lvl, $e }
    };
    ($lvl:expr, $if_log:block) => {
        $crate::if_log_enabled! { $lvl, $if_log else {} }
    };
    ($lvl:expr, $if_log:block else $else_block:block) => {
        if $crate::level_to_log!($lvl) <= $crate::log::STATIC_MAX_LEVEL {
            #[allow(unused_braces)]
            $if_log
        } else {
            $else_block
        }
    };
}
