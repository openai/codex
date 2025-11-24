/// Prints to [`stdout`][crate::stdout].
///
/// Equivalent to the [`println!`] macro except that a newline is not printed at
/// the end of the message.
///
/// Note that stdout is frequently line-buffered by default so it may be
/// necessary to use [`std::io::Write::flush()`] to ensure the output is emitted
/// immediately.
///
/// **NOTE:** The `print!` macro will lock the standard output on each call. If you call
/// `print!` within a hot loop, this behavior may be the bottleneck of the loop.
/// To avoid this, lock stdout with [`AutoStream::lock`][crate::AutoStream::lock]:
/// ```
/// #  #[cfg(feature = "auto")] {
/// use std::io::Write as _;
///
/// let mut lock = anstream::stdout().lock();
/// write!(lock, "hello world").unwrap();
/// # }
/// ```
///
/// Use `print!` only for the primary output of your program. Use
/// [`eprint!`] instead to print error and progress messages.
///
/// **NOTE:** Not all `print!` calls will be captured in tests like [`std::print!`]
/// - Capturing will automatically be activated in test binaries
/// - Otherwise, only when the `test` feature is enabled
///
/// # Panics
///
/// Panics if writing to `stdout` fails for any reason **except** broken pipe.
///
/// Writing to non-blocking stdout can cause an error, which will lead
/// this macro to panic.
///
/// # Examples
///
/// ```
/// #  #[cfg(feature = "auto")] {
/// use std::io::Write as _;
/// use anstream::print;
/// use anstream::stdout;
///
/// print!("this ");
/// print!("will ");
/// print!("be ");
/// print!("on ");
/// print!("the ");
/// print!("same ");
/// print!("line ");
///
/// stdout().flush().unwrap();
///
/// print!("this string has a newline, why not choose println! instead?\n");
///
/// stdout().flush().unwrap();
/// # }
/// ```
#[cfg(feature = "auto")]
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        if cfg!(test) || $crate::_macros::FEATURE_TEST_ACTIVATED {
            let target_stream = std::io::stdout();
            let buffer = $crate::_macros::to_adapted_string(&format_args!($($arg)*), &target_stream);
            ::std::print!("{}", buffer)
        } else {
            use std::io::Write as _;

            let mut stream = $crate::stdout();
            match ::std::write!(&mut stream, $($arg)*) {
                Err(e) if e.kind() != ::std::io::ErrorKind::BrokenPipe => {
                    ::std::panic!("failed printing to stdout: {e}");
                }
                Err(_) | Ok(_) => {}
            }
        }
    }};
}

/// Prints to [`stdout`][crate::stdout], with a newline.
///
/// On all platforms, the newline is the LINE FEED character (`\n`/`U+000A`) alone
/// (no additional CARRIAGE RETURN (`\r`/`U+000D`)).
///
/// This macro uses the same syntax as [`format!`], but writes to the standard output instead.
/// See [`std::fmt`] for more information.
///
/// **NOTE:** The `println!` macro will lock the standard output on each call. If you call
/// `println!` within a hot loop, this behavior may be the bottleneck of the loop.
/// To avoid this, lock stdout with [`AutoStream::lock`][crate::AutoStream::lock]:
/// ```
/// #  #[cfg(feature = "auto")] {
/// use std::io::Write as _;
///
/// let mut lock = anstream::stdout().lock();
/// writeln!(lock, "hello world").unwrap();
/// # }
/// ```
///
/// Use `println!` only for the primary output of your program. Use
/// [`eprintln!`] instead to print error and progress messages.
///
/// **NOTE:** Not all `println!` calls will be captured in tests like [`std::println!`]
/// - Capturing will automatically be activated in test binaries
/// - Otherwise, only when the `test` feature is enabled
///
/// # Panics
///
/// Panics if writing to `stdout` fails for any reason **except** broken pipe.
///
/// Writing to non-blocking stdout can cause an error, which will lead
/// this macro to panic.
///
/// # Examples
///
/// ```
/// #  #[cfg(feature = "auto")] {
/// use anstream::println;
///
/// println!(); // prints just a newline
/// println!("hello there!");
/// println!("format {} arguments", "some");
/// let local_variable = "some";
/// println!("format {local_variable} arguments");
/// # }
/// ```
#[cfg(feature = "auto")]
#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        if cfg!(test) || $crate::_macros::FEATURE_TEST_ACTIVATED {
            let target_stream = std::io::stdout();
            let buffer = $crate::_macros::to_adapted_string(&format_args!($($arg)*), &target_stream);
            ::std::println!("{}", buffer)
        } else {
            use std::io::Write as _;

            let mut stream = $crate::stdout();
            match ::std::writeln!(&mut stream, $($arg)*) {
                Err(e) if e.kind() != ::std::io::ErrorKind::BrokenPipe => {
                    ::std::panic!("failed printing to stdout: {e}");
                }
                Err(_) | Ok(_) => {}
            }
        }
    }};
}

