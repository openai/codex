use std::backtrace::Backtrace;

use super::*;

/// Asserts that backtrace returned by `Error::backtrace` method equals/not-equals
/// backtrace stored in object itself.
///
/// Comparison is done by converting backtraces to strings
/// and then comparing these strings.
///
/// ## Syntax
///
/// * Equals: `assert_bt!(==, ...)`
/// * Not-equals: `assert_bt!(!=, ...)`
///
/// ### Backtrace Access
///
/// Shortcut for named-structs with `backtrace` field.
/// Access backtrace as `error.backtrace`.
///
/// ```
/// assert_bt!(==, error);
/// ```
///
/// Full form for named- and tuple-structs.
/// Access backtrace as `error.some_other_field` and `error.1` respectively.
///
/// ```
/// assert_bt!(!=, error, some_other_field);
/// assert_bt!(==, error, 1);
/// ```
///
/// Access as a method call.
/// Useful for enums (i.e., you can define a method that will match on enum variants
/// and return backtrace for each variant).
/// Access backtrace as `error.get_stored_backtrace_method()`.
///
/// ```
/// assert_bt!(!=, error, .get_stored_backtrace_method);
/// ```
macro_rules! assert_bt {
    (@impl $macro:ident, $error:expr, $backtrace:expr) => {
        $macro!(::core::error::request_ref::<Backtrace>(&$error).unwrap().to_string(), $backtrace.to_string());
    };
    (@expand $macro:ident, $error:expr, .$backtrace:ident) => {
        assert_bt!(@impl $macro, $error, $error.$backtrace())
    };
    (@expand $macro:ident, $error:expr, .$backtrace:tt) => {
        assert_bt!(@impl $macro, $error, $error.$backtrace)
    };
    (@expand $macro:ident, $error:expr, $backtrace:ident) => {
        assert_bt!(@impl $macro, $error, $error.$backtrace)
    };
    (@expand $macro:ident, $error:expr, $backtrace:expr) => {
        assert_bt!(@impl $macro, $error, $backtrace)
    };
    (@expand $macro:ident, $error:expr) => {
        assert_bt!(@expand $macro, $error, backtrace)
    };
    (==, $($args:tt)*) => {
        assert_bt!(@expand assert_eq, $($args)*)
    };
    (!=, $($args:tt)*) => {
        assert_bt!(@expand assert_ne, $($args)*)
    };
}

mod derives_for_enums_with_backtrace;
mod derives_for_generic_enums_with_backtrace;
mod derives_for_generic_structs_with_backtrace;
mod derives_for_structs_with_backtrace;

derive_display!(BacktraceErr);
#[derive(Debug)]
struct BacktraceErr {
    backtrace: Backtrace,
}

impl Default for BacktraceErr {
    fn default() -> Self {
        Self {
            backtrace: Backtrace::force_capture(),
        }
    }
}

impl Error for BacktraceErr {
    fn provide<'a>(&'a self, request: &mut std::error::Request<'a>) {
        request
            .provide_ref::<Backtrace>(&self.backtrace)
            .provide_value::<i32>(42);
    }
}
