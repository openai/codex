/// Asserts that a given configuration is set.
///
/// # Examples
///
/// A project will simply fail to compile if the given configuration is not set.
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// // We're not masochists
/// # #[cfg(not(target_pointer_width = "16"))] // Just in case
/// assert_cfg!(not(target_pointer_width = "16"));
/// ```
///
/// If a project does not support a set of configurations, you may want to
/// report why. There is the option of providing a compile error message string:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// # #[cfg(any(unix, windows))]
/// assert_cfg!(any(unix, windows), "There is only support for Unix or Windows");
///
/// // User needs to specify a database back-end
/// # #[cfg(target_pointer_width = "0")] // Impossible
/// assert_cfg!(all(not(all(feature = "mysql", feature = "mongodb")),
///                 any(    feature = "mysql", feature = "mongodb")),
///             "Must exclusively use MySQL or MongoDB as database back-end");
/// ```
///
/// Some configurations are impossible. For example, we can't be compiling for
/// both macOS _and_ Windows simultaneously:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_cfg!(all(target_os = "macos",
///                 target_os = "windows"),
///             "No, that's not how it works! ಠ_ಠ");
/// ```
#[macro_export]
macro_rules! assert_cfg {
    () => {};
    ($($cfg:meta)+, $msg:expr $(,)?) => {
        #[cfg(not($($cfg)+))]
        compile_error!($msg);
    };
    ($($cfg:tt)*) => {
        #[cfg(not($($cfg)*))]
        compile_error!(concat!("Cfg does not pass: ", stringify!($($cfg)*)));
    };
}