/// Prints to [`stderr`][crate::stderr].
///
/// Equivalent to the [`print!`] macro, except that output goes to
/// `stderr` instead of `stdout`. See [`print!`] for
/// example usage.
///
/// Use `eprint!` only for error and progress messages. Use `print!`
/// instead for the primary output of your program.
///
/// **NOTE:** Not all `eprint!` calls will be captured in tests like [`std::eprint!`]
/// - Capturing will automatically be activated in test binaries
/// - Otherwise, only when the `test` feature is enabled
///
/// # Panics
///
/// Panics if writing to `stderr` fails for any reason **except** broken pipe.
///
/// Writing to non-blocking stdout can cause an error, which will lead
/// this macro to panic.
///
/// # Examples
///
/// ```
/// #  #[cfg(feature = "auto")] {
/// use anstream::eprint;
///
/// eprint!("Error: Could not complete task");
/// # }
/// ```
#[cfg(feature = "auto")]
#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        if cfg!(test) || $crate::_macros::FEATURE_TEST_ACTIVATED {
            let target_stream = std::io::stderr();
            let buffer = $crate::_macros::to_adapted_string(&format_args!($($arg)*), &target_stream);
            ::std::eprint!("{}", buffer)
        } else {
            use std::io::Write as _;

            let mut stream = $crate::stderr();
            match ::std::write!(&mut stream, $($arg)*) {
                Err(e) if e.kind() != ::std::io::ErrorKind::BrokenPipe => {
                    ::std::panic!("failed printing to stderr: {e}");
                }
                Err(_) | Ok(_) => {}
            }
        }
    }};
}

/// Prints to [`stderr`][crate::stderr], with a newline.
///
/// Equivalent to the [`println!`] macro, except that output goes to
/// `stderr` instead of `stdout`. See [`println!`] for
/// example usage.
///
/// Use `eprintln!` only for error and progress messages. Use `println!`
/// instead for the primary output of your program.
///
/// **NOTE:** Not all `eprintln!` calls will be captured in tests like [`std::eprintln!`]
/// - Capturing will automatically be activated in test binaries
/// - Otherwise, only when the `test` feature is enabled
///
/// # Panics
///
/// Panics if writing to `stderr` fails for any reason **except** broken pipe.
///
/// Writing to non-blocking stdout can cause an error, which will lead
/// this macro to panic.
///
/// # Examples
///
/// ```
/// #  #[cfg(feature = "auto")] {
/// use anstream::eprintln;
///
/// eprintln!("Error: Could not complete task");
/// # }
/// ```
#[cfg(feature = "auto")]
#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::eprint!("\n")
    };
    ($($arg:tt)*) => {{
        if cfg!(test) || $crate::_macros::FEATURE_TEST_ACTIVATED {
            let target_stream = std::io::stderr();
            let buffer = $crate::_macros::to_adapted_string(&format_args!($($arg)*), &target_stream);
            ::std::eprintln!("{}", buffer)
        } else {
            use std::io::Write as _;

            let mut stream = $crate::stderr();
            match ::std::writeln!(&mut stream, $($arg)*) {
                Err(e) if e.kind() != ::std::io::ErrorKind::BrokenPipe => {
                    ::std::panic!("failed printing to stderr: {e}");
                }
                Err(_) | Ok(_) => {}
            }
        }
    }};
}

