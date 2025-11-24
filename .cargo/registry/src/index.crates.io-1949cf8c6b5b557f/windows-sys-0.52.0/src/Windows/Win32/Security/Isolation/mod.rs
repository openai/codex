#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("userenv.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateAppContainerProfile(pszappcontainername : ::windows_sys::core::PCWSTR, pszdisplayname : ::windows_sys::core::PCWSTR, pszdescription : ::windows_sys::core::PCWSTR, pcapabilities : *const super:: SID_AND_ATTRIBUTES, dwcapabilitycount : u32, ppsidappcontainersid : *mut super::super::Foundation:: PSID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("userenv.dll" "system" fn DeleteAppContainerProfile(pszappcontainername : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("userenv.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DeriveAppContainerSidFromAppContainerName(pszappcontainername : ::windows_sys::core::PCWSTR, ppsidappcontainersid : *mut super::super::Foundation:: PSID) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("userenv.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DeriveRestrictedAppContainerSidFromAppContainerSidAndRestrictedName(psidappcontainersid : super::super::Foundation:: PSID, pszrestrictedappcontainername : ::windows_sys::core::PCWSTR, ppsidrestrictedappcontainersid : *mut super::super::Foundation:: PSID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("userenv.dll" "system" fn GetAppContainerFolderPath(pszappcontainersid : ::windows_sys::core::PCWSTR, ppszpath : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetAppContainerNamedObjectPath(token : super::super::Foundation:: HANDLE, appcontainersid : super::super::Foundation:: PSID, objectpathlength : u32, objectpath : ::windows_sys::core::PWSTR, returnlength : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("userenv.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetAppContainerRegistryLocation(desiredaccess : u32, phappcontainerkey : *mut super::super::System::Registry:: HKEY) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("isolatedwindowsenvironmentutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCrossIsolatedEnvironmentClipboardContent(iscrossisolatedenvironmentclipboardcontent : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-security-isolatedcontainer-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsProcessInIsolatedContainer(isprocessinisolatedcontainer : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("isolatedwindowsenvironmentutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsProcessInIsolatedWindowsEnvironment(isprocessinisolatedwindowsenvironment : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-security-isolatedcontainer-l1-1-1.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsProcessInWDAGContainer(reserved : *const ::core::ffi::c_void, isprocessinwdagcontainer : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
pub type IIsolatedAppLauncher = *mut ::core::ffi::c_void;
pub const IsolatedAppLauncher: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbc812430_e75e_4fd1_9641_1f9f1e2d9a1f);
pub const WDAG_CLIPBOARD_TAG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CrossIsolatedEnvironmentContent");
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IsolatedAppLauncherTelemetryParameters {
    pub EnableForLaunch: super::super::Foundation::BOOL,
    pub CorrelationGUID: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IsolatedAppLauncherTelemetryParameters {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IsolatedAppLauncherTelemetryParameters {
    fn clone(&self) -> Self {
        *self
    }
}
