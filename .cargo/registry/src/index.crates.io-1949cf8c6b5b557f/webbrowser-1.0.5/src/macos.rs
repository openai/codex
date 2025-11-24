use crate::{Browser, BrowserOptions, Error, ErrorKind, Result, TargetType};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::TCFType;
use core_foundation::error::{CFError, CFErrorRef};
use core_foundation::url::{CFURLRef, CFURL};
use std::os::raw::c_void;

/// Deal with opening of browsers on Mac OS X using Core Foundation framework
pub(super) fn open_browser_internal(
    browser: Browser,
    target: &TargetType,
    options: &BrowserOptions,
) -> Result<()> {
    // create the CFUrl for the browser
    let browser_cf_url = match browser {
        Browser::Firefox => create_cf_url("file:///Applications/Firefox.app/"),
        Browser::Chrome => create_cf_url("file:///Applications/Google Chrome.app/"),
        Browser::Opera => create_cf_url("file:///Applications/Opera.app/"),
        Browser::Safari => create_cf_url("file:///Applications/Safari.app/"),
        Browser::Default => {
            if let Some(dummy_url) = create_cf_url("https://") {
                let mut err: CFErrorRef = std::ptr::null_mut();
                let result = unsafe {
                    LSCopyDefaultApplicationURLForURL(
                        dummy_url.as_concrete_TypeRef(),
                        LSROLE_VIEWER,
                        &mut err,
                    )
                };
                if result.is_null() {
                    log::error!("failed to get default browser: {}", unsafe {
                        CFError::wrap_under_create_rule(err)
                    });
                    create_cf_url(DEFAULT_BROWSER_URL)
                } else {
                    let cf_url = unsafe { CFURL::wrap_under_create_rule(result) };
                    log::trace!("default browser is {:?}", &cf_url);
                    Some(cf_url)
                }
            } else {
                create_cf_url(DEFAULT_BROWSER_URL)
            }
        }
        _ => {
            return Err(Error::new(
                ErrorKind::NotFound,
                "browser not supported on macos",
            ))
        }
    }
    .ok_or_else(|| Error::new(ErrorKind::Other, "failed to create CFURL"))?;

    let cf_url = create_cf_url(target.as_ref())
        .ok_or_else(|| Error::new(ErrorKind::Other, "failed to create CFURL"))?;

    let urls_v = [cf_url];
    let urls_arr = CFArray::<CFURL>::from_CFTypes(&urls_v);
    let spec = LSLaunchURLSpec {
        app_url: browser_cf_url.as_concrete_TypeRef(),
        item_urls: urls_arr.as_concrete_TypeRef(),
        pass_thru_params: std::ptr::null(),
        launch_flags: LS_LAUNCH_FLAG_DEFAULTS | LS_LAUNCH_FLAG_ASYNC,
        async_ref_con: std::ptr::null(),
    };

    // handle dry-run scenario
    if options.dry_run {
        return if let Some(path) = browser_cf_url.to_path() {
            if path.is_dir() {
                log::debug!("dry-run: not actually opening the browser {}", &browser);
                Ok(())
            } else {
                log::debug!("dry-run: browser {} not found", &browser);
                Err(Error::new(ErrorKind::NotFound, "browser not found"))
            }
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "unable to convert app url to path",
            ))
        };
    }

    // launch the browser
    log::trace!("about to start browser: {} for {}", &browser, &target);
    let mut launched_app: CFURLRef = std::ptr::null_mut();
    let status = unsafe { LSOpenFromURLSpec(&spec, &mut launched_app) };
    log::trace!("received status: {}", status);
    if status == 0 {
        Ok(())
    } else {
        Err(Error::from(LSError::from(status)))
    }
}

