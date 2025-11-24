#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("ualapi.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn UalInstrument(data : *const UAL_DATA_BLOB) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ualapi.dll" "system" fn UalRegisterProduct(wszproductname : ::windows_sys::core::PCWSTR, wszrolename : ::windows_sys::core::PCWSTR, wszguid : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("ualapi.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn UalStart(data : *const UAL_DATA_BLOB) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("ualapi.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn UalStop(data : *const UAL_DATA_BLOB) -> ::windows_sys::core::HRESULT);
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct UAL_DATA_BLOB {
    pub Size: u32,
    pub RoleGuid: ::windows_sys::core::GUID,
    pub TenantId: ::windows_sys::core::GUID,
    pub Address: super::super::Networking::WinSock::SOCKADDR_STORAGE,
    pub UserName: [u16; 260],
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for UAL_DATA_BLOB {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for UAL_DATA_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
