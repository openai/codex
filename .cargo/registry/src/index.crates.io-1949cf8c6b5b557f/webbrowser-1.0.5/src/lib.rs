//! Rust library to open URLs and local files in the web browsers available on a platform, with guarantees of [Consistent Behaviour](#consistent-behaviour).
//!
//! Inspired by the [webbrowser](https://docs.python.org/2/library/webbrowser.html) python library.
//!
//! ## Examples
//!
//! ```no_run
//! use webbrowser;
//!
//! if webbrowser::open("http://github.com").is_ok() {
//!     // ...
//! }
//! ```
//!
//! ## Platform Support Status
//!
//! | Platform              | Supported | Browsers | Test status |
//! |-----------------------|-----------|----------|-------------|
//! | macOS                 | ✅        | default + [others](https://docs.rs/webbrowser/latest/webbrowser/enum.Browser.html) | ✅ |
//! | windows               | ✅        | default only | ✅ |
//! | linux/wsl             | ✅        | default only (respects $BROWSER env var, so can be used with other browsers) | ✅ |
//! | android               | ✅        | default only | ✅ |
//! | iOS/tvOS/visionOS     | ✅        | default only | ✅ |
//! | wasm                  | ✅        | default only | ✅ |
//! | unix (*bsd, aix etc.) | ✅        | default only (respects $BROWSER env var, so can be used with other browsers) | Manual |
//!
//! ## Consistent Behaviour
//! `webbrowser` defines consistent behaviour on all platforms as follows:
//! * **Browser guarantee** - This library guarantees that the browser is opened, even for local files - the only crate to make such guarantees
//!   at the time of this writing. Alternative libraries rely on existing system commands, which may lead to an editor being opened (instead
//!   of the browser) for local html files, leading to an inconsistent behaviour for users.
//! * **Non-Blocking** for GUI based browsers (e.g. Firefox, Chrome etc.), while **Blocking** for text based browser (e.g. lynx etc.)
//! * **Suppressed output** by default for GUI based browsers, so that their stdout/stderr don't pollute the main program's output. This can be
//!   overridden by `webbrowser::open_browser_with_options`.
//!
//! ## Crate Features
//! `webbrowser` optionally allows the following features to be configured:
//! * `hardened` - this disables handling of non-http(s) urls (e.g. `file:///`) as a hard security precaution
//! * `disable-wsl` - this disables WSL `file` implementation (`http` still works)
//! * `wasm-console` - this enables logging to wasm console (valid only on wasm platform)

#[cfg_attr(
    any(target_os = "ios", target_os = "tvos", target_os = "visionos"),
    path = "ios.rs"
)]
#[cfg_attr(target_os = "macos", path = "macos.rs")]
#[cfg_attr(target_os = "android", path = "android.rs")]
#[cfg_attr(target_family = "wasm", path = "wasm.rs")]
#[cfg_attr(windows, path = "windows.rs")]
#[cfg_attr(
    all(
        unix,
        not(any(
            target_os = "ios",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "macos",
            target_os = "android",
            target_family = "wasm",
            windows,
        )),
    ),
    path = "unix.rs"
)]
mod os;

#[cfg(any(
    windows,
    all(
        unix,
        not(any(
            target_os = "ios",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "macos",
            target_os = "android",
            target_family = "wasm",
        )),
    ),
))]
pub(crate) mod common;

use std::fmt::Display;
use std::io::{Error, ErrorKind, Result};
use std::ops::Deref;
use std::str::FromStr;
use std::{error, fmt};

#[derive(Debug, Default, Eq, PartialEq, Copy, Clone, Hash)]
/// Browser types available
pub enum Browser {
    ///Operating system's default browser
    #[default]
    Default,

    ///Mozilla Firefox
    Firefox,

    ///Microsoft's Internet Explorer
    InternetExplorer,

    ///Google Chrome
    Chrome,

    ///Opera
    Opera,

    ///Mac OS Safari
    Safari,

    ///Haiku's WebPositive
    WebPositive,
}

impl Browser {
    /// Returns true if there is likely a browser detected in the system
    pub fn is_available() -> bool {
        Browser::Default.exists()
    }

    /// Returns true if this specific browser is detected in the system
    pub fn exists(&self) -> bool {
        open_browser_with_options(
            *self,
            "https://rootnet.in",
            BrowserOptions::new().with_dry_run(true),
        )
        .is_ok()
    }
}

