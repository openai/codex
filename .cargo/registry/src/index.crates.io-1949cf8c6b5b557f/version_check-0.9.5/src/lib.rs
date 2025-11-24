//! This tiny crate checks that the running or installed `rustc` meets some
//! version requirements. The version is queried by calling the Rust compiler
//! with `--version`. The path to the compiler is determined first via the
//! `RUSTC` environment variable. If it is not set, then `rustc` is used. If
//! that fails, no determination is made, and calls return `None`.
//!
//! # Examples
//!
//! **Note:** Please see [feature detection] for a note on enabling unstable
//! features based on detection via this crate.
//!
//! [feature detection]: crate#feature-detection
//!
//! * Set a `cfg` flag in `build.rs` if the running compiler was determined to
//!   be at least version `1.13.0`:
//!
//!   ```rust
//!   extern crate version_check as rustc;
//!
//!   if rustc::is_min_version("1.13.0").unwrap_or(false) {
//!       println!("cargo:rustc-cfg=question_mark_operator");
//!   }
//!   ```
//!
//!   See [`is_max_version`] or [`is_exact_version`] to check if the compiler
//!   is _at most_ or _exactly_ a certain version.
//!   <br /><br />
//!
//! * Check that the running compiler was released on or after `2018-12-18`:
//!
//!   ```rust
//!   extern crate version_check as rustc;
//!
//!   match rustc::is_min_date("2018-12-18") {
//!       Some(true) => "Yep! It's recent!",
//!       Some(false) => "No, it's older.",
//!       None => "Couldn't determine the rustc version."
//!   };
//!   ```
//!
//!   See [`is_max_date`] or [`is_exact_date`] to check if the compiler was
//!   released _prior to_ or _exactly on_ a certain date.
//!   <br /><br />
//!
//! * Check that the running compiler supports feature flags:
//!
//!   ```rust
//!   extern crate version_check as rustc;
//!
//!   match rustc::is_feature_flaggable() {
//!       Some(true) => "Yes! It's a dev or nightly release!",
//!       Some(false) => "No, it's stable or beta.",
//!       None => "Couldn't determine the rustc version."
//!   };
//!   ```
//!
//!   Please see the note on [feature detection].
//!   <br /><br />
//!
//! * Check that the running compiler supports a specific feature:
//!
//!   ```rust
//!   extern crate version_check as rustc;
//!
//!   if let Some(true) = rustc::supports_feature("doc_cfg") {
//!      println!("cargo:rustc-cfg=has_doc_cfg");
//!   }
//!   ```
//!
//!   Please see the note on [feature detection].
//!   <br /><br />
//!
//! * Check that the running compiler is on the stable channel:
//!
//!   ```rust
//!   extern crate version_check as rustc;
//!
//!   match rustc::Channel::read() {
//!       Some(c) if c.is_stable() => format!("Yes! It's stable."),
//!       Some(c) => format!("No, the channel {} is not stable.", c),
//!       None => format!("Couldn't determine the rustc version.")
//!   };
//!   ```
//!
//! To interact with the version, release date, and release channel as structs,
//! use [`Version`], [`Date`], and [`Channel`], respectively. The [`triple()`]
//! function returns all three values efficiently.
//!
//! # Feature Detection
//!
//! While this crate can be used to determine if the current compiler supports
//! an unstable feature, no crate can determine whether that feature will work
//! in a way that you expect ad infinitum. If the feature changes in an
//! incompatible way, then your crate, as well as all of its transitive
//! dependents, will fail to build. As a result, great care should be taken when
//! enabling nightly features even when they're supported by the compiler.
//!
//! One common mitigation used in practice is to make using unstable features
//! transitively opt-in via a crate feature or `cfg` so that broken builds only
//! affect those that explicitly asked for the feature. Another complementary
//! approach is to probe `rustc` at build-time by asking it to compile a small
//! but exemplary program that determines whether the feature works as expected,
//! enabling the feature only if the probe succeeds. Finally, eschewing these
//! recommendations, you should track the `nightly` channel closely to minimize
//! the total impact of a nightly breakages.
//!
//! # Alternatives
//!
//! This crate is dead simple with no dependencies. If you need something more
//! and don't care about panicking if the version cannot be obtained, or if you
//! don't mind adding dependencies, see
//! [rustc_version](https://crates.io/crates/rustc_version).

