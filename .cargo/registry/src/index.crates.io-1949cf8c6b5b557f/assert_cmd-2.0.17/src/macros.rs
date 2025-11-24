/// Allows you to pull the name from your Cargo.toml at compile time.
///
/// # Examples
///
/// ```should_panic
/// use assert_cmd::Command;
///
/// let mut cmd = Command::cargo_bin(assert_cmd::crate_name!()).unwrap();
/// let assert = cmd
///     .arg("-A")
///     .env("stdout", "hello")
///     .env("exit", "42")
///     .write_stdin("42")
///     .assert();
/// assert
///     .failure()
///     .code(42)
///     .stdout("hello\n");
/// ```
#[macro_export]
macro_rules! crate_name {
    () => {
        env!("CARGO_PKG_NAME")
    };
}

/// The absolute path to a binary target's executable.
///
/// The `bin_target_name` is the name of the binary
/// target, exactly as-is.
///
/// **NOTE:** This is only set when building an integration test or benchmark.
///
/// ## Example
///
/// ```rust,no_run
/// #[test]
/// fn cli_tests() {
///     trycmd::TestCases::new()
///         .default_bin_path(trycmd::cargo_bin!("bin-fixture"))
///         .case("tests/cmd/*.trycmd");
/// }
/// ```
#[macro_export]
#[doc(hidden)]
macro_rules! cargo_bin {
    ($bin_target_name:expr) => {
        ::std::path::Path::new(env!(concat!("CARGO_BIN_EXE_", $bin_target_name)))
    };
}