///The Error type for parsing a string into a Browser.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct ParseBrowserError;

impl fmt::Display for ParseBrowserError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Invalid browser given")
    }
}

impl error::Error for ParseBrowserError {
    fn description(&self) -> &str {
        "invalid browser"
    }
}

impl fmt::Display for Browser {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Browser::Default => f.write_str("Default"),
            Browser::Firefox => f.write_str("Firefox"),
            Browser::InternetExplorer => f.write_str("Internet Explorer"),
            Browser::Chrome => f.write_str("Chrome"),
            Browser::Opera => f.write_str("Opera"),
            Browser::Safari => f.write_str("Safari"),
            Browser::WebPositive => f.write_str("WebPositive"),
        }
    }
}

impl FromStr for Browser {
    type Err = ParseBrowserError;

    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        match s {
            "firefox" => Ok(Browser::Firefox),
            "default" => Ok(Browser::Default),
            "ie" | "internet explorer" | "internetexplorer" => Ok(Browser::InternetExplorer),
            "chrome" => Ok(Browser::Chrome),
            "opera" => Ok(Browser::Opera),
            "safari" => Ok(Browser::Safari),
            "webpositive" => Ok(Browser::WebPositive),
            _ => Err(ParseBrowserError),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
/// BrowserOptions to override certain default behaviour. Any option named as a `hint` is
/// not guaranteed to be honoured. Use [BrowserOptions::new()] to create.
///
/// e.g. by default, we suppress stdout/stderr, but that behaviour can be overridden here
pub struct BrowserOptions {
    suppress_output: bool,
    target_hint: String,
    dry_run: bool,
}

impl fmt::Display for BrowserOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!(
            "BrowserOptions(supress_output={}, target_hint={}, dry_run={})",
            self.suppress_output, self.target_hint, self.dry_run
        ))
    }
}

impl std::default::Default for BrowserOptions {
    fn default() -> Self {
        let target_hint = String::from(option_env!("WEBBROWSER_WASM_TARGET").unwrap_or("_blank"));
        BrowserOptions {
            suppress_output: true,
            target_hint,
            dry_run: false,
        }
    }
}

impl BrowserOptions {
    /// Create a new instance. Configure it with one of the `with_` methods.
    pub fn new() -> Self {
        Self::default()
    }

    /// Determines whether stdout/stderr of the appropriate browser command is suppressed
    /// or not
    pub fn with_suppress_output(&mut self, suppress_output: bool) -> &mut Self {
        self.suppress_output = suppress_output;
        self
    }

    /// Hint to the browser to open the url in the corresponding
    /// [target](https://www.w3schools.com/tags/att_a_target.asp). Note that this is just
    /// a hint, it may or may not be honoured (currently guaranteed only in wasm).

    // TODO:remove this lint suppression once we're past the MSRV of 1.63 as that's when
    // clone_into() became stable.
    #[allow(clippy::all)]
    pub fn with_target_hint(&mut self, target_hint: &str) -> &mut Self {
        self.target_hint = target_hint.to_owned();
        self
    }

    /// Do not do an actual execution, just return true if this would've likely
    /// succeeded. Note the "likely" here - it's still indicative than guaranteed.
    pub fn with_dry_run(&mut self, dry_run: bool) -> &mut Self {
        self.dry_run = dry_run;
        self
    }
}

/// Opens the URL on the default browser of this platform
///
/// Returns Ok(..) so long as the browser invocation was successful. An Err(..) is returned in the
/// following scenarios:
/// * The requested browser was not found
/// * There was an error in opening the browser
/// * `hardened` feature is enabled, and the URL was not a valid http(s) url, say a `file:///`
/// * On ios/android/wasm, if the url is not a valid http(s) url
///
/// Equivalent to:
/// ```no_run
/// # use webbrowser::{Browser, open_browser};
/// # let url = "http://example.com";
/// open_browser(Browser::Default, url);
/// ```
///
/// # Examples
/// ```no_run
/// use webbrowser;
///
/// if webbrowser::open("http://github.com").is_ok() {
///     // ...
/// }
/// ```
pub fn open(url: &str) -> Result<()> {
    open_browser(Browser::Default, url)
}