/// Create a Core Foundation CFURL object given a rust-y `url`
fn create_cf_url(url: &str) -> Option<CFURL> {
    let url_u8 = url.as_bytes();
    let url_ref = unsafe {
        core_foundation::url::CFURLCreateWithBytes(
            std::ptr::null(),
            url_u8.as_ptr(),
            url_u8.len() as isize,
            core_foundation::string::kCFStringEncodingUTF8,
            std::ptr::null(),
        )
    };

    if url_ref.is_null() {
        None
    } else {
        Some(unsafe { CFURL::wrap_under_create_rule(url_ref) })
    }
}

type OSStatus = i32;

/// A subset of Launch Services error codes as picked from (`Result Codes` section)
/// https://developer.apple.com/documentation/coreservices/launch_services?language=objc#1661359
enum LSError {
    Unknown(OSStatus),
    ApplicationNotFound,
    NoLaunchPermission,
}

impl From<OSStatus> for LSError {
    fn from(status: OSStatus) -> Self {
        match status {
            // -43 is file not found, while -10814 is launch services err code
            -43 | -10814 => Self::ApplicationNotFound,
            -10826 => Self::NoLaunchPermission,
            _ => Self::Unknown(status),
        }
    }
}

impl std::fmt::Display for LSError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(code) => write!(f, "ls_error: code {}", code),
            Self::ApplicationNotFound => f.write_str("ls_error: application not found"),
            Self::NoLaunchPermission => f.write_str("ls_error: no launch permission"),
        }
    }
}

impl std::fmt::Debug for LSError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl From<LSError> for Error {
    fn from(err: LSError) -> Self {
        let kind = match err {
            LSError::Unknown(_) => ErrorKind::Other,
            LSError::ApplicationNotFound => ErrorKind::NotFound,
            LSError::NoLaunchPermission => ErrorKind::PermissionDenied,
        };
        Error::new(kind, err.to_string())
    }
}

type LSRolesMask = u32;

// as per https://developer.apple.com/documentation/coreservices/lsrolesmask/klsrolesviewer?language=objc
const LSROLE_VIEWER: LSRolesMask = 0x00000002;

// as per https://developer.apple.com/documentation/coreservices/lslaunchflags/klslaunchdefaults?language=objc
const LS_LAUNCH_FLAG_DEFAULTS: u32 = 0x00000001;
const LS_LAUNCH_FLAG_ASYNC: u32 = 0x00010000;

#[repr(C, packed(2))] // Header contains `#pragma pack(push, 2)`.
struct LSLaunchURLSpec {
    app_url: CFURLRef,
    item_urls: CFArrayRef,
    pass_thru_params: *const c_void,
    launch_flags: u32,
    async_ref_con: *const c_void,
}

// Define the functions in CoreServices that we'll be using to open the browser
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    /// Used to get the default browser configured for the user. See:
    /// https://developer.apple.com/documentation/coreservices/1448824-lscopydefaultapplicationurlforur?language=objc
    fn LSCopyDefaultApplicationURLForURL(
        inURL: CFURLRef,
        inRoleMask: LSRolesMask,
        outError: *mut CFErrorRef,
    ) -> CFURLRef;

    /// Used to launch the browser to open a url
    /// https://developer.apple.com/documentation/coreservices/1441986-lsopenfromurlspec?language=objc
    fn LSOpenFromURLSpec(
        inLaunchSpec: *const LSLaunchURLSpec,
        outLaunchedURL: *mut CFURLRef,
    ) -> OSStatus;
}

/// We assume Safari to be the default browser, if deductions fail for any reason
const DEFAULT_BROWSER_URL: &str = "file:///Applications/Safari.app/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_non_existing_browser() {
        let _ = env_logger::try_init();
        if let Err(err) = open_browser_internal(
            Browser::Opera,
            &TargetType::try_from("https://github.com").expect("failed to parse url"),
            &BrowserOptions::default(),
        ) {
            assert_eq!(err.kind(), ErrorKind::NotFound);
        } else {
            panic!("expected opening non-existing browser to fail");
        }
    }

    #[test]
    fn test_existence() {
        let _ = env_logger::try_init();
        assert!(Browser::Safari.exists());
        assert!(!Browser::Opera.exists());
    }
}
