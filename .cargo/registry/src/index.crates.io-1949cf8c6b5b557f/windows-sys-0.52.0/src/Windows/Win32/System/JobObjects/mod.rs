#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AssignProcessToJobObject(hjob : super::super::Foundation:: HANDLE, hprocess : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn CreateJobObjectA(lpjobattributes : *const super::super::Security:: SECURITY_ATTRIBUTES, lpname : ::windows_sys::core::PCSTR) -> super::super::Foundation:: HANDLE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn CreateJobObjectW(lpjobattributes : *const super::super::Security:: SECURITY_ATTRIBUTES, lpname : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateJobSet(numjob : u32, userjobset : *const JOB_SET_ARRAY, flags : u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn FreeMemoryJobObject(buffer : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsProcessInJob(processhandle : super::super::Foundation:: HANDLE, jobhandle : super::super::Foundation:: HANDLE, result : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenJobObjectA(dwdesiredaccess : u32, binherithandle : super::super::Foundation:: BOOL, lpname : ::windows_sys::core::PCSTR) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenJobObjectW(dwdesiredaccess : u32, binherithandle : super::super::Foundation:: BOOL, lpname : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QueryInformationJobObject(hjob : super::super::Foundation:: HANDLE, jobobjectinformationclass : JOBOBJECTINFOCLASS, lpjobobjectinformation : *mut ::core::ffi::c_void, cbjobobjectinformationlength : u32, lpreturnlength : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QueryIoRateControlInformationJobObject(hjob : super::super::Foundation:: HANDLE, volumename : ::windows_sys::core::PCWSTR, infoblocks : *mut *mut JOBOBJECT_IO_RATE_CONTROL_INFORMATION, infoblockcount : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetInformationJobObject(hjob : super::super::Foundation:: HANDLE, jobobjectinformationclass : JOBOBJECTINFOCLASS, lpjobobjectinformation : *const ::core::ffi::c_void, cbjobobjectinformationlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetIoRateControlInformationJobObject(hjob : super::super::Foundation:: HANDLE, ioratecontrolinfo : *const JOBOBJECT_IO_RATE_CONTROL_INFORMATION) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TerminateJobObject(hjob : super::super::Foundation:: HANDLE, uexitcode : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UserHandleGrantAccess(huserhandle : super::super::Foundation:: HANDLE, hjob : super::super::Foundation:: HANDLE, bgrant : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
pub const JOBOBJECT_IO_ATTRIBUTION_CONTROL_DISABLE: JOBOBJECT_IO_ATTRIBUTION_CONTROL_FLAGS = 2i32;
pub const JOBOBJECT_IO_ATTRIBUTION_CONTROL_ENABLE: JOBOBJECT_IO_ATTRIBUTION_CONTROL_FLAGS = 1i32;
pub const JOBOBJECT_IO_ATTRIBUTION_CONTROL_VALID_FLAGS: JOBOBJECT_IO_ATTRIBUTION_CONTROL_FLAGS = 3i32;
pub const JOB_OBJECT_BASIC_LIMIT_VALID_FLAGS: JOB_OBJECT_LIMIT = 255u32;
pub const JOB_OBJECT_CPU_RATE_CONTROL_ENABLE: JOB_OBJECT_CPU_RATE_CONTROL = 1u32;
pub const JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP: JOB_OBJECT_CPU_RATE_CONTROL = 4u32;
pub const JOB_OBJECT_CPU_RATE_CONTROL_MIN_MAX_RATE: JOB_OBJECT_CPU_RATE_CONTROL = 16u32;
pub const JOB_OBJECT_CPU_RATE_CONTROL_NOTIFY: JOB_OBJECT_CPU_RATE_CONTROL = 8u32;
pub const JOB_OBJECT_CPU_RATE_CONTROL_VALID_FLAGS: JOB_OBJECT_CPU_RATE_CONTROL = 31u32;
pub const JOB_OBJECT_CPU_RATE_CONTROL_WEIGHT_BASED: JOB_OBJECT_CPU_RATE_CONTROL = 2u32;
pub const JOB_OBJECT_EXTENDED_LIMIT_VALID_FLAGS: JOB_OBJECT_LIMIT = 32767u32;
pub const JOB_OBJECT_IO_RATE_CONTROL_ENABLE: JOB_OBJECT_IO_RATE_CONTROL_FLAGS = 1i32;
pub const JOB_OBJECT_IO_RATE_CONTROL_FORCE_UNIT_ACCESS_ALL: JOB_OBJECT_IO_RATE_CONTROL_FLAGS = 4i32;
pub const JOB_OBJECT_IO_RATE_CONTROL_FORCE_UNIT_ACCESS_ON_SOFT_CAP: JOB_OBJECT_IO_RATE_CONTROL_FLAGS = 8i32;
pub const JOB_OBJECT_IO_RATE_CONTROL_STANDALONE_VOLUME: JOB_OBJECT_IO_RATE_CONTROL_FLAGS = 2i32;
pub const JOB_OBJECT_IO_RATE_CONTROL_VALID_FLAGS: JOB_OBJECT_IO_RATE_CONTROL_FLAGS = 15i32;
pub const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: JOB_OBJECT_LIMIT = 8u32;
pub const JOB_OBJECT_LIMIT_AFFINITY: JOB_OBJECT_LIMIT = 16u32;
pub const JOB_OBJECT_LIMIT_BREAKAWAY_OK: JOB_OBJECT_LIMIT = 2048u32;
pub const JOB_OBJECT_LIMIT_CPU_RATE_CONTROL: JOB_OBJECT_LIMIT = 262144u32;
pub const JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION: JOB_OBJECT_LIMIT = 1024u32;
pub const JOB_OBJECT_LIMIT_IO_RATE_CONTROL: JOB_OBJECT_LIMIT = 524288u32;
pub const JOB_OBJECT_LIMIT_JOB_MEMORY: JOB_OBJECT_LIMIT = 512u32;
pub const JOB_OBJECT_LIMIT_JOB_MEMORY_HIGH: JOB_OBJECT_LIMIT = 512u32;
pub const JOB_OBJECT_LIMIT_JOB_MEMORY_LOW: JOB_OBJECT_LIMIT = 32768u32;
pub const JOB_OBJECT_LIMIT_JOB_READ_BYTES: JOB_OBJECT_LIMIT = 65536u32;
pub const JOB_OBJECT_LIMIT_JOB_TIME: JOB_OBJECT_LIMIT = 4u32;
pub const JOB_OBJECT_LIMIT_JOB_WRITE_BYTES: JOB_OBJECT_LIMIT = 131072u32;
pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: JOB_OBJECT_LIMIT = 8192u32;
pub const JOB_OBJECT_LIMIT_NET_RATE_CONTROL: JOB_OBJECT_LIMIT = 1048576u32;
pub const JOB_OBJECT_LIMIT_PRESERVE_JOB_TIME: JOB_OBJECT_LIMIT = 64u32;
pub const JOB_OBJECT_LIMIT_PRIORITY_CLASS: JOB_OBJECT_LIMIT = 32u32;
pub const JOB_OBJECT_LIMIT_PROCESS_MEMORY: JOB_OBJECT_LIMIT = 256u32;
pub const JOB_OBJECT_LIMIT_PROCESS_TIME: JOB_OBJECT_LIMIT = 2u32;
pub const JOB_OBJECT_LIMIT_RATE_CONTROL: JOB_OBJECT_LIMIT = 262144u32;
pub const JOB_OBJECT_LIMIT_SCHEDULING_CLASS: JOB_OBJECT_LIMIT = 128u32;
pub const JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK: JOB_OBJECT_LIMIT = 4096u32;
pub const JOB_OBJECT_LIMIT_SUBSET_AFFINITY: JOB_OBJECT_LIMIT = 16384u32;
pub const JOB_OBJECT_LIMIT_VALID_FLAGS: JOB_OBJECT_LIMIT = 524287u32;
pub const JOB_OBJECT_LIMIT_WORKINGSET: JOB_OBJECT_LIMIT = 1u32;
pub const JOB_OBJECT_NET_RATE_CONTROL_DSCP_TAG: JOB_OBJECT_NET_RATE_CONTROL_FLAGS = 4i32;
pub const JOB_OBJECT_NET_RATE_CONTROL_ENABLE: JOB_OBJECT_NET_RATE_CONTROL_FLAGS = 1i32;
pub const JOB_OBJECT_NET_RATE_CONTROL_MAX_BANDWIDTH: JOB_OBJECT_NET_RATE_CONTROL_FLAGS = 2i32;
pub const JOB_OBJECT_NET_RATE_CONTROL_VALID_FLAGS: JOB_OBJECT_NET_RATE_CONTROL_FLAGS = 7i32;
pub const JOB_OBJECT_NOTIFICATION_LIMIT_VALID_FLAGS: JOB_OBJECT_LIMIT = 2064900u32;
pub const JOB_OBJECT_POST_AT_END_OF_JOB: JOB_OBJECT_TERMINATE_AT_END_ACTION = 1u32;
pub const JOB_OBJECT_SECURITY_FILTER_TOKENS: JOB_OBJECT_SECURITY = 8u32;
pub const JOB_OBJECT_SECURITY_NO_ADMIN: JOB_OBJECT_SECURITY = 1u32;
pub const JOB_OBJECT_SECURITY_ONLY_TOKEN: JOB_OBJECT_SECURITY = 4u32;
pub const JOB_OBJECT_SECURITY_RESTRICTED_TOKEN: JOB_OBJECT_SECURITY = 2u32;
pub const JOB_OBJECT_SECURITY_VALID_FLAGS: JOB_OBJECT_SECURITY = 15u32;
pub const JOB_OBJECT_TERMINATE_AT_END_OF_JOB: JOB_OBJECT_TERMINATE_AT_END_ACTION = 0u32;
pub const JOB_OBJECT_UILIMIT_DESKTOP: JOB_OBJECT_UILIMIT = 64u32;
pub const JOB_OBJECT_UILIMIT_DISPLAYSETTINGS: JOB_OBJECT_UILIMIT = 16u32;
pub const JOB_OBJECT_UILIMIT_EXITWINDOWS: JOB_OBJECT_UILIMIT = 128u32;
pub const JOB_OBJECT_UILIMIT_GLOBALATOMS: JOB_OBJECT_UILIMIT = 32u32;
pub const JOB_OBJECT_UILIMIT_HANDLES: JOB_OBJECT_UILIMIT = 1u32;
pub const JOB_OBJECT_UILIMIT_NONE: JOB_OBJECT_UILIMIT = 0u32;
pub const JOB_OBJECT_UILIMIT_READCLIPBOARD: JOB_OBJECT_UILIMIT = 2u32;
pub const JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS: JOB_OBJECT_UILIMIT = 8u32;
pub const JOB_OBJECT_UILIMIT_WRITECLIPBOARD: JOB_OBJECT_UILIMIT = 4u32;
pub const JobObjectAssociateCompletionPortInformation: JOBOBJECTINFOCLASS = 7i32;
pub const JobObjectBasicAccountingInformation: JOBOBJECTINFOCLASS = 1i32;
pub const JobObjectBasicAndIoAccountingInformation: JOBOBJECTINFOCLASS = 8i32;
pub const JobObjectBasicLimitInformation: JOBOBJECTINFOCLASS = 2i32;
pub const JobObjectBasicProcessIdList: JOBOBJECTINFOCLASS = 3i32;
pub const JobObjectBasicUIRestrictions: JOBOBJECTINFOCLASS = 4i32;
pub const JobObjectCompletionCounter: JOBOBJECTINFOCLASS = 17i32;
pub const JobObjectCompletionFilter: JOBOBJECTINFOCLASS = 16i32;
pub const JobObjectCpuRateControlInformation: JOBOBJECTINFOCLASS = 15i32;
pub const JobObjectCreateSilo: JOBOBJECTINFOCLASS = 35i32;
pub const JobObjectEndOfJobTimeInformation: JOBOBJECTINFOCLASS = 6i32;
pub const JobObjectExtendedLimitInformation: JOBOBJECTINFOCLASS = 9i32;
pub const JobObjectGroupInformation: JOBOBJECTINFOCLASS = 11i32;
pub const JobObjectGroupInformationEx: JOBOBJECTINFOCLASS = 14i32;
pub const JobObjectJobSetInformation: JOBOBJECTINFOCLASS = 10i32;
pub const JobObjectLimitViolationInformation: JOBOBJECTINFOCLASS = 13i32;
pub const JobObjectLimitViolationInformation2: JOBOBJECTINFOCLASS = 34i32;
pub const JobObjectNetRateControlInformation: JOBOBJECTINFOCLASS = 32i32;
pub const JobObjectNotificationLimitInformation: JOBOBJECTINFOCLASS = 12i32;
pub const JobObjectNotificationLimitInformation2: JOBOBJECTINFOCLASS = 33i32;
pub const JobObjectReserved10Information: JOBOBJECTINFOCLASS = 27i32;
pub const JobObjectReserved11Information: JOBOBJECTINFOCLASS = 28i32;
pub const JobObjectReserved12Information: JOBOBJECTINFOCLASS = 29i32;
pub const JobObjectReserved13Information: JOBOBJECTINFOCLASS = 30i32;
pub const JobObjectReserved14Information: JOBOBJECTINFOCLASS = 31i32;
pub const JobObjectReserved15Information: JOBOBJECTINFOCLASS = 37i32;
pub const JobObjectReserved16Information: JOBOBJECTINFOCLASS = 38i32;
pub const JobObjectReserved17Information: JOBOBJECTINFOCLASS = 39i32;
pub const JobObjectReserved18Information: JOBOBJECTINFOCLASS = 40i32;
pub const JobObjectReserved19Information: JOBOBJECTINFOCLASS = 41i32;
pub const JobObjectReserved1Information: JOBOBJECTINFOCLASS = 18i32;
pub const JobObjectReserved20Information: JOBOBJECTINFOCLASS = 42i32;
pub const JobObjectReserved21Information: JOBOBJECTINFOCLASS = 43i32;
pub const JobObjectReserved22Information: JOBOBJECTINFOCLASS = 44i32;
pub const JobObjectReserved23Information: JOBOBJECTINFOCLASS = 45i32;
pub const JobObjectReserved24Information: JOBOBJECTINFOCLASS = 46i32;
pub const JobObjectReserved25Information: JOBOBJECTINFOCLASS = 47i32;
pub const JobObjectReserved26Information: JOBOBJECTINFOCLASS = 48i32;
pub const JobObjectReserved27Information: JOBOBJECTINFOCLASS = 49i32;
pub const JobObjectReserved2Information: JOBOBJECTINFOCLASS = 19i32;
pub const JobObjectReserved3Information: JOBOBJECTINFOCLASS = 20i32;
pub const JobObjectReserved4Information: JOBOBJECTINFOCLASS = 21i32;
pub const JobObjectReserved5Information: JOBOBJECTINFOCLASS = 22i32;
pub const JobObjectReserved6Information: JOBOBJECTINFOCLASS = 23i32;
pub const JobObjectReserved7Information: JOBOBJECTINFOCLASS = 24i32;
pub const JobObjectReserved8Information: JOBOBJECTINFOCLASS = 25i32;
pub const JobObjectReserved9Information: JOBOBJECTINFOCLASS = 26i32;
pub const JobObjectSecurityLimitInformation: JOBOBJECTINFOCLASS = 5i32;
pub const JobObjectSiloBasicInformation: JOBOBJECTINFOCLASS = 36i32;
pub const MaxJobObjectInfoClass: JOBOBJECTINFOCLASS = 50i32;
pub const ToleranceHigh: JOBOBJECT_RATE_CONTROL_TOLERANCE = 3i32;
pub const ToleranceIntervalLong: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL = 3i32;
pub const ToleranceIntervalMedium: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL = 2i32;
pub const ToleranceIntervalShort: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL = 1i32;
pub const ToleranceLow: JOBOBJECT_RATE_CONTROL_TOLERANCE = 1i32;
pub const ToleranceMedium: JOBOBJECT_RATE_CONTROL_TOLERANCE = 2i32;
pub type JOBOBJECTINFOCLASS = i32;
pub type JOBOBJECT_IO_ATTRIBUTION_CONTROL_FLAGS = i32;
pub type JOBOBJECT_RATE_CONTROL_TOLERANCE = i32;
pub type JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL = i32;
pub type JOB_OBJECT_CPU_RATE_CONTROL = u32;
pub type JOB_OBJECT_IO_RATE_CONTROL_FLAGS = i32;
pub type JOB_OBJECT_LIMIT = u32;
pub type JOB_OBJECT_NET_RATE_CONTROL_FLAGS = i32;
pub type JOB_OBJECT_SECURITY = u32;
pub type JOB_OBJECT_TERMINATE_AT_END_ACTION = u32;
pub type JOB_OBJECT_UILIMIT = u32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
    pub CompletionKey: *mut ::core::ffi::c_void,
    pub CompletionPort: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for JOBOBJECT_ASSOCIATE_COMPLETION_PORT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_BASIC_ACCOUNTING_INFORMATION {
    pub TotalUserTime: i64,
    pub TotalKernelTime: i64,
    pub ThisPeriodTotalUserTime: i64,
    pub ThisPeriodTotalKernelTime: i64,
    pub TotalPageFaultCount: u32,
    pub TotalProcesses: u32,
    pub ActiveProcesses: u32,
    pub TotalTerminatedProcesses: u32,
}
impl ::core::marker::Copy for JOBOBJECT_BASIC_ACCOUNTING_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_BASIC_ACCOUNTING_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Threading\"`"]
#[cfg(feature = "Win32_System_Threading")]
pub struct JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION {
    pub BasicInfo: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    pub IoInfo: super::Threading::IO_COUNTERS,
}
#[cfg(feature = "Win32_System_Threading")]
impl ::core::marker::Copy for JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION {}
#[cfg(feature = "Win32_System_Threading")]
impl ::core::clone::Clone for JOBOBJECT_BASIC_AND_IO_ACCOUNTING_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
    pub PerProcessUserTimeLimit: i64,
    pub PerJobUserTimeLimit: i64,
    pub LimitFlags: JOB_OBJECT_LIMIT,
    pub MinimumWorkingSetSize: usize,
    pub MaximumWorkingSetSize: usize,
    pub ActiveProcessLimit: u32,
    pub Affinity: usize,
    pub PriorityClass: u32,
    pub SchedulingClass: u32,
}
impl ::core::marker::Copy for JOBOBJECT_BASIC_LIMIT_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_BASIC_LIMIT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_BASIC_PROCESS_ID_LIST {
    pub NumberOfAssignedProcesses: u32,
    pub NumberOfProcessIdsInList: u32,
    pub ProcessIdList: [usize; 1],
}
impl ::core::marker::Copy for JOBOBJECT_BASIC_PROCESS_ID_LIST {}
impl ::core::clone::Clone for JOBOBJECT_BASIC_PROCESS_ID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_BASIC_UI_RESTRICTIONS {
    pub UIRestrictionsClass: JOB_OBJECT_UILIMIT,
}
impl ::core::marker::Copy for JOBOBJECT_BASIC_UI_RESTRICTIONS {}
impl ::core::clone::Clone for JOBOBJECT_BASIC_UI_RESTRICTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
    pub ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL,
    pub Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0,
}
impl ::core::marker::Copy for JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 {
    pub CpuRate: u32,
    pub Weight: u32,
    pub Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0_0,
}
impl ::core::marker::Copy for JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 {}
impl ::core::clone::Clone for JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0_0 {
    pub MinRate: u16,
    pub MaxRate: u16,
}
impl ::core::marker::Copy for JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0_0 {}
impl ::core::clone::Clone for JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_END_OF_JOB_TIME_INFORMATION {
    pub EndOfJobTimeAction: JOB_OBJECT_TERMINATE_AT_END_ACTION,
}
impl ::core::marker::Copy for JOBOBJECT_END_OF_JOB_TIME_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_END_OF_JOB_TIME_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Threading\"`"]
#[cfg(feature = "Win32_System_Threading")]
pub struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    pub BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    pub IoInfo: super::Threading::IO_COUNTERS,
    pub ProcessMemoryLimit: usize,
    pub JobMemoryLimit: usize,
    pub PeakProcessMemoryUsed: usize,
    pub PeakJobMemoryUsed: usize,
}
#[cfg(feature = "Win32_System_Threading")]
impl ::core::marker::Copy for JOBOBJECT_EXTENDED_LIMIT_INFORMATION {}
#[cfg(feature = "Win32_System_Threading")]
impl ::core::clone::Clone for JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_IO_ATTRIBUTION_INFORMATION {
    pub ControlFlags: u32,
    pub ReadStats: JOBOBJECT_IO_ATTRIBUTION_STATS,
    pub WriteStats: JOBOBJECT_IO_ATTRIBUTION_STATS,
}
impl ::core::marker::Copy for JOBOBJECT_IO_ATTRIBUTION_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_IO_ATTRIBUTION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_IO_ATTRIBUTION_STATS {
    pub IoCount: usize,
    pub TotalNonOverlappedQueueTime: u64,
    pub TotalNonOverlappedServiceTime: u64,
    pub TotalSize: u64,
}
impl ::core::marker::Copy for JOBOBJECT_IO_ATTRIBUTION_STATS {}
impl ::core::clone::Clone for JOBOBJECT_IO_ATTRIBUTION_STATS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_IO_RATE_CONTROL_INFORMATION {
    pub MaxIops: i64,
    pub MaxBandwidth: i64,
    pub ReservationIops: i64,
    pub VolumeName: ::windows_sys::core::PCWSTR,
    pub BaseIoSize: u32,
    pub ControlFlags: u32,
}
impl ::core::marker::Copy for JOBOBJECT_IO_RATE_CONTROL_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_IO_RATE_CONTROL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V1 {
    pub MaxIops: i64,
    pub MaxBandwidth: i64,
    pub ReservationIops: i64,
    pub VolumeName: ::windows_sys::core::PWSTR,
    pub BaseIoSize: u32,
    pub ControlFlags: JOB_OBJECT_IO_RATE_CONTROL_FLAGS,
    pub VolumeNameLength: u16,
}
impl ::core::marker::Copy for JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V1 {}
impl ::core::clone::Clone for JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V2 {
    pub MaxIops: i64,
    pub MaxBandwidth: i64,
    pub ReservationIops: i64,
    pub VolumeName: ::windows_sys::core::PWSTR,
    pub BaseIoSize: u32,
    pub ControlFlags: JOB_OBJECT_IO_RATE_CONTROL_FLAGS,
    pub VolumeNameLength: u16,
    pub CriticalReservationIops: i64,
    pub ReservationBandwidth: i64,
    pub CriticalReservationBandwidth: i64,
    pub MaxTimePercent: i64,
    pub ReservationTimePercent: i64,
    pub CriticalReservationTimePercent: i64,
}
impl ::core::marker::Copy for JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V2 {}
impl ::core::clone::Clone for JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V3 {
    pub MaxIops: i64,
    pub MaxBandwidth: i64,
    pub ReservationIops: i64,
    pub VolumeName: ::windows_sys::core::PWSTR,
    pub BaseIoSize: u32,
    pub ControlFlags: JOB_OBJECT_IO_RATE_CONTROL_FLAGS,
    pub VolumeNameLength: u16,
    pub CriticalReservationIops: i64,
    pub ReservationBandwidth: i64,
    pub CriticalReservationBandwidth: i64,
    pub MaxTimePercent: i64,
    pub ReservationTimePercent: i64,
    pub CriticalReservationTimePercent: i64,
    pub SoftMaxIops: i64,
    pub SoftMaxBandwidth: i64,
    pub SoftMaxTimePercent: i64,
    pub LimitExcessNotifyIops: i64,
    pub LimitExcessNotifyBandwidth: i64,
    pub LimitExcessNotifyTimePercent: i64,
}
impl ::core::marker::Copy for JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V3 {}
impl ::core::clone::Clone for JOBOBJECT_IO_RATE_CONTROL_INFORMATION_NATIVE_V3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_JOBSET_INFORMATION {
    pub MemberLevel: u32,
}
impl ::core::marker::Copy for JOBOBJECT_JOBSET_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_JOBSET_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_LIMIT_VIOLATION_INFORMATION {
    pub LimitFlags: JOB_OBJECT_LIMIT,
    pub ViolationLimitFlags: JOB_OBJECT_LIMIT,
    pub IoReadBytes: u64,
    pub IoReadBytesLimit: u64,
    pub IoWriteBytes: u64,
    pub IoWriteBytesLimit: u64,
    pub PerJobUserTime: i64,
    pub PerJobUserTimeLimit: i64,
    pub JobMemory: u64,
    pub JobMemoryLimit: u64,
    pub RateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub RateControlToleranceLimit: JOBOBJECT_RATE_CONTROL_TOLERANCE,
}
impl ::core::marker::Copy for JOBOBJECT_LIMIT_VIOLATION_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_LIMIT_VIOLATION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2 {
    pub LimitFlags: JOB_OBJECT_LIMIT,
    pub ViolationLimitFlags: JOB_OBJECT_LIMIT,
    pub IoReadBytes: u64,
    pub IoReadBytesLimit: u64,
    pub IoWriteBytes: u64,
    pub IoWriteBytesLimit: u64,
    pub PerJobUserTime: i64,
    pub PerJobUserTimeLimit: i64,
    pub JobMemory: u64,
    pub Anonymous1: JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_0,
    pub Anonymous2: JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_1,
    pub Anonymous3: JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_2,
    pub JobLowMemoryLimit: u64,
    pub IoRateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub IoRateControlToleranceLimit: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub NetRateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub NetRateControlToleranceLimit: JOBOBJECT_RATE_CONTROL_TOLERANCE,
}
impl ::core::marker::Copy for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2 {}
impl ::core::clone::Clone for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_0 {
    pub JobHighMemoryLimit: u64,
    pub JobMemoryLimit: u64,
}
impl ::core::marker::Copy for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_0 {}
impl ::core::clone::Clone for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_1 {
    pub RateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub CpuRateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
}
impl ::core::marker::Copy for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_1 {}
impl ::core::clone::Clone for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_2 {
    pub RateControlToleranceLimit: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub CpuRateControlToleranceLimit: JOBOBJECT_RATE_CONTROL_TOLERANCE,
}
impl ::core::marker::Copy for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_2 {}
impl ::core::clone::Clone for JOBOBJECT_LIMIT_VIOLATION_INFORMATION_2_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_NET_RATE_CONTROL_INFORMATION {
    pub MaxBandwidth: u64,
    pub ControlFlags: JOB_OBJECT_NET_RATE_CONTROL_FLAGS,
    pub DscpTag: u8,
}
impl ::core::marker::Copy for JOBOBJECT_NET_RATE_CONTROL_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_NET_RATE_CONTROL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION {
    pub IoReadBytesLimit: u64,
    pub IoWriteBytesLimit: u64,
    pub PerJobUserTimeLimit: i64,
    pub JobMemoryLimit: u64,
    pub RateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub RateControlToleranceInterval: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL,
    pub LimitFlags: JOB_OBJECT_LIMIT,
}
impl ::core::marker::Copy for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION {}
impl ::core::clone::Clone for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2 {
    pub IoReadBytesLimit: u64,
    pub IoWriteBytesLimit: u64,
    pub PerJobUserTimeLimit: i64,
    pub Anonymous1: JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_0,
    pub Anonymous2: JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_1,
    pub Anonymous3: JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_2,
    pub LimitFlags: JOB_OBJECT_LIMIT,
    pub IoRateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub JobLowMemoryLimit: u64,
    pub IoRateControlToleranceInterval: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL,
    pub NetRateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub NetRateControlToleranceInterval: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL,
}
impl ::core::marker::Copy for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2 {}
impl ::core::clone::Clone for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_0 {
    pub JobHighMemoryLimit: u64,
    pub JobMemoryLimit: u64,
}
impl ::core::marker::Copy for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_0 {}
impl ::core::clone::Clone for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_1 {
    pub RateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
    pub CpuRateControlTolerance: JOBOBJECT_RATE_CONTROL_TOLERANCE,
}
impl ::core::marker::Copy for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_1 {}
impl ::core::clone::Clone for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_2 {
    pub RateControlToleranceInterval: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL,
    pub CpuRateControlToleranceInterval: JOBOBJECT_RATE_CONTROL_TOLERANCE_INTERVAL,
}
impl ::core::marker::Copy for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_2 {}
impl ::core::clone::Clone for JOBOBJECT_NOTIFICATION_LIMIT_INFORMATION_2_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct JOBOBJECT_SECURITY_LIMIT_INFORMATION {
    pub SecurityLimitFlags: JOB_OBJECT_SECURITY,
    pub JobToken: super::super::Foundation::HANDLE,
    pub SidsToDisable: *mut super::super::Security::TOKEN_GROUPS,
    pub PrivilegesToDelete: *mut super::super::Security::TOKEN_PRIVILEGES,
    pub RestrictedSids: *mut super::super::Security::TOKEN_GROUPS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for JOBOBJECT_SECURITY_LIMIT_INFORMATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for JOBOBJECT_SECURITY_LIMIT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct JOB_SET_ARRAY {
    pub JobHandle: super::super::Foundation::HANDLE,
    pub MemberLevel: u32,
    pub Flags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for JOB_SET_ARRAY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for JOB_SET_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