/// Opens the specified URL on the specific browser (if available) requested. Return semantics are
/// the same as for [open](fn.open.html).
///
/// # Examples
/// ```no_run
/// use webbrowser::{open_browser, Browser};
///
/// if open_browser(Browser::Firefox, "http://github.com").is_ok() {
///     // ...
/// }
/// ```
pub fn open_browser(browser: Browser, url: &str) -> Result<()> {
    open_browser_with_options(browser, url, &BrowserOptions::default())
}

/// Opens the specified URL on the specific browser (if available) requested, while overriding the
/// default options.
///
/// Return semantics are
/// the same as for [open](fn.open.html).
///
/// # Examples
/// ```no_run
/// use webbrowser::{open_browser_with_options, Browser, BrowserOptions};
///
/// if open_browser_with_options(Browser::Default, "http://github.com", BrowserOptions::new().with_suppress_output(false)).is_ok() {
///     // ...
/// }
/// ```
pub fn open_browser_with_options(
    browser: Browser,
    url: &str,
    options: &BrowserOptions,
) -> Result<()> {
    let target = TargetType::try_from(url)?;

    // if feature:hardened is enabled, make sure we accept only HTTP(S) URLs
    #[cfg(feature = "hardened")]
    if !target.is_http() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "only http/https urls allowed",
        ));
    }

    if cfg!(any(
        target_os = "ios",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "macos",
        target_os = "android",
        target_family = "wasm",
        windows,
        unix,
    )) {
        os::open_browser_internal(browser, &target, options)
    } else {
        Err(Error::new(ErrorKind::NotFound, "unsupported platform"))
    }
}

/// The link we're trying to open, represented as a URL. Local files get represented
/// via `file://...` URLs
struct TargetType(url::Url);

impl TargetType {
    /// Returns true if this target represents an HTTP url, false otherwise
    #[cfg(any(
        feature = "hardened",
        target_os = "android",
        target_os = "ios",
        target_os = "tvos",
        target_os = "visionos",
        target_family = "wasm"
    ))]
    fn is_http(&self) -> bool {
        matches!(self.0.scheme(), "http" | "https")
    }

    /// If `target` represents a valid http/https url, return the str corresponding to it
    /// else return `std::io::Error` of kind `std::io::ErrorKind::InvalidInput`
    #[cfg(any(
        target_os = "android",
        target_os = "ios",
        target_os = "tvos",
        target_os = "visionos",
        target_family = "wasm"
    ))]
    fn get_http_url(&self) -> Result<&str> {
        if self.is_http() {
            Ok(self.0.as_str())
        } else {
            Err(Error::new(ErrorKind::InvalidInput, "not an http url"))
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn from_file_path(value: &str) -> Result<Self> {
        let pb = std::path::PathBuf::from(value);
        let url = url::Url::from_file_path(if pb.is_relative() {
            std::env::current_dir()?.join(pb)
        } else {
            pb
        })
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "failed to convert path to url"))?;

        Ok(Self(url))
    }
}

impl Deref for TargetType {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl Display for TargetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self as &str).fmt(f)
    }
}

impl std::convert::TryFrom<&str> for TargetType {
    type Error = Error;

    #[cfg(target_family = "wasm")]
    fn try_from(value: &str) -> Result<Self> {
        url::Url::parse(value)
            .map(|u| Ok(Self(u)))
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "invalid url for wasm"))?
    }

    #[cfg(not(target_family = "wasm"))]
    fn try_from(value: &str) -> Result<Self> {
        match url::Url::parse(value) {
            Ok(u) => {
                if u.scheme().len() == 1 && cfg!(windows) {
                    // this can happen in windows that C:\abc.html gets parsed as scheme "C"
                    Self::from_file_path(value)
                } else {
                    Ok(Self(u))
                }
            }
            Err(_) => Self::from_file_path(value),
        }
    }
}

#[test]
#[ignore]
fn test_open_firefox() {
    assert!(open_browser(Browser::Firefox, "http://github.com").is_ok());
}

#[test]
#[ignore]
fn test_open_chrome() {
    assert!(open_browser(Browser::Chrome, "http://github.com").is_ok());
}

#[test]
#[ignore]
fn test_open_safari() {
    assert!(open_browser(Browser::Safari, "http://github.com").is_ok());
}

#[test]
#[ignore]
fn test_open_webpositive() {
    assert!(open_browser(Browser::WebPositive, "http://github.com").is_ok());
}