/// Panics the current thread.
///
/// This allows a program to terminate immediately and provide feedback
/// to the caller of the program.
///
/// This macro is the perfect way to assert conditions in example code and in
/// tests. `panic!` is closely tied with the `unwrap` method of both
/// [`Option`][ounwrap] and [`Result`][runwrap] enums. Both implementations call
/// `panic!` when they are set to [`None`] or [`Err`] variants.
///
/// When using `panic!()` you can specify a string payload, that is built using
/// the [`format!`] syntax. That payload is used when injecting the panic into
/// the calling Rust thread, causing the thread to panic entirely.
///
/// The behavior of the default `std` hook, i.e. the code that runs directly
/// after the panic is invoked, is to print the message payload to
/// `stderr` along with the file/line/column information of the `panic!()`
/// call. You can override the panic hook using [`std::panic::set_hook()`].
/// Inside the hook a panic can be accessed as a `&dyn Any + Send`,
/// which contains either a `&str` or `String` for regular `panic!()` invocations.
/// To panic with a value of another other type, [`panic_any`] can be used.
///
/// See also the macro [`compile_error!`], for raising errors during compilation.
///
/// # When to use `panic!` vs `Result`
///
/// The Rust language provides two complementary systems for constructing /
/// representing, reporting, propagating, reacting to, and discarding errors. These
/// responsibilities are collectively known as "error handling." `panic!` and
/// `Result` are similar in that they are each the primary interface of their
/// respective error handling systems; however, the meaning these interfaces attach
/// to their errors and the responsibilities they fulfill within their respective
/// error handling systems differ.
///
/// The `panic!` macro is used to construct errors that represent a bug that has
/// been detected in your program. With `panic!` you provide a message that
/// describes the bug and the language then constructs an error with that message,
/// reports it, and propagates it for you.
///
/// `Result` on the other hand is used to wrap other types that represent either
/// the successful result of some computation, `Ok(T)`, or error types that
/// represent an anticipated runtime failure mode of that computation, `Err(E)`.
/// `Result` is used alongside user defined types which represent the various
/// anticipated runtime failure modes that the associated computation could
/// encounter. `Result` must be propagated manually, often with the help of the
/// `?` operator and `Try` trait, and they must be reported manually, often with
/// the help of the `Error` trait.
///
/// For more detailed information about error handling check out the [book] or the
/// [`std::result`] module docs.
///
/// [ounwrap]: Option::unwrap
/// [runwrap]: Result::unwrap
/// [`std::panic::set_hook()`]: ../std/panic/fn.set_hook.html
/// [`panic_any`]: ../std/panic/fn.panic_any.html
/// [`Box`]: ../std/boxed/struct.Box.html
/// [`format!`]: ../std/macro.format.html
/// [book]: ../book/ch09-00-error-handling.html
/// [`std::result`]: ../std/result/index.html
///
/// # Current implementation
///
/// If the main thread panics it will terminate all your threads and end your
/// program with code `101`.
///
/// # Examples
///
/// ```should_panic
/// # #![allow(unreachable_code)]
/// use anstream::panic;
/// panic!();
/// panic!("this is a terrible mistake!");
/// panic!("this is a {} {message}", "fancy", message = "message");
/// ```
#[cfg(feature = "auto")]
#[macro_export]
macro_rules! panic {
    () => {
        ::std::panic!()
    };
    ($($arg:tt)*) => {{
        let target_stream = std::io::stderr();
        let buffer = $crate::_macros::to_adapted_string(&format_args!($($arg)*), &target_stream);
        ::std::panic!("{}", buffer)
    }};
}

#[cfg(feature = "auto")]
pub const FEATURE_TEST_ACTIVATED: bool = cfg!(feature = "test");

#[cfg(feature = "auto")]
pub fn to_adapted_string(
    display: &dyn std::fmt::Display,
    stream: &impl crate::stream::RawStream,
) -> String {
    use std::io::Write as _;

    let choice = crate::AutoStream::choice(stream);
    let buffer = Vec::new();
    let mut stream = crate::AutoStream::new(buffer, choice);
    // Ignore errors rather than panic
    let _ = ::std::write!(&mut stream, "{display}");
    let buffer = stream.into_inner();
    // Should be UTF-8 but not wanting to panic
    let buffer = String::from_utf8_lossy(&buffer).into_owned();
    buffer
}
