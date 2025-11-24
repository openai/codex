use std::ffi::c_void;

use objc2::{class, msg_send, rc::Retained, Encode, Encoding, MainThreadMarker};
use objc2_foundation::{NSDictionary, NSObject, NSString, NSURL};

use crate::{Browser, BrowserOptions, Error, ErrorKind, Result, TargetType};

/// Returns `UIApplication`
#[allow(non_snake_case)]
fn sharedApplication(mtm: MainThreadMarker) -> Option<Retained<NSObject>> {
    let _ = mtm;
    // SAFETY: The signature is correct, and we hold `MainThreadMarker`, so we
    // know we're on the main thread where it's safe to access the shared
    // UIApplication.
    //
    // NOTE: `sharedApplication` is declared as returning non-NULL, but it
    // will only do so inside `UIApplicationMain`; if called outside, the
    // shared application is NULL.
    unsafe { msg_send![class!(UIApplication), sharedApplication] }
}

/// Fake `block` to not have to depend on the `block2` crate just to set this to an empty/`None` block.
#[repr(transparent)]
struct FakeBlock(*const c_void);

// SAFETY: The type is `#[repr(transparent)]` over a pointer (same layout as `Option<&block::Block<...>>`).
unsafe impl Encode for FakeBlock {
    const ENCODING: Encoding = Encoding::Block;
}

#[doc(alias = "openURL_options_completionHandler")]
fn open_url(app: &NSObject, url: &NSURL, options: &NSDictionary) {
    let fake_handler = FakeBlock(std::ptr::null());
    unsafe { msg_send![app, openURL: url, options: options, completionHandler: fake_handler] }
}

/// Deal with opening of browsers on iOS/tvOS/visionOS.
///
/// watchOS doesn't have a browser, so this won't work there.
pub(super) fn open_browser_internal(
    _browser: Browser,
    target: &TargetType,
    options: &BrowserOptions,
) -> Result<()> {
    // ensure we're opening only http/https urls, failing otherwise
    let url = target.get_http_url()?;

    // always return true for a dry run
    if options.dry_run {
        return Ok(());
    }

    let mtm = MainThreadMarker::new().ok_or(Error::new(
        ErrorKind::Other,
        "UIApplication must be retrieved on the main thread",
    ))?;

    let app = sharedApplication(mtm).ok_or(Error::new(
        ErrorKind::Other,
        "UIApplication is NULL, perhaps UIApplicationMain has not been executed?",
    ))?;

    // Create ns string class from our string
    let url_string = NSString::from_str(url);
    // Create NSURL object with given string
    let url_object = unsafe { NSURL::URLWithString(&url_string) }.ok_or(Error::new(
        ErrorKind::Other,
        "Failed creating NSURL; is the URL valid?",
    ))?;
    // empty options dictionary
    let options = NSDictionary::new();

    // Open url
    open_url(&app, &url_object, &options);
    Ok(())
}
