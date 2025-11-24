#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn CreateMailslotA(lpname : ::windows_sys::core::PCSTR, nmaxmessagesize : u32, lreadtimeout : u32, lpsecurityattributes : *const super::super::Security:: SECURITY_ATTRIBUTES) -> super::super::Foundation:: HANDLE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn CreateMailslotW(lpname : ::windows_sys::core::PCWSTR, nmaxmessagesize : u32, lreadtimeout : u32, lpsecurityattributes : *const super::super::Security:: SECURITY_ATTRIBUTES) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetMailslotInfo(hmailslot : super::super::Foundation:: HANDLE, lpmaxmessagesize : *mut u32, lpnextsize : *mut u32, lpmessagecount : *mut u32, lpreadtimeout : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetMailslotInfo(hmailslot : super::super::Foundation:: HANDLE, lreadtimeout : u32) -> super::super::Foundation:: BOOL);
