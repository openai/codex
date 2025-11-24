::windows_targets::link!("srclient.dll" "system" fn SRRemoveRestorePoint(dwrpnum : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sfc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SRSetRestorePointA(prestoreptspec : *const RESTOREPOINTINFOA, psmgrstatus : *mut STATEMGRSTATUS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("sfc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SRSetRestorePointW(prestoreptspec : *const RESTOREPOINTINFOW, psmgrstatus : *mut STATEMGRSTATUS) -> super::super::Foundation:: BOOL);
pub const ACCESSIBILITY_SETTING: u32 = 3u32;
pub const APPLICATION_INSTALL: RESTOREPOINTINFO_TYPE = 0u32;
pub const APPLICATION_RUN: u32 = 5u32;
pub const APPLICATION_UNINSTALL: RESTOREPOINTINFO_TYPE = 1u32;
pub const BACKUP: u32 = 15u32;
pub const BACKUP_RECOVERY: u32 = 14u32;
pub const BEGIN_NESTED_SYSTEM_CHANGE: RESTOREPOINTINFO_EVENT_TYPE = 102u32;
pub const BEGIN_NESTED_SYSTEM_CHANGE_NORP: u32 = 104u32;
pub const BEGIN_SYSTEM_CHANGE: RESTOREPOINTINFO_EVENT_TYPE = 100u32;
pub const CANCELLED_OPERATION: RESTOREPOINTINFO_TYPE = 13u32;
pub const CHECKPOINT: u32 = 7u32;
pub const CRITICAL_UPDATE: u32 = 18u32;
pub const DESKTOP_SETTING: u32 = 2u32;
pub const DEVICE_DRIVER_INSTALL: RESTOREPOINTINFO_TYPE = 10u32;
pub const END_NESTED_SYSTEM_CHANGE: RESTOREPOINTINFO_EVENT_TYPE = 103u32;
pub const END_SYSTEM_CHANGE: RESTOREPOINTINFO_EVENT_TYPE = 101u32;
pub const FIRSTRUN: u32 = 11u32;
pub const MANUAL_CHECKPOINT: u32 = 16u32;
pub const MAX_DESC: u32 = 64u32;
pub const MAX_DESC_W: u32 = 256u32;
pub const MAX_EVENT: u32 = 104u32;
pub const MAX_RPT: u32 = 18u32;
pub const MIN_EVENT: u32 = 100u32;
pub const MIN_RPT: u32 = 0u32;
pub const MODIFY_SETTINGS: RESTOREPOINTINFO_TYPE = 12u32;
pub const OE_SETTING: u32 = 4u32;
pub const RESTORE: u32 = 6u32;
pub const WINDOWS_BOOT: u32 = 9u32;
pub const WINDOWS_SHUTDOWN: u32 = 8u32;
pub const WINDOWS_UPDATE: u32 = 17u32;
pub type RESTOREPOINTINFO_EVENT_TYPE = u32;
pub type RESTOREPOINTINFO_TYPE = u32;
#[repr(C, packed(1))]
pub struct RESTOREPOINTINFOA {
    pub dwEventType: RESTOREPOINTINFO_EVENT_TYPE,
    pub dwRestorePtType: RESTOREPOINTINFO_TYPE,
    pub llSequenceNumber: i64,
    pub szDescription: [u8; 64],
}
impl ::core::marker::Copy for RESTOREPOINTINFOA {}
impl ::core::clone::Clone for RESTOREPOINTINFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESTOREPOINTINFOEX {
    pub ftCreation: super::super::Foundation::FILETIME,
    pub dwEventType: u32,
    pub dwRestorePtType: u32,
    pub dwRPNum: u32,
    pub szDescription: [u16; 256],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESTOREPOINTINFOEX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESTOREPOINTINFOEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct RESTOREPOINTINFOW {
    pub dwEventType: RESTOREPOINTINFO_EVENT_TYPE,
    pub dwRestorePtType: RESTOREPOINTINFO_TYPE,
    pub llSequenceNumber: i64,
    pub szDescription: [u16; 256],
}
impl ::core::marker::Copy for RESTOREPOINTINFOW {}
impl ::core::clone::Clone for RESTOREPOINTINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct STATEMGRSTATUS {
    pub nStatus: super::super::Foundation::WIN32_ERROR,
    pub llSequenceNumber: i64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for STATEMGRSTATUS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for STATEMGRSTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
