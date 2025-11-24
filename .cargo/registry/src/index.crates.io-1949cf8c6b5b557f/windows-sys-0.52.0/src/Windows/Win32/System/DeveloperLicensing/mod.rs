#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wsclient.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AcquireDeveloperLicense(hwndparent : super::super::Foundation:: HWND, pexpiration : *mut super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wsclient.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CheckDeveloperLicense(pexpiration : *mut super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wsclient.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RemoveDeveloperLicense(hwndparent : super::super::Foundation:: HWND) -> ::windows_sys::core::HRESULT);
