#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("verifier.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn VerifierEnumerateResource(process : super::super::Foundation:: HANDLE, flags : VERIFIER_ENUM_RESOURCE_FLAGS, resourcetype : u32, resourcecallback : AVRF_RESOURCE_ENUMERATE_CALLBACK, enumerationcontext : *mut ::core::ffi::c_void) -> u32);
pub const AVRF_ENUM_RESOURCES_FLAGS_DONT_RESOLVE_TRACES: VERIFIER_ENUM_RESOURCE_FLAGS = 2u32;
pub const AVRF_ENUM_RESOURCES_FLAGS_SUSPEND: VERIFIER_ENUM_RESOURCE_FLAGS = 1u32;
pub const AVRF_MAX_TRACES: u32 = 32u32;
pub const AllocationStateBusy: eUserAllocationState = 1i32;
pub const AllocationStateFree: eUserAllocationState = 2i32;
pub const AllocationStateUnknown: eUserAllocationState = 0i32;
pub const AvrfResourceHandleTrace: eAvrfResourceTypes = 1i32;
pub const AvrfResourceHeapAllocation: eAvrfResourceTypes = 0i32;
pub const AvrfResourceMax: eAvrfResourceTypes = 2i32;
pub const HeapEnumerationEverything: eHeapEnumerationLevel = 0i32;
pub const HeapEnumerationStop: eHeapEnumerationLevel = -1i32;
pub const HeapFullPageHeap: eHeapAllocationState = 1073741824i32;
pub const HeapMetadata: eHeapAllocationState = -2147483648i32;
pub const HeapStateMask: eHeapAllocationState = -65536i32;
pub const OperationDbBADREF: eHANDLE_TRACE_OPERATIONS = 3i32;
pub const OperationDbCLOSE: eHANDLE_TRACE_OPERATIONS = 2i32;
pub const OperationDbOPEN: eHANDLE_TRACE_OPERATIONS = 1i32;
pub const OperationDbUnused: eHANDLE_TRACE_OPERATIONS = 0i32;
pub type VERIFIER_ENUM_RESOURCE_FLAGS = u32;
pub type eAvrfResourceTypes = i32;
pub type eHANDLE_TRACE_OPERATIONS = i32;
pub type eHeapAllocationState = i32;
pub type eHeapEnumerationLevel = i32;
pub type eUserAllocationState = i32;
#[repr(C)]
pub struct AVRF_BACKTRACE_INFORMATION {
    pub Depth: u32,
    pub Index: u32,
    pub ReturnAddresses: [u64; 32],
}
impl ::core::marker::Copy for AVRF_BACKTRACE_INFORMATION {}
impl ::core::clone::Clone for AVRF_BACKTRACE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AVRF_HANDLE_OPERATION {
    pub Handle: u64,
    pub ProcessId: u32,
    pub ThreadId: u32,
    pub OperationType: u32,
    pub Spare0: u32,
    pub BackTraceInformation: AVRF_BACKTRACE_INFORMATION,
}
impl ::core::marker::Copy for AVRF_HANDLE_OPERATION {}
impl ::core::clone::Clone for AVRF_HANDLE_OPERATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AVRF_HEAP_ALLOCATION {
    pub HeapHandle: u64,
    pub UserAllocation: u64,
    pub UserAllocationSize: u64,
    pub Allocation: u64,
    pub AllocationSize: u64,
    pub UserAllocationState: u32,
    pub HeapState: u32,
    pub HeapContext: u64,
    pub BackTraceInformation: *mut AVRF_BACKTRACE_INFORMATION,
}
impl ::core::marker::Copy for AVRF_HEAP_ALLOCATION {}
impl ::core::clone::Clone for AVRF_HEAP_ALLOCATION {
    fn clone(&self) -> Self {
        *self
    }
}
pub type AVRF_HANDLEOPERATION_ENUMERATE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(handleoperation: *mut AVRF_HANDLE_OPERATION, enumerationcontext: *mut ::core::ffi::c_void, enumerationlevel: *mut u32) -> u32>;
pub type AVRF_HEAPALLOCATION_ENUMERATE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(heapallocation: *mut AVRF_HEAP_ALLOCATION, enumerationcontext: *mut ::core::ffi::c_void, enumerationlevel: *mut u32) -> u32>;
pub type AVRF_RESOURCE_ENUMERATE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(resourcedescription: *mut ::core::ffi::c_void, enumerationcontext: *mut ::core::ffi::c_void, enumerationlevel: *mut u32) -> u32>;
