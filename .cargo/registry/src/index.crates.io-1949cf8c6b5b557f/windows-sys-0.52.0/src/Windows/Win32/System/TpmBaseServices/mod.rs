#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tbs.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDeviceID(pbwindowsaik : *mut u8, cbwindowsaik : u32, pcbresult : *mut u32, pfprotectedbytpm : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tbs.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDeviceIDString(pszwindowsaik : ::windows_sys::core::PWSTR, cchwindowsaik : u32, pcchresult : *mut u32, pfprotectedbytpm : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_Context_Create(pcontextparams : *const TBS_CONTEXT_PARAMS, phcontext : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_Create_Windows_Key(keyhandle : u32) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_GetDeviceInfo(size : u32, info : *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_Get_OwnerAuth(hcontext : *const ::core::ffi::c_void, ownerauthtype : u32, poutputbuf : *mut u8, poutputbuflen : *mut u32) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_Get_TCG_Log(hcontext : *const ::core::ffi::c_void, poutputbuf : *mut u8, poutputbuflen : *mut u32) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_Get_TCG_Log_Ex(logtype : u32, pboutput : *mut u8, pcboutput : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tbs.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Tbsi_Is_Tpm_Present() -> super::super::Foundation:: BOOL);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_Physical_Presence_Command(hcontext : *const ::core::ffi::c_void, pabinput : *const u8, cbinput : u32, paboutput : *mut u8, pcboutput : *mut u32) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsi_Revoke_Attestation() -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsip_Cancel_Commands(hcontext : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsip_Context_Close(hcontext : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("tbs.dll" "system" fn Tbsip_Submit_Command(hcontext : *const ::core::ffi::c_void, locality : TBS_COMMAND_LOCALITY, priority : TBS_COMMAND_PRIORITY, pabcommand : *const u8, cbcommand : u32, pabresult : *mut u8, pcbresult : *mut u32) -> u32);
pub const TBS_COMMAND_LOCALITY_FOUR: TBS_COMMAND_LOCALITY = 4u32;
pub const TBS_COMMAND_LOCALITY_ONE: TBS_COMMAND_LOCALITY = 1u32;
pub const TBS_COMMAND_LOCALITY_THREE: TBS_COMMAND_LOCALITY = 3u32;
pub const TBS_COMMAND_LOCALITY_TWO: TBS_COMMAND_LOCALITY = 2u32;
pub const TBS_COMMAND_LOCALITY_ZERO: TBS_COMMAND_LOCALITY = 0u32;
pub const TBS_COMMAND_PRIORITY_HIGH: TBS_COMMAND_PRIORITY = 300u32;
pub const TBS_COMMAND_PRIORITY_LOW: TBS_COMMAND_PRIORITY = 100u32;
pub const TBS_COMMAND_PRIORITY_MAX: TBS_COMMAND_PRIORITY = 2147483648u32;
pub const TBS_COMMAND_PRIORITY_NORMAL: TBS_COMMAND_PRIORITY = 200u32;
pub const TBS_COMMAND_PRIORITY_SYSTEM: TBS_COMMAND_PRIORITY = 400u32;
pub const TBS_CONTEXT_VERSION_ONE: u32 = 1u32;
pub const TBS_CONTEXT_VERSION_TWO: u32 = 2u32;
pub const TBS_OWNERAUTH_TYPE_ADMIN: u32 = 2u32;
pub const TBS_OWNERAUTH_TYPE_ENDORSEMENT: u32 = 4u32;
pub const TBS_OWNERAUTH_TYPE_ENDORSEMENT_20: u32 = 12u32;
pub const TBS_OWNERAUTH_TYPE_FULL: u32 = 1u32;
pub const TBS_OWNERAUTH_TYPE_STORAGE_20: u32 = 13u32;
pub const TBS_OWNERAUTH_TYPE_USER: u32 = 3u32;
pub const TBS_SUCCESS: u32 = 0u32;
pub const TBS_TCGLOG_DRTM_BOOT: u32 = 4u32;
pub const TBS_TCGLOG_DRTM_CURRENT: u32 = 1u32;
pub const TBS_TCGLOG_DRTM_RESUME: u32 = 5u32;
pub const TBS_TCGLOG_SRTM_BOOT: u32 = 2u32;
pub const TBS_TCGLOG_SRTM_CURRENT: u32 = 0u32;
pub const TBS_TCGLOG_SRTM_RESUME: u32 = 3u32;
pub const TPM_IFTYPE_1: u32 = 1u32;
pub const TPM_IFTYPE_EMULATOR: u32 = 4u32;
pub const TPM_IFTYPE_HW: u32 = 3u32;
pub const TPM_IFTYPE_SPB: u32 = 5u32;
pub const TPM_IFTYPE_TRUSTZONE: u32 = 2u32;
pub const TPM_IFTYPE_UNKNOWN: u32 = 0u32;
pub const TPM_VERSION_12: u32 = 1u32;
pub const TPM_VERSION_20: u32 = 2u32;
pub const TPM_VERSION_UNKNOWN: u32 = 0u32;
pub const TPM_WNF_INFO_CLEAR_SUCCESSFUL: u32 = 1u32;
pub const TPM_WNF_INFO_NO_REBOOT_REQUIRED: u32 = 1u32;
pub const TPM_WNF_INFO_OWNERSHIP_SUCCESSFUL: u32 = 2u32;
pub type TBS_COMMAND_LOCALITY = u32;
pub type TBS_COMMAND_PRIORITY = u32;
#[repr(C)]
pub struct TBS_CONTEXT_PARAMS {
    pub version: u32,
}
impl ::core::marker::Copy for TBS_CONTEXT_PARAMS {}
impl ::core::clone::Clone for TBS_CONTEXT_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TBS_CONTEXT_PARAMS2 {
    pub version: u32,
    pub Anonymous: TBS_CONTEXT_PARAMS2_0,
}
impl ::core::marker::Copy for TBS_CONTEXT_PARAMS2 {}
impl ::core::clone::Clone for TBS_CONTEXT_PARAMS2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union TBS_CONTEXT_PARAMS2_0 {
    pub Anonymous: TBS_CONTEXT_PARAMS2_0_0,
    pub asUINT32: u32,
}
impl ::core::marker::Copy for TBS_CONTEXT_PARAMS2_0 {}
impl ::core::clone::Clone for TBS_CONTEXT_PARAMS2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TBS_CONTEXT_PARAMS2_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for TBS_CONTEXT_PARAMS2_0_0 {}
impl ::core::clone::Clone for TBS_CONTEXT_PARAMS2_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TPM_DEVICE_INFO {
    pub structVersion: u32,
    pub tpmVersion: u32,
    pub tpmInterfaceType: u32,
    pub tpmImpRevision: u32,
}
impl ::core::marker::Copy for TPM_DEVICE_INFO {}
impl ::core::clone::Clone for TPM_DEVICE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TPM_WNF_PROVISIONING {
    pub status: u32,
    pub message: [u8; 28],
}
impl ::core::marker::Copy for TPM_WNF_PROVISIONING {}
impl ::core::clone::Clone for TPM_WNF_PROVISIONING {
    fn clone(&self) -> Self {
        *self
    }
}