#![allow(deprecated)]

mod version;
mod channel;
mod date;

use std::env;
use std::process::Command;

#[doc(inline)] pub use version::*;
#[doc(inline)] pub use channel::*;
#[doc(inline)] pub use date::*;

/// Parses (version, date) as available from rustc version string.
fn version_and_date_from_rustc_version(s: &str) -> (Option<String>, Option<String>) {
    let last_line = s.lines().last().unwrap_or(s);
    let mut components = last_line.trim().split(" ");
    let version = components.nth(1);
    let date = components.filter(|c| c.ends_with(')')).next()
        .map(|s| s.trim_right().trim_right_matches(")").trim_left().trim_left_matches('('));
    (version.map(|s| s.to_string()), date.map(|s| s.to_string()))
}

/// Parses (version, date) as available from rustc verbose version output.
fn version_and_date_from_rustc_verbose_version(s: &str) -> (Option<String>, Option<String>) {
    let (mut version, mut date) = (None, None);
    for line in s.lines() {
        let split = |s: &str| s.splitn(2, ":").nth(1).map(|s| s.trim().to_string());
        match line.trim().split(" ").nth(0) {
            Some("rustc") => {
                let (v, d) = version_and_date_from_rustc_version(line);
                version = version.or(v);
                date = date.or(d);
            },
            Some("release:") => version = split(line),
            Some("commit-date:") if line.ends_with("unknown") => date = None,
            Some("commit-date:") => date = split(line),
            _ => continue
        }
    }

    (version, date)
}

/// Returns (version, date) as available from `rustc --version`.
fn get_version_and_date() -> Option<(Option<String>, Option<String>)> {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    Command::new(rustc).arg("--verbose").arg("--version").output().ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| version_and_date_from_rustc_verbose_version(&s))
}

/// Reads the triple of [`Version`], [`Channel`], and [`Date`] of the installed
/// or running `rustc`.
///
/// If any attribute cannot be determined (see the [top-level
/// documentation](crate)), returns `None`.
///
/// To obtain only one of three attributes, use [`Version::read()`],
/// [`Channel::read()`], or [`Date::read()`].
pub fn triple() -> Option<(Version, Channel, Date)> {
    let (version_str, date_str) = match get_version_and_date() {
        Some((Some(version), Some(date))) => (version, date),
        _ => return None
    };

    // Can't use `?` or `try!` for `Option` in 1.0.0.
    match Version::parse(&version_str) {
        Some(version) => match Channel::parse(&version_str) {
            Some(channel) => match Date::parse(&date_str) {
                Some(date) => Some((version, channel, date)),
                _ => None,
            },
            _ => None,
        },
        _ => None
    }
}

/// Checks that the running or installed `rustc` was released **on or after**
/// some date.
///
/// The format of `min_date` must be YYYY-MM-DD. For instance: `2016-12-20` or
/// `2017-01-09`.
///
/// If the date cannot be retrieved or parsed, or if `min_date` could not be
/// parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// was release on or after `min_date` and `false` otherwise.
pub fn is_min_date(min_date: &str) -> Option<bool> {
    match (Date::read(), Date::parse(min_date)) {
        (Some(rustc_date), Some(min_date)) => Some(rustc_date >= min_date),
        _ => None
    }
}

/// Checks that the running or installed `rustc` was released **on or before**
/// some date.
///
/// The format of `max_date` must be YYYY-MM-DD. For instance: `2016-12-20` or
/// `2017-01-09`.
///
/// If the date cannot be retrieved or parsed, or if `max_date` could not be
/// parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// was release on or before `max_date` and `false` otherwise.
pub fn is_max_date(max_date: &str) -> Option<bool> {
    match (Date::read(), Date::parse(max_date)) {
        (Some(rustc_date), Some(max_date)) => Some(rustc_date <= max_date),
        _ => None
    }
}

