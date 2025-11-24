use std::panic::Location;

use eyre::{OptionExt as _, WrapErr};

struct LocationHandler {
    actual: Option<&'static str>,
    expected: &'static str,
}

impl LocationHandler {
    fn new(expected: &'static str) -> Self {
        LocationHandler {
            actual: None,
            expected,
        }
    }
}

impl eyre::EyreHandler for LocationHandler {
    fn debug(
        &self,
        _error: &(dyn std::error::Error + 'static),
        _f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // we assume that if the compiler is new enough to support
        // `track_caller` that we will always have `actual` be `Some`, so we can
        // safely skip the assertion if the location is `None` which should only
        // happen in older rust versions.
        if let Some(actual) = self.actual {
            assert_eq!(self.expected, actual);
        }

        Ok(())
    }

    fn track_caller(&mut self, location: &'static Location<'static>) {
        dbg!(location);
        self.actual = Some(location.file());
    }
}

#[test]
fn test_wrap_err() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    let err = read_path("totally_fake_path")
        .wrap_err("oopsie")
        .unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[cfg(not(miri))]
fn read_path(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

#[cfg(miri)]
fn read_path(_path: &str) -> Result<String, std::io::Error> {
    // Miri doesn't support reading files, so we just return an error
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Miri doesn't support reading files",
    ))
}

#[test]
fn test_wrap_err_with() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    let err = read_path("totally_fake_path")
        .wrap_err_with(|| "oopsie")
        .unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[test]
fn test_option_ok_or_eyre() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    let err = None::<()>.ok_or_eyre("oopsie").unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[test]
fn test_context() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    let err = read_path("totally_fake_path")
        .context("oopsie")
        .unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[test]
fn test_with_context() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    let err = read_path("totally_fake_path")
        .with_context(|| "oopsie")
        .unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[test]
fn test_option_compat_wrap_err() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    use eyre::ContextCompat;
    let err = None::<()>.wrap_err("oopsie").unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[test]
fn test_option_compat_wrap_err_with() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    use eyre::ContextCompat;
    let err = None::<()>.wrap_err_with(|| "oopsie").unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[test]
fn test_option_compat_context() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    use eyre::ContextCompat;
    let err = None::<()>.context("oopsie").unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}

#[test]
fn test_option_compat_with_context() {
    let _ = eyre::set_hook(Box::new(|_e| {
        let expected_location = file!();
        Box::new(LocationHandler::new(expected_location))
    }));

    use eyre::ContextCompat;
    let err = None::<()>.with_context(|| "oopsie").unwrap_err();

    // should panic if the location isn't in our crate
    println!("{:?}", err);
}
