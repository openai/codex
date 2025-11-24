#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtNotifyChangeMultipleKeys(masterkeyhandle : super::super::super::Win32::Foundation:: HANDLE, count : u32, subordinateobjects : *const super::super::Foundation:: OBJECT_ATTRIBUTES, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, completionfilter : u32, watchtree : super::super::super::Win32::Foundation:: BOOLEAN, buffer : *mut ::core::ffi::c_void, buffersize : u32, asynchronous : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtQueryMultipleValueKey(keyhandle : super::super::super::Win32::Foundation:: HANDLE, valueentries : *mut KEY_VALUE_ENTRY, entrycount : u32, valuebuffer : *mut ::core::ffi::c_void, bufferlength : *mut u32, requiredbufferlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtRenameKey(keyhandle : super::super::super::Win32::Foundation:: HANDLE, newname : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtSetInformationKey(keyhandle : super::super::super::Win32::Foundation:: HANDLE, keysetinformationclass : KEY_SET_INFORMATION_CLASS, keysetinformation : *const ::core::ffi::c_void, keysetinformationlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwSetInformationKey(keyhandle : super::super::super::Win32::Foundation:: HANDLE, keysetinformationclass : KEY_SET_INFORMATION_CLASS, keysetinformation : *const ::core::ffi::c_void, keysetinformationlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
pub const KeyControlFlagsInformation: KEY_SET_INFORMATION_CLASS = 2i32;
pub const KeySetDebugInformation: KEY_SET_INFORMATION_CLASS = 4i32;
pub const KeySetHandleTagsInformation: KEY_SET_INFORMATION_CLASS = 5i32;
pub const KeySetLayerInformation: KEY_SET_INFORMATION_CLASS = 6i32;
pub const KeySetVirtualizationInformation: KEY_SET_INFORMATION_CLASS = 3i32;
pub const KeyWow64FlagsInformation: KEY_SET_INFORMATION_CLASS = 1i32;
pub const KeyWriteTimeInformation: KEY_SET_INFORMATION_CLASS = 0i32;
pub const MaxKeySetInfoClass: KEY_SET_INFORMATION_CLASS = 7i32;
pub type KEY_SET_INFORMATION_CLASS = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct KEY_VALUE_ENTRY {
    pub ValueName: *mut super::super::super::Win32::Foundation::UNICODE_STRING,
    pub DataLength: u32,
    pub DataOffset: u32,
    pub Type: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for KEY_VALUE_ENTRY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for KEY_VALUE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct REG_QUERY_MULTIPLE_VALUE_KEY_INFORMATION {
    pub Object: *mut ::core::ffi::c_void,
    pub ValueEntries: *mut KEY_VALUE_ENTRY,
    pub EntryCount: u32,
    pub ValueBuffer: *mut ::core::ffi::c_void,
    pub BufferLength: *mut u32,
    pub RequiredBufferLength: *mut u32,
    pub CallContext: *mut ::core::ffi::c_void,
    pub ObjectContext: *mut ::core::ffi::c_void,
    pub Reserved: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for REG_QUERY_MULTIPLE_VALUE_KEY_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for REG_QUERY_MULTIPLE_VALUE_KEY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REG_SET_INFORMATION_KEY_INFORMATION {
    pub Object: *mut ::core::ffi::c_void,
    pub KeySetInformationClass: KEY_SET_INFORMATION_CLASS,
    pub KeySetInformation: *mut ::core::ffi::c_void,
    pub KeySetInformationLength: u32,
    pub CallContext: *mut ::core::ffi::c_void,
    pub ObjectContext: *mut ::core::ffi::c_void,
    pub Reserved: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for REG_SET_INFORMATION_KEY_INFORMATION {}
impl ::core::clone::Clone for REG_SET_INFORMATION_KEY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