/// Checks that the running or installed `rustc` was released **exactly** on
/// some date.
///
/// The format of `date` must be YYYY-MM-DD. For instance: `2016-12-20` or
/// `2017-01-09`.
///
/// If the date cannot be retrieved or parsed, or if `date` could not be parsed,
/// returns `None`. Otherwise returns `true` if the installed `rustc` was
/// release on `date` and `false` otherwise.
pub fn is_exact_date(date: &str) -> Option<bool> {
    match (Date::read(), Date::parse(date)) {
        (Some(rustc_date), Some(date)) => Some(rustc_date == date),
        _ => None
    }
}

/// Checks that the running or installed `rustc` is **at least** some minimum
/// version.
///
/// The format of `min_version` is a semantic version: `1.3.0`, `1.15.0-beta`,
/// `1.14.0`, `1.16.0-nightly`, etc.
///
/// If the version cannot be retrieved or parsed, or if `min_version` could not
/// be parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// is at least `min_version` and `false` otherwise.
pub fn is_min_version(min_version: &str) -> Option<bool> {
    match (Version::read(), Version::parse(min_version)) {
        (Some(rustc_ver), Some(min_ver)) => Some(rustc_ver >= min_ver),
        _ => None
    }
}

/// Checks that the running or installed `rustc` is **at most** some maximum
/// version.
///
/// The format of `max_version` is a semantic version: `1.3.0`, `1.15.0-beta`,
/// `1.14.0`, `1.16.0-nightly`, etc.
///
/// If the version cannot be retrieved or parsed, or if `max_version` could not
/// be parsed, returns `None`. Otherwise returns `true` if the installed `rustc`
/// is at most `max_version` and `false` otherwise.
pub fn is_max_version(max_version: &str) -> Option<bool> {
    match (Version::read(), Version::parse(max_version)) {
        (Some(rustc_ver), Some(max_ver)) => Some(rustc_ver <= max_ver),
        _ => None
    }
}

/// Checks that the running or installed `rustc` is **exactly** some version.
///
/// The format of `version` is a semantic version: `1.3.0`, `1.15.0-beta`,
/// `1.14.0`, `1.16.0-nightly`, etc.
///
/// If the version cannot be retrieved or parsed, or if `version` could not be
/// parsed, returns `None`. Otherwise returns `true` if the installed `rustc` is
/// exactly `version` and `false` otherwise.
pub fn is_exact_version(version: &str) -> Option<bool> {
    match (Version::read(), Version::parse(version)) {
        (Some(rustc_ver), Some(version)) => Some(rustc_ver == version),
        _ => None
    }
}

/// Checks whether the running or installed `rustc` supports feature flags.
///
/// Returns true if the channel is either "nightly" or "dev".
///
/// **Please see the note on [feature detection](crate#feature-detection).**
///
/// Note that support for specific `rustc` features can be enabled or disabled
/// via the `allow-features` compiler flag, which this function _does not_
/// check. That is, this function _does not_ check whether a _specific_ feature
/// is supported, but instead whether features are supported at all. To check
/// for support for a specific feature, use [`supports_feature()`].
///
/// If the version could not be determined, returns `None`. Otherwise returns
/// `true` if the running version supports feature flags and `false` otherwise.
pub fn is_feature_flaggable() -> Option<bool> {
    Channel::read().map(|c| c.supports_features())
}

