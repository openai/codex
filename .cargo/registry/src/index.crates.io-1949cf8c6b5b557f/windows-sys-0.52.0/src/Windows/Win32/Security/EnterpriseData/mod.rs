::windows_targets::link!("efswrt.dll" "system" fn ProtectFileToEnterpriseIdentity(fileorfolderpath : ::windows_sys::core::PCWSTR, identity : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("srpapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SrpCloseThreadNetworkContext(threadnetworkcontext : *mut HTHREAD_NETWORK_CONTEXT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("srpapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SrpCreateThreadNetworkContext(enterpriseid : ::windows_sys::core::PCWSTR, threadnetworkcontext : *mut HTHREAD_NETWORK_CONTEXT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("srpapi.dll" "system" fn SrpDisablePermissiveModeFileEncryption() -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_Packaging_Appx"))]
::windows_targets::link!("srpapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_Packaging_Appx\"`"] fn SrpDoesPolicyAllowAppExecution(packageid : *const super::super::Storage::Packaging::Appx:: PACKAGE_ID, isallowed : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("srpapi.dll" "system" fn SrpEnablePermissiveModeFileEncryption(enterpriseid : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("srpapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SrpGetEnterpriseIds(tokenhandle : super::super::Foundation:: HANDLE, numberofbytes : *mut u32, enterpriseids : *mut ::windows_sys::core::PCWSTR, enterpriseidcount : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("srpapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SrpGetEnterprisePolicy(tokenhandle : super::super::Foundation:: HANDLE, policyflags : *mut ENTERPRISE_DATA_POLICIES) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("srpapi.dll" "system" fn SrpHostingInitialize(version : SRPHOSTING_VERSION, r#type : SRPHOSTING_TYPE, pvdata : *const ::core::ffi::c_void, cbdata : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("srpapi.dll" "system" fn SrpHostingTerminate(r#type : SRPHOSTING_TYPE) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("srpapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SrpIsTokenService(tokenhandle : super::super::Foundation:: HANDLE, istokenservice : *mut u8) -> super::super::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("srpapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SrpSetTokenEnterpriseId(tokenhandle : super::super::Foundation:: HANDLE, enterpriseid : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("efswrt.dll" "system" fn UnprotectFile(fileorfolderpath : ::windows_sys::core::PCWSTR, options : *const FILE_UNPROTECT_OPTIONS) -> ::windows_sys::core::HRESULT);
pub type IProtectionPolicyManagerInterop = *mut ::core::ffi::c_void;
pub type IProtectionPolicyManagerInterop2 = *mut ::core::ffi::c_void;
pub type IProtectionPolicyManagerInterop3 = *mut ::core::ffi::c_void;
pub const ENTERPRISE_POLICY_ALLOWED: ENTERPRISE_DATA_POLICIES = 1i32;
pub const ENTERPRISE_POLICY_ENLIGHTENED: ENTERPRISE_DATA_POLICIES = 2i32;
pub const ENTERPRISE_POLICY_EXEMPT: ENTERPRISE_DATA_POLICIES = 4i32;
pub const ENTERPRISE_POLICY_NONE: ENTERPRISE_DATA_POLICIES = 0i32;
pub const SRPHOSTING_TYPE_NONE: SRPHOSTING_TYPE = 0i32;
pub const SRPHOSTING_TYPE_WINHTTP: SRPHOSTING_TYPE = 1i32;
pub const SRPHOSTING_TYPE_WININET: SRPHOSTING_TYPE = 2i32;
pub const SRPHOSTING_VERSION1: SRPHOSTING_VERSION = 1i32;
pub type ENTERPRISE_DATA_POLICIES = i32;
pub type SRPHOSTING_TYPE = i32;
pub type SRPHOSTING_VERSION = i32;
#[repr(C)]
pub struct FILE_UNPROTECT_OPTIONS {
    pub audit: u8,
}
impl ::core::marker::Copy for FILE_UNPROTECT_OPTIONS {}
impl ::core::clone::Clone for FILE_UNPROTECT_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HTHREAD_NETWORK_CONTEXT {
    pub ThreadId: u32,
    pub ThreadContext: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HTHREAD_NETWORK_CONTEXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HTHREAD_NETWORK_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
