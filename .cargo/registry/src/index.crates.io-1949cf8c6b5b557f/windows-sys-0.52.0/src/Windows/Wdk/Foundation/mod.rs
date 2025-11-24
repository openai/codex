#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtClose(handle : super::super::Win32::Foundation:: HANDLE) -> super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtQueryObject(handle : super::super::Win32::Foundation:: HANDLE, objectinformationclass : OBJECT_INFORMATION_CLASS, objectinformation : *mut ::core::ffi::c_void, objectinformationlength : u32, returnlength : *mut u32) -> super::super::Win32::Foundation:: NTSTATUS);
pub const DontUseThisType: POOL_TYPE = 3i32;
pub const DontUseThisTypeSession: POOL_TYPE = 35i32;
pub const IoPriorityCritical: IO_PRIORITY_HINT = 4i32;
pub const IoPriorityHigh: IO_PRIORITY_HINT = 3i32;
pub const IoPriorityLow: IO_PRIORITY_HINT = 1i32;
pub const IoPriorityNormal: IO_PRIORITY_HINT = 2i32;
pub const IoPriorityVeryLow: IO_PRIORITY_HINT = 0i32;
pub const LockQueueAfdWorkQueueLock: KSPIN_LOCK_QUEUE_NUMBER = 13i32;
pub const LockQueueBcbLock: KSPIN_LOCK_QUEUE_NUMBER = 14i32;
pub const LockQueueIoCancelLock: KSPIN_LOCK_QUEUE_NUMBER = 7i32;
pub const LockQueueIoCompletionLock: KSPIN_LOCK_QUEUE_NUMBER = 11i32;
pub const LockQueueIoDatabaseLock: KSPIN_LOCK_QUEUE_NUMBER = 10i32;
pub const LockQueueIoVpbLock: KSPIN_LOCK_QUEUE_NUMBER = 9i32;
pub const LockQueueMasterLock: KSPIN_LOCK_QUEUE_NUMBER = 5i32;
pub const LockQueueMaximumLock: KSPIN_LOCK_QUEUE_NUMBER = 17i32;
pub const LockQueueNonPagedPoolLock: KSPIN_LOCK_QUEUE_NUMBER = 6i32;
pub const LockQueueNtfsStructLock: KSPIN_LOCK_QUEUE_NUMBER = 12i32;
pub const LockQueueUnusedSpare0: KSPIN_LOCK_QUEUE_NUMBER = 0i32;
pub const LockQueueUnusedSpare1: KSPIN_LOCK_QUEUE_NUMBER = 1i32;
pub const LockQueueUnusedSpare15: KSPIN_LOCK_QUEUE_NUMBER = 15i32;
pub const LockQueueUnusedSpare16: KSPIN_LOCK_QUEUE_NUMBER = 16i32;
pub const LockQueueUnusedSpare2: KSPIN_LOCK_QUEUE_NUMBER = 2i32;
pub const LockQueueUnusedSpare3: KSPIN_LOCK_QUEUE_NUMBER = 3i32;
pub const LockQueueUnusedSpare8: KSPIN_LOCK_QUEUE_NUMBER = 8i32;
pub const LockQueueVacbLock: KSPIN_LOCK_QUEUE_NUMBER = 4i32;
pub const MaxIoPriorityTypes: IO_PRIORITY_HINT = 5i32;
pub const MaxPoolType: POOL_TYPE = 7i32;
pub const NonPagedPool: POOL_TYPE = 0i32;
pub const NonPagedPoolBase: POOL_TYPE = 0i32;
pub const NonPagedPoolBaseCacheAligned: POOL_TYPE = 4i32;
pub const NonPagedPoolBaseCacheAlignedMustS: POOL_TYPE = 6i32;
pub const NonPagedPoolBaseMustSucceed: POOL_TYPE = 2i32;
pub const NonPagedPoolCacheAligned: POOL_TYPE = 4i32;
pub const NonPagedPoolCacheAlignedMustS: POOL_TYPE = 6i32;
pub const NonPagedPoolCacheAlignedMustSSession: POOL_TYPE = 38i32;
pub const NonPagedPoolCacheAlignedSession: POOL_TYPE = 36i32;
pub const NonPagedPoolExecute: POOL_TYPE = 0i32;
pub const NonPagedPoolMustSucceed: POOL_TYPE = 2i32;
pub const NonPagedPoolMustSucceedSession: POOL_TYPE = 34i32;
pub const NonPagedPoolNx: POOL_TYPE = 512i32;
pub const NonPagedPoolNxCacheAligned: POOL_TYPE = 516i32;
pub const NonPagedPoolSession: POOL_TYPE = 32i32;
pub const NonPagedPoolSessionNx: POOL_TYPE = 544i32;
pub const ObjectBasicInformation: OBJECT_INFORMATION_CLASS = 0i32;
pub const ObjectTypeInformation: OBJECT_INFORMATION_CLASS = 2i32;
pub const PagedPool: POOL_TYPE = 1i32;
pub const PagedPoolCacheAligned: POOL_TYPE = 5i32;
pub const PagedPoolCacheAlignedSession: POOL_TYPE = 37i32;
pub const PagedPoolSession: POOL_TYPE = 33i32;
pub type IO_PRIORITY_HINT = i32;
pub type KSPIN_LOCK_QUEUE_NUMBER = i32;
pub type OBJECT_INFORMATION_CLASS = i32;
pub type POOL_TYPE = i32;
#[repr(C)]
#[doc = "Required features: `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct ACCESS_STATE {
    pub OperationID: super::super::Win32::Foundation::LUID,
    pub SecurityEvaluated: super::super::Win32::Foundation::BOOLEAN,
    pub GenerateAudit: super::super::Win32::Foundation::BOOLEAN,
    pub GenerateOnClose: super::super::Win32::Foundation::BOOLEAN,
    pub PrivilegesAllocated: super::super::Win32::Foundation::BOOLEAN,
    pub Flags: u32,
    pub RemainingDesiredAccess: u32,
    pub PreviouslyGrantedAccess: u32,
    pub OriginalDesiredAccess: u32,
    pub SubjectSecurityContext: SECURITY_SUBJECT_CONTEXT,
    pub SecurityDescriptor: super::super::Win32::Security::PSECURITY_DESCRIPTOR,
    pub AuxData: *mut ::core::ffi::c_void,
    pub Privileges: ACCESS_STATE_0,
    pub AuditPrivileges: super::super::Win32::Foundation::BOOLEAN,
    pub ObjectName: super::super::Win32::Foundation::UNICODE_STRING,
    pub ObjectTypeName: super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for ACCESS_STATE {}
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for ACCESS_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
pub union ACCESS_STATE_0 {
    pub InitialPrivilegeSet: super::System::SystemServices::INITIAL_PRIVILEGE_SET,
    pub PrivilegeSet: super::super::Win32::Security::PRIVILEGE_SET,
}
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for ACCESS_STATE_0 {}
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for ACCESS_STATE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct DEVICE_OBJECT {
    pub Type: i16,
    pub Size: u16,
    pub ReferenceCount: i32,
    pub DriverObject: *mut DRIVER_OBJECT,
    pub NextDevice: *mut DEVICE_OBJECT,
    pub AttachedDevice: *mut DEVICE_OBJECT,
    pub CurrentIrp: *mut IRP,
    pub Timer: PIO_TIMER,
    pub Flags: u32,
    pub Characteristics: u32,
    pub Vpb: *mut VPB,
    pub DeviceExtension: *mut ::core::ffi::c_void,
    pub DeviceType: u32,
    pub StackSize: i8,
    pub Queue: DEVICE_OBJECT_0,
    pub AlignmentRequirement: u32,
    pub DeviceQueue: KDEVICE_QUEUE,
    pub Dpc: KDPC,
    pub ActiveThreadCount: u32,
    pub SecurityDescriptor: super::super::Win32::Security::PSECURITY_DESCRIPTOR,
    pub DeviceLock: KEVENT,
    pub SectorSize: u16,
    pub Spare1: u16,
    pub DeviceObjectExtension: *mut DEVOBJ_EXTENSION,
    pub Reserved: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for DEVICE_OBJECT {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for DEVICE_OBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union DEVICE_OBJECT_0 {
    pub ListEntry: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub Wcb: super::System::SystemServices::WAIT_CONTEXT_BLOCK,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for DEVICE_OBJECT_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for DEVICE_OBJECT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct DEVOBJ_EXTENSION {
    pub Type: i16,
    pub Size: u16,
    pub DeviceObject: *mut DEVICE_OBJECT,
    pub PowerFlags: u32,
    pub Dope: *mut _DEVICE_OBJECT_POWER_EXTENSION,
    pub ExtensionFlags: u32,
    pub DeviceNode: *mut ::core::ffi::c_void,
    pub AttachedTo: *mut DEVICE_OBJECT,
    pub StartIoCount: i32,
    pub StartIoKey: i32,
    pub StartIoFlags: u32,
    pub Vpb: *mut VPB,
    pub DependencyNode: *mut ::core::ffi::c_void,
    pub InterruptContext: *mut ::core::ffi::c_void,
    pub InterruptCount: i32,
    pub VerifierContext: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for DEVOBJ_EXTENSION {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for DEVOBJ_EXTENSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER {
    pub Anonymous: DISPATCHER_HEADER_0,
    pub SignalState: i32,
    pub WaitListHead: super::super::Win32::System::Kernel::LIST_ENTRY,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0 {
    pub Anonymous1: DISPATCHER_HEADER_0_0,
    pub Anonymous2: DISPATCHER_HEADER_0_1,
    pub Anonymous3: DISPATCHER_HEADER_0_2,
    pub Anonymous4: DISPATCHER_HEADER_0_3,
    pub Anonymous5: DISPATCHER_HEADER_0_4,
    pub Anonymous6: DISPATCHER_HEADER_0_5,
    pub Anonymous7: DISPATCHER_HEADER_0_6,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0_0 {
    pub Lock: i32,
    pub LockNV: i32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_1 {
    pub Type: u8,
    pub Signalling: u8,
    pub Size: u8,
    pub Reserved1: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_2 {
    pub TimerType: u8,
    pub Anonymous1: DISPATCHER_HEADER_0_2_0,
    pub Hand: u8,
    pub Anonymous2: DISPATCHER_HEADER_0_2_1,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_2 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0_2_0 {
    pub TimerControlFlags: u8,
    pub Anonymous: DISPATCHER_HEADER_0_2_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_2_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_2_0_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_2_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_2_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0_2_1 {
    pub TimerMiscFlags: u8,
    pub Anonymous: DISPATCHER_HEADER_0_2_1_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_2_1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_2_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_2_1_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_2_1_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_2_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_3 {
    pub Timer2Type: u8,
    pub Anonymous: DISPATCHER_HEADER_0_3_0,
    pub Timer2ComponentId: u8,
    pub Timer2RelativeId: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_3 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0_3_0 {
    pub Timer2Flags: u8,
    pub Anonymous: DISPATCHER_HEADER_0_3_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_3_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_3_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_3_0_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_3_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_3_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_4 {
    pub QueueType: u8,
    pub Anonymous: DISPATCHER_HEADER_0_4_0,
    pub QueueSize: u8,
    pub QueueReserved: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_4 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0_4_0 {
    pub QueueControlFlags: u8,
    pub Anonymous: DISPATCHER_HEADER_0_4_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_4_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_4_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_4_0_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_4_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_4_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_5 {
    pub ThreadType: u8,
    pub ThreadReserved: u8,
    pub Anonymous1: DISPATCHER_HEADER_0_5_0,
    pub Anonymous2: DISPATCHER_HEADER_0_5_1,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_5 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0_5_0 {
    pub ThreadControlFlags: u8,
    pub Anonymous: DISPATCHER_HEADER_0_5_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_5_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_5_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_5_0_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_5_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_5_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union DISPATCHER_HEADER_0_5_1 {
    pub DebugActive: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_5_1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_5_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_HEADER_0_6 {
    pub MutantType: u8,
    pub MutantSize: u8,
    pub DpcActive: super::super::Win32::Foundation::BOOLEAN,
    pub MutantReserved: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_HEADER_0_6 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_HEADER_0_6 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type DMA_COMMON_BUFFER_VECTOR = isize;
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct DRIVER_EXTENSION {
    pub DriverObject: *mut DRIVER_OBJECT,
    pub AddDevice: PDRIVER_ADD_DEVICE,
    pub Count: u32,
    pub ServiceKeyName: super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for DRIVER_EXTENSION {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for DRIVER_EXTENSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct DRIVER_OBJECT {
    pub Type: i16,
    pub Size: i16,
    pub DeviceObject: *mut DEVICE_OBJECT,
    pub Flags: u32,
    pub DriverStart: *mut ::core::ffi::c_void,
    pub DriverSize: u32,
    pub DriverSection: *mut ::core::ffi::c_void,
    pub DriverExtension: *mut DRIVER_EXTENSION,
    pub DriverName: super::super::Win32::Foundation::UNICODE_STRING,
    pub HardwareDatabase: *mut super::super::Win32::Foundation::UNICODE_STRING,
    pub FastIoDispatch: *mut FAST_IO_DISPATCH,
    pub DriverInit: PDRIVER_INITIALIZE,
    pub DriverStartIo: PDRIVER_STARTIO,
    pub DriverUnload: PDRIVER_UNLOAD,
    pub MajorFunction: [PDRIVER_DISPATCH; 28],
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for DRIVER_OBJECT {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for DRIVER_OBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type ECP_HEADER = isize;
pub type ECP_LIST = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct ERESOURCE {
    pub SystemResourcesList: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub OwnerTable: *mut OWNER_ENTRY,
    pub ActiveCount: i16,
    pub Anonymous1: ERESOURCE_0,
    pub SharedWaiters: *mut ::core::ffi::c_void,
    pub ExclusiveWaiters: *mut ::core::ffi::c_void,
    pub OwnerEntry: OWNER_ENTRY,
    pub ActiveEntries: u32,
    pub ContentionCount: u32,
    pub NumberOfSharedWaiters: u32,
    pub NumberOfExclusiveWaiters: u32,
    pub Anonymous2: ERESOURCE_1,
    pub SpinLock: usize,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for ERESOURCE {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for ERESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub union ERESOURCE_0 {
    pub Flag: u16,
    pub Anonymous: ERESOURCE_0_0,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for ERESOURCE_0 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for ERESOURCE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct ERESOURCE_0_0 {
    pub ReservedLowFlags: u8,
    pub WaiterPriority: u8,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for ERESOURCE_0_0 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for ERESOURCE_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub union ERESOURCE_1 {
    pub Address: *mut ::core::ffi::c_void,
    pub CreatorBackTraceIndex: usize,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for ERESOURCE_1 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for ERESOURCE_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FAST_IO_DISPATCH {
    pub SizeOfFastIoDispatch: u32,
    pub FastIoCheckIfPossible: PFAST_IO_CHECK_IF_POSSIBLE,
    pub FastIoRead: PFAST_IO_READ,
    pub FastIoWrite: PFAST_IO_WRITE,
    pub FastIoQueryBasicInfo: PFAST_IO_QUERY_BASIC_INFO,
    pub FastIoQueryStandardInfo: PFAST_IO_QUERY_STANDARD_INFO,
    pub FastIoLock: PFAST_IO_LOCK,
    pub FastIoUnlockSingle: PFAST_IO_UNLOCK_SINGLE,
    pub FastIoUnlockAll: PFAST_IO_UNLOCK_ALL,
    pub FastIoUnlockAllByKey: PFAST_IO_UNLOCK_ALL_BY_KEY,
    pub FastIoDeviceControl: PFAST_IO_DEVICE_CONTROL,
    pub AcquireFileForNtCreateSection: PFAST_IO_ACQUIRE_FILE,
    pub ReleaseFileForNtCreateSection: PFAST_IO_RELEASE_FILE,
    pub FastIoDetachDevice: PFAST_IO_DETACH_DEVICE,
    pub FastIoQueryNetworkOpenInfo: PFAST_IO_QUERY_NETWORK_OPEN_INFO,
    pub AcquireForModWrite: PFAST_IO_ACQUIRE_FOR_MOD_WRITE,
    pub MdlRead: PFAST_IO_MDL_READ,
    pub MdlReadComplete: PFAST_IO_MDL_READ_COMPLETE,
    pub PrepareMdlWrite: PFAST_IO_PREPARE_MDL_WRITE,
    pub MdlWriteComplete: PFAST_IO_MDL_WRITE_COMPLETE,
    pub FastIoReadCompressed: PFAST_IO_READ_COMPRESSED,
    pub FastIoWriteCompressed: PFAST_IO_WRITE_COMPRESSED,
    pub MdlReadCompleteCompressed: PFAST_IO_MDL_READ_COMPLETE_COMPRESSED,
    pub MdlWriteCompleteCompressed: PFAST_IO_MDL_WRITE_COMPLETE_COMPRESSED,
    pub FastIoQueryOpen: PFAST_IO_QUERY_OPEN,
    pub ReleaseForModWrite: PFAST_IO_RELEASE_FOR_MOD_WRITE,
    pub AcquireForCcFlush: PFAST_IO_ACQUIRE_FOR_CCFLUSH,
    pub ReleaseForCcFlush: PFAST_IO_RELEASE_FOR_CCFLUSH,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FAST_IO_DISPATCH {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FAST_IO_DISPATCH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct FAST_MUTEX {
    pub Count: i32,
    pub Owner: *mut ::core::ffi::c_void,
    pub Contention: u32,
    pub Event: KEVENT,
    pub OldIrql: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for FAST_MUTEX {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for FAST_MUTEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FILE_OBJECT {
    pub Type: i16,
    pub Size: i16,
    pub DeviceObject: *mut DEVICE_OBJECT,
    pub Vpb: *mut VPB,
    pub FsContext: *mut ::core::ffi::c_void,
    pub FsContext2: *mut ::core::ffi::c_void,
    pub SectionObjectPointer: *mut SECTION_OBJECT_POINTERS,
    pub PrivateCacheMap: *mut ::core::ffi::c_void,
    pub FinalStatus: super::super::Win32::Foundation::NTSTATUS,
    pub RelatedFileObject: *mut FILE_OBJECT,
    pub LockOperation: super::super::Win32::Foundation::BOOLEAN,
    pub DeletePending: super::super::Win32::Foundation::BOOLEAN,
    pub ReadAccess: super::super::Win32::Foundation::BOOLEAN,
    pub WriteAccess: super::super::Win32::Foundation::BOOLEAN,
    pub DeleteAccess: super::super::Win32::Foundation::BOOLEAN,
    pub SharedRead: super::super::Win32::Foundation::BOOLEAN,
    pub SharedWrite: super::super::Win32::Foundation::BOOLEAN,
    pub SharedDelete: super::super::Win32::Foundation::BOOLEAN,
    pub Flags: u32,
    pub FileName: super::super::Win32::Foundation::UNICODE_STRING,
    pub CurrentByteOffset: i64,
    pub Waiters: u32,
    pub Busy: u32,
    pub LastLock: *mut ::core::ffi::c_void,
    pub Lock: KEVENT,
    pub Event: KEVENT,
    pub CompletionContext: *mut IO_COMPLETION_CONTEXT,
    pub IrpListLock: usize,
    pub IrpList: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub FileObjectExtension: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FILE_OBJECT {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FILE_OBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type IOMMU_DMA_DEVICE = isize;
pub type IOMMU_DMA_DOMAIN = isize;
#[repr(C)]
pub struct IO_COMPLETION_CONTEXT {
    pub Port: *mut ::core::ffi::c_void,
    pub Key: *mut ::core::ffi::c_void,
    pub UsageCount: isize,
}
impl ::core::marker::Copy for IO_COMPLETION_CONTEXT {}
impl ::core::clone::Clone for IO_COMPLETION_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct IO_SECURITY_CONTEXT {
    pub SecurityQos: *mut super::super::Win32::Security::SECURITY_QUALITY_OF_SERVICE,
    pub AccessState: *mut ACCESS_STATE,
    pub DesiredAccess: u32,
    pub FullCreateOptions: u32,
}
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for IO_SECURITY_CONTEXT {}
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for IO_SECURITY_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION {
    pub MajorFunction: u8,
    pub MinorFunction: u8,
    pub Flags: u8,
    pub Control: u8,
    pub Parameters: IO_STACK_LOCATION_0,
    pub DeviceObject: *mut DEVICE_OBJECT,
    pub FileObject: *mut FILE_OBJECT,
    pub CompletionRoutine: PIO_COMPLETION_ROUTINE,
    pub Context: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IO_STACK_LOCATION_0 {
    pub Create: IO_STACK_LOCATION_0_2,
    pub CreatePipe: IO_STACK_LOCATION_0_1,
    pub CreateMailslot: IO_STACK_LOCATION_0_0,
    pub Read: IO_STACK_LOCATION_0_25,
    pub Write: IO_STACK_LOCATION_0_38,
    pub QueryDirectory: IO_STACK_LOCATION_0_16,
    pub NotifyDirectory: IO_STACK_LOCATION_0_10,
    pub NotifyDirectoryEx: IO_STACK_LOCATION_0_9,
    pub QueryFile: IO_STACK_LOCATION_0_18,
    pub SetFile: IO_STACK_LOCATION_0_28,
    pub QueryEa: IO_STACK_LOCATION_0_17,
    pub SetEa: IO_STACK_LOCATION_0_27,
    pub QueryVolume: IO_STACK_LOCATION_0_23,
    pub SetVolume: IO_STACK_LOCATION_0_32,
    pub FileSystemControl: IO_STACK_LOCATION_0_5,
    pub LockControl: IO_STACK_LOCATION_0_7,
    pub DeviceIoControl: IO_STACK_LOCATION_0_4,
    pub QuerySecurity: IO_STACK_LOCATION_0_22,
    pub SetSecurity: IO_STACK_LOCATION_0_31,
    pub MountVolume: IO_STACK_LOCATION_0_8,
    pub VerifyVolume: IO_STACK_LOCATION_0_35,
    pub Scsi: IO_STACK_LOCATION_0_26,
    pub QueryQuota: IO_STACK_LOCATION_0_21,
    pub SetQuota: IO_STACK_LOCATION_0_30,
    pub QueryDeviceRelations: IO_STACK_LOCATION_0_14,
    pub QueryInterface: IO_STACK_LOCATION_0_20,
    pub DeviceCapabilities: IO_STACK_LOCATION_0_3,
    pub FilterResourceRequirements: IO_STACK_LOCATION_0_6,
    pub ReadWriteConfig: IO_STACK_LOCATION_0_24,
    pub SetLock: IO_STACK_LOCATION_0_29,
    pub QueryId: IO_STACK_LOCATION_0_19,
    pub QueryDeviceText: IO_STACK_LOCATION_0_15,
    pub UsageNotification: IO_STACK_LOCATION_0_34,
    pub WaitWake: IO_STACK_LOCATION_0_37,
    pub PowerSequence: IO_STACK_LOCATION_0_12,
    pub Power: IO_STACK_LOCATION_0_13,
    pub StartDevice: IO_STACK_LOCATION_0_33,
    pub WMI: IO_STACK_LOCATION_0_36,
    pub Others: IO_STACK_LOCATION_0_11,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_0 {
    pub SecurityContext: *mut IO_SECURITY_CONTEXT,
    pub Options: u32,
    pub Reserved: u16,
    pub ShareAccess: u16,
    pub Parameters: *mut super::System::SystemServices::MAILSLOT_CREATE_PARAMETERS,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_1 {
    pub SecurityContext: *mut IO_SECURITY_CONTEXT,
    pub Options: u32,
    pub Reserved: u16,
    pub ShareAccess: u16,
    pub Parameters: *mut super::System::SystemServices::NAMED_PIPE_CREATE_PARAMETERS,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_1 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_2 {
    pub SecurityContext: *mut IO_SECURITY_CONTEXT,
    pub Options: u32,
    pub FileAttributes: u16,
    pub ShareAccess: u16,
    pub EaLength: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_2 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_3 {
    pub Capabilities: *mut super::System::SystemServices::DEVICE_CAPABILITIES,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_3 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_4 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub IoControlCode: u32,
    pub Type3InputBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_4 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_5 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub FsControlCode: u32,
    pub Type3InputBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_5 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_6 {
    pub IoResourceRequirementList: *mut super::System::SystemServices::IO_RESOURCE_REQUIREMENTS_LIST,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_6 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_7 {
    pub Length: *mut i64,
    pub Key: u32,
    pub ByteOffset: i64,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_7 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_8 {
    pub Vpb: *mut VPB,
    pub DeviceObject: *mut DEVICE_OBJECT,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_8 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_8 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_9 {
    pub Length: u32,
    pub CompletionFilter: u32,
    pub DirectoryNotifyInformationClass: super::System::SystemServices::DIRECTORY_NOTIFY_INFORMATION_CLASS,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_9 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_9 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_10 {
    pub Length: u32,
    pub CompletionFilter: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_10 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_10 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_11 {
    pub Argument1: *mut ::core::ffi::c_void,
    pub Argument2: *mut ::core::ffi::c_void,
    pub Argument3: *mut ::core::ffi::c_void,
    pub Argument4: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_11 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_11 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_12 {
    pub PowerSequence: *mut super::System::SystemServices::POWER_SEQUENCE,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_12 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_12 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_13 {
    pub Anonymous: IO_STACK_LOCATION_0_13_0,
    pub Type: super::System::SystemServices::POWER_STATE_TYPE,
    pub State: super::System::SystemServices::POWER_STATE,
    pub ShutdownType: super::super::Win32::System::Power::POWER_ACTION,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_13 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_13 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IO_STACK_LOCATION_0_13_0 {
    pub SystemContext: u32,
    pub SystemPowerStateContext: super::System::SystemServices::SYSTEM_POWER_STATE_CONTEXT,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_13_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_13_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_14 {
    pub Type: super::System::SystemServices::DEVICE_RELATION_TYPE,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_14 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_14 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_15 {
    pub DeviceTextType: super::System::SystemServices::DEVICE_TEXT_TYPE,
    pub LocaleId: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_15 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_15 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_16 {
    pub Length: u32,
    pub FileName: *mut super::super::Win32::Foundation::UNICODE_STRING,
    pub FileInformationClass: super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
    pub FileIndex: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_16 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_16 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_17 {
    pub Length: u32,
    pub EaList: *mut ::core::ffi::c_void,
    pub EaListLength: u32,
    pub EaIndex: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_17 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_17 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_18 {
    pub Length: u32,
    pub FileInformationClass: super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_18 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_18 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_19 {
    pub IdType: super::System::SystemServices::BUS_QUERY_ID_TYPE,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_19 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_19 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_20 {
    pub InterfaceType: *const ::windows_sys::core::GUID,
    pub Size: u16,
    pub Version: u16,
    pub Interface: *mut super::System::SystemServices::INTERFACE,
    pub InterfaceSpecificData: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_20 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_20 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_21 {
    pub Length: u32,
    pub StartSid: super::super::Win32::Foundation::PSID,
    pub SidList: *mut super::Storage::FileSystem::FILE_GET_QUOTA_INFORMATION,
    pub SidListLength: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_21 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_21 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_22 {
    pub SecurityInformation: u32,
    pub Length: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_22 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_22 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_23 {
    pub Length: u32,
    pub FsInformationClass: super::Storage::FileSystem::FS_INFORMATION_CLASS,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_23 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_23 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_24 {
    pub WhichSpace: u32,
    pub Buffer: *mut ::core::ffi::c_void,
    pub Offset: u32,
    pub Length: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_24 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_24 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_25 {
    pub Length: u32,
    pub Key: u32,
    pub ByteOffset: i64,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_25 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_25 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_26 {
    pub Srb: *mut _SCSI_REQUEST_BLOCK,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_26 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_26 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_27 {
    pub Length: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_27 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_27 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_28 {
    pub Length: u32,
    pub FileInformationClass: super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
    pub FileObject: *mut FILE_OBJECT,
    pub Anonymous: IO_STACK_LOCATION_0_28_0,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_28 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_28 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IO_STACK_LOCATION_0_28_0 {
    pub Anonymous: IO_STACK_LOCATION_0_28_0_0,
    pub ClusterCount: u32,
    pub DeleteHandle: super::super::Win32::Foundation::HANDLE,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_28_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_28_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_28_0_0 {
    pub ReplaceIfExists: super::super::Win32::Foundation::BOOLEAN,
    pub AdvanceOnly: super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_28_0_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_28_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_29 {
    pub Lock: super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_29 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_29 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_30 {
    pub Length: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_30 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_30 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_31 {
    pub SecurityInformation: u32,
    pub SecurityDescriptor: super::super::Win32::Security::PSECURITY_DESCRIPTOR,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_31 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_31 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_32 {
    pub Length: u32,
    pub FsInformationClass: super::Storage::FileSystem::FS_INFORMATION_CLASS,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_32 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_33 {
    pub AllocatedResources: *mut super::System::SystemServices::CM_RESOURCE_LIST,
    pub AllocatedResourcesTranslated: *mut super::System::SystemServices::CM_RESOURCE_LIST,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_33 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_33 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_34 {
    pub InPath: super::super::Win32::Foundation::BOOLEAN,
    pub Reserved: [super::super::Win32::Foundation::BOOLEAN; 3],
    pub Type: super::System::SystemServices::DEVICE_USAGE_NOTIFICATION_TYPE,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_34 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_34 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_35 {
    pub Vpb: *mut VPB,
    pub DeviceObject: *mut DEVICE_OBJECT,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_35 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_35 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_36 {
    pub ProviderId: usize,
    pub DataPath: *mut ::core::ffi::c_void,
    pub BufferSize: u32,
    pub Buffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_36 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_36 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_37 {
    pub PowerState: super::super::Win32::System::Power::SYSTEM_POWER_STATE,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_37 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_37 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_STACK_LOCATION_0_38 {
    pub Length: u32,
    pub Key: u32,
    pub ByteOffset: i64,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_STACK_LOCATION_0_38 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_STACK_LOCATION_0_38 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IRP {
    pub Type: i16,
    pub Size: u16,
    pub MdlAddress: *mut MDL,
    pub Flags: u32,
    pub AssociatedIrp: IRP_1,
    pub ThreadListEntry: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub IoStatus: super::super::Win32::System::IO::IO_STATUS_BLOCK,
    pub RequestorMode: i8,
    pub PendingReturned: super::super::Win32::Foundation::BOOLEAN,
    pub StackCount: u8,
    pub CurrentLocation: u8,
    pub Cancel: super::super::Win32::Foundation::BOOLEAN,
    pub CancelIrql: u8,
    pub ApcEnvironment: i8,
    pub AllocationFlags: u8,
    pub Anonymous: IRP_0,
    pub UserEvent: *mut KEVENT,
    pub Overlay: IRP_2,
    pub CancelRoutine: PDRIVER_CANCEL,
    pub UserBuffer: *mut ::core::ffi::c_void,
    pub Tail: IRP_3,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_0 {
    pub UserIosb: *mut super::super::Win32::System::IO::IO_STATUS_BLOCK,
    pub IoRingContext: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_1 {
    pub MasterIrp: *mut IRP,
    pub IrpCount: i32,
    pub SystemBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_1 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_2 {
    pub AsynchronousParameters: IRP_2_0,
    pub AllocationSize: i64,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_2 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IRP_2_0 {
    pub Anonymous1: IRP_2_0_0,
    pub Anonymous2: IRP_2_0_1,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_2_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_2_0_0 {
    pub UserApcRoutine: super::super::Win32::System::IO::PIO_APC_ROUTINE,
    pub IssuingProcess: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_2_0_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_2_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_2_0_1 {
    pub UserApcContext: *mut ::core::ffi::c_void,
    pub IoRing: *mut _IORING_OBJECT,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_2_0_1 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_2_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_3 {
    pub Overlay: IRP_3_0,
    pub Apc: super::System::SystemServices::KAPC,
    pub CompletionKey: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_3 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IRP_3_0 {
    pub Anonymous1: IRP_3_0_0,
    pub Thread: PETHREAD,
    pub AuxiliaryBuffer: ::windows_sys::core::PSTR,
    pub Anonymous2: IRP_3_0_1,
    pub OriginalFileObject: *mut FILE_OBJECT,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_3_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_3_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_3_0_0 {
    pub DeviceQueueEntry: super::System::SystemServices::KDEVICE_QUEUE_ENTRY,
    pub Anonymous: IRP_3_0_0_0,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_3_0_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_3_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IRP_3_0_0_0 {
    pub DriverContext: [*mut ::core::ffi::c_void; 4],
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_3_0_0_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_3_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IRP_3_0_1 {
    pub ListEntry: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub Anonymous: IRP_3_0_1_0,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_3_0_1 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_3_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union IRP_3_0_1_0 {
    pub CurrentStackLocation: *mut IO_STACK_LOCATION,
    pub PacketType: u32,
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IRP_3_0_1_0 {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IRP_3_0_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KDEVICE_QUEUE {
    pub Type: i16,
    pub Size: i16,
    pub DeviceListHead: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub Lock: usize,
    pub Busy: super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KDEVICE_QUEUE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KDEVICE_QUEUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct KDPC {
    pub Anonymous: KDPC_0,
    pub DpcListEntry: super::super::Win32::System::Kernel::SINGLE_LIST_ENTRY,
    pub ProcessorHistory: usize,
    pub DeferredRoutine: PKDEFERRED_ROUTINE,
    pub DeferredContext: *mut ::core::ffi::c_void,
    pub SystemArgument1: *mut ::core::ffi::c_void,
    pub SystemArgument2: *mut ::core::ffi::c_void,
    pub DpcData: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for KDPC {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for KDPC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub union KDPC_0 {
    pub TargetInfoAsUlong: u32,
    pub Anonymous: KDPC_0_0,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for KDPC_0 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for KDPC_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct KDPC_0_0 {
    pub Type: u8,
    pub Importance: u8,
    pub Number: u16,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for KDPC_0_0 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for KDPC_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type KENLISTMENT = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KEVENT {
    pub Header: DISPATCHER_HEADER,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KEVENT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KEVENT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type KGDT = isize;
pub type KIDT = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KMUTANT {
    pub Header: DISPATCHER_HEADER,
    pub MutantListEntry: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub OwnerThread: *mut isize,
    pub Anonymous: KMUTANT_0,
    pub ApcDisable: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KMUTANT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KMUTANT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union KMUTANT_0 {
    pub MutantFlags: u8,
    pub Anonymous: KMUTANT_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KMUTANT_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KMUTANT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KMUTANT_0_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KMUTANT_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KMUTANT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type KPCR = isize;
pub type KPRCB = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KQUEUE {
    pub Header: DISPATCHER_HEADER,
    pub EntryListHead: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub CurrentCount: u32,
    pub MaximumCount: u32,
    pub ThreadListHead: super::super::Win32::System::Kernel::LIST_ENTRY,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KQUEUE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KQUEUE {
    fn clone(&self) -> Self {
        *self
    }
}
pub type KRESOURCEMANAGER = isize;
pub type KTM = isize;
pub type KTRANSACTION = isize;
pub type KTSS = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KWAIT_BLOCK {
    pub WaitListEntry: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub WaitType: u8,
    pub BlockState: u8,
    pub WaitKey: u16,
    pub Anonymous: KWAIT_BLOCK_0,
    pub Object: *mut ::core::ffi::c_void,
    pub SparePtr: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KWAIT_BLOCK {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KWAIT_BLOCK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union KWAIT_BLOCK_0 {
    pub Thread: *mut isize,
    pub NotificationQueue: *mut KQUEUE,
    pub Dpc: *mut KDPC,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KWAIT_BLOCK_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KWAIT_BLOCK_0 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type LOADER_PARAMETER_BLOCK = isize;
#[repr(C)]
pub struct MDL {
    pub Next: *mut MDL,
    pub Size: i16,
    pub MdlFlags: i16,
    pub Process: *mut isize,
    pub MappedSystemVa: *mut ::core::ffi::c_void,
    pub StartVa: *mut ::core::ffi::c_void,
    pub ByteCount: u32,
    pub ByteOffset: u32,
}
impl ::core::marker::Copy for MDL {}
impl ::core::clone::Clone for MDL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct OBJECT_ATTRIBUTES {
    pub Length: u32,
    pub RootDirectory: super::super::Win32::Foundation::HANDLE,
    pub ObjectName: *const super::super::Win32::Foundation::UNICODE_STRING,
    pub Attributes: u32,
    pub SecurityDescriptor: *const ::core::ffi::c_void,
    pub SecurityQualityOfService: *const ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for OBJECT_ATTRIBUTES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for OBJECT_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OBJECT_ATTRIBUTES32 {
    pub Length: u32,
    pub RootDirectory: u32,
    pub ObjectName: u32,
    pub Attributes: u32,
    pub SecurityDescriptor: u32,
    pub SecurityQualityOfService: u32,
}
impl ::core::marker::Copy for OBJECT_ATTRIBUTES32 {}
impl ::core::clone::Clone for OBJECT_ATTRIBUTES32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OBJECT_ATTRIBUTES64 {
    pub Length: u32,
    pub RootDirectory: u64,
    pub ObjectName: u64,
    pub Attributes: u32,
    pub SecurityDescriptor: u64,
    pub SecurityQualityOfService: u64,
}
impl ::core::marker::Copy for OBJECT_ATTRIBUTES64 {}
impl ::core::clone::Clone for OBJECT_ATTRIBUTES64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct OBJECT_NAME_INFORMATION {
    pub Name: super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for OBJECT_NAME_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for OBJECT_NAME_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OWNER_ENTRY {
    pub OwnerThread: usize,
    pub Anonymous: OWNER_ENTRY_0,
}
impl ::core::marker::Copy for OWNER_ENTRY {}
impl ::core::clone::Clone for OWNER_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union OWNER_ENTRY_0 {
    pub Anonymous: OWNER_ENTRY_0_0,
    pub TableSize: u32,
}
impl ::core::marker::Copy for OWNER_ENTRY_0 {}
impl ::core::clone::Clone for OWNER_ENTRY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OWNER_ENTRY_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for OWNER_ENTRY_0_0 {}
impl ::core::clone::Clone for OWNER_ENTRY_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PAFFINITY_TOKEN = isize;
pub type PBUS_HANDLER = isize;
pub type PCALLBACK_OBJECT = isize;
pub type PDEVICE_HANDLER_OBJECT = isize;
pub type PEJOB = isize;
pub type PEPROCESS = isize;
pub type PESILO = isize;
pub type PETHREAD = isize;
pub type PEX_RUNDOWN_REF_CACHE_AWARE = isize;
pub type PEX_TIMER = isize;
pub type PIO_REMOVE_LOCK_TRACKING_BLOCK = isize;
pub type PIO_TIMER = isize;
pub type PIO_WORKITEM = isize;
pub type PKINTERRUPT = isize;
pub type PKPROCESS = isize;
pub type PKTHREAD = isize;
pub type PNOTIFY_SYNC = isize;
pub type POBJECT_TYPE = isize;
pub type POHANDLE = isize;
pub type PPCW_BUFFER = isize;
pub type PPCW_INSTANCE = isize;
pub type PPCW_REGISTRATION = isize;
pub type PRKPROCESS = isize;
pub type PRKTHREAD = isize;
pub type PSILO_MONITOR = isize;
#[repr(C)]
pub struct RTL_SPLAY_LINKS {
    pub Parent: *mut RTL_SPLAY_LINKS,
    pub LeftChild: *mut RTL_SPLAY_LINKS,
    pub RightChild: *mut RTL_SPLAY_LINKS,
}
impl ::core::marker::Copy for RTL_SPLAY_LINKS {}
impl ::core::clone::Clone for RTL_SPLAY_LINKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SECTION_OBJECT_POINTERS {
    pub DataSectionObject: *mut ::core::ffi::c_void,
    pub SharedCacheMap: *mut ::core::ffi::c_void,
    pub ImageSectionObject: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for SECTION_OBJECT_POINTERS {}
impl ::core::clone::Clone for SECTION_OBJECT_POINTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub struct SECURITY_SUBJECT_CONTEXT {
    pub ClientToken: *mut ::core::ffi::c_void,
    pub ImpersonationLevel: super::super::Win32::Security::SECURITY_IMPERSONATION_LEVEL,
    pub PrimaryToken: *mut ::core::ffi::c_void,
    pub ProcessAuditId: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Security")]
impl ::core::marker::Copy for SECURITY_SUBJECT_CONTEXT {}
#[cfg(feature = "Win32_Security")]
impl ::core::clone::Clone for SECURITY_SUBJECT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type SspiAsyncContext = isize;
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct TARGET_DEVICE_CUSTOM_NOTIFICATION {
    pub Version: u16,
    pub Size: u16,
    pub Event: ::windows_sys::core::GUID,
    pub FileObject: *mut FILE_OBJECT,
    pub NameBufferOffset: i32,
    pub CustomDataBuffer: [u8; 1],
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for TARGET_DEVICE_CUSTOM_NOTIFICATION {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for TARGET_DEVICE_CUSTOM_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Storage_FileSystem\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct VPB {
    pub Type: i16,
    pub Size: i16,
    pub Flags: u16,
    pub VolumeLabelLength: u16,
    pub DeviceObject: *mut DEVICE_OBJECT,
    pub RealDevice: *mut DEVICE_OBJECT,
    pub SerialNumber: u32,
    pub ReferenceCount: u32,
    pub VolumeLabel: [u16; 32],
}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for VPB {}
#[cfg(all(feature = "Wdk_Storage_FileSystem", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for VPB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct WORK_QUEUE_ITEM {
    pub List: super::super::Win32::System::Kernel::LIST_ENTRY,
    pub WorkerRoutine: PWORKER_THREAD_ROUTINE,
    pub Parameter: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for WORK_QUEUE_ITEM {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for WORK_QUEUE_ITEM {
    fn clone(&self) -> Self {
        *self
    }
}
pub type _DEVICE_OBJECT_POWER_EXTENSION = isize;
pub type _IORING_OBJECT = isize;
pub type _SCSI_REQUEST_BLOCK = isize;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PDRIVER_ADD_DEVICE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
pub type PDRIVER_CANCEL = ::core::option::Option<unsafe extern "system" fn() -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PDRIVER_DISPATCH = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PDRIVER_INITIALIZE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
pub type PDRIVER_STARTIO = ::core::option::Option<unsafe extern "system" fn() -> ()>;
pub type PDRIVER_UNLOAD = ::core::option::Option<unsafe extern "system" fn() -> ()>;
pub type PFAST_IO_ACQUIRE_FILE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_ACQUIRE_FOR_CCFLUSH = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_ACQUIRE_FOR_MOD_WRITE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_CHECK_IF_POSSIBLE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
pub type PFAST_IO_DETACH_DEVICE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_DEVICE_CONTROL = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_LOCK = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_MDL_READ = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_MDL_READ_COMPLETE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_MDL_READ_COMPLETE_COMPRESSED = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_MDL_WRITE_COMPLETE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_MDL_WRITE_COMPLETE_COMPRESSED = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_PREPARE_MDL_WRITE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_QUERY_BASIC_INFO = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_QUERY_NETWORK_OPEN_INFO = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_QUERY_OPEN = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_QUERY_STANDARD_INFO = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_READ = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_READ_COMPRESSED = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
pub type PFAST_IO_RELEASE_FILE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_RELEASE_FOR_CCFLUSH = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_RELEASE_FOR_MOD_WRITE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_UNLOCK_ALL = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_UNLOCK_ALL_BY_KEY = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_UNLOCK_SINGLE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_WRITE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFAST_IO_WRITE_COMPRESSED = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::BOOLEAN>;
pub type PFREE_FUNCTION = ::core::option::Option<unsafe extern "system" fn() -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PIO_COMPLETION_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> super::super::Win32::Foundation::NTSTATUS>;
pub type PKDEFERRED_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
pub type PWORKER_THREAD_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
