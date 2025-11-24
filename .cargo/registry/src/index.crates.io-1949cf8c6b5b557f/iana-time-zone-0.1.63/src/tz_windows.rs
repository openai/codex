#[path = "windows_bindings.rs"]
#[allow(missing_debug_implementations, clippy::undocumented_unsafe_blocks)]
mod windows_bindings;
use windows_bindings::Windows::Globalization::Calendar;

impl From<windows_core::Error> for crate::GetTimezoneError {
    fn from(orig: windows_core::Error) -> Self {
        crate::GetTimezoneError::IoError(std::io::Error::new(std::io::ErrorKind::Other, orig))
    }
}

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    let cal = Calendar::new()?;
    let tz_hstring = cal.GetTimeZone()?;
    Ok(tz_hstring.to_string())
}
