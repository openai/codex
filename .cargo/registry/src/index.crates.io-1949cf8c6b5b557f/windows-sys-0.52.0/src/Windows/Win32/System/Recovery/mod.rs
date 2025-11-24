#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ApplicationRecoveryFinished(bsuccess : super::super::Foundation:: BOOL) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ApplicationRecoveryInProgress(pbcancelled : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_WindowsProgramming\"`"] fn GetApplicationRecoveryCallback(hprocess : super::super::Foundation:: HANDLE, precoverycallback : *mut super::WindowsProgramming:: APPLICATION_RECOVERY_CALLBACK, ppvparameter : *mut *mut ::core::ffi::c_void, pdwpinginterval : *mut u32, pdwflags : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetApplicationRestartSettings(hprocess : super::super::Foundation:: HANDLE, pwzcommandline : ::windows_sys::core::PWSTR, pcchsize : *mut u32, pdwflags : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_WindowsProgramming")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_System_WindowsProgramming\"`"] fn RegisterApplicationRecoveryCallback(precoveycallback : super::WindowsProgramming:: APPLICATION_RECOVERY_CALLBACK, pvparameter : *const ::core::ffi::c_void, dwpinginterval : u32, dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn RegisterApplicationRestart(pwzcommandline : ::windows_sys::core::PCWSTR, dwflags : REGISTER_APPLICATION_RESTART_FLAGS) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn UnregisterApplicationRecoveryCallback() -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn UnregisterApplicationRestart() -> ::windows_sys::core::HRESULT);
pub const RESTART_NO_CRASH: REGISTER_APPLICATION_RESTART_FLAGS = 1u32;
pub const RESTART_NO_HANG: REGISTER_APPLICATION_RESTART_FLAGS = 2u32;
pub const RESTART_NO_PATCH: REGISTER_APPLICATION_RESTART_FLAGS = 4u32;
pub const RESTART_NO_REBOOT: REGISTER_APPLICATION_RESTART_FLAGS = 8u32;
pub type REGISTER_APPLICATION_RESTART_FLAGS = u32;
