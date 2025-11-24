#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmAddFilter(dwsessionhandle : u32, strmodulename : ::windows_sys::core::PCWSTR, pprocess : *const RM_UNIQUE_PROCESS, strserviceshortname : ::windows_sys::core::PCWSTR, filteraction : RM_FILTER_ACTION) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmCancelCurrentTask(dwsessionhandle : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmEndSession(dwsessionhandle : u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmGetFilterList(dwsessionhandle : u32, pbfilterbuf : *mut u8, cbfilterbuf : u32, cbfilterbufneeded : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmGetList(dwsessionhandle : u32, pnprocinfoneeded : *mut u32, pnprocinfo : *mut u32, rgaffectedapps : *mut RM_PROCESS_INFO, lpdwrebootreasons : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmJoinSession(psessionhandle : *mut u32, strsessionkey : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmRegisterResources(dwsessionhandle : u32, nfiles : u32, rgsfilenames : *const ::windows_sys::core::PCWSTR, napplications : u32, rgapplications : *const RM_UNIQUE_PROCESS, nservices : u32, rgsservicenames : *const ::windows_sys::core::PCWSTR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmRemoveFilter(dwsessionhandle : u32, strmodulename : ::windows_sys::core::PCWSTR, pprocess : *const RM_UNIQUE_PROCESS, strserviceshortname : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmRestart(dwsessionhandle : u32, dwrestartflags : u32, fnstatus : RM_WRITE_STATUS_CALLBACK) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmShutdown(dwsessionhandle : u32, lactionflags : u32, fnstatus : RM_WRITE_STATUS_CALLBACK) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rstrtmgr.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RmStartSession(psessionhandle : *mut u32, dwsessionflags : u32, strsessionkey : ::windows_sys::core::PWSTR) -> super::super::Foundation:: WIN32_ERROR);
pub const CCH_RM_MAX_APP_NAME: u32 = 255u32;
pub const CCH_RM_MAX_SVC_NAME: u32 = 63u32;
pub const CCH_RM_SESSION_KEY: u32 = 32u32;
pub const RM_INVALID_PROCESS: i32 = -1i32;
pub const RM_INVALID_TS_SESSION: i32 = -1i32;
pub const RmConsole: RM_APP_TYPE = 5i32;
pub const RmCritical: RM_APP_TYPE = 1000i32;
pub const RmExplorer: RM_APP_TYPE = 4i32;
pub const RmFilterTriggerFile: RM_FILTER_TRIGGER = 1i32;
pub const RmFilterTriggerInvalid: RM_FILTER_TRIGGER = 0i32;
pub const RmFilterTriggerProcess: RM_FILTER_TRIGGER = 2i32;
pub const RmFilterTriggerService: RM_FILTER_TRIGGER = 3i32;
pub const RmForceShutdown: RM_SHUTDOWN_TYPE = 1i32;
pub const RmInvalidFilterAction: RM_FILTER_ACTION = 0i32;
pub const RmMainWindow: RM_APP_TYPE = 1i32;
pub const RmNoRestart: RM_FILTER_ACTION = 1i32;
pub const RmNoShutdown: RM_FILTER_ACTION = 2i32;
pub const RmOtherWindow: RM_APP_TYPE = 2i32;
pub const RmRebootReasonCriticalProcess: RM_REBOOT_REASON = 4i32;
pub const RmRebootReasonCriticalService: RM_REBOOT_REASON = 8i32;
pub const RmRebootReasonDetectedSelf: RM_REBOOT_REASON = 16i32;
pub const RmRebootReasonNone: RM_REBOOT_REASON = 0i32;
pub const RmRebootReasonPermissionDenied: RM_REBOOT_REASON = 1i32;
pub const RmRebootReasonSessionMismatch: RM_REBOOT_REASON = 2i32;
pub const RmService: RM_APP_TYPE = 3i32;
pub const RmShutdownOnlyRegistered: RM_SHUTDOWN_TYPE = 16i32;
pub const RmStatusErrorOnRestart: RM_APP_STATUS = 32i32;
pub const RmStatusErrorOnStop: RM_APP_STATUS = 16i32;
pub const RmStatusRestartMasked: RM_APP_STATUS = 128i32;
pub const RmStatusRestarted: RM_APP_STATUS = 8i32;
pub const RmStatusRunning: RM_APP_STATUS = 1i32;
pub const RmStatusShutdownMasked: RM_APP_STATUS = 64i32;
pub const RmStatusStopped: RM_APP_STATUS = 2i32;
pub const RmStatusStoppedOther: RM_APP_STATUS = 4i32;
pub const RmStatusUnknown: RM_APP_STATUS = 0i32;
pub const RmUnknownApp: RM_APP_TYPE = 0i32;
pub type RM_APP_STATUS = i32;
pub type RM_APP_TYPE = i32;
pub type RM_FILTER_ACTION = i32;
pub type RM_FILTER_TRIGGER = i32;
pub type RM_REBOOT_REASON = i32;
pub type RM_SHUTDOWN_TYPE = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RM_FILTER_INFO {
    pub FilterAction: RM_FILTER_ACTION,
    pub FilterTrigger: RM_FILTER_TRIGGER,
    pub cbNextOffset: u32,
    pub Anonymous: RM_FILTER_INFO_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RM_FILTER_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RM_FILTER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union RM_FILTER_INFO_0 {
    pub strFilename: ::windows_sys::core::PWSTR,
    pub Process: RM_UNIQUE_PROCESS,
    pub strServiceShortName: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RM_FILTER_INFO_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RM_FILTER_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RM_PROCESS_INFO {
    pub Process: RM_UNIQUE_PROCESS,
    pub strAppName: [u16; 256],
    pub strServiceShortName: [u16; 64],
    pub ApplicationType: RM_APP_TYPE,
    pub AppStatus: u32,
    pub TSSessionId: u32,
    pub bRestartable: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RM_PROCESS_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RM_PROCESS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RM_UNIQUE_PROCESS {
    pub dwProcessId: u32,
    pub ProcessStartTime: super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RM_UNIQUE_PROCESS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RM_UNIQUE_PROCESS {
    fn clone(&self) -> Self {
        *self
    }
}
pub type RM_WRITE_STATUS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(npercentcomplete: u32) -> ()>;
