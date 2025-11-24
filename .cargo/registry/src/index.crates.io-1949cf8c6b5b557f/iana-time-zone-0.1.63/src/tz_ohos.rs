//! OpenHarmony doesn't have `/etc/localtime`, we have to use it's "Time Service" to get the timezone information:
//!
//! - [API Reference](https://gitee.com/openharmony/docs/blob/43726785b4033887cd1a838aaaca5e255897a71e/en/application-dev/reference/apis-basic-services-kit/_time_service.md#oh_timeservice_gettimezone)

use crate::ffi_utils::buffer::{tzname_buf, MAX_LEN};
use crate::GetTimezoneError;
use std::ffi::{c_char, CStr};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
enum TimeService_ErrCode {
    TIMESERVICE_ERR_OK = 0,
    TIMESERVICE_ERR_INTERNAL_ERROR = 13000001,
    TIMESERVICE_ERR_INVALID_PARAMETER = 13000002,
}

#[link(name = "time_service_ndk", kind = "dylib")]
extern "C" {
    fn OH_TimeService_GetTimeZone(timeZone: *mut c_char, len: u32) -> TimeService_ErrCode;
}

/// TODO: Change this `CStr::from_bytes_until_nul` once MSRV was bumped above 1.69.0
fn from_bytes_until_nul(bytes: &[u8]) -> Option<&CStr> {
    let nul_pos = bytes.iter().position(|&b| b == 0)?;
    // SAFETY:
    // 1. nul_pos + 1 <= bytes.len()
    // 2. We know there is a nul byte at nul_pos, so this slice (ending at the nul byte) is a well-formed C string.
    Some(unsafe { CStr::from_bytes_with_nul_unchecked(&bytes[..=nul_pos]) })
}

pub(crate) fn get_timezone_inner() -> Result<String, GetTimezoneError> {
    let mut time_zone = tzname_buf();
    // SAFETY:
    // `time_zone` is a valid buffer with a length of 40 bytes.
    let ret = unsafe {
        OH_TimeService_GetTimeZone(time_zone.as_mut_ptr().cast::<c_char>(), MAX_LEN as u32 - 1)
    };
    if ret != TimeService_ErrCode::TIMESERVICE_ERR_OK {
        return Err(GetTimezoneError::OsError);
    }
    from_bytes_until_nul(&time_zone)
        .and_then(|x| x.to_str().ok())
        .map(|x| x.to_owned())
        .ok_or(GetTimezoneError::OsError)
}
