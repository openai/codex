//! Utilities for parsing X11 display strings.

mod connect_instruction;
pub use connect_instruction::ConnectAddress;

use crate::errors::DisplayParsingError;
use alloc::string::{String, ToString};

/// A parsed X11 display string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDisplay {
    /// The hostname of the computer we nned to connect to.
    ///
    /// This is an empty string if we are connecting to the
    /// local host.
    pub host: String,
    /// The protocol we are communicating over.
    ///
    /// This is `None` if the protocol may be determined
    /// automatically.
    pub protocol: Option<String>,
    /// The index of the display we are connecting to.
    pub display: u16,
    /// The index of the screen that we are using as the
    /// default screen.
    pub screen: u16,
}

impl ParsedDisplay {
    /// Get an iterator over `ConnectAddress`es from this parsed display for connecting
    /// to the server.
    pub fn connect_instruction(&self) -> impl Iterator<Item = ConnectAddress<'_>> {
        connect_instruction::connect_addresses(self)
    }
}

/// Parse an X11 display string.
///
/// If `dpy_name` is `None`, the display is parsed from the environment variable `DISPLAY`.
///
/// This function is only available when the `std` feature is enabled.
#[cfg(feature = "std")]
pub fn parse_display(dpy_name: Option<&str>) -> Result<ParsedDisplay, DisplayParsingError> {
    fn file_exists(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    match dpy_name {
        Some(dpy_name) => parse_display_with_file_exists_callback(dpy_name, file_exists),
        // If no dpy name was provided, use the env var. If no env var exists, return an error.
        None => match std::env::var("DISPLAY") {
            Ok(dpy_name) => parse_display_with_file_exists_callback(&dpy_name, file_exists),
            Err(std::env::VarError::NotPresent) => Err(DisplayParsingError::DisplayNotSet),
            Err(std::env::VarError::NotUnicode(_)) => Err(DisplayParsingError::NotUnicode),
        },
    }
}

/// Parse an X11 display string.
///
/// If `dpy_name` is `None`, the display is parsed from the environment variable `DISPLAY`.
///
/// The parameter `file_exists` is called to check whether a given string refers to an existing
/// file. This function does not need to check the file type.
pub fn parse_display_with_file_exists_callback(
    dpy_name: &str,
    file_exists: impl Fn(&str) -> bool,
) -> Result<ParsedDisplay, DisplayParsingError> {
    let malformed = || DisplayParsingError::MalformedValue(dpy_name.to_string().into());
    let map_malformed = |_| malformed();

    if dpy_name.starts_with('/') {
        return parse_display_direct_path(dpy_name, file_exists);
    }
    if let Some(remaining) = dpy_name.strip_prefix("unix:") {
        return parse_display_direct_path(remaining, file_exists);
    }

    // Everything up to the last '/' is the protocol. This part is optional.
    let (protocol, remaining) = if let Some(pos) = dpy_name.rfind('/') {
        (Some(&dpy_name[..pos]), &dpy_name[pos + 1..])
    } else {
        (None, dpy_name)
    };

    // Everything up to the last ':' is the host. This part is required.
    let pos = remaining.rfind(':').ok_or_else(malformed)?;
    let (host, remaining) = (&remaining[..pos], &remaining[pos + 1..]);

    // The remaining part is display.screen. The display is required and the screen optional.
    let (display, screen) = match remaining.find('.') {
        Some(pos) => (&remaining[..pos], &remaining[pos + 1..]),
        None => (remaining, "0"),
    };

    // Parse the display and screen number
    let (display, screen) = (
        display.parse().map_err(map_malformed)?,
        screen.parse().map_err(map_malformed)?,
    );

    let host = host.to_string();
    let protocol = protocol.map(|p| p.to_string());
    Ok(ParsedDisplay {
        host,
        protocol,
        display,
        screen,
    })
}

// Check for "launchd mode" where we get the full path to a unix socket
fn parse_display_direct_path(
    dpy_name: &str,
    file_exists: impl Fn(&str) -> bool,
) -> Result<ParsedDisplay, DisplayParsingError> {
    if file_exists(dpy_name) {
        return Ok(ParsedDisplay {
            host: dpy_name.to_string(),
            protocol: Some("unix".to_string()),
            display: 0,
            screen: 0,
        });
    }

    // Optionally, a screen number may be appended as ".n".
    if let Some((path, screen)) = dpy_name.rsplit_once('.') {
        if file_exists(path) {
            return Ok(ParsedDisplay {
                host: path.to_string(),
                protocol: Some("unix".to_string()),
                display: 0,
                screen: screen.parse().map_err(|_| {
                    DisplayParsingError::MalformedValue(dpy_name.to_string().into())
                })?,
            });
        }
    }
    Err(DisplayParsingError::MalformedValue(
        dpy_name.to_string().into(),
    ))
}

#[cfg(test)]
mod test {
    use super::{
        parse_display, parse_display_with_file_exists_callback, DisplayParsingError, ParsedDisplay,
    };
    use alloc::string::ToString;
    use core::cell::RefCell;

    fn do_parse_display(input: &str) -> Result<ParsedDisplay, DisplayParsingError> {
        std::env::set_var("DISPLAY", input);
        let result1 = parse_display(None);

        std::env::remove_var("DISPLAY");
        let result2 = parse_display(Some(input));

        assert_eq!(result1, result2);
        result1
    }

    // The tests modify environment variables. This is process-global. Thus, the tests in this
    // module cannot be run concurrently. We achieve this by having only a single test functions
    // that calls all other functions.
    #[test]
    fn test_parsing() {
        test_missing_input();
        xcb_good_cases();
        xcb_bad_cases();
        own_good_cases();
        own_bad_cases();
    }

    fn test_missing_input() {
        std::env::remove_var("DISPLAY");
        assert_eq!(parse_display(None), Err(DisplayParsingError::DisplayNotSet));
    }

    fn own_good_cases() {
        // The XCB test suite does not test protocol parsing
        for (input, output) in &[
            (
                "foo/bar:1",
                ParsedDisplay {
                    host: "bar".to_string(),
                    protocol: Some("foo".to_string()),
                    display: 1,
                    screen: 0,
                },
            ),
            (
                "foo/bar:1.2",
                ParsedDisplay {
                    host: "bar".to_string(),
                    protocol: Some("foo".to_string()),
                    display: 1,
                    screen: 2,
                },
            ),
            (
                "a:b/c/foo:bar:1.2",
                ParsedDisplay {
                    host: "foo:bar".to_string(),
                    protocol: Some("a:b/c".to_string()),
                    display: 1,
                    screen: 2,
                },
            ),
        ] {
            assert_eq!(
                do_parse_display(input).as_ref(),
                Ok(output),
                "Failed parsing correctly: {}",
                input
            );
        }
    }

    fn own_bad_cases() {
        let non_existing_file = concat!(env!("CARGO_MANIFEST_DIR"), "/this_file_does_not_exist");
        assert_eq!(
            do_parse_display(non_existing_file),
            Err(DisplayParsingError::MalformedValue(
                non_existing_file.to_string().into()
            )),
            "Unexpectedly parsed: {}",
            non_existing_file
        );
    }

    // Based on libxcb's test suite; (C) 2001-2006 Bart Massey, Jamey Sharp, and Josh Triplett
    fn xcb_good_cases() {
        // The libxcb code creates a temporary file. We can just use a known-to-exist file.
        let existing_file = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

        for (input, output) in &[
            // unix in "launchd mode"
            (
                existing_file,
                ParsedDisplay {
                    host: existing_file.to_string(),
                    protocol: Some("unix".to_string()),
                    display: 0,
                    screen: 0,
                },
            ),
            (
                &alloc::format!("unix:{existing_file}"),
                ParsedDisplay {
                    host: existing_file.to_string(),
                    protocol: Some("unix".to_string()),
                    display: 0,
                    screen: 0,
                },
            ),
            (
                &alloc::format!("unix:{existing_file}.1"),
                ParsedDisplay {
                    host: existing_file.to_string(),
                    protocol: Some("unix".to_string()),
                    display: 0,
                    screen: 1,
                },
            ),
            (
                &alloc::format!("{existing_file}.1"),
                ParsedDisplay {
                    host: existing_file.to_string(),
                    protocol: Some("unix".to_string()),
                    display: 0,
                    screen: 1,
                },
            ),
            // unix
            (
                ":0",
                ParsedDisplay {
                    host: "".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                ":1",
                ParsedDisplay {
                    host: "".to_string(),
                    protocol: None,
                    display: 1,
                    screen: 0,
                },
            ),
            (
                ":0.1",
                ParsedDisplay {
                    host: "".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
            // ip
            (
                "x.org:0",
                ParsedDisplay {
                    host: "x.org".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "expo:0",
                ParsedDisplay {
                    host: "expo".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "bigmachine:1",
                ParsedDisplay {
                    host: "bigmachine".to_string(),
                    protocol: None,
                    display: 1,
                    screen: 0,
                },
            ),
            (
                "hydra:0.1",
                ParsedDisplay {
                    host: "hydra".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
            // ipv4
            (
                "198.112.45.11:0",
                ParsedDisplay {
                    host: "198.112.45.11".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "198.112.45.11:0.1",
                ParsedDisplay {
                    host: "198.112.45.11".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
            // ipv6
            (
                ":::0",
                ParsedDisplay {
                    host: "::".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "1:::0",
                ParsedDisplay {
                    host: "1::".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "::1:0",
                ParsedDisplay {
                    host: "::1".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "::1:0.1",
                ParsedDisplay {
                    host: "::1".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
            (
                "::127.0.0.1:0",
                ParsedDisplay {
                    host: "::127.0.0.1".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "::ffff:127.0.0.1:0",
                ParsedDisplay {
                    host: "::ffff:127.0.0.1".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "2002:83fc:3052::1:0",
                ParsedDisplay {
                    host: "2002:83fc:3052::1".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "2002:83fc:3052::1:0.1",
                ParsedDisplay {
                    host: "2002:83fc:3052::1".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
            (
                "[::]:0",
                ParsedDisplay {
                    host: "[::]".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "[1::]:0",
                ParsedDisplay {
                    host: "[1::]".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "[::1]:0",
                ParsedDisplay {
                    host: "[::1]".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "[::1]:0.1",
                ParsedDisplay {
                    host: "[::1]".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
            (
                "[::127.0.0.1]:0",
                ParsedDisplay {
                    host: "[::127.0.0.1]".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "[2002:83fc:d052::1]:0",
                ParsedDisplay {
                    host: "[2002:83fc:d052::1]".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "[2002:83fc:d052::1]:0.1",
                ParsedDisplay {
                    host: "[2002:83fc:d052::1]".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
            // decnet
            (
                "myws::0",
                ParsedDisplay {
                    host: "myws:".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "big::0",
                ParsedDisplay {
                    host: "big:".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 0,
                },
            ),
            (
                "hydra::0.1",
                ParsedDisplay {
                    host: "hydra:".to_string(),
                    protocol: None,
                    display: 0,
                    screen: 1,
                },
            ),
        ] {
            assert_eq!(
                do_parse_display(input).as_ref(),
                Ok(output),
                "Failed parsing correctly: {}",
                input
            );
        }
    }

    // Based on libxcb's test suite; (C) 2001-2006 Bart Massey, Jamey Sharp, and Josh Triplett
    fn xcb_bad_cases() {
        for input in &[
            "",
            ":",
            "::",
            ":::",
            ":.",
            ":a",
            ":a.",
            ":0.",
            ":.a",
            ":.0",
            ":0.a",
            ":0.0.",
            "127.0.0.1",
            "127.0.0.1:",
            "127.0.0.1::",
            "::127.0.0.1",
            "::127.0.0.1:",
            "::127.0.0.1::",
            "::ffff:127.0.0.1",
            "::ffff:127.0.0.1:",
            "::ffff:127.0.0.1::",
            "localhost",
            "localhost:",
            "localhost::",
        ] {
            assert_eq!(
                do_parse_display(input),
                Err(DisplayParsingError::MalformedValue(
                    input.to_string().into()
                )),
                "Unexpectedly parsed: {}",
                input
            );
        }
    }

    fn make_unix_path(host: &str, screen: u16) -> Result<ParsedDisplay, DisplayParsingError> {
        Ok(ParsedDisplay {
            host: host.to_string(),
            protocol: Some("unix".to_string()),
            display: 0,
            screen,
        })
    }

    #[test]
    fn test_file_exists_callback_direct_path() {
        fn run_test(display: &str, expected_path: &str) {
            let called = RefCell::new(0);
            let callback = |path: &_| {
                assert_eq!(path, expected_path);
                let mut called = called.borrow_mut();
                assert_eq!(*called, 0);
                *called += 1;
                true
            };
            let result = parse_display_with_file_exists_callback(display, callback);
            assert_eq!(*called.borrow(), 1);
            assert_eq!(result, make_unix_path(expected_path, 0));
        }

        run_test("/path/to/file", "/path/to/file");
        run_test("/path/to/file.123", "/path/to/file.123");
        run_test("unix:whatever", "whatever");
        run_test("unix:whatever.123", "whatever.123");
    }

    #[test]
    fn test_file_exists_callback_direct_path_with_screen() {
        fn run_test(display: &str, expected_path: &str) {
            let called = RefCell::new(0);
            let callback = |path: &_| {
                let mut called = called.borrow_mut();
                *called += 1;
                match *called {
                    1 => {
                        assert_eq!(path, alloc::format!("{expected_path}.42"));
                        false
                    }
                    2 => {
                        assert_eq!(path, expected_path);
                        true
                    }
                    _ => panic!("Unexpected call count {}", *called),
                }
            };
            let result = parse_display_with_file_exists_callback(display, callback);
            assert_eq!(*called.borrow(), 2);
            assert_eq!(result, make_unix_path(expected_path, 42));
        }

        run_test("/path/to/file.42", "/path/to/file");
        run_test("unix:whatever.42", "whatever");
    }

    #[test]
    fn test_file_exists_callback_not_called_without_path() {
        let callback = |path: &str| unreachable!("Called with {path}");
        let result = parse_display_with_file_exists_callback("foo/bar:1.2", callback);
        assert_eq!(
            result,
            Ok(ParsedDisplay {
                host: "bar".to_string(),
                protocol: Some("foo".to_string()),
                display: 1,
                screen: 2,
            },)
        );
    }
}