/// Checks whether the running or installed `rustc` supports `feature`.
///
/// **Please see the note on [feature detection](crate#feature-detection).**
///
/// Returns _true_ _iff_ [`is_feature_flaggable()`] returns `true` _and_ the
/// feature is not disabled via exclusion in `allow-features` via `RUSTFLAGS` or
/// `CARGO_ENCODED_RUSTFLAGS`. If the version could not be determined, returns
/// `None`.
///
/// # Example
///
/// ```rust
/// use version_check as rustc;
///
/// if let Some(true) = rustc::supports_feature("doc_cfg") {
///    println!("cargo:rustc-cfg=has_doc_cfg");
/// }
/// ```
pub fn supports_feature(feature: &str) -> Option<bool> {
    match is_feature_flaggable() {
        Some(true) => { /* continue */ }
        Some(false) => return Some(false),
        None => return None,
    }

    let env_flags = env::var_os("CARGO_ENCODED_RUSTFLAGS")
        .map(|flags| (flags, '\x1f'))
        .or_else(|| env::var_os("RUSTFLAGS").map(|flags| (flags, ' ')));

    if let Some((flags, delim)) = env_flags {
        const ALLOW_FEATURES: &'static str = "allow-features=";

        let rustflags = flags.to_string_lossy();
        let allow_features = rustflags.split(delim)
            .map(|flag| flag.trim_left_matches("-Z").trim())
            .filter(|flag| flag.starts_with(ALLOW_FEATURES))
            .map(|flag| &flag[ALLOW_FEATURES.len()..]);

        if let Some(allow_features) = allow_features.last() {
            return Some(allow_features.split(',').any(|f| f.trim() == feature));
        }
    }

    // If there are no `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` or they don't
    // contain an `allow-features` flag, assume compiler allows all features.
    Some(true)
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::version_and_date_from_rustc_version;
    use super::version_and_date_from_rustc_verbose_version;

    macro_rules! check_parse {
        (@ $f:expr, $s:expr => $v:expr, $d:expr) => ({
            if let (Some(v), d) = $f(&$s) {
                let e_d: Option<&str> = $d.into();
                assert_eq!((v, d), ($v.to_string(), e_d.map(|s| s.into())));
            } else {
                panic!("{:?} didn't parse for version testing.", $s);
            }
        });
        ($f:expr, $s:expr => $v:expr, $d:expr) => ({
            let warn = "warning: invalid logging spec 'warning', ignoring it";
            let warn2 = "warning: sorry, something went wrong :(sad)";
            check_parse!(@ $f, $s => $v, $d);
            check_parse!(@ $f, &format!("{}\n{}", warn, $s) => $v, $d);
            check_parse!(@ $f, &format!("{}\n{}", warn2, $s) => $v, $d);
            check_parse!(@ $f, &format!("{}\n{}\n{}", warn, warn2, $s) => $v, $d);
            check_parse!(@ $f, &format!("{}\n{}\n{}", warn2, warn, $s) => $v, $d);
        })
    }

    macro_rules! check_terse_parse {
        ($($s:expr => $v:expr, $d:expr,)+) => {$(
            check_parse!(version_and_date_from_rustc_version, $s => $v, $d);
        )+}
    }

    macro_rules! check_verbose_parse {
        ($($s:expr => $v:expr, $d:expr,)+) => {$(
            check_parse!(version_and_date_from_rustc_verbose_version, $s => $v, $d);
        )+}
    }

    #[test]
    fn test_version_parse() {
        check_terse_parse! {
            "rustc 1.18.0" => "1.18.0", None,
            "rustc 1.8.0" => "1.8.0", None,
            "rustc 1.20.0-nightly" => "1.20.0-nightly", None,
            "rustc 1.20" => "1.20", None,
            "rustc 1.3" => "1.3", None,
            "rustc 1" => "1", None,
            "rustc 1.5.1-beta" => "1.5.1-beta", None,
            "rustc 1.20.0 (2017-07-09)" => "1.20.0", Some("2017-07-09"),
            "rustc 1.20.0-dev (2017-07-09)" => "1.20.0-dev", Some("2017-07-09"),
            "rustc 1.20.0-nightly (d84693b93 2017-07-09)" => "1.20.0-nightly", Some("2017-07-09"),
            "rustc 1.20.0 (d84693b93 2017-07-09)" => "1.20.0", Some("2017-07-09"),
            "rustc 1.30.0-nightly (3bc2ca7e4 2018-09-20)" => "1.30.0-nightly", Some("2018-09-20"),
        };
    }

    #[test]
    fn test_verbose_version_parse() {
        check_verbose_parse! {
            "rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)\n\
                binary: rustc\n\
                commit-hash: a59de37e99060162a2674e3ff45409ac73595c0e\n\
                commit-date: 2015-05-13\n\
                build-date: 2015-05-14\n\
                host: x86_64-unknown-linux-gnu\n\
                release: 1.0.0" => "1.0.0", Some("2015-05-13"),

            "rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)\n\
                commit-hash: a59de37e99060162a2674e3ff45409ac73595c0e\n\
                commit-date: 2015-05-13\n\
                build-date: 2015-05-14\n\
                host: x86_64-unknown-linux-gnu\n\
                release: 1.0.0" => "1.0.0", Some("2015-05-13"),

            "rustc 1.50.0 (cb75ad5db 2021-02-10)\n\
                binary: rustc\n\
                commit-hash: cb75ad5db02783e8b0222fee363c5f63f7e2cf5b\n\
                commit-date: 2021-02-10\n\
                host: x86_64-unknown-linux-gnu\n\
                release: 1.50.0" => "1.50.0", Some("2021-02-10"),

            "rustc 1.52.0-nightly (234781afe 2021-03-07)\n\
                binary: rustc\n\
                commit-hash: 234781afe33d3f339b002f85f948046d8476cfc9\n\
                commit-date: 2021-03-07\n\
                host: x86_64-unknown-linux-gnu\n\
                release: 1.52.0-nightly\n\
                LLVM version: 12.0.0" => "1.52.0-nightly", Some("2021-03-07"),

            "rustc 1.41.1\n\
                binary: rustc\n\
                commit-hash: unknown\n\
                commit-date: unknown\n\
                host: x86_64-unknown-linux-gnu\n\
                release: 1.41.1\n\
                LLVM version: 7.0" => "1.41.1", None,

            "rustc 1.49.0\n\
                binary: rustc\n\
                commit-hash: unknown\n\
                commit-date: unknown\n\
                host: x86_64-unknown-linux-gnu\n\
                release: 1.49.0" => "1.49.0", None,

            "rustc 1.50.0 (Fedora 1.50.0-1.fc33)\n\
                binary: rustc\n\
                commit-hash: unknown\n\
                commit-date: unknown\n\
                host: x86_64-unknown-linux-gnu\n\
                release: 1.50.0" => "1.50.0", None,
        };
    }

    fn read_static(verbose: bool, channel: &str, minor: usize) -> String {
        use std::fs::File;
        use std::path::Path;
        use std::io::{BufReader, Read};

        let subdir = if verbose { "verbose" } else { "terse" };
        let path = Path::new(STATIC_PATH)
            .join(channel)
            .join(subdir)
            .join(format!("rustc-1.{}.0", minor));

        let file = File::open(path).unwrap();
        let mut buf_reader = BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents).unwrap();
        contents
    }

    static STATIC_PATH: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/static");

    static DATES: [&'static str; 51] = [
        "2015-05-13", "2015-06-19", "2015-08-03", "2015-09-15", "2015-10-27",
        "2015-12-04", "2016-01-19", "2016-02-29", "2016-04-11", "2016-05-18",
        "2016-07-03", "2016-08-15", "2016-09-23", "2016-11-07", "2016-12-16",
        "2017-01-19", "2017-03-10", "2017-04-24", "2017-06-06", "2017-07-17",
        "2017-08-27", "2017-10-09", "2017-11-20", "2018-01-01", "2018-02-12",
        "2018-03-25", "2018-05-07", "2018-06-19", "2018-07-30", "2018-09-11",
        "2018-10-24", "2018-12-04", "2019-01-16", "2019-02-28", "2019-04-10",
        "2019-05-20", "2019-07-03", "2019-08-13", "2019-09-23", "2019-11-04",
        "2019-12-16", "2020-01-27", "2020-03-09", "2020-04-20", "2020-06-01",
        "2020-07-13", "2020-08-24", "2020-10-07", "2020-11-16", "2020-12-29",
        "2021-02-10",
    ];

    #[test]
    fn test_stable_compatibility() {
        if env::var_os("FORCE_STATIC").is_none() && fs::metadata(STATIC_PATH).is_err() {
            // We exclude `/static` when we package `version_check`, so don't
            // run if static files aren't present unless we know they should be.
            return;
        }

        // Ensure we can parse all output from all Linux stable releases.
        for v in 0..DATES.len() {
            let (version, date) = (&format!("1.{}.0", v), Some(DATES[v]));
            check_terse_parse!(read_static(false, "stable", v) => version, date,);
            check_verbose_parse!(read_static(true, "stable", v) => version, date,);
        }
    }

    #[test]
    fn test_parse_current() {
        let (version, channel) = (::Version::read(), ::Channel::read());
        assert!(version.is_some());
        assert!(channel.is_some());

        if let Ok(known_channel) = env::var("KNOWN_CHANNEL") {
            assert_eq!(channel, ::Channel::parse(&known_channel));
        }
    }
}
