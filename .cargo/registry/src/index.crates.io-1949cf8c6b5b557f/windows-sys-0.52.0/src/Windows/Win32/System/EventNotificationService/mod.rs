#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sensapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsDestinationReachableA(lpszdestination : ::windows_sys::core::PCSTR, lpqocinfo : *mut QOCINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sensapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsDestinationReachableW(lpszdestination : ::windows_sys::core::PCWSTR, lpqocinfo : *mut QOCINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sensapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsNetworkAlive(lpdwflags : *mut u32) -> super::super::Foundation:: BOOL);
pub type ISensLogon = *mut ::core::ffi::c_void;
pub type ISensLogon2 = *mut ::core::ffi::c_void;
pub type ISensNetwork = *mut ::core::ffi::c_void;
pub type ISensOnNow = *mut ::core::ffi::c_void;
pub const CONNECTION_AOL: u32 = 4u32;
pub const CONNECTION_LAN: SENS_CONNECTION_TYPE = 0u32;
pub const CONNECTION_WAN: SENS_CONNECTION_TYPE = 1u32;
pub const NETWORK_ALIVE_AOL: u32 = 4u32;
pub const NETWORK_ALIVE_INTERNET: u32 = 8u32;
pub const NETWORK_ALIVE_LAN: u32 = 1u32;
pub const NETWORK_ALIVE_WAN: u32 = 2u32;
pub const SENS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd597cafe_5b9f_11d1_8dd2_00aa004abd5e);
pub const SENSGUID_EVENTCLASS_LOGON: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd5978630_5b9f_11d1_8dd2_00aa004abd5e);
pub const SENSGUID_EVENTCLASS_LOGON2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd5978650_5b9f_11d1_8dd2_00aa004abd5e);
pub const SENSGUID_EVENTCLASS_NETWORK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd5978620_5b9f_11d1_8dd2_00aa004abd5e);
pub const SENSGUID_EVENTCLASS_ONNOW: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd5978640_5b9f_11d1_8dd2_00aa004abd5e);
pub const SENSGUID_PUBLISHER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5fee1bd6_5b9b_11d1_8dd2_00aa004abd5e);
pub const SENSGUID_SUBSCRIBER_LCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd3938ab0_5b9d_11d1_8dd2_00aa004abd5e);
pub const SENSGUID_SUBSCRIBER_WININET: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd3938ab5_5b9d_11d1_8dd2_00aa004abd5e);
pub type SENS_CONNECTION_TYPE = u32;
#[repr(C)]
pub struct QOCINFO {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwInSpeed: u32,
    pub dwOutSpeed: u32,
}
impl ::core::marker::Copy for QOCINFO {}
impl ::core::clone::Clone for QOCINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SENS_QOCINFO {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwOutSpeed: u32,
    pub dwInSpeed: u32,
}
impl ::core::marker::Copy for SENS_QOCINFO {}
impl ::core::clone::Clone for SENS_QOCINFO {
    fn clone(&self) -> Self {
        *self
    }
}
