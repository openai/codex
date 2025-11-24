#[cfg(feature = "Wdk_Storage_FileSystem_Minifilters")]
#[doc = "Required features: `\"Wdk_Storage_FileSystem_Minifilters\"`"]
pub mod Minifilters;
::windows_targets::link!("secur32.dll" "system" fn ApplyControlToken(phcontext : *const SecHandle, pinput : *const SecBufferDesc) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcAsyncCopyRead(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, buffer : *mut ::core::ffi::c_void, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, ioissuerthread : super::super::Foundation:: PETHREAD, asyncreadcontext : *const CC_ASYNC_READ_CONTEXT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcCanIWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, bytestowrite : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, retrying : u8) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CcCoherencyFlushAndPurgeCache(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, fileoffset : *const i64, length : u32, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, flags : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcCopyRead(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, buffer : *mut ::core::ffi::c_void, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcCopyReadEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, buffer : *mut ::core::ffi::c_void, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, ioissuerthread : super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcCopyWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, buffer : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcCopyWriteEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, buffer : *const ::core::ffi::c_void, ioissuerthread : super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcCopyWriteWontFlush(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcDeferWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, postroutine : PCC_POST_DEFERRED_WRITE, context1 : *const ::core::ffi::c_void, context2 : *const ::core::ffi::c_void, bytestowrite : u32, retrying : super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CcErrorCallbackRoutine(context : *const CC_ERROR_CALLBACK_CONTEXT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcFastCopyRead(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : u32, length : u32, pagecount : u32, buffer : *mut ::core::ffi::c_void, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcFastCopyWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : u32, length : u32, buffer : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CcFlushCache(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, fileoffset : *const i64, length : u32, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcGetDirtyPages(loghandle : *const ::core::ffi::c_void, dirtypageroutine : PDIRTY_PAGE_ROUTINE, context1 : *const ::core::ffi::c_void, context2 : *const ::core::ffi::c_void) -> i64);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcGetFileObjectFromBcb(bcb : *const ::core::ffi::c_void) -> *mut super::super::Foundation:: FILE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcGetFileObjectFromSectionPtrs(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS) -> *mut super::super::Foundation:: FILE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcGetFileObjectFromSectionPtrsRef(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS) -> *mut super::super::Foundation:: FILE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn CcGetFlushedValidData(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, bcblistheld : super::super::super::Win32::Foundation:: BOOLEAN) -> i64);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcInitializeCacheMap(fileobject : *const super::super::Foundation:: FILE_OBJECT, filesizes : *const CC_FILE_SIZES, pinaccess : super::super::super::Win32::Foundation:: BOOLEAN, callbacks : *const CACHE_MANAGER_CALLBACKS, lazywritecontext : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcInitializeCacheMapEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, filesizes : *const CC_FILE_SIZES, pinaccess : super::super::super::Win32::Foundation:: BOOLEAN, callbacks : *const CACHE_MANAGER_CALLBACKS, lazywritecontext : *const ::core::ffi::c_void, flags : u32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CcIsCacheManagerCallbackNeeded(status : super::super::super::Win32::Foundation:: NTSTATUS) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcIsThereDirtyData(vpb : *const super::super::Foundation:: VPB) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcIsThereDirtyDataEx(vpb : *const super::super::Foundation:: VPB, numberofdirtypages : *const u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcMapData(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, flags : u32, bcb : *mut *mut ::core::ffi::c_void, buffer : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcMdlRead(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, mdlchain : *mut *mut super::super::Foundation:: MDL, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcMdlReadComplete(fileobject : *const super::super::Foundation:: FILE_OBJECT, mdlchain : *const super::super::Foundation:: MDL) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcMdlWriteAbort(fileobject : *const super::super::Foundation:: FILE_OBJECT, mdlchain : *const super::super::Foundation:: MDL) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcMdlWriteComplete(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, mdlchain : *const super::super::Foundation:: MDL) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcPinMappedData(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, flags : u32, bcb : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcPinRead(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, flags : u32, bcb : *mut *mut ::core::ffi::c_void, buffer : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcPrepareMdlWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, mdlchain : *mut *mut super::super::Foundation:: MDL, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcPreparePinWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, zero : super::super::super::Win32::Foundation:: BOOLEAN, flags : u32, bcb : *mut *mut ::core::ffi::c_void, buffer : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn CcPurgeCacheSection(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, fileoffset : *const i64, length : u32, flags : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn CcRemapBcb(bcb : *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
::windows_targets::link!("ntoskrnl.exe" "system" fn CcRepinBcb(bcb : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcScheduleReadAhead(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcScheduleReadAheadEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, ioissuerthread : super::super::Foundation:: PETHREAD) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetAdditionalCacheAttributes(fileobject : *const super::super::Foundation:: FILE_OBJECT, disablereadahead : super::super::super::Win32::Foundation:: BOOLEAN, disablewritebehind : super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetAdditionalCacheAttributesEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, flags : u32) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn CcSetBcbOwnerPointer(bcb : *const ::core::ffi::c_void, ownerpointer : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetDirtyPageThreshold(fileobject : *const super::super::Foundation:: FILE_OBJECT, dirtypagethreshold : u32) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn CcSetDirtyPinnedData(bcbvoid : *const ::core::ffi::c_void, lsn : *const i64) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetFileSizes(fileobject : *const super::super::Foundation:: FILE_OBJECT, filesizes : *const CC_FILE_SIZES) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetFileSizesEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, filesizes : *const CC_FILE_SIZES) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetLogHandleForFile(fileobject : *const super::super::Foundation:: FILE_OBJECT, loghandle : *const ::core::ffi::c_void, flushtolsnroutine : PFLUSH_TO_LSN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetParallelFlushFile(fileobject : *const super::super::Foundation:: FILE_OBJECT, enableparallelflush : super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcSetReadAheadGranularity(fileobject : *const super::super::Foundation:: FILE_OBJECT, granularity : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcUninitializeCacheMap(fileobject : *const super::super::Foundation:: FILE_OBJECT, truncatesize : *const i64, uninitializeevent : *const CACHE_UNINITIALIZE_EVENT) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn CcUnpinData(bcb : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn CcUnpinDataForThread(bcb : *const ::core::ffi::c_void, resourcethreadid : usize) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn CcUnpinRepinnedBcb(bcb : *const ::core::ffi::c_void, writethrough : super::super::super::Win32::Foundation:: BOOLEAN, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CcWaitForCurrentLazyWriterActivity() -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn CcZeroData(fileobject : *const super::super::Foundation:: FILE_OBJECT, startoffset : *const i64, endoffset : *const i64, wait : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("secur32.dll" "system" fn CompleteAuthToken(phcontext : *const SecHandle, ptoken : *const SecBufferDesc) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn ExDisableResourceBoostLite(resource : *const super::super::Foundation:: ERESOURCE) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ExQueryPoolBlockSize(poolblock : *const ::core::ffi::c_void, quotacharged : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> usize);
::windows_targets::link!("secur32.dll" "system" fn ExportSecurityContext(phcontext : *const SecHandle, fflags : u32, ppackedcontext : *mut SecBuffer, ptoken : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlAcknowledgeEcp(ecpcontext : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlAcquireFileExclusive(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlAddBaseMcbEntry(mcb : *mut BASE_MCB, vbn : i64, lbn : i64, sectorcount : i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlAddBaseMcbEntryEx(mcb : *mut BASE_MCB, vbn : i64, lbn : i64, sectorcount : i64) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlAddLargeMcbEntry(mcb : *mut LARGE_MCB, vbn : i64, lbn : i64, sectorcount : i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlAddMcbEntry(mcb : *mut MCB, vbn : u32, lbn : u32, sectorcount : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlAddToTunnelCache(cache : *mut TUNNEL, directorykey : u64, shortname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, longname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, keybyshortname : super::super::super::Win32::Foundation:: BOOLEAN, datalength : u32, data : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlAddToTunnelCacheEx(cache : *mut TUNNEL, directorykey : u64, shortname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, longname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, flags : u32, datalength : u32, data : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FsRtlAllocateAePushLock(pooltype : super::super::Foundation:: POOL_TYPE, tag : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlAllocateExtraCreateParameter(ecptype : *const ::windows_sys::core::GUID, sizeofcontext : u32, flags : u32, cleanupcallback : PFSRTL_EXTRA_CREATE_PARAMETER_CLEANUP_CALLBACK, pooltag : u32, ecpcontext : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlAllocateExtraCreateParameterFromLookasideList(ecptype : *const ::windows_sys::core::GUID, sizeofcontext : u32, flags : u32, cleanupcallback : PFSRTL_EXTRA_CREATE_PARAMETER_CLEANUP_CALLBACK, lookasidelist : *mut ::core::ffi::c_void, ecpcontext : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlAllocateExtraCreateParameterList(flags : u32, ecplist : *mut *mut super::super::Foundation:: ECP_LIST) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlAllocateFileLock(completelockirproutine : PCOMPLETE_LOCK_IRP_ROUTINE, unlockroutine : PUNLOCK_ROUTINE) -> *mut FILE_LOCK);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlAllocateResource() -> *mut super::super::Foundation:: ERESOURCE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlAreNamesEqual(constantnamea : *const super::super::super::Win32::Foundation:: UNICODE_STRING, constantnameb : *const super::super::super::Win32::Foundation:: UNICODE_STRING, ignorecase : super::super::super::Win32::Foundation:: BOOLEAN, upcasetable : *const u16) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlAreThereCurrentOrInProgressFileLocks(filelock : *const FILE_LOCK) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlAreThereWaitingFileLocks(filelock : *const FILE_LOCK) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlAreVolumeStartupApplicationsComplete() -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlBalanceReads(targetdevice : *const super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCancellableWaitForMultipleObjects(count : u32, objectarray : *const *const ::core::ffi::c_void, waittype : super::super::super::Win32::System::Kernel:: WAIT_TYPE, timeout : *const i64, waitblockarray : *const super::super::Foundation:: KWAIT_BLOCK, irp : *const super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCancellableWaitForSingleObject(object : *const ::core::ffi::c_void, timeout : *const i64, irp : *const super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlChangeBackingFileObject(currentfileobject : *const super::super::Foundation:: FILE_OBJECT, newfileobject : *const super::super::Foundation:: FILE_OBJECT, changebackingtype : FSRTL_CHANGE_BACKING_TYPE, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCheckLockForOplockRequest(filelock : *const FILE_LOCK, allocationsize : *const i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCheckLockForReadAccess(filelock : *const FILE_LOCK, irp : *const super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCheckLockForWriteAccess(filelock : *const FILE_LOCK, irp : *const super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCheckOplock(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, context : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, postirproutine : POPLOCK_FS_PREPOST_IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCheckOplockEx(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, flags : u32, context : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, postirproutine : POPLOCK_FS_PREPOST_IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCheckOplockEx2(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, flags : u32, flagsex2 : u32, completionroutinecontext : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, postirproutine : POPLOCK_FS_PREPOST_IRP, timeout : u64, notifycontext : *const ::core::ffi::c_void, notifyroutine : POPLOCK_NOTIFY_ROUTINE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCheckUpperOplock(oplock : *const *const ::core::ffi::c_void, newloweroplockstate : u32, completionroutinecontext : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, prependroutine : POPLOCK_FS_PREPOST_IRP, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCopyRead(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, lockkey : u32, buffer : *mut ::core::ffi::c_void, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCopyWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, wait : super::super::super::Win32::Foundation:: BOOLEAN, lockkey : u32, buffer : *const ::core::ffi::c_void, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlCreateSectionForDataScan(sectionhandle : *mut super::super::super::Win32::Foundation:: HANDLE, sectionobject : *mut *mut ::core::ffi::c_void, sectionfilesize : *mut i64, fileobject : *const super::super::Foundation:: FILE_OBJECT, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, maximumsize : *const i64, sectionpageprotection : u32, allocationattributes : u32, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlCurrentBatchOplock(oplock : *const *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlCurrentOplock(oplock : *const *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlCurrentOplockH(oplock : *const *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlDeleteExtraCreateParameterLookasideList(lookaside : *mut ::core::ffi::c_void, flags : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlDeleteKeyFromTunnelCache(cache : *mut TUNNEL, directorykey : u64) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlDeleteTunnelCache(cache : *mut TUNNEL) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlDeregisterUncProvider(handle : super::super::super::Win32::Foundation:: HANDLE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlDismountComplete(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, dismountstatus : super::super::super::Win32::Foundation:: NTSTATUS) -> ());
#[cfg(feature = "Win32_System_Kernel")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_System_Kernel\"`"] fn FsRtlDissectDbcs(path : super::super::super::Win32::System::Kernel:: STRING, firstname : *mut super::super::super::Win32::System::Kernel:: STRING, remainingname : *mut super::super::super::Win32::System::Kernel:: STRING) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlDissectName(path : super::super::super::Win32::Foundation:: UNICODE_STRING, firstname : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, remainingname : *mut super::super::super::Win32::Foundation:: UNICODE_STRING) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlDoesDbcsContainWildCards(name : *const super::super::super::Win32::System::Kernel:: STRING) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlDoesNameContainWildCards(name : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlFastCheckLockForRead(filelock : *const FILE_LOCK, startingbyte : *const i64, length : *const i64, key : u32, fileobject : *const super::super::Foundation:: FILE_OBJECT, processid : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlFastCheckLockForWrite(filelock : *const FILE_LOCK, startingbyte : *const i64, length : *const i64, key : u32, fileobject : *const ::core::ffi::c_void, processid : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlFastUnlockAll(filelock : *const FILE_LOCK, fileobject : *const super::super::Foundation:: FILE_OBJECT, processid : super::super::Foundation:: PEPROCESS, context : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlFastUnlockAllByKey(filelock : *const FILE_LOCK, fileobject : *const super::super::Foundation:: FILE_OBJECT, processid : super::super::Foundation:: PEPROCESS, key : u32, context : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlFastUnlockSingle(filelock : *const FILE_LOCK, fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : *const i64, processid : super::super::Foundation:: PEPROCESS, key : u32, context : *const ::core::ffi::c_void, alreadysynchronized : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlFindExtraCreateParameter(ecplist : *const super::super::Foundation:: ECP_LIST, ecptype : *const ::windows_sys::core::GUID, ecpcontext : *mut *mut ::core::ffi::c_void, ecpcontextsize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlFindInTunnelCache(cache : *const TUNNEL, directorykey : u64, name : *const super::super::super::Win32::Foundation:: UNICODE_STRING, shortname : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, longname : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, datalength : *mut u32, data : *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlFindInTunnelCacheEx(cache : *const TUNNEL, directorykey : u64, name : *const super::super::super::Win32::Foundation:: UNICODE_STRING, shortname : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, longname : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, flags : u32, datalength : *mut u32, data : *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlFreeAePushLock(aepushlock : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlFreeExtraCreateParameter(ecpcontext : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FsRtlFreeExtraCreateParameterList(ecplist : *const super::super::Foundation:: ECP_LIST) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlFreeFileLock(filelock : *const FILE_LOCK) -> ());
#[cfg(feature = "Win32_System_Kernel")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_System_Kernel\"`"] fn FsRtlGetCurrentProcessLoaderList() -> *mut super::super::super::Win32::System::Kernel:: LIST_ENTRY);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlGetEcpListFromIrp(irp : *const super::super::Foundation:: IRP, ecplist : *mut *mut super::super::Foundation:: ECP_LIST) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlGetFileSize(fileobject : *const super::super::Foundation:: FILE_OBJECT, filesize : *mut i64) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlGetNextBaseMcbEntry(mcb : *const BASE_MCB, runindex : u32, vbn : *mut i64, lbn : *mut i64, sectorcount : *mut i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlGetNextExtraCreateParameter(ecplist : *const super::super::Foundation:: ECP_LIST, currentecpcontext : *const ::core::ffi::c_void, nextecptype : *mut ::windows_sys::core::GUID, nextecpcontext : *mut *mut ::core::ffi::c_void, nextecpcontextsize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlGetNextFileLock(filelock : *const FILE_LOCK, restart : super::super::super::Win32::Foundation:: BOOLEAN) -> *mut FILE_LOCK_INFO);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlGetNextLargeMcbEntry(mcb : *const LARGE_MCB, runindex : u32, vbn : *mut i64, lbn : *mut i64, sectorcount : *mut i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlGetNextMcbEntry(mcb : *const MCB, runindex : u32, vbn : *mut u32, lbn : *mut u32, sectorcount : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlGetSectorSizeInformation(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, sectorsizeinfo : *mut FILE_FS_SECTOR_SIZE_INFORMATION) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlGetSupportedFeatures(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, supportedfeatures : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlGetVirtualDiskNestingLevel(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, nestinglevel : *mut u32, nestingflags : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlIncrementCcFastMdlReadWait() -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlIncrementCcFastReadNoWait() -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlIncrementCcFastReadNotPossible() -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlIncrementCcFastReadResourceMiss() -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlIncrementCcFastReadWait() -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlInitExtraCreateParameterLookasideList(lookaside : *mut ::core::ffi::c_void, flags : u32, size : usize, tag : u32) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FsRtlInitializeBaseMcb(mcb : *mut BASE_MCB, pooltype : super::super::Foundation:: POOL_TYPE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlInitializeBaseMcbEx(mcb : *mut BASE_MCB, pooltype : super::super::Foundation:: POOL_TYPE, flags : u16) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FsRtlInitializeExtraCreateParameter(ecp : *mut super::super::Foundation:: ECP_HEADER, ecpflags : u32, cleanupcallback : PFSRTL_EXTRA_CREATE_PARAMETER_CLEANUP_CALLBACK, totalsize : u32, ecptype : *const ::windows_sys::core::GUID, listallocatedfrom : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlInitializeExtraCreateParameterList(ecplist : *mut super::super::Foundation:: ECP_LIST) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlInitializeFileLock(filelock : *mut FILE_LOCK, completelockirproutine : PCOMPLETE_LOCK_IRP_ROUTINE, unlockroutine : PUNLOCK_ROUTINE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlInitializeLargeMcb(mcb : *mut LARGE_MCB, pooltype : super::super::Foundation:: POOL_TYPE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlInitializeMcb(mcb : *mut MCB, pooltype : super::super::Foundation:: POOL_TYPE) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlInitializeOplock(oplock : *mut *mut ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlInitializeTunnelCache(cache : *mut TUNNEL) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlInsertExtraCreateParameter(ecplist : *mut super::super::Foundation:: ECP_LIST, ecpcontext : *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlInsertPerFileContext(perfilecontextpointer : *const *const ::core::ffi::c_void, ptr : *const FSRTL_PER_FILE_CONTEXT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlInsertPerFileObjectContext(fileobject : *const super::super::Foundation:: FILE_OBJECT, ptr : *const FSRTL_PER_FILEOBJECT_CONTEXT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlInsertPerStreamContext(perstreamcontext : *const FSRTL_ADVANCED_FCB_HEADER, ptr : *const FSRTL_PER_STREAM_CONTEXT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlIs32BitProcess(process : super::super::Foundation:: PEPROCESS) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlIsDaxVolume(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlIsDbcsInExpression(expression : *const super::super::super::Win32::System::Kernel:: STRING, name : *const super::super::super::Win32::System::Kernel:: STRING) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlIsEcpAcknowledged(ecpcontext : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlIsEcpFromUserMode(ecpcontext : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlIsExtentDangling(startpage : u32, numberofpages : u32, flags : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlIsFatDbcsLegal(dbcsname : super::super::super::Win32::System::Kernel:: STRING, wildcardspermissible : super::super::super::Win32::Foundation:: BOOLEAN, pathnamepermissible : super::super::super::Win32::Foundation:: BOOLEAN, leadingbackslashpermissible : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlIsHpfsDbcsLegal(dbcsname : super::super::super::Win32::System::Kernel:: STRING, wildcardspermissible : super::super::super::Win32::Foundation:: BOOLEAN, pathnamepermissible : super::super::super::Win32::Foundation:: BOOLEAN, leadingbackslashpermissible : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlIsMobileOS() -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlIsNameInExpression(expression : *const super::super::super::Win32::Foundation:: UNICODE_STRING, name : *const super::super::super::Win32::Foundation:: UNICODE_STRING, ignorecase : super::super::super::Win32::Foundation:: BOOLEAN, upcasetable : ::windows_sys::core::PCWSTR) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlIsNameInUnUpcasedExpression(expression : *const super::super::super::Win32::Foundation:: UNICODE_STRING, name : *const super::super::super::Win32::Foundation:: UNICODE_STRING, ignorecase : super::super::super::Win32::Foundation:: BOOLEAN, upcasetable : ::windows_sys::core::PCWSTR) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlIsNonEmptyDirectoryReparsePointAllowed(reparsetag : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlIsNtstatusExpected(exception : super::super::super::Win32::Foundation:: NTSTATUS) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlIsPagingFile(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlIsSystemPagingFile(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlIssueDeviceIoControl(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, ioctl : u32, flags : u8, inputbuffer : *const ::core::ffi::c_void, inputbufferlength : u32, outputbuffer : *const ::core::ffi::c_void, outputbufferlength : u32, iosbinformation : *mut usize) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlKernelFsControlFile(fileobject : *const super::super::Foundation:: FILE_OBJECT, fscontrolcode : u32, inputbuffer : *const ::core::ffi::c_void, inputbufferlength : u32, outputbuffer : *mut ::core::ffi::c_void, outputbufferlength : u32, retoutputbuffersize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlLogCcFlushError(filename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, flusherror : super::super::super::Win32::Foundation:: NTSTATUS, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlLookupBaseMcbEntry(mcb : *const BASE_MCB, vbn : i64, lbn : *mut i64, sectorcountfromlbn : *mut i64, startinglbn : *mut i64, sectorcountfromstartinglbn : *mut i64, index : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlLookupLargeMcbEntry(mcb : *const LARGE_MCB, vbn : i64, lbn : *mut i64, sectorcountfromlbn : *mut i64, startinglbn : *mut i64, sectorcountfromstartinglbn : *mut i64, index : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlLookupLastBaseMcbEntry(mcb : *const BASE_MCB, vbn : *mut i64, lbn : *mut i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlLookupLastBaseMcbEntryAndIndex(opaquemcb : *const BASE_MCB, largevbn : *mut i64, largelbn : *mut i64, index : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlLookupLastLargeMcbEntry(mcb : *const LARGE_MCB, vbn : *mut i64, lbn : *mut i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlLookupLastLargeMcbEntryAndIndex(opaquemcb : *const LARGE_MCB, largevbn : *mut i64, largelbn : *mut i64, index : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlLookupLastMcbEntry(mcb : *const MCB, vbn : *mut u32, lbn : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlLookupMcbEntry(mcb : *const MCB, vbn : u32, lbn : *mut u32, sectorcount : *mut u32, index : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlLookupPerFileContext(perfilecontextpointer : *const *const ::core::ffi::c_void, ownerid : *const ::core::ffi::c_void, instanceid : *const ::core::ffi::c_void) -> *mut FSRTL_PER_FILE_CONTEXT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlLookupPerFileObjectContext(fileobject : *const super::super::Foundation:: FILE_OBJECT, ownerid : *const ::core::ffi::c_void, instanceid : *const ::core::ffi::c_void) -> *mut FSRTL_PER_FILEOBJECT_CONTEXT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlLookupPerStreamContextInternal(streamcontext : *const FSRTL_ADVANCED_FCB_HEADER, ownerid : *const ::core::ffi::c_void, instanceid : *const ::core::ffi::c_void) -> *mut FSRTL_PER_STREAM_CONTEXT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlMdlReadCompleteDev(fileobject : *const super::super::Foundation:: FILE_OBJECT, mdlchain : *const super::super::Foundation:: MDL, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlMdlReadDev(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, lockkey : u32, mdlchain : *mut *mut super::super::Foundation:: MDL, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlMdlReadEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, lockkey : u32, mdlchain : *mut *mut super::super::Foundation:: MDL, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlMdlWriteCompleteDev(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, mdlchain : *const super::super::Foundation:: MDL, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlMupGetProviderIdFromName(pprovidername : *const super::super::super::Win32::Foundation:: UNICODE_STRING, pproviderid : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlMupGetProviderInfoFromFileObject(pfileobject : *const super::super::Foundation:: FILE_OBJECT, level : u32, pbuffer : *mut ::core::ffi::c_void, pbuffersize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlNormalizeNtstatus(exception : super::super::super::Win32::Foundation:: NTSTATUS, genericexception : super::super::super::Win32::Foundation:: NTSTATUS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlNotifyCleanup(notifysync : super::super::Foundation:: PNOTIFY_SYNC, notifylist : *const super::super::super::Win32::System::Kernel:: LIST_ENTRY, fscontext : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlNotifyCleanupAll(notifysync : super::super::Foundation:: PNOTIFY_SYNC, notifylist : *const super::super::super::Win32::System::Kernel:: LIST_ENTRY) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlNotifyFilterChangeDirectory(notifysync : super::super::Foundation:: PNOTIFY_SYNC, notifylist : *const super::super::super::Win32::System::Kernel:: LIST_ENTRY, fscontext : *const ::core::ffi::c_void, fulldirectoryname : *const super::super::super::Win32::System::Kernel:: STRING, watchtree : super::super::super::Win32::Foundation:: BOOLEAN, ignorebuffer : super::super::super::Win32::Foundation:: BOOLEAN, completionfilter : u32, notifyirp : *const super::super::Foundation:: IRP, traversecallback : PCHECK_FOR_TRAVERSE_ACCESS, subjectcontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, filtercallback : PFILTER_REPORT_CHANGE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlNotifyFilterReportChange(notifysync : super::super::Foundation:: PNOTIFY_SYNC, notifylist : *const super::super::super::Win32::System::Kernel:: LIST_ENTRY, fulltargetname : *const super::super::super::Win32::System::Kernel:: STRING, targetnameoffset : u16, streamname : *const super::super::super::Win32::System::Kernel:: STRING, normalizedparentname : *const super::super::super::Win32::System::Kernel:: STRING, filtermatch : u32, action : u32, targetcontext : *const ::core::ffi::c_void, filtercontext : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlNotifyFullChangeDirectory(notifysync : super::super::Foundation:: PNOTIFY_SYNC, notifylist : *const super::super::super::Win32::System::Kernel:: LIST_ENTRY, fscontext : *const ::core::ffi::c_void, fulldirectoryname : *mut super::super::super::Win32::System::Kernel:: STRING, watchtree : super::super::super::Win32::Foundation:: BOOLEAN, ignorebuffer : super::super::super::Win32::Foundation:: BOOLEAN, completionfilter : u32, notifyirp : *const super::super::Foundation:: IRP, traversecallback : PCHECK_FOR_TRAVERSE_ACCESS, subjectcontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlNotifyFullReportChange(notifysync : super::super::Foundation:: PNOTIFY_SYNC, notifylist : *const super::super::super::Win32::System::Kernel:: LIST_ENTRY, fulltargetname : *const super::super::super::Win32::System::Kernel:: STRING, targetnameoffset : u16, streamname : *const super::super::super::Win32::System::Kernel:: STRING, normalizedparentname : *const super::super::super::Win32::System::Kernel:: STRING, filtermatch : u32, action : u32, targetcontext : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FsRtlNotifyInitializeSync(notifysync : *mut super::super::Foundation:: PNOTIFY_SYNC) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FsRtlNotifyUninitializeSync(notifysync : *mut super::super::Foundation:: PNOTIFY_SYNC) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlNotifyVolumeEvent(fileobject : *const super::super::Foundation:: FILE_OBJECT, eventcode : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlNotifyVolumeEventEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, eventcode : u32, event : *const super::super::Foundation:: TARGET_DEVICE_CUSTOM_NOTIFICATION) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlNumberOfRunsInBaseMcb(mcb : *const BASE_MCB) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlNumberOfRunsInLargeMcb(mcb : *const LARGE_MCB) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlNumberOfRunsInMcb(mcb : *const MCB) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockBreakH(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, flags : u32, context : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, postirproutine : POPLOCK_FS_PREPOST_IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockBreakH2(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, flags : u32, context : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, postirproutine : POPLOCK_FS_PREPOST_IRP, grantedaccess : *const u32, shareaccess : *const u16) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockBreakToNone(oplock : *mut *mut ::core::ffi::c_void, irpsp : *const super::super::Foundation:: IO_STACK_LOCATION, irp : *const super::super::Foundation:: IRP, context : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, postirproutine : POPLOCK_FS_PREPOST_IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockBreakToNoneEx(oplock : *mut *mut ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, flags : u32, context : *const ::core::ffi::c_void, completionroutine : POPLOCK_WAIT_COMPLETE_ROUTINE, postirproutine : POPLOCK_FS_PREPOST_IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockFsctrl(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, opencount : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockFsctrlEx(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, opencount : u32, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FsRtlOplockGetAnyBreakOwnerProcess(oplock : *const *const ::core::ffi::c_void) -> super::super::Foundation:: PEPROCESS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlOplockIsFastIoPossible(oplock : *const *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockIsSharedRequest(irp : *const super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlOplockKeysEqual(fo1 : *const super::super::Foundation:: FILE_OBJECT, fo2 : *const super::super::Foundation:: FILE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlPostPagingFileStackOverflow(context : *const ::core::ffi::c_void, event : *const super::super::Foundation:: KEVENT, stackoverflowroutine : PFSRTL_STACK_OVERFLOW_ROUTINE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlPostStackOverflow(context : *const ::core::ffi::c_void, event : *const super::super::Foundation:: KEVENT, stackoverflowroutine : PFSRTL_STACK_OVERFLOW_ROUTINE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlPrepareMdlWriteDev(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, lockkey : u32, mdlchain : *mut *mut super::super::Foundation:: MDL, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlPrepareMdlWriteEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, lockkey : u32, mdlchain : *mut *mut super::super::Foundation:: MDL, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlPrepareToReuseEcp(ecpcontext : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlPrivateLock(filelock : *const FILE_LOCK, fileobject : *const super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : *const i64, processid : super::super::Foundation:: PEPROCESS, key : u32, failimmediately : super::super::super::Win32::Foundation:: BOOLEAN, exclusivelock : super::super::super::Win32::Foundation:: BOOLEAN, iosb : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, irp : *const super::super::Foundation:: IRP, context : *const ::core::ffi::c_void, alreadysynchronized : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlProcessFileLock(filelock : *const FILE_LOCK, irp : *const super::super::Foundation:: IRP, context : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlQueryCachedVdl(fileobject : *const super::super::Foundation:: FILE_OBJECT, vdl : *mut i64) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlQueryInformationFile(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, retfileinformationsize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlQueryKernelEaFile(fileobject : *const super::super::Foundation:: FILE_OBJECT, returnedeadata : *mut ::core::ffi::c_void, length : u32, returnsingleentry : super::super::super::Win32::Foundation:: BOOLEAN, ealist : *const ::core::ffi::c_void, ealistlength : u32, eaindex : *const u32, restartscan : super::super::super::Win32::Foundation:: BOOLEAN, lengthreturned : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlQueryMaximumVirtualDiskNestingLevel() -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlRegisterFileSystemFilterCallbacks(filterdriverobject : *const super::super::Foundation:: DRIVER_OBJECT, callbacks : *const FS_FILTER_CALLBACKS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlRegisterUncProvider(muphandle : *mut super::super::super::Win32::Foundation:: HANDLE, redirectordevicename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, mailslotssupported : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlRegisterUncProviderEx(muphandle : *mut super::super::super::Win32::Foundation:: HANDLE, redirdevname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlRegisterUncProviderEx2(redirdevname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, registration : *const FSRTL_UNC_PROVIDER_REGISTRATION, muphandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlReleaseFile(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlRemoveBaseMcbEntry(mcb : *mut BASE_MCB, vbn : i64, sectorcount : i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlRemoveDotsFromPath(originalstring : ::windows_sys::core::PWSTR, pathlength : u16, newlength : *mut u16) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FsRtlRemoveExtraCreateParameter(ecplist : *mut super::super::Foundation:: ECP_LIST, ecptype : *const ::windows_sys::core::GUID, ecpcontext : *mut *mut ::core::ffi::c_void, ecpcontextsize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlRemoveLargeMcbEntry(mcb : *mut LARGE_MCB, vbn : i64, sectorcount : i64) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlRemoveMcbEntry(mcb : *mut MCB, vbn : u32, sectorcount : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlRemovePerFileContext(perfilecontextpointer : *const *const ::core::ffi::c_void, ownerid : *const ::core::ffi::c_void, instanceid : *const ::core::ffi::c_void) -> *mut FSRTL_PER_FILE_CONTEXT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlRemovePerFileObjectContext(fileobject : *const super::super::Foundation:: FILE_OBJECT, ownerid : *const ::core::ffi::c_void, instanceid : *const ::core::ffi::c_void) -> *mut FSRTL_PER_FILEOBJECT_CONTEXT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlRemovePerStreamContext(streamcontext : *const FSRTL_ADVANCED_FCB_HEADER, ownerid : *const ::core::ffi::c_void, instanceid : *const ::core::ffi::c_void) -> *mut FSRTL_PER_STREAM_CONTEXT);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlResetBaseMcb(mcb : *mut BASE_MCB) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlResetLargeMcb(mcb : *mut LARGE_MCB, selfsynchronized : super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlSetDriverBacking(driverobj : *const super::super::Foundation:: DRIVER_OBJECT, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlSetEcpListIntoIrp(irp : *mut super::super::Foundation:: IRP, ecplist : *const super::super::Foundation:: ECP_LIST) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlSetKernelEaFile(fileobject : *const super::super::Foundation:: FILE_OBJECT, eabuffer : *const ::core::ffi::c_void, length : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlSplitBaseMcb(mcb : *mut BASE_MCB, vbn : i64, amount : i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlSplitLargeMcb(mcb : *mut LARGE_MCB, vbn : i64, amount : i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlTeardownPerFileContexts(perfilecontextpointer : *const *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlTeardownPerStreamContexts(advancedheader : *const FSRTL_ADVANCED_FCB_HEADER) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlTruncateBaseMcb(mcb : *mut BASE_MCB, vbn : i64) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlTruncateLargeMcb(mcb : *mut LARGE_MCB, vbn : i64) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlTruncateMcb(mcb : *mut MCB, vbn : u32) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlUninitializeBaseMcb(mcb : *const BASE_MCB) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlUninitializeFileLock(filelock : *mut FILE_LOCK) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlUninitializeLargeMcb(mcb : *mut LARGE_MCB) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FsRtlUninitializeMcb(mcb : *mut MCB) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlUninitializeOplock(oplock : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn FsRtlUpdateDiskCounters(bytesread : u64, byteswritten : u64) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlUpperOplockFsctrl(oplock : *const *const ::core::ffi::c_void, irp : *const super::super::Foundation:: IRP, opencount : u32, loweroplockstate : u32, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FsRtlValidateReparsePointBuffer(bufferlength : u32, reparsebuffer : *const REPARSE_DATA_BUFFER) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FsRtlVolumeDeviceToCorrelationId(volumedeviceobject : *const super::super::Foundation:: DEVICE_OBJECT, guid : *mut ::windows_sys::core::GUID) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
::windows_targets::link!("secur32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"] fn GetSecurityUserInfo(logonid : *const super::super::super::Win32::Foundation:: LUID, flags : u32, userinformation : *mut *mut super::super::super::Win32::Security::Authentication::Identity:: SECURITY_USER_DATA) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntoskrnl.exe" "system" fn IoAcquireVpbSpinLock(irql : *mut u8) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn IoApplyPriorityInfoThread(inputpriorityinfo : *const IO_PRIORITY_INFO, outputpriorityinfo : *mut IO_PRIORITY_INFO, thread : super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IoCheckDesiredAccess(desiredaccess : *mut u32, grantedaccess : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IoCheckEaBufferValidity(eabuffer : *const FILE_FULL_EA_INFORMATION, ealength : u32, erroroffset : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IoCheckFunctionAccess(grantedaccess : u32, majorfunction : u8, minorfunction : u8, iocontrolcode : u32, arg1 : *const ::core::ffi::c_void, arg2 : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoCheckQuerySetFileInformation(fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, length : u32, setoperation : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IoCheckQuerySetVolumeInformation(fsinformationclass : FS_INFORMATION_CLASS, length : u32, setoperation : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn IoCheckQuotaBufferValidity(quotabuffer : *const FILE_QUOTA_INFORMATION, quotalength : u32, erroroffset : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoClearFsTrackOffsetState(irp : *mut super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoCreateStreamFileObject(fileobject : *const super::super::Foundation:: FILE_OBJECT, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> *mut super::super::Foundation:: FILE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoCreateStreamFileObjectEx(fileobject : *const super::super::Foundation:: FILE_OBJECT, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, filehandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> *mut super::super::Foundation:: FILE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoCreateStreamFileObjectEx2(createoptions : *const IO_CREATE_STREAM_FILE_OPTIONS, fileobject : *const super::super::Foundation:: FILE_OBJECT, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, streamfileobject : *mut *mut super::super::Foundation:: FILE_OBJECT, filehandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoCreateStreamFileObjectLite(fileobject : *const super::super::Foundation:: FILE_OBJECT, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> *mut super::super::Foundation:: FILE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoEnumerateDeviceObjectList(driverobject : *const super::super::Foundation:: DRIVER_OBJECT, deviceobjectlist : *mut *mut super::super::Foundation:: DEVICE_OBJECT, deviceobjectlistsize : u32, actualnumberdeviceobjects : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoEnumerateRegisteredFiltersList(driverobjectlist : *mut *mut super::super::Foundation:: DRIVER_OBJECT, driverobjectlistsize : u32, actualnumberdriverobjects : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn IoFastQueryNetworkAttributes(objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, desiredaccess : u32, openoptions : u32, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *mut FILE_NETWORK_OPEN_INFORMATION) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetAttachedDevice(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> *mut super::super::Foundation:: DEVICE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetBaseFileSystemDeviceObject(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> *mut super::super::Foundation:: DEVICE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetDeviceAttachmentBaseRef(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> *mut super::super::Foundation:: DEVICE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetDeviceToVerify(thread : super::super::Foundation:: PETHREAD) -> *mut super::super::Foundation:: DEVICE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetDiskDeviceObject(filesystemdeviceobject : *const super::super::Foundation:: DEVICE_OBJECT, diskdeviceobject : *mut *mut super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Ioctl", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Ioctl\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetFsTrackOffsetState(irp : *const super::super::Foundation:: IRP, retfstrackoffsetblob : *mut *mut super::super::super::Win32::System::Ioctl:: IO_IRP_EXT_TRACK_OFFSET_HEADER, rettrackedoffset : *mut i64) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetLowerDeviceObject(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> *mut super::super::Foundation:: DEVICE_OBJECT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetOplockKeyContext(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> *mut OPLOCK_KEY_ECP_CONTEXT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetOplockKeyContextEx(fileobject : *const super::super::Foundation:: FILE_OBJECT) -> *mut OPLOCK_KEY_CONTEXT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetRequestorProcess(irp : *const super::super::Foundation:: IRP) -> super::super::Foundation:: PEPROCESS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetRequestorProcessId(irp : *const super::super::Foundation:: IRP) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoGetRequestorSessionId(irp : *const super::super::Foundation:: IRP, psessionid : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoIrpHasFsTrackOffsetExtensionType(irp : *const super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoIsOperationSynchronous(irp : *const super::super::Foundation:: IRP) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn IoIsSystemThread(thread : super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoIsValidNameGraftingBuffer(irp : *const super::super::Foundation:: IRP, reparsebuffer : *const REPARSE_DATA_BUFFER) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoPageRead(fileobject : *const super::super::Foundation:: FILE_OBJECT, memorydescriptorlist : *const super::super::Foundation:: MDL, startingoffset : *const i64, event : *const super::super::Foundation:: KEVENT, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoQueryFileDosDeviceName(fileobject : *const super::super::Foundation:: FILE_OBJECT, objectnameinformation : *mut *mut super::super::Foundation:: OBJECT_NAME_INFORMATION) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoQueryFileInformation(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, length : u32, fileinformation : *mut ::core::ffi::c_void, returnedlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoQueryVolumeInformation(fileobject : *const super::super::Foundation:: FILE_OBJECT, fsinformationclass : FS_INFORMATION_CLASS, length : u32, fsinformation : *mut ::core::ffi::c_void, returnedlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoQueueThreadIrp(irp : *const super::super::Foundation:: IRP) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoRegisterFileSystem(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoRegisterFsRegistrationChange(driverobject : *const super::super::Foundation:: DRIVER_OBJECT, drivernotificationroutine : PDRIVER_FS_NOTIFICATION) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoRegisterFsRegistrationChangeMountAware(driverobject : *const super::super::Foundation:: DRIVER_OBJECT, drivernotificationroutine : PDRIVER_FS_NOTIFICATION, synchronizewithmounts : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntoskrnl.exe" "system" fn IoReleaseVpbSpinLock(irql : u8) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoReplaceFileObjectName(fileobject : *const super::super::Foundation:: FILE_OBJECT, newfilename : ::windows_sys::core::PCWSTR, filenamelength : u16) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoRequestDeviceRemovalForReset(physicaldeviceobject : *const super::super::Foundation:: DEVICE_OBJECT, flags : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoRetrievePriorityInfo(irp : *const super::super::Foundation:: IRP, fileobject : *const super::super::Foundation:: FILE_OBJECT, thread : super::super::Foundation:: PETHREAD, priorityinfo : *mut IO_PRIORITY_INFO) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoSetDeviceToVerify(thread : super::super::Foundation:: PETHREAD, deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Ioctl", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Ioctl\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoSetFsTrackOffsetState(irp : *mut super::super::Foundation:: IRP, fstrackoffsetblob : *const super::super::super::Win32::System::Ioctl:: IO_IRP_EXT_TRACK_OFFSET_HEADER, trackedoffset : i64) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoSetInformation(fileobject : *const super::super::Foundation:: FILE_OBJECT, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, length : u32, fileinformation : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoSynchronousPageWrite(fileobject : *const super::super::Foundation:: FILE_OBJECT, memorydescriptorlist : *const super::super::Foundation:: MDL, startingoffset : *const i64, event : *const super::super::Foundation:: KEVENT, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn IoThreadToProcess(thread : super::super::Foundation:: PETHREAD) -> super::super::Foundation:: PEPROCESS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoUnregisterFileSystem(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoUnregisterFsRegistrationChange(driverobject : *const super::super::Foundation:: DRIVER_OBJECT, drivernotificationroutine : PDRIVER_FS_NOTIFICATION) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn IoVerifyVolume(deviceobject : *const super::super::Foundation:: DEVICE_OBJECT, allowrawmount : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn KeAcquireQueuedSpinLock(number : super::super::Foundation:: KSPIN_LOCK_QUEUE_NUMBER) -> u8);
::windows_targets::link!("ntoskrnl.exe" "system" fn KeAcquireSpinLockRaiseToSynch(spinlock : *mut usize) -> u8);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn KeAttachProcess(process : super::super::Foundation:: PRKPROCESS) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn KeDetachProcess() -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeInitializeMutant(mutant : *mut super::super::Foundation:: KMUTANT, initialowner : super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeInitializeQueue(queue : *mut super::super::Foundation:: KQUEUE, count : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeInsertHeadQueue(queue : *mut super::super::Foundation:: KQUEUE, entry : *mut super::super::super::Win32::System::Kernel:: LIST_ENTRY) -> i32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeInsertQueue(queue : *mut super::super::Foundation:: KQUEUE, entry : *mut super::super::super::Win32::System::Kernel:: LIST_ENTRY) -> i32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeReadStateMutant(mutant : *const super::super::Foundation:: KMUTANT) -> i32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeReadStateQueue(queue : *const super::super::Foundation:: KQUEUE) -> i32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeReleaseMutant(mutant : *mut super::super::Foundation:: KMUTANT, increment : i32, abandoned : super::super::super::Win32::Foundation:: BOOLEAN, wait : super::super::super::Win32::Foundation:: BOOLEAN) -> i32);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn KeReleaseQueuedSpinLock(number : super::super::Foundation:: KSPIN_LOCK_QUEUE_NUMBER, oldirql : u8) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeRemoveQueue(queue : *mut super::super::Foundation:: KQUEUE, waitmode : i8, timeout : *const i64) -> *mut super::super::super::Win32::System::Kernel:: LIST_ENTRY);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeRemoveQueueEx(queue : *mut super::super::Foundation:: KQUEUE, waitmode : i8, alertable : super::super::super::Win32::Foundation:: BOOLEAN, timeout : *const i64, entryarray : *mut *mut super::super::super::Win32::System::Kernel:: LIST_ENTRY, count : u32) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeRundownQueue(queue : *mut super::super::Foundation:: KQUEUE) -> *mut super::super::super::Win32::System::Kernel:: LIST_ENTRY);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn KeSetIdealProcessorThread(thread : super::super::Foundation:: PKTHREAD, processor : u8) -> u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn KeSetKernelStackSwapEnable(enable : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeStackAttachProcess(process : super::super::Foundation:: PRKPROCESS, apcstate : *mut KAPC_STATE) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn KeTryToAcquireQueuedSpinLock(number : super::super::Foundation:: KSPIN_LOCK_QUEUE_NUMBER, oldirql : *mut u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn KeUnstackDetachProcess(apcstate : *const KAPC_STATE) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("secur32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LsaFreeReturnBuffer(buffer : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("secur32.dll" "system" fn MakeSignature(phcontext : *const SecHandle, fqop : u32, pmessage : *const SecBufferDesc, messageseqno : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MapSecurityError(secstatus : ::windows_sys::core::HRESULT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn MmCanFileBeTruncated(sectionpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, newfilesize : *const i64) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn MmDoesFileHaveUserWritableReferences(sectionpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn MmFlushImageSection(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, flushtype : MMFLUSH_TYPE) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn MmForceSectionClosed(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, delayclose : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn MmForceSectionClosedEx(sectionobjectpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, forcecloseflags : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn MmGetMaximumFileSectionSize() -> u64);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn MmIsFileSectionActive(fssectionpointer : *const super::super::Foundation:: SECTION_OBJECT_POINTERS, flags : u32, sectionisactive : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MmIsRecursiveIoFault() -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn MmMdlPagesAreZero(mdl : *const super::super::Foundation:: MDL) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_FileSystem", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn MmPrefetchPages(numberoflists : u32, readlists : *const *const READ_LIST) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MmSetAddressRangeModified(address : *const ::core::ffi::c_void, length : usize) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtAccessCheckAndAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, objectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, desiredaccess : u32, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING, objectcreation : super::super::super::Win32::Foundation:: BOOLEAN, grantedaccess : *mut u32, accessstatus : *mut i32, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtAccessCheckByTypeAndAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, objectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, principalselfsid : super::super::super::Win32::Foundation:: PSID, desiredaccess : u32, audittype : super::super::super::Win32::Security:: AUDIT_EVENT_TYPE, flags : u32, objecttypelist : *const super::super::super::Win32::Security:: OBJECT_TYPE_LIST, objecttypelistlength : u32, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING, objectcreation : super::super::super::Win32::Foundation:: BOOLEAN, grantedaccess : *mut u32, accessstatus : *mut i32, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtAccessCheckByTypeResultListAndAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, objectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, principalselfsid : super::super::super::Win32::Foundation:: PSID, desiredaccess : u32, audittype : super::super::super::Win32::Security:: AUDIT_EVENT_TYPE, flags : u32, objecttypelist : *const super::super::super::Win32::Security:: OBJECT_TYPE_LIST, objecttypelistlength : u32, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING, objectcreation : super::super::super::Win32::Foundation:: BOOLEAN, grantedaccess : *mut u32, accessstatus : *mut i32, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtAccessCheckByTypeResultListAndAuditAlarmByHandle(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, clienttoken : super::super::super::Win32::Foundation:: HANDLE, objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, objectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, principalselfsid : super::super::super::Win32::Foundation:: PSID, desiredaccess : u32, audittype : super::super::super::Win32::Security:: AUDIT_EVENT_TYPE, flags : u32, objecttypelist : *const super::super::super::Win32::Security:: OBJECT_TYPE_LIST, objecttypelistlength : u32, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING, objectcreation : super::super::super::Win32::Foundation:: BOOLEAN, grantedaccess : *mut u32, accessstatus : *mut i32, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtAdjustGroupsToken(tokenhandle : super::super::super::Win32::Foundation:: HANDLE, resettodefault : super::super::super::Win32::Foundation:: BOOLEAN, newstate : *const super::super::super::Win32::Security:: TOKEN_GROUPS, bufferlength : u32, previousstate : *mut super::super::super::Win32::Security:: TOKEN_GROUPS, returnlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtAdjustPrivilegesToken(tokenhandle : super::super::super::Win32::Foundation:: HANDLE, disableallprivileges : super::super::super::Win32::Foundation:: BOOLEAN, newstate : *const super::super::super::Win32::Security:: TOKEN_PRIVILEGES, bufferlength : u32, previousstate : *mut super::super::super::Win32::Security:: TOKEN_PRIVILEGES, returnlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtAllocateVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *mut *mut ::core::ffi::c_void, zerobits : usize, regionsize : *mut usize, allocationtype : u32, protect : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtCancelIoFileEx(filehandle : super::super::super::Win32::Foundation:: HANDLE, iorequesttocancel : *const super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtCloseObjectAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, generateonclose : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_IO\"`"] fn NtCreateFile(filehandle : *mut super::super::super::Win32::Foundation:: HANDLE, desiredaccess : super::super::super::Win32::Storage::FileSystem:: FILE_ACCESS_RIGHTS, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, allocationsize : *const i64, fileattributes : super::super::super::Win32::Storage::FileSystem:: FILE_FLAGS_AND_ATTRIBUTES, shareaccess : super::super::super::Win32::Storage::FileSystem:: FILE_SHARE_MODE, createdisposition : NTCREATEFILE_CREATE_DISPOSITION, createoptions : NTCREATEFILE_CREATE_OPTIONS, eabuffer : *const ::core::ffi::c_void, ealength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn NtCreateSection(sectionhandle : *mut super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, maximumsize : *const i64, sectionpageprotection : u32, allocationattributes : u32, filehandle : super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"] fn NtCreateSectionEx(sectionhandle : *mut super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, maximumsize : *const i64, sectionpageprotection : u32, allocationattributes : u32, filehandle : super::super::super::Win32::Foundation:: HANDLE, extendedparameters : *mut super::super::super::Win32::System::Memory:: MEM_EXTENDED_PARAMETER, extendedparametercount : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtDeleteObjectAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, generateonclose : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtDuplicateToken(existingtokenhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, effectiveonly : super::super::super::Win32::Foundation:: BOOLEAN, tokentype : super::super::super::Win32::Security:: TOKEN_TYPE, newtokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtFilterToken(existingtokenhandle : super::super::super::Win32::Foundation:: HANDLE, flags : u32, sidstodisable : *const super::super::super::Win32::Security:: TOKEN_GROUPS, privilegestodelete : *const super::super::super::Win32::Security:: TOKEN_PRIVILEGES, restrictedsids : *const super::super::super::Win32::Security:: TOKEN_GROUPS, newtokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtFlushBuffersFileEx(filehandle : super::super::super::Win32::Foundation:: HANDLE, flags : u32, parameters : *const ::core::ffi::c_void, parameterssize : u32, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtFreeVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *mut *mut ::core::ffi::c_void, regionsize : *mut usize, freetype : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtFsControlFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fscontrolcode : u32, inputbuffer : *const ::core::ffi::c_void, inputbufferlength : u32, outputbuffer : *mut ::core::ffi::c_void, outputbufferlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtImpersonateAnonymousToken(threadhandle : super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtLockFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, byteoffset : *const i64, length : *const i64, key : u32, failimmediately : super::super::super::Win32::Foundation:: BOOLEAN, exclusivelock : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtOpenFile(filehandle : *mut super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, shareaccess : u32, openoptions : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtOpenObjectAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, objectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, clienttoken : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, grantedaccess : u32, privileges : *const super::super::super::Win32::Security:: PRIVILEGE_SET, objectcreation : super::super::super::Win32::Foundation:: BOOLEAN, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtOpenProcessToken(processhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, tokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtOpenProcessTokenEx(processhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, handleattributes : u32, tokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtOpenThreadToken(threadhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, openasself : super::super::super::Win32::Foundation:: BOOLEAN, tokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtOpenThreadTokenEx(threadhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, openasself : super::super::super::Win32::Foundation:: BOOLEAN, handleattributes : u32, tokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtPrivilegeCheck(clienttoken : super::super::super::Win32::Foundation:: HANDLE, requiredprivileges : *mut super::super::super::Win32::Security:: PRIVILEGE_SET, result : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtPrivilegeObjectAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, handleid : *const ::core::ffi::c_void, clienttoken : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, privileges : *const super::super::super::Win32::Security:: PRIVILEGE_SET, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtPrivilegedServiceAuditAlarm(subsystemname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, servicename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, clienttoken : super::super::super::Win32::Foundation:: HANDLE, privileges : *const super::super::super::Win32::Security:: PRIVILEGE_SET, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn NtQueryDirectoryFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, returnsingleentry : super::super::super::Win32::Foundation:: BOOLEAN, filename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, restartscan : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn NtQueryDirectoryFileEx(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, queryflags : u32, filename : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn NtQueryInformationByName(objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn NtQueryInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtQueryInformationToken(tokenhandle : super::super::super::Win32::Foundation:: HANDLE, tokeninformationclass : super::super::super::Win32::Security:: TOKEN_INFORMATION_CLASS, tokeninformation : *mut ::core::ffi::c_void, tokeninformationlength : u32, returnlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtQueryQuotaInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *mut ::core::ffi::c_void, length : u32, returnsingleentry : super::super::super::Win32::Foundation:: BOOLEAN, sidlist : *const ::core::ffi::c_void, sidlistlength : u32, startsid : super::super::super::Win32::Foundation:: PSID, restartscan : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtQuerySecurityObject(handle : super::super::super::Win32::Foundation:: HANDLE, securityinformation : u32, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, length : u32, lengthneeded : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtQueryVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *const ::core::ffi::c_void, memoryinformationclass : MEMORY_INFORMATION_CLASS, memoryinformation : *mut ::core::ffi::c_void, memoryinformationlength : usize, returnlength : *mut usize) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtQueryVolumeInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fsinformation : *mut ::core::ffi::c_void, length : u32, fsinformationclass : FS_INFORMATION_CLASS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtReadFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *mut ::core::ffi::c_void, length : u32, byteoffset : *const i64, key : *const u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn NtSetInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *const ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtSetInformationToken(tokenhandle : super::super::super::Win32::Foundation:: HANDLE, tokeninformationclass : super::super::super::Win32::Security:: TOKEN_INFORMATION_CLASS, tokeninformation : *const ::core::ffi::c_void, tokeninformationlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NtSetInformationVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, vminformationclass : VIRTUAL_MEMORY_INFORMATION_CLASS, numberofentries : usize, virtualaddresses : *const MEMORY_RANGE_ENTRY, vminformation : *const ::core::ffi::c_void, vminformationlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtSetQuotaInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *const ::core::ffi::c_void, length : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NtSetSecurityObject(handle : super::super::super::Win32::Foundation:: HANDLE, securityinformation : u32, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtSetVolumeInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fsinformation : *const ::core::ffi::c_void, length : u32, fsinformationclass : FS_INFORMATION_CLASS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtUnlockFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, byteoffset : *const i64, length : *const i64, key : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn NtWriteFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *const ::core::ffi::c_void, length : u32, byteoffset : *const i64, key : *const u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ObInsertObject(object : *const ::core::ffi::c_void, passedaccessstate : *mut super::super::Foundation:: ACCESS_STATE, desiredaccess : u32, objectpointerbias : u32, newobject : *mut *mut ::core::ffi::c_void, handle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ObIsKernelHandle(handle : super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn ObMakeTemporaryObject(object : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ObOpenObjectByPointer(object : *const ::core::ffi::c_void, handleattributes : u32, passedaccessstate : *const super::super::Foundation:: ACCESS_STATE, desiredaccess : u32, objecttype : super::super::Foundation:: POBJECT_TYPE, accessmode : i8, handle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ObOpenObjectByPointerWithTag(object : *const ::core::ffi::c_void, handleattributes : u32, passedaccessstate : *const super::super::Foundation:: ACCESS_STATE, desiredaccess : u32, objecttype : super::super::Foundation:: POBJECT_TYPE, accessmode : i8, tag : u32, handle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn ObQueryNameString(object : *const ::core::ffi::c_void, objectnameinfo : *mut super::super::Foundation:: OBJECT_NAME_INFORMATION, length : u32, returnlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ObQueryObjectAuditingByHandle(handle : super::super::super::Win32::Foundation:: HANDLE, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn PfxFindPrefix(prefixtable : *const PREFIX_TABLE, fullname : *const super::super::super::Win32::System::Kernel:: STRING) -> *mut PREFIX_TABLE_ENTRY);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn PfxInitialize(prefixtable : *mut PREFIX_TABLE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn PfxInsertPrefix(prefixtable : *const PREFIX_TABLE, prefix : *const super::super::super::Win32::System::Kernel:: STRING, prefixtableentry : *mut PREFIX_TABLE_ENTRY) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn PfxRemovePrefix(prefixtable : *const PREFIX_TABLE, prefixtableentry : *const PREFIX_TABLE_ENTRY) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn PoQueueShutdownWorkItem(workitem : *mut super::super::Foundation:: WORK_QUEUE_ITEM) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn PsAssignImpersonationToken(thread : super::super::Foundation:: PETHREAD, token : super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn PsChargePoolQuota(process : super::super::Foundation:: PEPROCESS, pooltype : super::super::Foundation:: POOL_TYPE, amount : usize) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn PsChargeProcessPoolQuota(process : super::super::Foundation:: PEPROCESS, pooltype : super::super::Foundation:: POOL_TYPE, amount : usize) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntoskrnl.exe" "system" fn PsDereferenceImpersonationToken(impersonationtoken : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn PsDereferencePrimaryToken(primarytoken : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn PsDisableImpersonation(thread : super::super::Foundation:: PETHREAD, impersonationstate : *mut super::super::super::Win32::Security:: SE_IMPERSONATION_STATE) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntoskrnl.exe" "system" fn PsGetProcessExitTime() -> i64);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn PsGetThreadProcess(thread : super::super::Foundation:: PETHREAD) -> super::super::Foundation:: PEPROCESS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn PsImpersonateClient(thread : super::super::Foundation:: PETHREAD, token : *const ::core::ffi::c_void, copyonopen : super::super::super::Win32::Foundation:: BOOLEAN, effectiveonly : super::super::super::Win32::Foundation:: BOOLEAN, impersonationlevel : super::super::super::Win32::Security:: SECURITY_IMPERSONATION_LEVEL) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PsIsDiskCountersEnabled() -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn PsIsSystemThread(thread : super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn PsIsThreadTerminating(thread : super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn PsLookupProcessByProcessId(processid : super::super::super::Win32::Foundation:: HANDLE, process : *mut super::super::Foundation:: PEPROCESS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn PsLookupThreadByThreadId(threadid : super::super::super::Win32::Foundation:: HANDLE, thread : *mut super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn PsReferenceImpersonationToken(thread : super::super::Foundation:: PETHREAD, copyonopen : *mut super::super::super::Win32::Foundation:: BOOLEAN, effectiveonly : *mut super::super::super::Win32::Foundation:: BOOLEAN, impersonationlevel : *mut super::super::super::Win32::Security:: SECURITY_IMPERSONATION_LEVEL) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn PsReferencePrimaryToken(process : super::super::Foundation:: PEPROCESS) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn PsRestoreImpersonation(thread : super::super::Foundation:: PETHREAD, impersonationstate : *const super::super::super::Win32::Security:: SE_IMPERSONATION_STATE) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn PsReturnPoolQuota(process : super::super::Foundation:: PEPROCESS, pooltype : super::super::Foundation:: POOL_TYPE, amount : usize) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn PsRevertToSelf() -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn PsUpdateDiskCounters(process : super::super::Foundation:: PEPROCESS, bytesread : u64, byteswritten : u64, readoperationcount : u32, writeoperationcount : u32, flushoperationcount : u32) -> ());
::windows_targets::link!("secur32.dll" "system" fn QuerySecurityContextToken(phcontext : *const SecHandle, token : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlAbsoluteToSelfRelativeSD(absolutesecuritydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, selfrelativesecuritydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, bufferlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlAddAccessAllowedAce(acl : *mut super::super::super::Win32::Security:: ACL, acerevision : u32, accessmask : u32, sid : super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlAddAccessAllowedAceEx(acl : *mut super::super::super::Win32::Security:: ACL, acerevision : u32, aceflags : u32, accessmask : u32, sid : super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlAddAce(acl : *mut super::super::super::Win32::Security:: ACL, acerevision : u32, startingaceindex : u32, acelist : *const ::core::ffi::c_void, acelistlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlAllocateAndInitializeSid(identifierauthority : *const super::super::super::Win32::Security:: SID_IDENTIFIER_AUTHORITY, subauthoritycount : u8, subauthority0 : u32, subauthority1 : u32, subauthority2 : u32, subauthority3 : u32, subauthority4 : u32, subauthority5 : u32, subauthority6 : u32, subauthority7 : u32, sid : *mut super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlAllocateAndInitializeSidEx(identifierauthority : *const super::super::super::Win32::Security:: SID_IDENTIFIER_AUTHORITY, subauthoritycount : u8, subauthorities : *const u32, sid : *mut super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntdll.dll" "system" fn RtlAllocateHeap(heaphandle : *const ::core::ffi::c_void, flags : u32, size : usize) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlAppendStringToString(destination : *mut super::super::super::Win32::System::Kernel:: STRING, source : *const super::super::super::Win32::System::Kernel:: STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCompareAltitudes(altitude1 : *const super::super::super::Win32::Foundation:: UNICODE_STRING, altitude2 : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> i32);
::windows_targets::link!("ntdll.dll" "system" fn RtlCompareMemoryUlong(source : *const ::core::ffi::c_void, length : usize, pattern : u32) -> usize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCompressBuffer(compressionformatandengine : u16, uncompressedbuffer : *const u8, uncompressedbuffersize : u32, compressedbuffer : *mut u8, compressedbuffersize : u32, uncompressedchunksize : u32, finalcompressedsize : *mut u32, workspace : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCompressChunks(uncompressedbuffer : *const u8, uncompressedbuffersize : u32, compressedbuffer : *mut u8, compressedbuffersize : u32, compresseddatainfo : *mut COMPRESSED_DATA_INFO, compresseddatainfolength : u32, workspace : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlConvertSidToUnicodeString(unicodestring : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, sid : super::super::super::Win32::Foundation:: PSID, allocatedestinationstring : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCopyLuid(destinationluid : *mut super::super::super::Win32::Foundation:: LUID, sourceluid : *const super::super::super::Win32::Foundation:: LUID) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCopySid(destinationsidlength : u32, destinationsid : super::super::super::Win32::Foundation:: PSID, sourcesid : super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlCreateAcl(acl : *mut super::super::super::Win32::Security:: ACL, acllength : u32, aclrevision : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCreateHeap(flags : u32, heapbase : *const ::core::ffi::c_void, reservesize : usize, commitsize : usize, lock : *const ::core::ffi::c_void, parameters : *const RTL_HEAP_PARAMETERS) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCreateServiceSid(servicename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, servicesid : super::super::super::Win32::Foundation:: PSID, servicesidlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCreateSystemVolumeInformationFolder(volumerootpath : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCreateUnicodeString(destinationstring : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, sourcestring : ::windows_sys::core::PCWSTR) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCreateVirtualAccountSid(name : *const super::super::super::Win32::Foundation:: UNICODE_STRING, basesubauthority : u32, sid : super::super::super::Win32::Foundation:: PSID, sidlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlCustomCPToUnicodeN(customcp : *const CPTABLEINFO, unicodestring : ::windows_sys::core::PWSTR, maxbytesinunicodestring : u32, bytesinunicodestring : *mut u32, customcpstring : ::windows_sys::core::PCSTR, bytesincustomcpstring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDecompressBuffer(compressionformat : u16, uncompressedbuffer : *mut u8, uncompressedbuffersize : u32, compressedbuffer : *const u8, compressedbuffersize : u32, finaluncompressedsize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDecompressBufferEx(compressionformat : u16, uncompressedbuffer : *mut u8, uncompressedbuffersize : u32, compressedbuffer : *const u8, compressedbuffersize : u32, finaluncompressedsize : *mut u32, workspace : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDecompressBufferEx2(compressionformat : u16, uncompressedbuffer : *mut u8, uncompressedbuffersize : u32, compressedbuffer : *const u8, compressedbuffersize : u32, uncompressedchunksize : u32, finaluncompressedsize : *mut u32, workspace : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDecompressChunks(uncompressedbuffer : *mut u8, uncompressedbuffersize : u32, compressedbuffer : *const u8, compressedbuffersize : u32, compressedtail : *const u8, compressedtailsize : u32, compresseddatainfo : *const COMPRESSED_DATA_INFO) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDecompressFragment(compressionformat : u16, uncompressedfragment : *mut u8, uncompressedfragmentsize : u32, compressedbuffer : *const u8, compressedbuffersize : u32, fragmentoffset : u32, finaluncompressedsize : *mut u32, workspace : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDecompressFragmentEx(compressionformat : u16, uncompressedfragment : *mut u8, uncompressedfragmentsize : u32, compressedbuffer : *const u8, compressedbuffersize : u32, fragmentoffset : u32, uncompressedchunksize : u32, finaluncompressedsize : *mut u32, workspace : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlDeleteAce(acl : *mut super::super::super::Win32::Security:: ACL, aceindex : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDescribeChunk(compressionformat : u16, compressedbuffer : *mut *mut u8, endofcompressedbufferplus1 : *const u8, chunkbuffer : *mut *mut u8, chunksize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntdll.dll" "system" fn RtlDestroyHeap(heaphandle : *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDowncaseUnicodeString(destinationstring : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, sourcestring : *const super::super::super::Win32::Foundation:: UNICODE_STRING, allocatedestinationstring : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDuplicateUnicodeString(flags : u32, stringin : *const super::super::super::Win32::Foundation:: UNICODE_STRING, stringout : *mut super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlEqualPrefixSid(sid1 : super::super::super::Win32::Foundation:: PSID, sid2 : super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlEqualSid(sid1 : super::super::super::Win32::Foundation:: PSID, sid2 : super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn RtlFindUnicodePrefix(prefixtable : *const UNICODE_PREFIX_TABLE, fullname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, caseinsensitiveindex : u32) -> *mut UNICODE_PREFIX_TABLE_ENTRY);
::windows_targets::link!("ntdll.dll" "system" fn RtlFreeHeap(heaphandle : *const ::core::ffi::c_void, flags : u32, baseaddress : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlFreeSid(sid : super::super::super::Win32::Foundation:: PSID) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlGenerate8dot3Name(name : *const super::super::super::Win32::Foundation:: UNICODE_STRING, allowextendedcharacters : super::super::super::Win32::Foundation:: BOOLEAN, context : *mut GENERATE_NAME_CONTEXT, name8dot3 : *mut super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlGetAce(acl : *const super::super::super::Win32::Security:: ACL, aceindex : u32, ace : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlGetCompressionWorkSpaceSize(compressionformatandengine : u16, compressbufferworkspacesize : *mut u32, compressfragmentworkspacesize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlGetDaclSecurityDescriptor(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, daclpresent : *mut super::super::super::Win32::Foundation:: BOOLEAN, dacl : *mut *mut super::super::super::Win32::Security:: ACL, dacldefaulted : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlGetGroupSecurityDescriptor(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, group : *mut super::super::super::Win32::Foundation:: PSID, groupdefaulted : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlGetOwnerSecurityDescriptor(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, owner : *mut super::super::super::Win32::Foundation:: PSID, ownerdefaulted : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlGetSaclSecurityDescriptor(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, saclpresent : *mut super::super::super::Win32::Foundation:: BOOLEAN, sacl : *mut *mut super::super::super::Win32::Security:: ACL, sacldefaulted : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlIdentifierAuthoritySid(sid : super::super::super::Win32::Foundation:: PSID) -> *mut super::super::super::Win32::Security:: SID_IDENTIFIER_AUTHORITY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIdnToAscii(flags : u32, sourcestring : ::windows_sys::core::PCWSTR, sourcestringlength : i32, destinationstring : ::windows_sys::core::PWSTR, destinationstringlength : *mut i32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIdnToNameprepUnicode(flags : u32, sourcestring : ::windows_sys::core::PCWSTR, sourcestringlength : i32, destinationstring : ::windows_sys::core::PWSTR, destinationstringlength : *mut i32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIdnToUnicode(flags : u32, sourcestring : ::windows_sys::core::PCWSTR, sourcestringlength : i32, destinationstring : ::windows_sys::core::PWSTR, destinationstringlength : *mut i32) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntdll.dll" "system" fn RtlInitCodePageTable(tablebase : *const u16, codepagetable : *mut CPTABLEINFO) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlInitUnicodeStringEx(destinationstring : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, sourcestring : ::windows_sys::core::PCWSTR) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlInitializeSid(sid : super::super::super::Win32::Foundation:: PSID, identifierauthority : *const super::super::super::Win32::Security:: SID_IDENTIFIER_AUTHORITY, subauthoritycount : u8) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlInitializeSidEx(sid : super::super::super::Win32::Foundation:: PSID, identifierauthority : *const super::super::super::Win32::Security:: SID_IDENTIFIER_AUTHORITY, subauthoritycount : u8, ...) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn RtlInitializeUnicodePrefix(prefixtable : *mut UNICODE_PREFIX_TABLE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn RtlInsertUnicodePrefix(prefixtable : *const UNICODE_PREFIX_TABLE, prefix : *const super::super::super::Win32::Foundation:: UNICODE_STRING, prefixtableentry : *mut UNICODE_PREFIX_TABLE_ENTRY) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIsCloudFilesPlaceholder(fileattributes : u32, reparsetag : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIsNonEmptyDirectoryReparsePointAllowed(reparsetag : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIsNormalizedString(normform : u32, sourcestring : ::windows_sys::core::PCWSTR, sourcestringlength : i32, normalized : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIsPartialPlaceholder(fileattributes : u32, reparsetag : u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIsPartialPlaceholderFileHandle(filehandle : super::super::super::Win32::Foundation:: HANDLE, ispartialplaceholder : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_WindowsProgramming\"`"] fn RtlIsPartialPlaceholderFileInfo(infobuffer : *const ::core::ffi::c_void, infoclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, ispartialplaceholder : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlIsSandboxedToken(context : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, previousmode : i8) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlIsValidOemCharacter(char : ::windows_sys::core::PWSTR) -> super::super::super::Win32::Foundation:: BOOLEAN);
::windows_targets::link!("ntdll.dll" "system" fn RtlLengthRequiredSid(subauthoritycount : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlLengthSid(sid : super::super::super::Win32::Foundation:: PSID) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlMultiByteToUnicodeN(unicodestring : ::windows_sys::core::PWSTR, maxbytesinunicodestring : u32, bytesinunicodestring : *mut u32, multibytestring : ::windows_sys::core::PCSTR, bytesinmultibytestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlMultiByteToUnicodeSize(bytesinunicodestring : *mut u32, multibytestring : ::windows_sys::core::PCSTR, bytesinmultibytestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn RtlNextUnicodePrefix(prefixtable : *const UNICODE_PREFIX_TABLE, restart : super::super::super::Win32::Foundation:: BOOLEAN) -> *mut UNICODE_PREFIX_TABLE_ENTRY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlNormalizeString(normform : u32, sourcestring : ::windows_sys::core::PCWSTR, sourcestringlength : i32, destinationstring : ::windows_sys::core::PWSTR, destinationstringlength : *mut i32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlNtStatusToDosErrorNoTeb(status : super::super::super::Win32::Foundation:: NTSTATUS) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlOemStringToCountedUnicodeString(destinationstring : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, sourcestring : *const super::super::super::Win32::System::Kernel:: STRING, allocatedestinationstring : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlOemStringToUnicodeString(destinationstring : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, sourcestring : *const super::super::super::Win32::System::Kernel:: STRING, allocatedestinationstring : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlOemToUnicodeN(unicodestring : ::windows_sys::core::PWSTR, maxbytesinunicodestring : u32, bytesinunicodestring : *mut u32, oemstring : ::windows_sys::core::PCSTR, bytesinoemstring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlPrefixString(string1 : *const super::super::super::Win32::System::Kernel:: STRING, string2 : *const super::super::super::Win32::System::Kernel:: STRING, caseinsensitive : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlQueryPackageIdentity(tokenobject : *const ::core::ffi::c_void, packagefullname : ::windows_sys::core::PWSTR, packagesize : *mut usize, appid : ::windows_sys::core::PWSTR, appidsize : *mut usize, packaged : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlQueryPackageIdentityEx(tokenobject : *const ::core::ffi::c_void, packagefullname : ::windows_sys::core::PWSTR, packagesize : *mut usize, appid : ::windows_sys::core::PWSTR, appidsize : *mut usize, dynamicid : *mut ::windows_sys::core::GUID, flags : *mut u64) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntdll.dll" "system" fn RtlQueryProcessPlaceholderCompatibilityMode() -> u8);
::windows_targets::link!("ntdll.dll" "system" fn RtlQueryThreadPlaceholderCompatibilityMode() -> u8);
::windows_targets::link!("ntdll.dll" "system" fn RtlRandom(seed : *mut u32) -> u32);
::windows_targets::link!("ntdll.dll" "system" fn RtlRandomEx(seed : *mut u32) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn RtlRemoveUnicodePrefix(prefixtable : *const UNICODE_PREFIX_TABLE, prefixtableentry : *const UNICODE_PREFIX_TABLE_ENTRY) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlReplaceSidInSd(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, oldsid : super::super::super::Win32::Foundation:: PSID, newsid : super::super::super::Win32::Foundation:: PSID, numchanges : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlReserveChunk(compressionformat : u16, compressedbuffer : *mut *mut u8, endofcompressedbufferplus1 : *const u8, chunkbuffer : *mut *mut u8, chunksize : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntdll.dll" "system" fn RtlSecondsSince1970ToTime(elapsedseconds : u32, time : *mut i64) -> ());
::windows_targets::link!("ntdll.dll" "system" fn RtlSecondsSince1980ToTime(elapsedseconds : u32, time : *mut i64) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlSelfRelativeToAbsoluteSD(selfrelativesecuritydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, absolutesecuritydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, absolutesecuritydescriptorsize : *mut u32, dacl : *mut super::super::super::Win32::Security:: ACL, daclsize : *mut u32, sacl : *mut super::super::super::Win32::Security:: ACL, saclsize : *mut u32, owner : super::super::super::Win32::Foundation:: PSID, ownersize : *mut u32, primarygroup : super::super::super::Win32::Foundation:: PSID, primarygroupsize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlSetGroupSecurityDescriptor(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, group : super::super::super::Win32::Foundation:: PSID, groupdefaulted : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn RtlSetOwnerSecurityDescriptor(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, owner : super::super::super::Win32::Foundation:: PSID, ownerdefaulted : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("ntdll.dll" "system" fn RtlSetProcessPlaceholderCompatibilityMode(mode : u8) -> u8);
::windows_targets::link!("ntdll.dll" "system" fn RtlSetThreadPlaceholderCompatibilityMode(mode : u8) -> u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlSubAuthorityCountSid(sid : super::super::super::Win32::Foundation:: PSID) -> *mut u8);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlSubAuthoritySid(sid : super::super::super::Win32::Foundation:: PSID, subauthority : u32) -> *mut u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlTimeToSecondsSince1980(time : *const i64, elapsedseconds : *mut u32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlUnicodeStringToCountedOemString(destinationstring : *mut super::super::super::Win32::System::Kernel:: STRING, sourcestring : *const super::super::super::Win32::Foundation:: UNICODE_STRING, allocatedestinationstring : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlUnicodeToCustomCPN(customcp : *const CPTABLEINFO, customcpstring : ::windows_sys::core::PSTR, maxbytesincustomcpstring : u32, bytesincustomcpstring : *mut u32, unicodestring : ::windows_sys::core::PCWSTR, bytesinunicodestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlUnicodeToMultiByteN(multibytestring : ::windows_sys::core::PSTR, maxbytesinmultibytestring : u32, bytesinmultibytestring : *mut u32, unicodestring : *const u16, bytesinunicodestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlUnicodeToOemN(oemstring : ::windows_sys::core::PSTR, maxbytesinoemstring : u32, bytesinoemstring : *mut u32, unicodestring : *const u16, bytesinunicodestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlUpcaseUnicodeStringToCountedOemString(destinationstring : *mut super::super::super::Win32::System::Kernel:: STRING, sourcestring : *const super::super::super::Win32::Foundation:: UNICODE_STRING, allocatedestinationstring : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlUpcaseUnicodeStringToOemString(destinationstring : *mut super::super::super::Win32::System::Kernel:: STRING, sourcestring : *const super::super::super::Win32::Foundation:: UNICODE_STRING, allocatedestinationstring : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlUpcaseUnicodeToCustomCPN(customcp : *const CPTABLEINFO, customcpstring : ::windows_sys::core::PSTR, maxbytesincustomcpstring : u32, bytesincustomcpstring : *mut u32, unicodestring : ::windows_sys::core::PCWSTR, bytesinunicodestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlUpcaseUnicodeToMultiByteN(multibytestring : ::windows_sys::core::PSTR, maxbytesinmultibytestring : u32, bytesinmultibytestring : *mut u32, unicodestring : *const u16, bytesinunicodestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlUpcaseUnicodeToOemN(oemstring : ::windows_sys::core::PSTR, maxbytesinoemstring : u32, bytesinoemstring : *mut u32, unicodestring : *const u16, bytesinunicodestring : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlValidSid(sid : super::super::super::Win32::Foundation:: PSID) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlValidateUnicodeString(flags : u32, string : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_System_Kernel")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_System_Kernel\"`"] fn RtlxOemStringToUnicodeSize(oemstring : *const super::super::super::Win32::System::Kernel:: STRING) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlxUnicodeStringToOemSize(unicodestring : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAccessCheckFromState(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, primarytokeninformation : *const super::super::super::Win32::Security:: TOKEN_ACCESS_INFORMATION, clienttokeninformation : *const super::super::super::Win32::Security:: TOKEN_ACCESS_INFORMATION, desiredaccess : u32, previouslygrantedaccess : u32, privileges : *mut *mut super::super::super::Win32::Security:: PRIVILEGE_SET, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING, accessmode : i8, grantedaccess : *mut u32, accessstatus : *mut i32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAccessCheckFromStateEx(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, primarytoken : *const ::core::ffi::c_void, clienttoken : *const ::core::ffi::c_void, desiredaccess : u32, previouslygrantedaccess : u32, privileges : *mut *mut super::super::super::Win32::Security:: PRIVILEGE_SET, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING, accessmode : i8, grantedaccess : *mut u32, accessstatus : *mut i32) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAdjustAccessStateForAccessConstraints(objecttype : *const ::core::ffi::c_void, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, accessstate : *mut super::super::Foundation:: ACCESS_STATE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAdjustAccessStateForTrustLabel(objecttype : *const ::core::ffi::c_void, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, accessstate : *mut super::super::Foundation:: ACCESS_STATE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAdjustObjectSecurity(objectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, originaldescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, proposeddescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, adjusteddescriptor : *mut super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, applyadjusteddescriptor : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAppendPrivileges(accessstate : *mut super::super::Foundation:: ACCESS_STATE, privileges : *const super::super::super::Win32::Security:: PRIVILEGE_SET) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeAuditFipsCryptoSelftests(bsuccess : super::super::super::Win32::Foundation:: BOOLEAN, selftestcode : u32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeAuditHardLinkCreation(filename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, linkname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, bsuccess : super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeAuditHardLinkCreationWithTransaction(filename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, linkname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, bsuccess : super::super::super::Win32::Foundation:: BOOLEAN, transactionid : *const ::windows_sys::core::GUID) -> ());
::windows_targets::link!("ntoskrnl.exe" "system" fn SeAuditTransactionStateChange(transactionid : *const ::windows_sys::core::GUID, resourcemanagerid : *const ::windows_sys::core::GUID, newtransactionstate : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingAnyFileEventsWithContext(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingAnyFileEventsWithContextEx(securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, stagingenabled : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingFileEvents(accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingFileEventsWithContext(accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingFileEventsWithContextEx(accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, stagingenabled : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingFileOrGlobalEvents(accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingHardLinkEvents(accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeAuditingHardLinkEventsWithContext(accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Security\"`"] fn SeCaptureSubjectContextEx(thread : super::super::Foundation:: PETHREAD, process : super::super::Foundation:: PEPROCESS, subjectcontext : *mut super::super::Foundation:: SECURITY_SUBJECT_CONTEXT) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeCheckForCriticalAceRemoval(currentdescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, newdescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, aceremoved : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeCreateClientSecurity(clientthread : super::super::Foundation:: PETHREAD, clientsecurityqos : *const super::super::super::Win32::Security:: SECURITY_QUALITY_OF_SERVICE, remotesession : super::super::super::Win32::Foundation:: BOOLEAN, clientcontext : *mut SECURITY_CLIENT_CONTEXT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeCreateClientSecurityFromSubjectContext(subjectcontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, clientsecurityqos : *const super::super::super::Win32::Security:: SECURITY_QUALITY_OF_SERVICE, serverisremote : super::super::super::Win32::Foundation:: BOOLEAN, clientcontext : *mut SECURITY_CLIENT_CONTEXT) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeDeleteClientSecurity(clientcontext : *mut SECURITY_CLIENT_CONTEXT) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeDeleteObjectAuditAlarm(object : *const ::core::ffi::c_void, handle : super::super::super::Win32::Foundation:: HANDLE) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeDeleteObjectAuditAlarmWithTransaction(object : *const ::core::ffi::c_void, handle : super::super::super::Win32::Foundation:: HANDLE, transactionid : *const ::windows_sys::core::GUID) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeExamineSacl(sacl : *const super::super::super::Win32::Security:: ACL, resourcesacl : *const super::super::super::Win32::Security:: ACL, token : *const ::core::ffi::c_void, desiredaccess : u32, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, generateaudit : *mut super::super::super::Win32::Foundation:: BOOLEAN, generatealarm : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeFilterToken(existingtoken : *const ::core::ffi::c_void, flags : u32, sidstodisable : *const super::super::super::Win32::Security:: TOKEN_GROUPS, privilegestodelete : *const super::super::super::Win32::Security:: TOKEN_PRIVILEGES, restrictedsids : *const super::super::super::Win32::Security:: TOKEN_GROUPS, filteredtoken : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeFreePrivileges(privileges : *const super::super::super::Win32::Security:: PRIVILEGE_SET) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeImpersonateClient(clientcontext : *const SECURITY_CLIENT_CONTEXT, serverthread : super::super::Foundation:: PETHREAD) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeImpersonateClientEx(clientcontext : *const SECURITY_CLIENT_CONTEXT, serverthread : super::super::Foundation:: PETHREAD) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn SeLocateProcessImageName(process : super::super::Foundation:: PEPROCESS, pimagefilename : *mut *mut super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeMarkLogonSessionForTerminationNotification(logonid : *const super::super::super::Win32::Foundation:: LUID) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn SeMarkLogonSessionForTerminationNotificationEx(logonid : *const super::super::super::Win32::Foundation:: LUID, pserversilo : super::super::Foundation:: PESILO) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeOpenObjectAuditAlarm(objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, object : *const ::core::ffi::c_void, absoluteobjectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, accessstate : *const super::super::Foundation:: ACCESS_STATE, objectcreated : super::super::super::Win32::Foundation:: BOOLEAN, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, accessmode : i8, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeOpenObjectAuditAlarmWithTransaction(objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, object : *const ::core::ffi::c_void, absoluteobjectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, accessstate : *const super::super::Foundation:: ACCESS_STATE, objectcreated : super::super::super::Win32::Foundation:: BOOLEAN, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, accessmode : i8, transactionid : *const ::windows_sys::core::GUID, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeOpenObjectForDeleteAuditAlarm(objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, object : *const ::core::ffi::c_void, absoluteobjectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, accessstate : *const super::super::Foundation:: ACCESS_STATE, objectcreated : super::super::super::Win32::Foundation:: BOOLEAN, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, accessmode : i8, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeOpenObjectForDeleteAuditAlarmWithTransaction(objecttypename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, object : *const ::core::ffi::c_void, absoluteobjectname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, accessstate : *const super::super::Foundation:: ACCESS_STATE, objectcreated : super::super::super::Win32::Foundation:: BOOLEAN, accessgranted : super::super::super::Win32::Foundation:: BOOLEAN, accessmode : i8, transactionid : *const ::windows_sys::core::GUID, generateonclose : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SePrivilegeCheck(requiredprivileges : *mut super::super::super::Win32::Security:: PRIVILEGE_SET, subjectsecuritycontext : *const super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, accessmode : i8) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeQueryAuthenticationIdToken(token : *const ::core::ffi::c_void, authenticationid : *mut super::super::super::Win32::Foundation:: LUID) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeQueryInformationToken(token : *const ::core::ffi::c_void, tokeninformationclass : super::super::super::Win32::Security:: TOKEN_INFORMATION_CLASS, tokeninformation : *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeQuerySecurityDescriptorInfo(securityinformation : *const u32, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, length : *mut u32, objectssecuritydescriptor : *mut super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn SeQueryServerSiloToken(token : *const ::core::ffi::c_void, pserversilo : *mut super::super::Foundation:: PESILO) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeQuerySessionIdToken(token : *const ::core::ffi::c_void, sessionid : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeQuerySessionIdTokenEx(token : *const ::core::ffi::c_void, sessionid : *mut u32, isservicesession : *mut super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeRegisterLogonSessionTerminatedRoutine(callbackroutine : PSE_LOGON_SESSION_TERMINATED_ROUTINE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeRegisterLogonSessionTerminatedRoutineEx(callbackroutine : PSE_LOGON_SESSION_TERMINATED_ROUTINE_EX, context : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"] fn SeReportSecurityEventWithSubCategory(flags : u32, sourcename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, usersid : super::super::super::Win32::Foundation:: PSID, auditparameters : *const super::super::super::Win32::Security::Authentication::Identity:: SE_ADT_PARAMETER_ARRAY, auditsubcategoryid : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeSetAccessStateGenericMapping(accessstate : *mut super::super::Foundation:: ACCESS_STATE, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeSetSecurityDescriptorInfo(object : *const ::core::ffi::c_void, securityinformation : *const u32, modificationdescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, objectssecuritydescriptor : *mut super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, pooltype : super::super::Foundation:: POOL_TYPE, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeSetSecurityDescriptorInfoEx(object : *const ::core::ffi::c_void, securityinformation : *const u32, modificationdescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, objectssecuritydescriptor : *mut super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, autoinheritflags : u32, pooltype : super::super::Foundation:: POOL_TYPE, genericmapping : *const super::super::super::Win32::Security:: GENERIC_MAPPING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeShouldCheckForAccessRightsFromParent(objecttype : *const ::core::ffi::c_void, childdescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, accessstate : *const super::super::Foundation:: ACCESS_STATE) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SeTokenFromAccessInformation(accessinformation : *const super::super::super::Win32::Security:: TOKEN_ACCESS_INFORMATION, token : *mut ::core::ffi::c_void, length : u32, requiredlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeTokenIsAdmin(token : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeTokenIsRestricted(token : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeTokenIsWriteRestricted(token : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn SeTokenType(token : *const ::core::ffi::c_void) -> super::super::super::Win32::Security:: TOKEN_TYPE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeUnregisterLogonSessionTerminatedRoutine(callbackroutine : PSE_LOGON_SESSION_TERMINATED_ROUTINE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntoskrnl.exe" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SeUnregisterLogonSessionTerminatedRoutineEx(callbackroutine : PSE_LOGON_SESSION_TERMINATED_ROUTINE_EX, context : *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SecLookupAccountName(name : *const super::super::super::Win32::Foundation:: UNICODE_STRING, sidsize : *mut u32, sid : super::super::super::Win32::Foundation:: PSID, nameuse : *mut super::super::super::Win32::Security:: SID_NAME_USE, domainsize : *mut u32, referenceddomain : *mut super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SecLookupAccountSid(sid : super::super::super::Win32::Foundation:: PSID, namesize : *mut u32, namebuffer : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, domainsize : *mut u32, domainbuffer : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, nameuse : *mut super::super::super::Win32::Security:: SID_NAME_USE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn SecLookupWellKnownSid(sidtype : super::super::super::Win32::Security:: WELL_KNOWN_SID_TYPE, sid : super::super::super::Win32::Foundation:: PSID, sidbuffersize : u32, sidsize : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SecMakeSPN(serviceclass : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, servicename : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, instancename : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, instanceport : u16, referrer : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, spn : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, length : *mut u32, allocate : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SecMakeSPNEx(serviceclass : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, servicename : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, instancename : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, instanceport : u16, referrer : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, targetinfo : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, spn : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, length : *mut u32, allocate : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SecMakeSPNEx2(serviceclass : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, servicename : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, instancename : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, instanceport : u16, referrer : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, intargetinfo : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, spn : *mut super::super::super::Win32::Foundation:: UNICODE_STRING, totalsize : *mut u32, allocate : super::super::super::Win32::Foundation:: BOOLEAN, istargetinfomarshaled : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("secur32.dll" "system" fn SetContextAttributesW(phcontext : *const SecHandle, ulattribute : u32, pbuffer : *const ::core::ffi::c_void, cbbuffer : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiAcceptSecurityContextAsync(asynccontext : *mut super::super::Foundation:: SspiAsyncContext, phcredential : *const SecHandle, phcontext : *const SecHandle, pinput : *const SecBufferDesc, fcontextreq : u32, targetdatarep : u32, phnewcontext : *const SecHandle, poutput : *const SecBufferDesc, pfcontextattr : *const u32, ptsexpiry : *const i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Security_Authentication_Identity"))]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"] fn SspiAcquireCredentialsHandleAsyncA(asynccontext : *mut super::super::Foundation:: SspiAsyncContext, pszprincipal : ::windows_sys::core::PCSTR, pszpackage : ::windows_sys::core::PCSTR, fcredentialuse : u32, pvlogonid : *const ::core::ffi::c_void, pauthdata : *const ::core::ffi::c_void, pgetkeyfn : super::super::super::Win32::Security::Authentication::Identity:: SEC_GET_KEY_FN, pvgetkeyargument : *const ::core::ffi::c_void, phcredential : *const SecHandle, ptsexpiry : *const i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"] fn SspiAcquireCredentialsHandleAsyncW(asynccontext : *mut super::super::Foundation:: SspiAsyncContext, pszprincipal : *const super::super::super::Win32::Foundation:: UNICODE_STRING, pszpackage : *const super::super::super::Win32::Foundation:: UNICODE_STRING, fcredentialuse : u32, pvlogonid : *const ::core::ffi::c_void, pauthdata : *const ::core::ffi::c_void, pgetkeyfn : super::super::super::Win32::Security::Authentication::Identity:: SEC_GET_KEY_FN, pvgetkeyargument : *const ::core::ffi::c_void, phcredential : *const SecHandle, ptsexpiry : *const i64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiCreateAsyncContext() -> *mut super::super::Foundation:: SspiAsyncContext);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiDeleteSecurityContextAsync(asynccontext : *mut super::super::Foundation:: SspiAsyncContext, phcontext : *const SecHandle) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiFreeAsyncContext(handle : *const super::super::Foundation:: SspiAsyncContext) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiFreeCredentialsHandleAsync(asynccontext : *mut super::super::Foundation:: SspiAsyncContext, phcredential : *const SecHandle) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiGetAsyncCallStatus(handle : *const super::super::Foundation:: SspiAsyncContext) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiInitializeSecurityContextAsyncA(asynccontext : *mut super::super::Foundation:: SspiAsyncContext, phcredential : *const SecHandle, phcontext : *const SecHandle, psztargetname : ::windows_sys::core::PCSTR, fcontextreq : u32, reserved1 : u32, targetdatarep : u32, pinput : *const SecBufferDesc, reserved2 : u32, phnewcontext : *const SecHandle, poutput : *const SecBufferDesc, pfcontextattr : *const u32, ptsexpiry : *const i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn SspiInitializeSecurityContextAsyncW(asynccontext : *mut super::super::Foundation:: SspiAsyncContext, phcredential : *const SecHandle, phcontext : *const SecHandle, psztargetname : *const super::super::super::Win32::Foundation:: UNICODE_STRING, fcontextreq : u32, reserved1 : u32, targetdatarep : u32, pinput : *const SecBufferDesc, reserved2 : u32, phnewcontext : *const SecHandle, poutput : *const SecBufferDesc, pfcontextattr : *const u32, ptsexpiry : *const i64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn SspiReinitAsyncContext(handle : *mut super::super::Foundation:: SspiAsyncContext) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("ksecdd.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn SspiSetAsyncNotifyCallback(context : *const super::super::Foundation:: SspiAsyncContext, callback : SspiAsyncNotifyCallback, callbackdata : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("secur32.dll" "system" fn VerifySignature(phcontext : *const SecHandle, pmessage : *const SecBufferDesc, messageseqno : u32, pfqop : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwAllocateVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *mut *mut ::core::ffi::c_void, zerobits : usize, regionsize : *mut usize, allocationtype : u32, protect : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"] fn ZwAllocateVirtualMemoryEx(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *mut *mut ::core::ffi::c_void, regionsize : *mut usize, allocationtype : u32, pageprotection : u32, extendedparameters : *mut super::super::super::Win32::System::Memory:: MEM_EXTENDED_PARAMETER, extendedparametercount : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn ZwCreateEvent(eventhandle : *mut super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, eventtype : super::super::super::Win32::System::Kernel:: EVENT_TYPE, initialstate : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn ZwDeleteFile(objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwDuplicateObject(sourceprocesshandle : super::super::super::Win32::Foundation:: HANDLE, sourcehandle : super::super::super::Win32::Foundation:: HANDLE, targetprocesshandle : super::super::super::Win32::Foundation:: HANDLE, targethandle : *mut super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, handleattributes : u32, options : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ZwDuplicateToken(existingtokenhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, effectiveonly : super::super::super::Win32::Foundation:: BOOLEAN, tokentype : super::super::super::Win32::Security:: TOKEN_TYPE, newtokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwFlushBuffersFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwFlushBuffersFileEx(filehandle : super::super::super::Win32::Foundation:: HANDLE, flags : u32, parameters : *const ::core::ffi::c_void, parameterssize : u32, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwFlushVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *mut *mut ::core::ffi::c_void, regionsize : *mut usize, iostatus : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwFreeVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *mut *mut ::core::ffi::c_void, regionsize : *mut usize, freetype : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwFsControlFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fscontrolcode : u32, inputbuffer : *const ::core::ffi::c_void, inputbufferlength : u32, outputbuffer : *mut ::core::ffi::c_void, outputbufferlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwLockFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, byteoffset : *const i64, length : *const i64, key : u32, failimmediately : super::super::super::Win32::Foundation:: BOOLEAN, exclusivelock : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwNotifyChangeKey(keyhandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, completionfilter : u32, watchtree : super::super::super::Win32::Foundation:: BOOLEAN, buffer : *mut ::core::ffi::c_void, buffersize : u32, asynchronous : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn ZwOpenDirectoryObject(directoryhandle : *mut super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwOpenProcessTokenEx(processhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, handleattributes : u32, tokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwOpenThreadTokenEx(threadhandle : super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, openasself : super::super::super::Win32::Foundation:: BOOLEAN, handleattributes : u32, tokenhandle : *mut super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn ZwQueryDirectoryFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, returnsingleentry : super::super::super::Win32::Foundation:: BOOLEAN, filename : *const super::super::super::Win32::Foundation:: UNICODE_STRING, restartscan : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn ZwQueryDirectoryFileEx(filehandle : super::super::super::Win32::Foundation:: HANDLE, event : super::super::super::Win32::Foundation:: HANDLE, apcroutine : super::super::super::Win32::System::IO:: PIO_APC_ROUTINE, apccontext : *const ::core::ffi::c_void, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, queryflags : u32, filename : *const super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwQueryEaFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *mut ::core::ffi::c_void, length : u32, returnsingleentry : super::super::super::Win32::Foundation:: BOOLEAN, ealist : *const ::core::ffi::c_void, ealistlength : u32, eaindex : *const u32, restartscan : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn ZwQueryFullAttributesFile(objectattributes : *const super::super::Foundation:: OBJECT_ATTRIBUTES, fileinformation : *mut FILE_NETWORK_OPEN_INFORMATION) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ZwQueryInformationToken(tokenhandle : super::super::super::Win32::Foundation:: HANDLE, tokeninformationclass : super::super::super::Win32::Security:: TOKEN_INFORMATION_CLASS, tokeninformation : *mut ::core::ffi::c_void, tokeninformationlength : u32, returnlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn ZwQueryObject(handle : super::super::super::Win32::Foundation:: HANDLE, objectinformationclass : super::super::Foundation:: OBJECT_INFORMATION_CLASS, objectinformation : *mut ::core::ffi::c_void, objectinformationlength : u32, returnlength : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwQueryQuotaInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *mut ::core::ffi::c_void, length : u32, returnsingleentry : super::super::super::Win32::Foundation:: BOOLEAN, sidlist : *const ::core::ffi::c_void, sidlistlength : u32, startsid : super::super::super::Win32::Foundation:: PSID, restartscan : super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ZwQuerySecurityObject(handle : super::super::super::Win32::Foundation:: HANDLE, securityinformation : u32, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, length : u32, lengthneeded : *mut u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwQueryVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, baseaddress : *const ::core::ffi::c_void, memoryinformationclass : MEMORY_INFORMATION_CLASS, memoryinformation : *mut ::core::ffi::c_void, memoryinformationlength : usize, returnlength : *mut usize) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwQueryVolumeInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fsinformation : *mut ::core::ffi::c_void, length : u32, fsinformationclass : FS_INFORMATION_CLASS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwSetEaFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *const ::core::ffi::c_void, length : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwSetEvent(eventhandle : super::super::super::Win32::Foundation:: HANDLE, previousstate : *mut i32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ZwSetInformationToken(tokenhandle : super::super::super::Win32::Foundation:: HANDLE, tokeninformationclass : super::super::super::Win32::Security:: TOKEN_INFORMATION_CLASS, tokeninformation : *const ::core::ffi::c_void, tokeninformationlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwSetInformationVirtualMemory(processhandle : super::super::super::Win32::Foundation:: HANDLE, vminformationclass : VIRTUAL_MEMORY_INFORMATION_CLASS, numberofentries : usize, virtualaddresses : *const MEMORY_RANGE_ENTRY, vminformation : *const ::core::ffi::c_void, vminformationlength : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwSetQuotaInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *const ::core::ffi::c_void, length : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn ZwSetSecurityObject(handle : super::super::super::Win32::Foundation:: HANDLE, securityinformation : u32, securitydescriptor : super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwSetVolumeInformationFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fsinformation : *const ::core::ffi::c_void, length : u32, fsinformationclass : FS_INFORMATION_CLASS) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn ZwUnlockFile(filehandle : super::super::super::Win32::Foundation:: HANDLE, iostatusblock : *mut super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, byteoffset : *const i64, length : *const i64, key : u32) -> super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdll.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ZwWaitForSingleObject(handle : super::super::super::Win32::Foundation:: HANDLE, alertable : super::super::super::Win32::Foundation:: BOOLEAN, timeout : *const i64) -> super::super::super::Win32::Foundation:: NTSTATUS);
pub const ATOMIC_CREATE_ECP_IN_FLAG_BEST_EFFORT: u32 = 256u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_EOF_SPECIFIED: u32 = 4u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_FILE_ATTRIBUTES_SPECIFIED: u32 = 32u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_GEN_FLAGS_SPECIFIED: u32 = 32768u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_MARK_USN_SOURCE_INFO: u32 = 2048u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_OPERATION_MASK: u32 = 255u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_OP_FLAGS_SPECIFIED: u32 = 128u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_REPARSE_POINT_SPECIFIED: u32 = 2u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_SPARSE_SPECIFIED: u32 = 1u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_SUPPRESS_DIR_CHANGE_NOTIFY: u32 = 1024u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_SUPPRESS_FILE_ATTRIBUTE_INHERITANCE: u32 = 64u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_SUPPRESS_PARENT_TIMESTAMPS_UPDATE: u32 = 512u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_TIMESTAMPS_SPECIFIED: u32 = 16u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_VDL_SPECIFIED: u32 = 8u32;
pub const ATOMIC_CREATE_ECP_IN_FLAG_WRITE_USN_CLOSE_RECORD: u32 = 4096u32;
pub const ATOMIC_CREATE_ECP_IN_OP_FLAG_CASE_SENSITIVE_FLAGS_SPECIFIED: u32 = 1u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_EOF_SET: u32 = 4u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_FILE_ATTRIBUTES_RETURNED: u32 = 512u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_FILE_ATTRIBUTES_SET: u32 = 32u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_FILE_ATTRIBUTE_INHERITANCE_SUPPRESSED: u32 = 64u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_OPERATION_MASK: u32 = 255u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_OP_FLAGS_HONORED: u32 = 128u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_REPARSE_POINT_SET: u32 = 2u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_SPARSE_SET: u32 = 1u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_TIMESTAMPS_RETURNED: u32 = 256u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_TIMESTAMPS_SET: u32 = 16u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_USN_CLOSE_RECORD_WRITTEN: u32 = 2048u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_USN_RETURNED: u32 = 4096u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_USN_SOURCE_INFO_MARKED: u32 = 1024u32;
pub const ATOMIC_CREATE_ECP_OUT_FLAG_VDL_SET: u32 = 8u32;
pub const ATOMIC_CREATE_ECP_OUT_OP_FLAG_CASE_SENSITIVE_FLAGS_SET: u32 = 1u32;
pub const AuditAccessCheck: SE_AUDIT_OPERATION = 2i32;
pub const AuditCloseNonObject: SE_AUDIT_OPERATION = 9i32;
pub const AuditCloseObject: SE_AUDIT_OPERATION = 5i32;
pub const AuditDeleteObject: SE_AUDIT_OPERATION = 6i32;
pub const AuditHandleCreation: SE_AUDIT_OPERATION = 12i32;
pub const AuditObjectReference: SE_AUDIT_OPERATION = 11i32;
pub const AuditOpenNonObject: SE_AUDIT_OPERATION = 10i32;
pub const AuditOpenObject: SE_AUDIT_OPERATION = 3i32;
pub const AuditOpenObjectForDelete: SE_AUDIT_OPERATION = 7i32;
pub const AuditOpenObjectForDeleteWithTransaction: SE_AUDIT_OPERATION = 8i32;
pub const AuditOpenObjectWithTransaction: SE_AUDIT_OPERATION = 4i32;
pub const AuditPrivilegeObject: SE_AUDIT_OPERATION = 0i32;
pub const AuditPrivilegeService: SE_AUDIT_OPERATION = 1i32;
pub const CACHE_MANAGER_CALLBACKS_EX_V1: u32 = 1u32;
pub const CACHE_USE_DIRECT_ACCESS_MAPPING: u32 = 1u32;
pub const CACHE_VALID_FLAGS: u32 = 1u32;
pub const CC_ACQUIRE_DONT_WAIT: u32 = 1u32;
pub const CC_ACQUIRE_SUPPORTS_ASYNC_LAZYWRITE: u32 = 1u32;
pub const CC_AGGRESSIVE_UNMAP_BEHIND: u32 = 1u32;
pub const CC_DISABLE_DIRTY_PAGE_TRACKING: u32 = 8u32;
pub const CC_DISABLE_READ_AHEAD: u32 = 2u32;
pub const CC_DISABLE_UNMAP_BEHIND: u32 = 32u32;
pub const CC_DISABLE_WRITE_BEHIND: u32 = 4u32;
pub const CC_ENABLE_CPU_CACHE: u32 = 268435456u32;
pub const CC_ENABLE_DISK_IO_ACCOUNTING: u32 = 16u32;
pub const CC_FLUSH_AND_PURGE_GATHER_DIRTY_BITS: u32 = 2u32;
pub const CC_FLUSH_AND_PURGE_NO_PURGE: u32 = 1u32;
pub const CC_FLUSH_AND_PURGE_WRITEABLE_VIEWS_NOTSEEN: u32 = 4u32;
pub const COMPRESSION_ENGINE_MASK: u32 = 65280u32;
pub const COMPRESSION_ENGINE_MAX: u32 = 512u32;
pub const COMPRESSION_FORMAT_MASK: u32 = 255u32;
pub const COMPRESSION_FORMAT_MAX: u32 = 5u32;
pub const CREATE_REDIRECTION_FLAGS_SERVICED_FROM_LAYER: u32 = 1u32;
pub const CREATE_REDIRECTION_FLAGS_SERVICED_FROM_REGISTERED_LAYER: u32 = 4u32;
pub const CREATE_REDIRECTION_FLAGS_SERVICED_FROM_REMOTE_LAYER: u32 = 8u32;
pub const CREATE_REDIRECTION_FLAGS_SERVICED_FROM_SCRATCH: u32 = 2u32;
pub const CREATE_REDIRECTION_FLAGS_SERVICED_FROM_USER_MODE: u32 = 16u32;
pub const CSV_SET_HANDLE_PROPERTIES_ECP_CONTEXT_FLAGS_VALID_ONLY_IF_CSV_COORDINATOR: u32 = 1u32;
pub const ChangeDataControlArea: FSRTL_CHANGE_BACKING_TYPE = 0i32;
pub const ChangeImageControlArea: FSRTL_CHANGE_BACKING_TYPE = 1i32;
pub const ChangeSharedCacheMap: FSRTL_CHANGE_BACKING_TYPE = 2i32;
pub const CsvCsvFsInternalFileObject: CSV_DOWN_LEVEL_FILE_TYPE = 1i32;
pub const CsvDownLevelFileObject: CSV_DOWN_LEVEL_FILE_TYPE = 0i32;
pub const DD_MUP_DEVICE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("\\Device\\Mup");
pub const DEVICE_RESET_KEEP_STACK: u32 = 4u32;
pub const DEVICE_RESET_RESERVED_0: u32 = 1u32;
pub const DEVICE_RESET_RESERVED_1: u32 = 2u32;
pub const DO_BOOT_CRITICAL: u32 = 536870912u32;
pub const DO_BUFFERED_IO: u32 = 4u32;
pub const DO_BUS_ENUMERATED_DEVICE: u32 = 4096u32;
pub const DO_DAX_VOLUME: u32 = 268435456u32;
pub const DO_DEVICE_HAS_NAME: u32 = 64u32;
pub const DO_DEVICE_INITIALIZING: u32 = 128u32;
pub const DO_DEVICE_IRP_REQUIRES_EXTENSION: u32 = 134217728u32;
pub const DO_DEVICE_TO_BE_RESET: u32 = 67108864u32;
pub const DO_DIRECT_IO: u32 = 16u32;
pub const DO_DISALLOW_EXECUTE: u32 = 8388608u32;
pub const DO_EXCLUSIVE: u32 = 8u32;
pub const DO_FORCE_NEITHER_IO: u32 = 524288u32;
pub const DO_LONG_TERM_REQUESTS: u32 = 512u32;
pub const DO_LOW_PRIORITY_FILESYSTEM: u32 = 65536u32;
pub const DO_MAP_IO_BUFFER: u32 = 32u32;
pub const DO_NEVER_LAST_DEVICE: u32 = 1024u32;
pub const DO_NOT_PURGE_DIRTY_PAGES: u32 = 4u32;
pub const DO_NOT_RETRY_PURGE: u32 = 2u32;
pub const DO_POWER_INRUSH: u32 = 16384u32;
pub const DO_POWER_PAGABLE: u32 = 8192u32;
pub const DO_SHUTDOWN_REGISTERED: u32 = 2048u32;
pub const DO_SUPPORTS_PERSISTENT_ACLS: u32 = 131072u32;
pub const DO_SUPPORTS_TRANSACTIONS: u32 = 262144u32;
pub const DO_SYSTEM_BOOT_PARTITION: u32 = 256u32;
pub const DO_SYSTEM_CRITICAL_PARTITION: u32 = 4194304u32;
pub const DO_SYSTEM_SYSTEM_PARTITION: u32 = 2097152u32;
pub const DO_VERIFY_VOLUME: u32 = 2u32;
pub const DO_VOLUME_DEVICE_OBJECT: u32 = 1048576u32;
pub const DfsLinkTrackingInformation: LINK_TRACKING_INFORMATION_TYPE = 1i32;
pub const EA_NAME_NETWORK_OPEN_ECP_INTEGRITY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ECP{c584edbf-00df-4d28-00b8-8435baca8911e8}-INTEGRITY");
pub const EA_NAME_NETWORK_OPEN_ECP_INTEGRITY_U: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ECP{c584edbf-00df-4d28-00b8-8435baca8911e8}-INTEGRITY");
pub const EA_NAME_NETWORK_OPEN_ECP_PRIVACY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ECP{c584edbf-00df-4d28-00b8-8435baca8911e8}-PRIVACY");
pub const EA_NAME_NETWORK_OPEN_ECP_PRIVACY_U: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ECP{c584edbf-00df-4d28-00b8-8435baca8911e8}-PRIVACY");
pub const ECP_OPEN_PARAMETERS_FLAG_FAIL_ON_CASE_SENSITIVE_DIR: u32 = 16u32;
pub const ECP_OPEN_PARAMETERS_FLAG_IGNORE_DIR_CASE_SENSITIVITY: u32 = 8u32;
pub const ECP_OPEN_PARAMETERS_FLAG_OPEN_FOR_DELETE: u32 = 4u32;
pub const ECP_OPEN_PARAMETERS_FLAG_OPEN_FOR_READ: u32 = 1u32;
pub const ECP_OPEN_PARAMETERS_FLAG_OPEN_FOR_WRITE: u32 = 2u32;
pub const ECP_TYPE_CLFS_CREATE_CONTAINER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8650c9fe_0cec_8bf6_bd1e_835956541090);
pub const ECP_TYPE_IO_STOP_ON_SYMLINK_FILTER_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x940e5d56_1646_4d3c_87b6_577ec36a1466);
pub const ECP_TYPE_OPEN_REPARSE_GUID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x323eb6a8_affd_4d95_8230_863bce09d37a);
pub const EVENT_INCREMENT: u32 = 1u32;
pub const EqualTo: FSRTL_COMPARISON_RESULT = 0i32;
pub const FILE_ACTION_ADDED_STREAM: u32 = 6u32;
pub const FILE_ACTION_ID_NOT_TUNNELLED: u32 = 10u32;
pub const FILE_ACTION_MODIFIED_STREAM: u32 = 8u32;
pub const FILE_ACTION_REMOVED_BY_DELETE: u32 = 9u32;
pub const FILE_ACTION_REMOVED_STREAM: u32 = 7u32;
pub const FILE_ACTION_TUNNELLED_ID_COLLISION: u32 = 11u32;
pub const FILE_CLEANUP_FILE_DELETED: u32 = 4u32;
pub const FILE_CLEANUP_FILE_REMAINS: u32 = 2u32;
pub const FILE_CLEANUP_LINK_DELETED: u32 = 8u32;
pub const FILE_CLEANUP_POSIX_STYLE_DELETE: u32 = 32u32;
pub const FILE_CLEANUP_STREAM_DELETED: u32 = 16u32;
pub const FILE_CLEANUP_UNKNOWN: u32 = 0u32;
pub const FILE_CLEANUP_WRONG_DEVICE: u32 = 1u32;
pub const FILE_COMPLETE_IF_OPLOCKED: NTCREATEFILE_CREATE_OPTIONS = 256u32;
pub const FILE_CONTAINS_EXTENDED_CREATE_INFORMATION: NTCREATEFILE_CREATE_OPTIONS = 268435456u32;
pub const FILE_CREATE: NTCREATEFILE_CREATE_DISPOSITION = 2u32;
pub const FILE_CREATE_TREE_CONNECTION: NTCREATEFILE_CREATE_OPTIONS = 128u32;
pub const FILE_DELETE_ON_CLOSE: NTCREATEFILE_CREATE_OPTIONS = 4096u32;
pub const FILE_DIRECTORY_FILE: NTCREATEFILE_CREATE_OPTIONS = 1u32;
pub const FILE_DISALLOW_EXCLUSIVE: NTCREATEFILE_CREATE_OPTIONS = 131072u32;
pub const FILE_DISPOSITION_DELETE: FILE_DISPOSITION_INFORMATION_EX_FLAGS = 1u32;
pub const FILE_DISPOSITION_DO_NOT_DELETE: FILE_DISPOSITION_INFORMATION_EX_FLAGS = 0u32;
pub const FILE_DISPOSITION_FORCE_IMAGE_SECTION_CHECK: FILE_DISPOSITION_INFORMATION_EX_FLAGS = 4u32;
pub const FILE_DISPOSITION_IGNORE_READONLY_ATTRIBUTE: FILE_DISPOSITION_INFORMATION_EX_FLAGS = 16u32;
pub const FILE_DISPOSITION_ON_CLOSE: FILE_DISPOSITION_INFORMATION_EX_FLAGS = 8u32;
pub const FILE_DISPOSITION_POSIX_SEMANTICS: FILE_DISPOSITION_INFORMATION_EX_FLAGS = 2u32;
pub const FILE_EA_TYPE_ASCII: u32 = 65533u32;
pub const FILE_EA_TYPE_ASN1: u32 = 65501u32;
pub const FILE_EA_TYPE_BINARY: u32 = 65534u32;
pub const FILE_EA_TYPE_BITMAP: u32 = 65531u32;
pub const FILE_EA_TYPE_EA: u32 = 65518u32;
pub const FILE_EA_TYPE_FAMILY_IDS: u32 = 65281u32;
pub const FILE_EA_TYPE_ICON: u32 = 65529u32;
pub const FILE_EA_TYPE_METAFILE: u32 = 65530u32;
pub const FILE_EA_TYPE_MVMT: u32 = 65503u32;
pub const FILE_EA_TYPE_MVST: u32 = 65502u32;
pub const FILE_ID_GLOBAL_TX_DIR_INFO_FLAG_VISIBLE_OUTSIDE_TX: u32 = 4u32;
pub const FILE_ID_GLOBAL_TX_DIR_INFO_FLAG_VISIBLE_TO_TX: u32 = 2u32;
pub const FILE_ID_GLOBAL_TX_DIR_INFO_FLAG_WRITELOCKED: u32 = 1u32;
pub const FILE_LINK_FORCE_RESIZE_SOURCE_SR: u32 = 256u32;
pub const FILE_LINK_FORCE_RESIZE_SR: u32 = 384u32;
pub const FILE_LINK_FORCE_RESIZE_TARGET_SR: u32 = 128u32;
pub const FILE_LINK_IGNORE_READONLY_ATTRIBUTE: u32 = 64u32;
pub const FILE_LINK_NO_DECREASE_AVAILABLE_SPACE: u32 = 32u32;
pub const FILE_LINK_NO_INCREASE_AVAILABLE_SPACE: u32 = 16u32;
pub const FILE_LINK_POSIX_SEMANTICS: u32 = 2u32;
pub const FILE_LINK_PRESERVE_AVAILABLE_SPACE: u32 = 48u32;
pub const FILE_LINK_REPLACE_IF_EXISTS: u32 = 1u32;
pub const FILE_LINK_SUPPRESS_STORAGE_RESERVE_INHERITANCE: u32 = 8u32;
pub const FILE_NEED_EA: u32 = 128u32;
pub const FILE_NON_DIRECTORY_FILE: NTCREATEFILE_CREATE_OPTIONS = 64u32;
pub const FILE_NOTIFY_CHANGE_EA: u32 = 128u32;
pub const FILE_NOTIFY_CHANGE_NAME: u32 = 3u32;
pub const FILE_NOTIFY_CHANGE_STREAM_NAME: u32 = 512u32;
pub const FILE_NOTIFY_CHANGE_STREAM_SIZE: u32 = 1024u32;
pub const FILE_NOTIFY_CHANGE_STREAM_WRITE: u32 = 2048u32;
pub const FILE_NOTIFY_VALID_MASK: u32 = 4095u32;
pub const FILE_NO_COMPRESSION: NTCREATEFILE_CREATE_OPTIONS = 32768u32;
pub const FILE_NO_EA_KNOWLEDGE: NTCREATEFILE_CREATE_OPTIONS = 512u32;
pub const FILE_NO_INTERMEDIATE_BUFFERING: NTCREATEFILE_CREATE_OPTIONS = 8u32;
pub const FILE_OPBATCH_BREAK_UNDERWAY: u32 = 9u32;
pub const FILE_OPEN: NTCREATEFILE_CREATE_DISPOSITION = 1u32;
pub const FILE_OPEN_BY_FILE_ID: NTCREATEFILE_CREATE_OPTIONS = 8192u32;
pub const FILE_OPEN_FOR_BACKUP_INTENT: NTCREATEFILE_CREATE_OPTIONS = 16384u32;
pub const FILE_OPEN_FOR_FREE_SPACE_QUERY: NTCREATEFILE_CREATE_OPTIONS = 8388608u32;
pub const FILE_OPEN_IF: NTCREATEFILE_CREATE_DISPOSITION = 3u32;
pub const FILE_OPEN_NO_RECALL: NTCREATEFILE_CREATE_OPTIONS = 4194304u32;
pub const FILE_OPEN_REPARSE_POINT: NTCREATEFILE_CREATE_OPTIONS = 2097152u32;
pub const FILE_OPEN_REQUIRING_OPLOCK: NTCREATEFILE_CREATE_OPTIONS = 65536u32;
pub const FILE_OPLOCK_BROKEN_TO_LEVEL_2: u32 = 7u32;
pub const FILE_OPLOCK_BROKEN_TO_NONE: u32 = 8u32;
pub const FILE_OVERWRITE: NTCREATEFILE_CREATE_DISPOSITION = 4u32;
pub const FILE_OVERWRITE_IF: NTCREATEFILE_CREATE_DISPOSITION = 5u32;
pub const FILE_PIPE_ACCEPT_REMOTE_CLIENTS: u32 = 0u32;
pub const FILE_PIPE_BYTE_STREAM_MODE: u32 = 0u32;
pub const FILE_PIPE_BYTE_STREAM_TYPE: u32 = 0u32;
pub const FILE_PIPE_CLIENT_END: u32 = 0u32;
pub const FILE_PIPE_CLOSING_STATE: u32 = 4u32;
pub const FILE_PIPE_COMPLETE_OPERATION: u32 = 1u32;
pub const FILE_PIPE_COMPUTER_NAME_LENGTH: u32 = 15u32;
pub const FILE_PIPE_CONNECTED_STATE: u32 = 3u32;
pub const FILE_PIPE_DISCONNECTED_STATE: u32 = 1u32;
pub const FILE_PIPE_FULL_DUPLEX: u32 = 2u32;
pub const FILE_PIPE_INBOUND: u32 = 0u32;
pub const FILE_PIPE_LISTENING_STATE: u32 = 2u32;
pub const FILE_PIPE_MESSAGE_MODE: u32 = 1u32;
pub const FILE_PIPE_MESSAGE_TYPE: u32 = 1u32;
pub const FILE_PIPE_OUTBOUND: u32 = 1u32;
pub const FILE_PIPE_QUEUE_OPERATION: u32 = 0u32;
pub const FILE_PIPE_READ_DATA: u32 = 0u32;
pub const FILE_PIPE_REJECT_REMOTE_CLIENTS: u32 = 2u32;
pub const FILE_PIPE_SERVER_END: u32 = 1u32;
pub const FILE_PIPE_SYMLINK_FLAG_GLOBAL: u32 = 1u32;
pub const FILE_PIPE_SYMLINK_FLAG_RELATIVE: u32 = 2u32;
pub const FILE_PIPE_TYPE_VALID_MASK: u32 = 3u32;
pub const FILE_PIPE_WRITE_SPACE: u32 = 1u32;
pub const FILE_RANDOM_ACCESS: NTCREATEFILE_CREATE_OPTIONS = 2048u32;
pub const FILE_RENAME_FORCE_RESIZE_SOURCE_SR: u32 = 256u32;
pub const FILE_RENAME_FORCE_RESIZE_SR: u32 = 384u32;
pub const FILE_RENAME_FORCE_RESIZE_TARGET_SR: u32 = 128u32;
pub const FILE_RENAME_IGNORE_READONLY_ATTRIBUTE: u32 = 64u32;
pub const FILE_RENAME_NO_DECREASE_AVAILABLE_SPACE: u32 = 32u32;
pub const FILE_RENAME_NO_INCREASE_AVAILABLE_SPACE: u32 = 16u32;
pub const FILE_RENAME_POSIX_SEMANTICS: u32 = 2u32;
pub const FILE_RENAME_PRESERVE_AVAILABLE_SPACE: u32 = 48u32;
pub const FILE_RENAME_REPLACE_IF_EXISTS: u32 = 1u32;
pub const FILE_RENAME_SUPPRESS_PIN_STATE_INHERITANCE: u32 = 4u32;
pub const FILE_RENAME_SUPPRESS_STORAGE_RESERVE_INHERITANCE: u32 = 8u32;
pub const FILE_RESERVE_OPFILTER: NTCREATEFILE_CREATE_OPTIONS = 1048576u32;
pub const FILE_SEQUENTIAL_ONLY: NTCREATEFILE_CREATE_OPTIONS = 4u32;
pub const FILE_SESSION_AWARE: NTCREATEFILE_CREATE_OPTIONS = 262144u32;
pub const FILE_SUPERSEDE: NTCREATEFILE_CREATE_DISPOSITION = 0u32;
pub const FILE_SYNCHRONOUS_IO_ALERT: NTCREATEFILE_CREATE_OPTIONS = 16u32;
pub const FILE_SYNCHRONOUS_IO_NONALERT: NTCREATEFILE_CREATE_OPTIONS = 32u32;
pub const FILE_VC_CONTENT_INDEX_DISABLED: u32 = 8u32;
pub const FILE_VC_LOG_QUOTA_LIMIT: u32 = 32u32;
pub const FILE_VC_LOG_QUOTA_THRESHOLD: u32 = 16u32;
pub const FILE_VC_LOG_VOLUME_LIMIT: u32 = 128u32;
pub const FILE_VC_LOG_VOLUME_THRESHOLD: u32 = 64u32;
pub const FILE_VC_QUOTAS_INCOMPLETE: u32 = 256u32;
pub const FILE_VC_QUOTAS_REBUILDING: u32 = 512u32;
pub const FILE_VC_QUOTA_ENFORCE: u32 = 2u32;
pub const FILE_VC_QUOTA_MASK: u32 = 3u32;
pub const FILE_VC_QUOTA_NONE: u32 = 0u32;
pub const FILE_VC_QUOTA_TRACK: u32 = 1u32;
pub const FILE_VC_VALID_MASK: u32 = 1023u32;
pub const FILE_WRITE_THROUGH: NTCREATEFILE_CREATE_OPTIONS = 2u32;
pub const FLAGS_DELAY_REASONS_BITMAP_SCANNED: u32 = 2u32;
pub const FLAGS_DELAY_REASONS_LOG_FILE_FULL: u32 = 1u32;
pub const FLAGS_END_OF_FILE_INFO_EX_EXTEND_PAGING: u32 = 1u32;
pub const FLAGS_END_OF_FILE_INFO_EX_NO_EXTRA_PAGING_EXTEND: u32 = 2u32;
pub const FLAGS_END_OF_FILE_INFO_EX_TIME_CONSTRAINED: u32 = 4u32;
pub const FSCTL_LMR_GET_LINK_TRACKING_INFORMATION: u32 = 1310952u32;
pub const FSCTL_LMR_SET_LINK_TRACKING_INFORMATION: u32 = 1310956u32;
pub const FSCTL_MAILSLOT_PEEK: u32 = 802819u32;
pub const FSCTL_PIPE_ASSIGN_EVENT: u32 = 1114112u32;
pub const FSCTL_PIPE_CREATE_SYMLINK: u32 = 1114188u32;
pub const FSCTL_PIPE_DELETE_SYMLINK: u32 = 1114192u32;
pub const FSCTL_PIPE_DISABLE_IMPERSONATE: u32 = 1114180u32;
pub const FSCTL_PIPE_DISCONNECT: u32 = 1114116u32;
pub const FSCTL_PIPE_FLUSH: u32 = 1146944u32;
pub const FSCTL_PIPE_GET_CONNECTION_ATTRIBUTE: u32 = 1114160u32;
pub const FSCTL_PIPE_GET_HANDLE_ATTRIBUTE: u32 = 1114168u32;
pub const FSCTL_PIPE_GET_PIPE_ATTRIBUTE: u32 = 1114152u32;
pub const FSCTL_PIPE_IMPERSONATE: u32 = 1114140u32;
pub const FSCTL_PIPE_INTERNAL_READ: u32 = 1138676u32;
pub const FSCTL_PIPE_INTERNAL_READ_OVFLOW: u32 = 1138688u32;
pub const FSCTL_PIPE_INTERNAL_TRANSCEIVE: u32 = 1171455u32;
pub const FSCTL_PIPE_INTERNAL_WRITE: u32 = 1155064u32;
pub const FSCTL_PIPE_LISTEN: u32 = 1114120u32;
pub const FSCTL_PIPE_PEEK: u32 = 1130508u32;
pub const FSCTL_PIPE_QUERY_CLIENT_PROCESS: u32 = 1114148u32;
pub const FSCTL_PIPE_QUERY_CLIENT_PROCESS_V2: u32 = 1114196u32;
pub const FSCTL_PIPE_QUERY_EVENT: u32 = 1114128u32;
pub const FSCTL_PIPE_SET_CLIENT_PROCESS: u32 = 1114144u32;
pub const FSCTL_PIPE_SET_CONNECTION_ATTRIBUTE: u32 = 1114164u32;
pub const FSCTL_PIPE_SET_HANDLE_ATTRIBUTE: u32 = 1114172u32;
pub const FSCTL_PIPE_SET_PIPE_ATTRIBUTE: u32 = 1114156u32;
pub const FSCTL_PIPE_SILO_ARRIVAL: u32 = 1146952u32;
pub const FSCTL_PIPE_TRANSCEIVE: u32 = 1163287u32;
pub const FSCTL_PIPE_WAIT: u32 = 1114136u32;
pub const FSRTL_ADD_TC_CASE_SENSITIVE: u32 = 1u32;
pub const FSRTL_ADD_TC_KEY_BY_SHORT_NAME: u32 = 2u32;
pub const FSRTL_ALLOCATE_ECPLIST_FLAG_CHARGE_QUOTA: u32 = 1u32;
pub const FSRTL_ALLOCATE_ECP_FLAG_CHARGE_QUOTA: u32 = 1u32;
pub const FSRTL_ALLOCATE_ECP_FLAG_NONPAGED_POOL: u32 = 2u32;
pub const FSRTL_AUXILIARY_FLAG_DEALLOCATE: u32 = 1u32;
pub const FSRTL_CC_FLUSH_ERROR_FLAG_NO_HARD_ERROR: u32 = 1u32;
pub const FSRTL_CC_FLUSH_ERROR_FLAG_NO_LOG_ENTRY: u32 = 2u32;
pub const FSRTL_DRIVER_BACKING_FLAG_USE_PAGE_FILE: u32 = 1u32;
pub const FSRTL_ECP_LOOKASIDE_FLAG_NONPAGED_POOL: u32 = 2u32;
pub const FSRTL_FAT_LEGAL: u32 = 1u32;
pub const FSRTL_FCB_HEADER_V0: u32 = 0u32;
pub const FSRTL_FCB_HEADER_V1: u32 = 1u32;
pub const FSRTL_FCB_HEADER_V2: u32 = 2u32;
pub const FSRTL_FCB_HEADER_V3: u32 = 3u32;
pub const FSRTL_FCB_HEADER_V4: u32 = 4u32;
pub const FSRTL_FIND_TC_CASE_SENSITIVE: u32 = 1u32;
pub const FSRTL_FLAG2_BYPASSIO_STREAM_PAUSED: u32 = 32u32;
pub const FSRTL_FLAG2_DO_MODIFIED_WRITE: u32 = 1u32;
pub const FSRTL_FLAG2_IS_PAGING_FILE: u32 = 8u32;
pub const FSRTL_FLAG2_PURGE_WHEN_MAPPED: u32 = 4u32;
pub const FSRTL_FLAG2_SUPPORTS_FILTER_CONTEXTS: u32 = 2u32;
pub const FSRTL_FLAG2_WRITABLE_USER_MAPPED_FILE: u32 = 16u32;
pub const FSRTL_FLAG_ACQUIRE_MAIN_RSRC_EX: u32 = 8u32;
pub const FSRTL_FLAG_ACQUIRE_MAIN_RSRC_SH: u32 = 16u32;
pub const FSRTL_FLAG_ADVANCED_HEADER: u32 = 64u32;
pub const FSRTL_FLAG_EOF_ADVANCE_ACTIVE: u32 = 128u32;
pub const FSRTL_FLAG_FILE_LENGTH_CHANGED: u32 = 2u32;
pub const FSRTL_FLAG_FILE_MODIFIED: u32 = 1u32;
pub const FSRTL_FLAG_LIMIT_MODIFIED_PAGES: u32 = 4u32;
pub const FSRTL_FLAG_USER_MAPPED_FILE: u32 = 32u32;
pub const FSRTL_HPFS_LEGAL: u32 = 2u32;
pub const FSRTL_NTFS_LEGAL: u32 = 4u32;
pub const FSRTL_OLE_LEGAL: u32 = 16u32;
pub const FSRTL_UNC_HARDENING_CAPABILITIES_INTEGRITY: u32 = 2u32;
pub const FSRTL_UNC_HARDENING_CAPABILITIES_MUTUAL_AUTH: u32 = 1u32;
pub const FSRTL_UNC_HARDENING_CAPABILITIES_PRIVACY: u32 = 4u32;
pub const FSRTL_UNC_PROVIDER_FLAGS_CONTAINER_AWARE: u32 = 8u32;
pub const FSRTL_UNC_PROVIDER_FLAGS_CSC_ENABLED: u32 = 2u32;
pub const FSRTL_UNC_PROVIDER_FLAGS_DOMAIN_SVC_AWARE: u32 = 4u32;
pub const FSRTL_UNC_PROVIDER_FLAGS_MAILSLOTS_SUPPORTED: u32 = 1u32;
pub const FSRTL_UNC_REGISTRATION_CURRENT_VERSION: u32 = 513u32;
pub const FSRTL_UNC_REGISTRATION_VERSION_0200: u32 = 512u32;
pub const FSRTL_UNC_REGISTRATION_VERSION_0201: u32 = 513u32;
pub const FSRTL_VIRTDISK_FULLY_ALLOCATED: u32 = 1u32;
pub const FSRTL_VIRTDISK_NO_DRIVE_LETTER: u32 = 2u32;
pub const FSRTL_VOLUME_BACKGROUND_FORMAT: u32 = 14u32;
pub const FSRTL_VOLUME_CHANGE_SIZE: u32 = 13u32;
pub const FSRTL_VOLUME_DISMOUNT: u32 = 1u32;
pub const FSRTL_VOLUME_DISMOUNT_FAILED: u32 = 2u32;
pub const FSRTL_VOLUME_FORCED_CLOSED: u32 = 10u32;
pub const FSRTL_VOLUME_INFO_MAKE_COMPAT: u32 = 11u32;
pub const FSRTL_VOLUME_LOCK: u32 = 3u32;
pub const FSRTL_VOLUME_LOCK_FAILED: u32 = 4u32;
pub const FSRTL_VOLUME_MOUNT: u32 = 6u32;
pub const FSRTL_VOLUME_NEEDS_CHKDSK: u32 = 7u32;
pub const FSRTL_VOLUME_PREPARING_EJECT: u32 = 12u32;
pub const FSRTL_VOLUME_UNLOCK: u32 = 5u32;
pub const FSRTL_VOLUME_WEARING_OUT: u32 = 9u32;
pub const FSRTL_VOLUME_WORM_NEAR_FULL: u32 = 8u32;
pub const FSRTL_WILD_CHARACTER: u32 = 8u32;
pub const FS_FILTER_SECTION_SYNC_IMAGE_EXTENTS_ARE_NOT_RVA: u32 = 8u32;
pub const FS_FILTER_SECTION_SYNC_IN_FLAG_DONT_UPDATE_LAST_ACCESS: u32 = 1u32;
pub const FS_FILTER_SECTION_SYNC_IN_FLAG_DONT_UPDATE_LAST_WRITE: u32 = 2u32;
pub const FS_FILTER_SECTION_SYNC_SUPPORTS_ASYNC_PARALLEL_IO: u32 = 1u32;
pub const FS_FILTER_SECTION_SYNC_SUPPORTS_DIRECT_MAP_DATA: u32 = 2u32;
pub const FS_FILTER_SECTION_SYNC_SUPPORTS_DIRECT_MAP_IMAGE: u32 = 4u32;
pub const FastIoIsNotPossible: FAST_IO_POSSIBLE = 0i32;
pub const FastIoIsPossible: FAST_IO_POSSIBLE = 1i32;
pub const FastIoIsQuestionable: FAST_IO_POSSIBLE = 2i32;
pub const FileFsAttributeInformation: FS_INFORMATION_CLASS = 5i32;
pub const FileFsControlInformation: FS_INFORMATION_CLASS = 6i32;
pub const FileFsDataCopyInformation: FS_INFORMATION_CLASS = 12i32;
pub const FileFsDeviceInformation: FS_INFORMATION_CLASS = 4i32;
pub const FileFsDriverPathInformation: FS_INFORMATION_CLASS = 9i32;
pub const FileFsFullSizeInformation: FS_INFORMATION_CLASS = 7i32;
pub const FileFsFullSizeInformationEx: FS_INFORMATION_CLASS = 14i32;
pub const FileFsLabelInformation: FS_INFORMATION_CLASS = 2i32;
pub const FileFsMaximumInformation: FS_INFORMATION_CLASS = 15i32;
pub const FileFsMetadataSizeInformation: FS_INFORMATION_CLASS = 13i32;
pub const FileFsObjectIdInformation: FS_INFORMATION_CLASS = 8i32;
pub const FileFsSectorSizeInformation: FS_INFORMATION_CLASS = 11i32;
pub const FileFsSizeInformation: FS_INFORMATION_CLASS = 3i32;
pub const FileFsVolumeFlagsInformation: FS_INFORMATION_CLASS = 10i32;
pub const FileFsVolumeInformation: FS_INFORMATION_CLASS = 1i32;
pub const GCR_ALLOW_LM: u32 = 4096u32;
pub const GCR_ALLOW_NO_TARGET: u32 = 8192u32;
pub const GCR_ALLOW_NTLM: u32 = 256u32;
pub const GCR_MACHINE_CREDENTIAL: u32 = 1024u32;
pub const GCR_NTLM3_PARMS: u32 = 32u32;
pub const GCR_TARGET_INFO: u32 = 64u32;
pub const GCR_USE_OEM_SET: u32 = 512u32;
pub const GCR_USE_OWF_PASSWORD: u32 = 2048u32;
pub const GCR_VSM_PROTECTED_PASSWORD: u32 = 16384u32;
pub const GENERATE_CLIENT_CHALLENGE: u32 = 16u32;
pub const GUID_ECP_ATOMIC_CREATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4720bd83_52ac_4104_a130_d1ec6a8cc8e5);
pub const GUID_ECP_CLOUDFILES_ATTRIBUTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2932ff52_8378_4fc1_8edb_6bdc8f602709);
pub const GUID_ECP_CREATE_REDIRECTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x188d6bd6_a126_4fa8_bdf2_1ccdf896f3e0);
pub const GUID_ECP_CSV_DOWN_LEVEL_OPEN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4248be44_647f_488f_8be5_a08aaf70f028);
pub const GUID_ECP_CSV_QUERY_FILE_REVISION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x44aec90b_de65_4d46_8fbf_763f9d970b1d);
pub const GUID_ECP_CSV_QUERY_FILE_REVISION_FILE_ID_128: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7a3a4aa1_aa74_4bc6_b070_ab56a38c1fed);
pub const GUID_ECP_CSV_SET_HANDLE_PROPERTIES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7a9fdd94_7b58_42bb_9740_3cb86983a615);
pub const GUID_ECP_DUAL_OPLOCK_KEY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x41621a14_b08b_4df1_b676_a05ffdf01bea);
pub const GUID_ECP_IO_DEVICE_HINT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf315b732_ac6b_4d4d_be0c_b3126490e1a3);
pub const GUID_ECP_NETWORK_APP_INSTANCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6aa6bc45_a7ef_4af7_9008_fa462e144d74);
pub const GUID_ECP_NETWORK_APP_INSTANCE_VERSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb7d082b9_563b_4f07_a07b_524a8116a010);
pub const GUID_ECP_NETWORK_OPEN_CONTEXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc584edbf_00df_4d28_b884_35baca8911e8);
pub const GUID_ECP_NFS_OPEN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf326d30c_e5f8_4fe7_ab74_f5a3196d92db);
pub const GUID_ECP_OPEN_PARAMETERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd0a93c3_3bb7_463d_accb_969d3435a5a5);
pub const GUID_ECP_OPLOCK_KEY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x48850596_3050_4be7_9863_fec350ce8d7f);
pub const GUID_ECP_PREFETCH_OPEN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe1777b21_847e_4837_aa45_64161d280655);
pub const GUID_ECP_QUERY_ON_CREATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1aca62e9_abb4_4ff2_bb5c_1c79025e417f);
pub const GUID_ECP_RKF_BYPASS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x02378cc6_f73c_489c_8282_564d1a99131b);
pub const GUID_ECP_SRV_OPEN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbebfaebc_aabf_489d_9d2c_e9e361102853);
pub const GreaterThan: FSRTL_COMPARISON_RESULT = 1i32;
pub const HEAP_CLASS_0: u32 = 0u32;
pub const HEAP_CLASS_1: u32 = 4096u32;
pub const HEAP_CLASS_2: u32 = 8192u32;
pub const HEAP_CLASS_3: u32 = 12288u32;
pub const HEAP_CLASS_4: u32 = 16384u32;
pub const HEAP_CLASS_5: u32 = 20480u32;
pub const HEAP_CLASS_6: u32 = 24576u32;
pub const HEAP_CLASS_7: u32 = 28672u32;
pub const HEAP_CLASS_8: u32 = 32768u32;
pub const HEAP_CLASS_MASK: u32 = 61440u32;
pub const HEAP_CREATE_ALIGN_16: u32 = 65536u32;
pub const HEAP_CREATE_ENABLE_EXECUTE: u32 = 262144u32;
pub const HEAP_CREATE_ENABLE_TRACING: u32 = 131072u32;
pub const HEAP_CREATE_HARDENED: u32 = 512u32;
pub const HEAP_CREATE_SEGMENT_HEAP: u32 = 256u32;
pub const HEAP_DISABLE_COALESCE_ON_FREE: u32 = 128u32;
pub const HEAP_FREE_CHECKING_ENABLED: u32 = 64u32;
pub const HEAP_GENERATE_EXCEPTIONS: u32 = 4u32;
pub const HEAP_GLOBAL_TAG: u32 = 2048u32;
pub const HEAP_GROWABLE: u32 = 2u32;
pub const HEAP_MAXIMUM_TAG: u32 = 4095u32;
pub const HEAP_NO_SERIALIZE: u32 = 1u32;
pub const HEAP_PSEUDO_TAG_FLAG: u32 = 32768u32;
pub const HEAP_REALLOC_IN_PLACE_ONLY: u32 = 16u32;
pub const HEAP_SETTABLE_USER_FLAG1: u32 = 512u32;
pub const HEAP_SETTABLE_USER_FLAG2: u32 = 1024u32;
pub const HEAP_SETTABLE_USER_FLAG3: u32 = 2048u32;
pub const HEAP_SETTABLE_USER_FLAGS: u32 = 3584u32;
pub const HEAP_SETTABLE_USER_VALUE: u32 = 256u32;
pub const HEAP_TAG_SHIFT: u32 = 18u32;
pub const HEAP_TAIL_CHECKING_ENABLED: u32 = 32u32;
pub const HEAP_ZERO_MEMORY: u32 = 8u32;
pub const HeapMemoryBasicInformation: HEAP_MEMORY_INFO_CLASS = 0i32;
pub const INVALID_PROCESSOR_INDEX: u32 = 4294967295u32;
pub const IOCTL_LMR_ARE_FILE_OBJECTS_ON_SAME_SERVER: u32 = 1310960u32;
pub const IOCTL_REDIR_QUERY_PATH: u32 = 1311119u32;
pub const IOCTL_REDIR_QUERY_PATH_EX: u32 = 1311123u32;
pub const IOCTL_VOLSNAP_FLUSH_AND_HOLD_WRITES: u32 = 5488640u32;
pub const IO_CD_ROM_INCREMENT: u32 = 1u32;
pub const IO_CREATE_STREAM_FILE_LITE: u32 = 2u32;
pub const IO_CREATE_STREAM_FILE_RAISE_ON_ERROR: u32 = 1u32;
pub const IO_DISK_INCREMENT: u32 = 1u32;
pub const IO_FILE_OBJECT_NON_PAGED_POOL_CHARGE: u32 = 64u32;
pub const IO_FILE_OBJECT_PAGED_POOL_CHARGE: u32 = 1024u32;
pub const IO_IGNORE_READONLY_ATTRIBUTE: u32 = 64u32;
pub const IO_MAILSLOT_INCREMENT: u32 = 2u32;
pub const IO_MM_PAGING_FILE: u32 = 16u32;
pub const IO_NAMED_PIPE_INCREMENT: u32 = 2u32;
pub const IO_NETWORK_INCREMENT: u32 = 2u32;
pub const IO_NO_INCREMENT: u32 = 0u32;
pub const IO_OPEN_PAGING_FILE: u32 = 2u32;
pub const IO_OPEN_TARGET_DIRECTORY: u32 = 4u32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_0: i32 = 96i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_1: i32 = 97i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_2: i32 = 98i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_3: i32 = 99i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_4: i32 = 100i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_5: i32 = 101i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_6: i32 = 102i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_7: i32 = 103i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_8: i32 = 104i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_9: i32 = 105i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_A: i32 = 106i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_B: i32 = 107i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_C: i32 = 108i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_D: i32 = 109i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_E: i32 = 110i32;
pub const IO_REPARSE_TAG_ACRONIS_HSM_F: i32 = 111i32;
pub const IO_REPARSE_TAG_ACTIVISION_HSM: i32 = 71i32;
pub const IO_REPARSE_TAG_ADA_HSM: i32 = 38i32;
pub const IO_REPARSE_TAG_ADOBE_HSM: i32 = 69i32;
pub const IO_REPARSE_TAG_ALERTBOOT: i32 = 536870988i32;
pub const IO_REPARSE_TAG_ALTIRIS_HSM: i32 = 25i32;
pub const IO_REPARSE_TAG_APPXSTRM: i32 = -1073741804i32;
pub const IO_REPARSE_TAG_ARCO_BACKUP: i32 = 59i32;
pub const IO_REPARSE_TAG_ARKIVIO: i32 = 12i32;
pub const IO_REPARSE_TAG_AURISTOR_FS: i32 = 73i32;
pub const IO_REPARSE_TAG_AUTN_HSM: i32 = 39i32;
pub const IO_REPARSE_TAG_BRIDGEHEAD_HSM: i32 = 22i32;
pub const IO_REPARSE_TAG_C2CSYSTEMS_HSM: i32 = 49i32;
pub const IO_REPARSE_TAG_CARINGO_HSM: i32 = 52i32;
pub const IO_REPARSE_TAG_CARROLL_HSM: i32 = 60i32;
pub const IO_REPARSE_TAG_CITRIX_PM: i32 = 54i32;
pub const IO_REPARSE_TAG_COMMVAULT: i32 = 14i32;
pub const IO_REPARSE_TAG_COMMVAULT_HSM: i32 = 29i32;
pub const IO_REPARSE_TAG_COMTRADE_HSM: i32 = 61i32;
pub const IO_REPARSE_TAG_CTERA_HSM: i32 = 78i32;
pub const IO_REPARSE_TAG_DATAFIRST_HSM: i32 = 48i32;
pub const IO_REPARSE_TAG_DATAGLOBAL_HSM: i32 = 46i32;
pub const IO_REPARSE_TAG_DATASTOR_SIS: i32 = 30i32;
pub const IO_REPARSE_TAG_DFM: i32 = -2147483626i32;
pub const IO_REPARSE_TAG_DOR_HSM: i32 = 82i32;
pub const IO_REPARSE_TAG_DOUBLE_TAKE_HSM: i32 = 34i32;
pub const IO_REPARSE_TAG_DOUBLE_TAKE_SIS: i32 = 41i32;
pub const IO_REPARSE_TAG_DRIVE_EXTENDER: i32 = -2147483643i32;
pub const IO_REPARSE_TAG_DROPBOX_HSM: i32 = 68i32;
pub const IO_REPARSE_TAG_EASEFILTER_HSM: i32 = 87i32;
pub const IO_REPARSE_TAG_EASEVAULT_HSM: i32 = 62i32;
pub const IO_REPARSE_TAG_EDSI_HSM: i32 = 31i32;
pub const IO_REPARSE_TAG_ELTAN_HSM: i32 = 43i32;
pub const IO_REPARSE_TAG_EMC_HSM: i32 = 57i32;
pub const IO_REPARSE_TAG_ENIGMA_HSM: i32 = 17i32;
pub const IO_REPARSE_TAG_FILTER_MANAGER: i32 = -2147483637i32;
pub const IO_REPARSE_TAG_GLOBAL360_HSM: i32 = 24i32;
pub const IO_REPARSE_TAG_GOOGLE_HSM: i32 = 65i32;
pub const IO_REPARSE_TAG_GRAU_DATASTORAGE_HSM: i32 = 28i32;
pub const IO_REPARSE_TAG_HDS_HCP_HSM: i32 = 72i32;
pub const IO_REPARSE_TAG_HDS_HSM: i32 = 63i32;
pub const IO_REPARSE_TAG_HERMES_HSM: i32 = 26i32;
pub const IO_REPARSE_TAG_HP_BACKUP: i32 = 67i32;
pub const IO_REPARSE_TAG_HP_DATA_PROTECT: i32 = 70i32;
pub const IO_REPARSE_TAG_HP_HSM: i32 = 32i32;
pub const IO_REPARSE_TAG_HSAG_HSM: i32 = 37i32;
pub const IO_REPARSE_TAG_HUBSTOR_HSM: i32 = 85i32;
pub const IO_REPARSE_TAG_IFSTEST_CONGRUENT: i32 = 9i32;
pub const IO_REPARSE_TAG_IIS_CACHE: i32 = -1610612720i32;
pub const IO_REPARSE_TAG_IMANAGE_HSM: i32 = 536870998i32;
pub const IO_REPARSE_TAG_INTERCOPE_HSM: i32 = 19i32;
pub const IO_REPARSE_TAG_ITSTATION: i32 = 74i32;
pub const IO_REPARSE_TAG_KOM_NETWORKS_HSM: i32 = 20i32;
pub const IO_REPARSE_TAG_LX_BLK: i32 = -2147483610i32;
pub const IO_REPARSE_TAG_LX_CHR: i32 = -2147483611i32;
pub const IO_REPARSE_TAG_LX_FIFO: i32 = -2147483612i32;
pub const IO_REPARSE_TAG_LX_SYMLINK: i32 = -1610612707i32;
pub const IO_REPARSE_TAG_MAGINATICS_RDR: i32 = 64i32;
pub const IO_REPARSE_TAG_MAXISCALE_HSM: i32 = 536870965i32;
pub const IO_REPARSE_TAG_MEMORY_TECH_HSM: i32 = 21i32;
pub const IO_REPARSE_TAG_MIMOSA_HSM: i32 = 36i32;
pub const IO_REPARSE_TAG_MOONWALK_HSM: i32 = 10i32;
pub const IO_REPARSE_TAG_MTALOS: i32 = 77i32;
pub const IO_REPARSE_TAG_NEUSHIELD: i32 = 81i32;
pub const IO_REPARSE_TAG_NEXSAN_HSM: i32 = 40i32;
pub const IO_REPARSE_TAG_NIPPON_HSM: i32 = 79i32;
pub const IO_REPARSE_TAG_NVIDIA_UNIONFS: i32 = 536870996i32;
pub const IO_REPARSE_TAG_OPENAFS_DFS: i32 = 55i32;
pub const IO_REPARSE_TAG_OSR_SAMPLE: i32 = 536870935i32;
pub const IO_REPARSE_TAG_OVERTONE: i32 = 15i32;
pub const IO_REPARSE_TAG_POINTSOFT_HSM: i32 = 27i32;
pub const IO_REPARSE_TAG_QI_TECH_HSM: i32 = 536870959i32;
pub const IO_REPARSE_TAG_QUADDRA_HSM: i32 = 66i32;
pub const IO_REPARSE_TAG_QUEST_HSM: i32 = 45i32;
pub const IO_REPARSE_TAG_REDSTOR_HSM: i32 = 80i32;
pub const IO_REPARSE_TAG_RIVERBED_HSM: i32 = 51i32;
pub const IO_REPARSE_TAG_SER_HSM: i32 = 33i32;
pub const IO_REPARSE_TAG_SHX_BACKUP: i32 = 83i32;
pub const IO_REPARSE_TAG_SOLUTIONSOFT: i32 = 536870925i32;
pub const IO_REPARSE_TAG_SONY_HSM: i32 = 42i32;
pub const IO_REPARSE_TAG_SPHARSOFT: i32 = 75i32;
pub const IO_REPARSE_TAG_SYMANTEC_HSM: i32 = 18i32;
pub const IO_REPARSE_TAG_SYMANTEC_HSM2: i32 = 16i32;
pub const IO_REPARSE_TAG_TSINGHUA_UNIVERSITY_RESEARCH: i32 = 11i32;
pub const IO_REPARSE_TAG_UTIXO_HSM: i32 = 44i32;
pub const IO_REPARSE_TAG_VALID_VALUES: u32 = 4026597375u32;
pub const IO_REPARSE_TAG_VMWARE_PM: i32 = 58i32;
pub const IO_REPARSE_TAG_WATERFORD: i32 = 50i32;
pub const IO_REPARSE_TAG_WISDATA_HSM: i32 = 35i32;
pub const IO_REPARSE_TAG_ZLTI_HSM: i32 = 56i32;
pub const IO_STOP_ON_SYMLINK: u32 = 8u32;
pub const KnownFolderDesktop: FILE_KNOWN_FOLDER_TYPE = 1i32;
pub const KnownFolderDocuments: FILE_KNOWN_FOLDER_TYPE = 2i32;
pub const KnownFolderDownloads: FILE_KNOWN_FOLDER_TYPE = 3i32;
pub const KnownFolderMax: FILE_KNOWN_FOLDER_TYPE = 7i32;
pub const KnownFolderMusic: FILE_KNOWN_FOLDER_TYPE = 4i32;
pub const KnownFolderNone: FILE_KNOWN_FOLDER_TYPE = 0i32;
pub const KnownFolderOther: FILE_KNOWN_FOLDER_TYPE = 7i32;
pub const KnownFolderPictures: FILE_KNOWN_FOLDER_TYPE = 5i32;
pub const KnownFolderVideos: FILE_KNOWN_FOLDER_TYPE = 6i32;
pub const LCN_CHECKSUM_VALID: _LCN_WEAK_REFERENCE_STATE = 2i32;
pub const LCN_WEAK_REFERENCE_VALID: _LCN_WEAK_REFERENCE_STATE = 1i32;
pub const LX_FILE_CASE_SENSITIVE_DIR: u32 = 16u32;
pub const LX_FILE_METADATA_DEVICE_ID_EA_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("$LXDEV");
pub const LX_FILE_METADATA_GID_EA_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("$LXGID");
pub const LX_FILE_METADATA_HAS_DEVICE_ID: u32 = 8u32;
pub const LX_FILE_METADATA_HAS_GID: u32 = 2u32;
pub const LX_FILE_METADATA_HAS_MODE: u32 = 4u32;
pub const LX_FILE_METADATA_HAS_UID: u32 = 1u32;
pub const LX_FILE_METADATA_MODE_EA_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("$LXMOD");
pub const LX_FILE_METADATA_UID_EA_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("$LXUID");
pub const LessThan: FSRTL_COMPARISON_RESULT = -1i32;
pub const MAP_DISABLE_PAGEFAULT_CLUSTERING: u32 = 256u32;
pub const MAP_HIGH_PRIORITY: u32 = 64u32;
pub const MAP_NO_READ: u32 = 16u32;
pub const MAP_WAIT: u32 = 1u32;
pub const MAXIMUM_LEADBYTES: u32 = 12u32;
pub const MAX_UNICODE_STACK_BUFFER_LENGTH: u32 = 256u32;
pub const MCB_FLAG_RAISE_ON_ALLOCATION_FAILURE: u32 = 1u32;
pub const MM_FORCE_CLOSED_DATA: u32 = 1u32;
pub const MM_FORCE_CLOSED_IMAGE: u32 = 2u32;
pub const MM_FORCE_CLOSED_LATER_OK: u32 = 4u32;
pub const MM_IS_FILE_SECTION_ACTIVE_DATA: u32 = 2u32;
pub const MM_IS_FILE_SECTION_ACTIVE_IMAGE: u32 = 1u32;
pub const MM_IS_FILE_SECTION_ACTIVE_USER: u32 = 4u32;
pub const MemoryBasicInformation: MEMORY_INFORMATION_CLASS = 0i32;
pub const MemoryType64KPage: RTL_MEMORY_TYPE = 2i32;
pub const MemoryTypeCustom: RTL_MEMORY_TYPE = 5i32;
pub const MemoryTypeHugePage: RTL_MEMORY_TYPE = 4i32;
pub const MemoryTypeLargePage: RTL_MEMORY_TYPE = 3i32;
pub const MemoryTypeMax: RTL_MEMORY_TYPE = 6i32;
pub const MemoryTypeNonPaged: RTL_MEMORY_TYPE = 1i32;
pub const MemoryTypePaged: RTL_MEMORY_TYPE = 0i32;
pub const MmFlushForDelete: MMFLUSH_TYPE = 0i32;
pub const MmFlushForWrite: MMFLUSH_TYPE = 1i32;
pub const MsvAvChannelBindings: MSV1_0_AVID = 10i32;
pub const MsvAvDnsComputerName: MSV1_0_AVID = 3i32;
pub const MsvAvDnsDomainName: MSV1_0_AVID = 4i32;
pub const MsvAvDnsTreeName: MSV1_0_AVID = 5i32;
pub const MsvAvEOL: MSV1_0_AVID = 0i32;
pub const MsvAvFlags: MSV1_0_AVID = 6i32;
pub const MsvAvNbComputerName: MSV1_0_AVID = 1i32;
pub const MsvAvNbDomainName: MSV1_0_AVID = 2i32;
pub const MsvAvRestrictions: MSV1_0_AVID = 8i32;
pub const MsvAvTargetName: MSV1_0_AVID = 9i32;
pub const MsvAvTimestamp: MSV1_0_AVID = 7i32;
pub const NETWORK_OPEN_ECP_IN_FLAG_DISABLE_HANDLE_COLLAPSING: u32 = 1u32;
pub const NETWORK_OPEN_ECP_IN_FLAG_DISABLE_HANDLE_DURABILITY: u32 = 2u32;
pub const NETWORK_OPEN_ECP_IN_FLAG_DISABLE_OPLOCKS: u32 = 4u32;
pub const NETWORK_OPEN_ECP_IN_FLAG_FORCE_BUFFERED_SYNCHRONOUS_IO_HACK: u32 = 2147483648u32;
pub const NETWORK_OPEN_ECP_IN_FLAG_FORCE_MAX_EOF_HACK: u32 = 1073741824u32;
pub const NETWORK_OPEN_ECP_IN_FLAG_REQ_MUTUAL_AUTH: u32 = 8u32;
pub const NETWORK_OPEN_ECP_OUT_FLAG_RET_MUTUAL_AUTH: u32 = 8u32;
pub const NO_8DOT3_NAME_PRESENT: u32 = 1u32;
pub const NetworkOpenIntegrityAny: NETWORK_OPEN_INTEGRITY_QUALIFIER = 0i32;
pub const NetworkOpenIntegrityEncrypted: NETWORK_OPEN_INTEGRITY_QUALIFIER = 3i32;
pub const NetworkOpenIntegrityMaximum: NETWORK_OPEN_INTEGRITY_QUALIFIER = 4i32;
pub const NetworkOpenIntegrityNone: NETWORK_OPEN_INTEGRITY_QUALIFIER = 1i32;
pub const NetworkOpenIntegritySigned: NETWORK_OPEN_INTEGRITY_QUALIFIER = 2i32;
pub const NetworkOpenLocationAny: NETWORK_OPEN_LOCATION_QUALIFIER = 0i32;
pub const NetworkOpenLocationLoopback: NETWORK_OPEN_LOCATION_QUALIFIER = 2i32;
pub const NetworkOpenLocationRemote: NETWORK_OPEN_LOCATION_QUALIFIER = 1i32;
pub const NotifyTypeCreate: FS_FILTER_STREAM_FO_NOTIFICATION_TYPE = 0i32;
pub const NotifyTypeRetired: FS_FILTER_STREAM_FO_NOTIFICATION_TYPE = 1i32;
pub const NtfsLinkTrackingInformation: LINK_TRACKING_INFORMATION_TYPE = 0i32;
pub const OPEN_REPARSE_POINT_OVERRIDE_CREATE_OPTION: u32 = 64u32;
pub const OPEN_REPARSE_POINT_REPARSE_ALWAYS: u32 = 126u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_CHILD_EXISTS: u32 = 2u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_CHILD_NOT_EXISTS: u32 = 4u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_DIRECTORY_FINAL_COMPONENT: u32 = 8u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_DIRECTORY_FINAL_COMPONENT_ALWAYS: u32 = 72u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_FINAL_COMPONENT: u32 = 40u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_FINAL_COMPONENT_ALWAYS: u32 = 104u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_NON_DIRECTORY_FINAL_COMPONENT: u32 = 32u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_NON_DIRECTORY_FINAL_COMPONENT_ALWAYS: u32 = 96u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_NON_DIRECTORY_NON_FINAL_COMPONENT: u32 = 16u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_NON_DIRECTORY_NON_FINAL_COMPONENT_ALWAYS: u32 = 80u32;
pub const OPEN_REPARSE_POINT_REPARSE_IF_NON_FINAL_COMPONENT: u32 = 22u32;
pub const OPEN_REPARSE_POINT_RETURN_REPARSE_DATA_BUFFER: u32 = 128u32;
pub const OPEN_REPARSE_POINT_TAG_ENCOUNTERED: u32 = 1u32;
pub const OPEN_REPARSE_POINT_VERSION_EX: u32 = 2147483648u32;
pub const OPLOCK_FLAG_BACK_OUT_ATOMIC_OPLOCK: u32 = 4u32;
pub const OPLOCK_FLAG_BREAKING_FOR_SHARING_VIOLATION: u32 = 128u32;
pub const OPLOCK_FLAG_CLOSING_DELETE_ON_CLOSE: u32 = 32u32;
pub const OPLOCK_FLAG_COMPLETE_IF_OPLOCKED: u32 = 1u32;
pub const OPLOCK_FLAG_IGNORE_OPLOCK_KEYS: u32 = 8u32;
pub const OPLOCK_FLAG_OPLOCK_KEY_CHECK_ONLY: u32 = 2u32;
pub const OPLOCK_FLAG_PARENT_OBJECT: u32 = 16u32;
pub const OPLOCK_FLAG_REMOVING_FILE_OR_LINK: u32 = 64u32;
pub const OPLOCK_FSCTRL_FLAG_ALL_KEYS_MATCH: u32 = 1u32;
pub const OPLOCK_NOTIFY_BREAK_WAIT_INTERIM_TIMEOUT: OPLOCK_NOTIFY_REASON = 0i32;
pub const OPLOCK_NOTIFY_BREAK_WAIT_TERMINATED: OPLOCK_NOTIFY_REASON = 1i32;
pub const OPLOCK_UPPER_FLAG_CHECK_NO_BREAK: u32 = 65536u32;
pub const OPLOCK_UPPER_FLAG_NOTIFY_REFRESH_READ: u32 = 131072u32;
pub const PIN_CALLER_TRACKS_DIRTY_DATA: u32 = 32u32;
pub const PIN_EXCLUSIVE: u32 = 2u32;
pub const PIN_HIGH_PRIORITY: u32 = 64u32;
pub const PIN_IF_BCB: u32 = 8u32;
pub const PIN_NO_READ: u32 = 4u32;
pub const PIN_VERIFY_REQUIRED: u32 = 128u32;
pub const PIN_WAIT: u32 = 1u32;
pub const POLICY_AUDIT_SUBCATEGORY_COUNT: u32 = 59u32;
pub const PO_CB_AC_STATUS: u32 = 1u32;
pub const PO_CB_BUTTON_COLLISION: u32 = 2u32;
pub const PO_CB_LID_SWITCH_STATE: u32 = 4u32;
pub const PO_CB_PROCESSOR_POWER_POLICY: u32 = 5u32;
pub const PO_CB_SYSTEM_POWER_POLICY: u32 = 0u32;
pub const PO_CB_SYSTEM_STATE_LOCK: u32 = 3u32;
pub const PSMP_MAXIMUM_SYSAPP_CLAIM_VALUES: u32 = 4u32;
pub const PSMP_MINIMUM_SYSAPP_CLAIM_VALUES: u32 = 2u32;
pub const PURGE_WITH_ACTIVE_VIEWS: u32 = 8u32;
pub const QUERY_DIRECT_ACCESS_DATA_EXTENTS: u32 = 2u32;
pub const QUERY_DIRECT_ACCESS_IMAGE_EXTENTS: u32 = 1u32;
pub const QoCFileEaInformation: u32 = 4u32;
pub const QoCFileLxInformation: u32 = 2u32;
pub const QoCFileStatInformation: u32 = 1u32;
pub const REFS_COMPRESSION_FORMAT_LZ4: REFS_COMPRESSION_FORMATS = 1i32;
pub const REFS_COMPRESSION_FORMAT_MAX: REFS_COMPRESSION_FORMATS = 3i32;
pub const REFS_COMPRESSION_FORMAT_UNCOMPRESSED: REFS_COMPRESSION_FORMATS = 0i32;
pub const REFS_COMPRESSION_FORMAT_ZSTD: REFS_COMPRESSION_FORMATS = 2i32;
pub const REFS_DEALLOCATE_RANGES_ALLOCATOR_CAA: REFS_DEALLOCATE_RANGES_ALLOCATOR = 2i32;
pub const REFS_DEALLOCATE_RANGES_ALLOCATOR_MAA: REFS_DEALLOCATE_RANGES_ALLOCATOR = 3i32;
pub const REFS_DEALLOCATE_RANGES_ALLOCATOR_NONE: REFS_DEALLOCATE_RANGES_ALLOCATOR = 0i32;
pub const REFS_DEALLOCATE_RANGES_ALLOCATOR_SAA: REFS_DEALLOCATE_RANGES_ALLOCATOR = 1i32;
pub const REFS_SET_VOLUME_COMPRESSION_INFO_FLAG_COMPRESS_SYNC: REFS_SET_VOLUME_COMPRESSION_INFO_FLAGS = 1i32;
pub const REFS_SET_VOLUME_COMPRESSION_INFO_FLAG_MAX: REFS_SET_VOLUME_COMPRESSION_INFO_FLAGS = 1i32;
pub const REFS_STREAM_EXTENT_PROPERTY_CRC32: _REFS_STREAM_EXTENT_PROPERTIES = 128i32;
pub const REFS_STREAM_EXTENT_PROPERTY_CRC64: _REFS_STREAM_EXTENT_PROPERTIES = 256i32;
pub const REFS_STREAM_EXTENT_PROPERTY_GHOSTED: _REFS_STREAM_EXTENT_PROPERTIES = 512i32;
pub const REFS_STREAM_EXTENT_PROPERTY_READONLY: _REFS_STREAM_EXTENT_PROPERTIES = 1024i32;
pub const REFS_STREAM_EXTENT_PROPERTY_SPARSE: _REFS_STREAM_EXTENT_PROPERTIES = 8i32;
pub const REFS_STREAM_EXTENT_PROPERTY_STREAM_RESERVED: _REFS_STREAM_EXTENT_PROPERTIES = 32i32;
pub const REFS_STREAM_EXTENT_PROPERTY_VALID: _REFS_STREAM_EXTENT_PROPERTIES = 16i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_CLEAR_SHADOW_BTREE: REFS_STREAM_SNAPSHOT_OPERATION = 6i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_CREATE: REFS_STREAM_SNAPSHOT_OPERATION = 1i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_INVALID: REFS_STREAM_SNAPSHOT_OPERATION = 0i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_LIST: REFS_STREAM_SNAPSHOT_OPERATION = 2i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_MAX: REFS_STREAM_SNAPSHOT_OPERATION = 6i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_QUERY_DELTAS: REFS_STREAM_SNAPSHOT_OPERATION = 3i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_REVERT: REFS_STREAM_SNAPSHOT_OPERATION = 4i32;
pub const REFS_STREAM_SNAPSHOT_OPERATION_SET_SHADOW_BTREE: REFS_STREAM_SNAPSHOT_OPERATION = 5i32;
pub const REMOTE_PROTOCOL_FLAG_INTEGRITY: u32 = 16u32;
pub const REMOTE_PROTOCOL_FLAG_LOOPBACK: u32 = 1u32;
pub const REMOTE_PROTOCOL_FLAG_MUTUAL_AUTH: u32 = 32u32;
pub const REMOTE_PROTOCOL_FLAG_OFFLINE: u32 = 2u32;
pub const REMOTE_PROTOCOL_FLAG_PERSISTENT_HANDLE: u32 = 4u32;
pub const REMOTE_PROTOCOL_FLAG_PRIVACY: u32 = 8u32;
pub const REMOVED_8DOT3_NAME: u32 = 2u32;
pub const REPARSE_DATA_EX_FLAG_GIVEN_TAG_OR_NONE: u32 = 1u32;
pub const RETURN_NON_NT_USER_SESSION_KEY: u32 = 8u32;
pub const RETURN_PRIMARY_LOGON_DOMAINNAME: u32 = 4u32;
pub const RETURN_PRIMARY_USERNAME: u32 = 2u32;
pub const RETURN_RESERVED_PARAMETER: u32 = 128u32;
pub const RPI_SMB2_SERVERCAP_DFS: u32 = 1u32;
pub const RPI_SMB2_SERVERCAP_DIRECTORY_LEASING: u32 = 32u32;
pub const RPI_SMB2_SERVERCAP_ENCRYPTION_AWARE: u32 = 64u32;
pub const RPI_SMB2_SERVERCAP_LARGEMTU: u32 = 4u32;
pub const RPI_SMB2_SERVERCAP_LEASING: u32 = 2u32;
pub const RPI_SMB2_SERVERCAP_MULTICHANNEL: u32 = 8u32;
pub const RPI_SMB2_SERVERCAP_PERSISTENT_HANDLES: u32 = 16u32;
pub const RPI_SMB2_SHARECAP_ACCESS_BASED_DIRECTORY_ENUM: u32 = 256u32;
pub const RPI_SMB2_SHARECAP_ASYMMETRIC_SCALEOUT: u32 = 1024u32;
pub const RPI_SMB2_SHARECAP_CLUSTER: u32 = 64u32;
pub const RPI_SMB2_SHARECAP_CONTINUOUS_AVAILABILITY: u32 = 16u32;
pub const RPI_SMB2_SHARECAP_DFS: u32 = 8u32;
pub const RPI_SMB2_SHARECAP_ENCRYPTED: u32 = 128u32;
pub const RPI_SMB2_SHARECAP_IDENTITY_REMOTING: u32 = 512u32;
pub const RPI_SMB2_SHARECAP_SCALEOUT: u32 = 32u32;
pub const RPI_SMB2_SHARECAP_TIMEWARP: u32 = 2u32;
pub const RPI_SMB2_SHARETYPE_DISK: u32 = 0u32;
pub const RPI_SMB2_SHARETYPE_PIPE: u32 = 1u32;
pub const RPI_SMB2_SHARETYPE_PRINT: u32 = 2u32;
pub const RTL_DUPLICATE_UNICODE_STRING_ALLOCATE_NULL_STRING: u32 = 2u32;
pub const RTL_DUPLICATE_UNICODE_STRING_NULL_TERMINATE: u32 = 1u32;
pub const RTL_HEAP_MEMORY_LIMIT_CURRENT_VERSION: u32 = 1u32;
pub const RTL_SYSTEM_VOLUME_INFORMATION_FOLDER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("System Volume Information");
pub const SECURITY_ANONYMOUS_LOGON_RID: i32 = 7i32;
pub const SEGMENT_HEAP_FLG_USE_PAGE_HEAP: u32 = 1u32;
pub const SEGMENT_HEAP_PARAMETERS_VERSION: u32 = 3u32;
pub const SEGMENT_HEAP_PARAMS_VALID_FLAGS: u32 = 1u32;
pub const SEMAPHORE_INCREMENT: u32 = 1u32;
pub const SET_PURGE_FAILURE_MODE_DISABLED: u32 = 2u32;
pub const SE_BACKUP_PRIVILEGES_CHECKED: u32 = 256u32;
pub const SE_DACL_UNTRUSTED: u32 = 64u32;
pub const SE_SERVER_SECURITY: u32 = 128u32;
pub const SPECIAL_ENCRYPTED_OPEN: u32 = 262144u32;
pub const SRV_OPEN_ECP_CONTEXT_VERSION_2: u32 = 2u32;
pub const SUPPORTED_FS_FEATURES_BYPASS_IO: u32 = 8u32;
pub const SUPPORTED_FS_FEATURES_OFFLOAD_READ: u32 = 1u32;
pub const SUPPORTED_FS_FEATURES_OFFLOAD_WRITE: u32 = 2u32;
pub const SUPPORTED_FS_FEATURES_QUERY_OPEN: u32 = 4u32;
pub const SYMLINK_DIRECTORY: u32 = 2147483648u32;
pub const SYMLINK_FILE: u32 = 1073741824u32;
pub const SYMLINK_FLAG_RELATIVE: u32 = 1u32;
pub const SYMLINK_RESERVED_MASK: u32 = 4026531840u32;
pub const SYSTEM_PAGE_PRIORITY_BITS: u32 = 3u32;
pub const SharedVirtualDiskCDPSnapshotsSupported: SharedVirtualDiskSupportType = 7i32;
pub const SharedVirtualDiskHandleStateFileShared: SharedVirtualDiskHandleState = 1i32;
pub const SharedVirtualDiskHandleStateHandleShared: SharedVirtualDiskHandleState = 3i32;
pub const SharedVirtualDiskHandleStateNone: SharedVirtualDiskHandleState = 0i32;
pub const SharedVirtualDiskSnapshotsSupported: SharedVirtualDiskSupportType = 3i32;
pub const SharedVirtualDisksSupported: SharedVirtualDiskSupportType = 1i32;
pub const SharedVirtualDisksUnsupported: SharedVirtualDiskSupportType = 0i32;
pub const SrvInstanceTypeCsv: SRV_INSTANCE_TYPE = 2i32;
pub const SrvInstanceTypePrimary: SRV_INSTANCE_TYPE = 1i32;
pub const SrvInstanceTypeSBL: SRV_INSTANCE_TYPE = 3i32;
pub const SrvInstanceTypeSR: SRV_INSTANCE_TYPE = 4i32;
pub const SrvInstanceTypeUndefined: SRV_INSTANCE_TYPE = 0i32;
pub const SrvInstanceTypeVSMB: SRV_INSTANCE_TYPE = 5i32;
pub const SyncTypeCreateSection: FS_FILTER_SECTION_SYNC_TYPE = 1i32;
pub const SyncTypeOther: FS_FILTER_SECTION_SYNC_TYPE = 0i32;
pub const TOKEN_AUDIT_NO_CHILD_PROCESS: u32 = 2097152u32;
pub const TOKEN_AUDIT_REDIRECTION_TRUST: u32 = 8388608u32;
pub const TOKEN_DO_NOT_USE_GLOBAL_ATTRIBS_FOR_QUERY: u32 = 131072u32;
pub const TOKEN_ENFORCE_REDIRECTION_TRUST: u32 = 4194304u32;
pub const TOKEN_HAS_BACKUP_PRIVILEGE: u32 = 2u32;
pub const TOKEN_HAS_IMPERSONATE_PRIVILEGE: u32 = 128u32;
pub const TOKEN_HAS_OWN_CLAIM_ATTRIBUTES: u32 = 32768u32;
pub const TOKEN_HAS_RESTORE_PRIVILEGE: u32 = 4u32;
pub const TOKEN_HAS_TRAVERSE_PRIVILEGE: u32 = 1u32;
pub const TOKEN_IS_FILTERED: u32 = 2048u32;
pub const TOKEN_IS_RESTRICTED: u32 = 16u32;
pub const TOKEN_LEARNING_MODE_LOGGING: u32 = 16777216u32;
pub const TOKEN_LOWBOX: u32 = 16384u32;
pub const TOKEN_NOT_LOW: u32 = 8192u32;
pub const TOKEN_NO_CHILD_PROCESS: u32 = 524288u32;
pub const TOKEN_NO_CHILD_PROCESS_UNLESS_SECURE: u32 = 1048576u32;
pub const TOKEN_PERMISSIVE_LEARNING_MODE: u32 = 50331648u32;
pub const TOKEN_PRIVATE_NAMESPACE: u32 = 65536u32;
pub const TOKEN_SANDBOX_INERT: u32 = 64u32;
pub const TOKEN_SESSION_NOT_REFERENCED: u32 = 32u32;
pub const TOKEN_UIACCESS: u32 = 4096u32;
pub const TOKEN_VIRTUALIZE_ALLOWED: u32 = 512u32;
pub const TOKEN_VIRTUALIZE_ENABLED: u32 = 1024u32;
pub const TOKEN_WRITE_RESTRICTED: u32 = 8u32;
pub const UNINITIALIZE_CACHE_MAPS: u32 = 1u32;
pub const USE_PRIMARY_PASSWORD: u32 = 1u32;
pub const USN_DELETE_FLAG_DELETE: u32 = 1u32;
pub const VACB_MAPPING_GRANULARITY: u32 = 262144u32;
pub const VACB_OFFSET_SHIFT: u32 = 18u32;
pub const VALID_INHERIT_FLAGS: u32 = 31u32;
pub const VOLSNAPCONTROLTYPE: u32 = 83u32;
pub const VmPrefetchInformation: VIRTUAL_MEMORY_INFORMATION_CLASS = 0i32;
pub const WCIFS_REDIRECTION_FLAGS_CREATE_SERVICED_FROM_LAYER: u32 = 1u32;
pub const WCIFS_REDIRECTION_FLAGS_CREATE_SERVICED_FROM_REGISTERED_LAYER: u32 = 4u32;
pub const WCIFS_REDIRECTION_FLAGS_CREATE_SERVICED_FROM_REMOTE_LAYER: u32 = 8u32;
pub const WCIFS_REDIRECTION_FLAGS_CREATE_SERVICED_FROM_SCRATCH: u32 = 2u32;
pub type CSV_DOWN_LEVEL_FILE_TYPE = i32;
pub type FAST_IO_POSSIBLE = i32;
pub type FILE_DISPOSITION_INFORMATION_EX_FLAGS = u32;
pub type FILE_KNOWN_FOLDER_TYPE = i32;
pub type FSRTL_CHANGE_BACKING_TYPE = i32;
pub type FSRTL_COMPARISON_RESULT = i32;
pub type FS_FILTER_SECTION_SYNC_TYPE = i32;
pub type FS_FILTER_STREAM_FO_NOTIFICATION_TYPE = i32;
pub type FS_INFORMATION_CLASS = i32;
pub type HEAP_MEMORY_INFO_CLASS = i32;
pub type LINK_TRACKING_INFORMATION_TYPE = i32;
pub type MEMORY_INFORMATION_CLASS = i32;
pub type MMFLUSH_TYPE = i32;
pub type MSV1_0_AVID = i32;
pub type NETWORK_OPEN_INTEGRITY_QUALIFIER = i32;
pub type NETWORK_OPEN_LOCATION_QUALIFIER = i32;
pub type NTCREATEFILE_CREATE_DISPOSITION = u32;
pub type NTCREATEFILE_CREATE_OPTIONS = u32;
pub type OPLOCK_NOTIFY_REASON = i32;
pub type REFS_COMPRESSION_FORMATS = i32;
pub type REFS_DEALLOCATE_RANGES_ALLOCATOR = i32;
pub type REFS_SET_VOLUME_COMPRESSION_INFO_FLAGS = i32;
pub type REFS_STREAM_SNAPSHOT_OPERATION = i32;
pub type RTL_MEMORY_TYPE = i32;
pub type SE_AUDIT_OPERATION = i32;
pub type SRV_INSTANCE_TYPE = i32;
pub type SharedVirtualDiskHandleState = i32;
pub type SharedVirtualDiskSupportType = i32;
pub type VIRTUAL_MEMORY_INFORMATION_CLASS = i32;
pub type _LCN_WEAK_REFERENCE_STATE = i32;
pub type _REFS_STREAM_EXTENT_PROPERTIES = i32;
#[repr(C)]
pub struct ACE_HEADER {
    pub AceType: u8,
    pub AceFlags: u8,
    pub AceSize: u16,
}
impl ::core::marker::Copy for ACE_HEADER {}
impl ::core::clone::Clone for ACE_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ATOMIC_CREATE_ECP_CONTEXT {
    pub Size: u16,
    pub InFlags: u16,
    pub OutFlags: u16,
    pub ReparseBufferLength: u16,
    pub ReparseBuffer: *mut REPARSE_DATA_BUFFER,
    pub FileSize: i64,
    pub ValidDataLength: i64,
    pub FileTimestamps: *mut FILE_TIMESTAMPS,
    pub FileAttributes: u32,
    pub UsnSourceInfo: u32,
    pub Usn: i64,
    pub SuppressFileAttributeInheritanceMask: u32,
    pub InOpFlags: u32,
    pub OutOpFlags: u32,
    pub InGenFlags: u32,
    pub OutGenFlags: u32,
    pub CaseSensitiveFlagsMask: u32,
    pub InCaseSensitiveFlags: u32,
    pub OutCaseSensitiveFlags: u32,
}
impl ::core::marker::Copy for ATOMIC_CREATE_ECP_CONTEXT {}
impl ::core::clone::Clone for ATOMIC_CREATE_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BASE_MCB {
    pub MaximumPairCount: u32,
    pub PairCount: u32,
    pub PoolType: u16,
    pub Flags: u16,
    pub Mapping: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for BASE_MCB {}
impl ::core::clone::Clone for BASE_MCB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BOOT_AREA_INFO {
    pub BootSectorCount: u32,
    pub BootSectors: [BOOT_AREA_INFO_0; 2],
}
impl ::core::marker::Copy for BOOT_AREA_INFO {}
impl ::core::clone::Clone for BOOT_AREA_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BOOT_AREA_INFO_0 {
    pub Offset: i64,
}
impl ::core::marker::Copy for BOOT_AREA_INFO_0 {}
impl ::core::clone::Clone for BOOT_AREA_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CACHE_MANAGER_CALLBACKS {
    pub AcquireForLazyWrite: PACQUIRE_FOR_LAZY_WRITE,
    pub ReleaseFromLazyWrite: PRELEASE_FROM_LAZY_WRITE,
    pub AcquireForReadAhead: PACQUIRE_FOR_READ_AHEAD,
    pub ReleaseFromReadAhead: PRELEASE_FROM_READ_AHEAD,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CACHE_MANAGER_CALLBACKS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CACHE_MANAGER_CALLBACKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CACHE_MANAGER_CALLBACKS_EX {
    pub Version: u16,
    pub Size: u16,
    pub Functions: CACHE_MANAGER_CALLBACK_FUNCTIONS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CACHE_MANAGER_CALLBACKS_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CACHE_MANAGER_CALLBACKS_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CACHE_MANAGER_CALLBACK_FUNCTIONS {
    pub AcquireForLazyWriteEx: PACQUIRE_FOR_LAZY_WRITE_EX,
    pub ReleaseFromLazyWrite: PRELEASE_FROM_LAZY_WRITE,
    pub AcquireForReadAhead: PACQUIRE_FOR_READ_AHEAD,
    pub ReleaseFromReadAhead: PRELEASE_FROM_READ_AHEAD,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CACHE_MANAGER_CALLBACK_FUNCTIONS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CACHE_MANAGER_CALLBACK_FUNCTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct CACHE_UNINITIALIZE_EVENT {
    pub Next: *mut CACHE_UNINITIALIZE_EVENT,
    pub Event: super::super::Foundation::KEVENT,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for CACHE_UNINITIALIZE_EVENT {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for CACHE_UNINITIALIZE_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
pub struct CC_ASYNC_READ_CONTEXT {
    pub CompletionRoutine: PASYNC_READ_COMPLETION_CALLBACK,
    pub Context: *mut ::core::ffi::c_void,
    pub Mdl: *mut super::super::Foundation::MDL,
    pub RequestorMode: i8,
    pub NestingLevel: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
impl ::core::marker::Copy for CC_ASYNC_READ_CONTEXT {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
impl ::core::clone::Clone for CC_ASYNC_READ_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CC_ERROR_CALLBACK_CONTEXT {
    pub NodeByteSize: i16,
    pub ErrorCode: super::super::super::Win32::Foundation::NTSTATUS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CC_ERROR_CALLBACK_CONTEXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CC_ERROR_CALLBACK_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CC_FILE_SIZES {
    pub AllocationSize: i64,
    pub FileSize: i64,
    pub ValidDataLength: i64,
}
impl ::core::marker::Copy for CC_FILE_SIZES {}
impl ::core::clone::Clone for CC_FILE_SIZES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COMPRESSED_DATA_INFO {
    pub CompressionFormatAndEngine: u16,
    pub CompressionUnitShift: u8,
    pub ChunkShift: u8,
    pub ClusterShift: u8,
    pub Reserved: u8,
    pub NumberOfChunks: u16,
    pub CompressedChunkSizes: [u32; 1],
}
impl ::core::marker::Copy for COMPRESSED_DATA_INFO {}
impl ::core::clone::Clone for COMPRESSED_DATA_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CONTAINER_ROOT_INFO_INPUT {
    pub Flags: u32,
}
impl ::core::marker::Copy for CONTAINER_ROOT_INFO_INPUT {}
impl ::core::clone::Clone for CONTAINER_ROOT_INFO_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CONTAINER_ROOT_INFO_OUTPUT {
    pub ContainerRootIdLength: u16,
    pub ContainerRootId: [u8; 1],
}
impl ::core::marker::Copy for CONTAINER_ROOT_INFO_OUTPUT {}
impl ::core::clone::Clone for CONTAINER_ROOT_INFO_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CONTAINER_VOLUME_STATE {
    pub Flags: u32,
}
impl ::core::marker::Copy for CONTAINER_VOLUME_STATE {}
impl ::core::clone::Clone for CONTAINER_VOLUME_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct COPY_INFORMATION {
    pub SourceFileObject: *mut super::super::Foundation::FILE_OBJECT,
    pub SourceFileOffset: i64,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for COPY_INFORMATION {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for COPY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPTABLEINFO {
    pub CodePage: u16,
    pub MaximumCharacterSize: u16,
    pub DefaultChar: u16,
    pub UniDefaultChar: u16,
    pub TransDefaultChar: u16,
    pub TransUniDefaultChar: u16,
    pub DBCSCodePage: u16,
    pub LeadByte: [u8; 12],
    pub MultiByteTable: *mut u16,
    pub WideCharTable: *mut ::core::ffi::c_void,
    pub DBCSRanges: *mut u16,
    pub DBCSOffsets: *mut u16,
}
impl ::core::marker::Copy for CPTABLEINFO {}
impl ::core::clone::Clone for CPTABLEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct CREATE_REDIRECTION_ECP_CONTEXT {
    pub Size: u16,
    pub Flags: u16,
    pub FileId: super::super::super::Win32::Storage::FileSystem::FILE_ID_128,
    pub VolumeGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for CREATE_REDIRECTION_ECP_CONTEXT {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for CREATE_REDIRECTION_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CREATE_USN_JOURNAL_DATA {
    pub MaximumSize: u64,
    pub AllocationDelta: u64,
}
impl ::core::marker::Copy for CREATE_USN_JOURNAL_DATA {}
impl ::core::clone::Clone for CREATE_USN_JOURNAL_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CSV_DOWN_LEVEL_OPEN_ECP_CONTEXT {
    pub Version: u32,
    pub IsResume: super::super::super::Win32::Foundation::BOOLEAN,
    pub FileType: CSV_DOWN_LEVEL_FILE_TYPE,
    pub SourceNodeId: u32,
    pub DestinationNodeId: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CSV_DOWN_LEVEL_OPEN_ECP_CONTEXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CSV_DOWN_LEVEL_OPEN_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CSV_QUERY_FILE_REVISION_ECP_CONTEXT {
    pub FileId: i64,
    pub FileRevision: [i64; 3],
}
impl ::core::marker::Copy for CSV_QUERY_FILE_REVISION_ECP_CONTEXT {}
impl ::core::clone::Clone for CSV_QUERY_FILE_REVISION_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct CSV_QUERY_FILE_REVISION_ECP_CONTEXT_FILE_ID_128 {
    pub FileId: super::super::super::Win32::Storage::FileSystem::FILE_ID_128,
    pub FileRevision: [i64; 3],
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for CSV_QUERY_FILE_REVISION_ECP_CONTEXT_FILE_ID_128 {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for CSV_QUERY_FILE_REVISION_ECP_CONTEXT_FILE_ID_128 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CSV_SET_HANDLE_PROPERTIES_ECP_CONTEXT {
    pub Size: usize,
    pub PauseTimeoutInSeconds: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for CSV_SET_HANDLE_PROPERTIES_ECP_CONTEXT {}
impl ::core::clone::Clone for CSV_SET_HANDLE_PROPERTIES_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DUAL_OPLOCK_KEY_ECP_CONTEXT {
    pub ParentOplockKey: ::windows_sys::core::GUID,
    pub TargetOplockKey: ::windows_sys::core::GUID,
    pub ParentOplockKeySet: super::super::super::Win32::Foundation::BOOLEAN,
    pub TargetOplockKeySet: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DUAL_OPLOCK_KEY_ECP_CONTEXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DUAL_OPLOCK_KEY_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DUPLICATE_CLUSTER_DATA {
    pub SourceLcn: i64,
    pub TargetFileOffset: i64,
    pub DuplicationLimit: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for DUPLICATE_CLUSTER_DATA {}
impl ::core::clone::Clone for DUPLICATE_CLUSTER_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ECP_OPEN_PARAMETERS {
    pub Size: u16,
    pub Reserved: u16,
    pub Flags: u32,
}
impl ::core::marker::Copy for ECP_OPEN_PARAMETERS {}
impl ::core::clone::Clone for ECP_OPEN_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct EOF_WAIT_BLOCK {
    pub EofWaitLinks: super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub Event: super::super::Foundation::KEVENT,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for EOF_WAIT_BLOCK {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for EOF_WAIT_BLOCK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXTENT_READ_CACHE_INFO_BUFFER {
    pub AllocatedCache: i64,
    pub PopulatedCache: i64,
    pub InErrorCache: i64,
}
impl ::core::marker::Copy for EXTENT_READ_CACHE_INFO_BUFFER {}
impl ::core::clone::Clone for EXTENT_READ_CACHE_INFO_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_ACCESS_INFORMATION {
    pub AccessFlags: u32,
}
impl ::core::marker::Copy for FILE_ACCESS_INFORMATION {}
impl ::core::clone::Clone for FILE_ACCESS_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_ALIGNMENT_INFORMATION {
    pub AlignmentRequirement: u32,
}
impl ::core::marker::Copy for FILE_ALIGNMENT_INFORMATION {}
impl ::core::clone::Clone for FILE_ALIGNMENT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_ALLOCATION_INFORMATION {
    pub AllocationSize: i64,
}
impl ::core::marker::Copy for FILE_ALLOCATION_INFORMATION {}
impl ::core::clone::Clone for FILE_ALLOCATION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_ALL_INFORMATION {
    pub BasicInformation: FILE_BASIC_INFORMATION,
    pub StandardInformation: FILE_STANDARD_INFORMATION,
    pub InternalInformation: FILE_INTERNAL_INFORMATION,
    pub EaInformation: FILE_EA_INFORMATION,
    pub AccessInformation: FILE_ACCESS_INFORMATION,
    pub PositionInformation: FILE_POSITION_INFORMATION,
    pub ModeInformation: FILE_MODE_INFORMATION,
    pub AlignmentInformation: FILE_ALIGNMENT_INFORMATION,
    pub NameInformation: FILE_NAME_INFORMATION,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_ALL_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_ALL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_BASIC_INFORMATION {
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub FileAttributes: u32,
}
impl ::core::marker::Copy for FILE_BASIC_INFORMATION {}
impl ::core::clone::Clone for FILE_BASIC_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_BOTH_DIR_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub EaSize: u32,
    pub ShortNameLength: i8,
    pub ShortName: [u16; 12],
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_BOTH_DIR_INFORMATION {}
impl ::core::clone::Clone for FILE_BOTH_DIR_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_CASE_SENSITIVE_INFORMATION {
    pub Flags: u32,
}
impl ::core::marker::Copy for FILE_CASE_SENSITIVE_INFORMATION {}
impl ::core::clone::Clone for FILE_CASE_SENSITIVE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_COMPLETION_INFORMATION {
    pub Port: super::super::super::Win32::Foundation::HANDLE,
    pub Key: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_COMPLETION_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_COMPLETION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_COMPRESSION_INFORMATION {
    pub CompressedFileSize: i64,
    pub CompressionFormat: u16,
    pub CompressionUnitShift: u8,
    pub ChunkShift: u8,
    pub ClusterShift: u8,
    pub Reserved: [u8; 3],
}
impl ::core::marker::Copy for FILE_COMPRESSION_INFORMATION {}
impl ::core::clone::Clone for FILE_COMPRESSION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_DIRECTORY_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_DIRECTORY_INFORMATION {}
impl ::core::clone::Clone for FILE_DIRECTORY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_DISPOSITION_INFORMATION {
    pub DeleteFile: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_DISPOSITION_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_DISPOSITION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_DISPOSITION_INFORMATION_EX {
    pub Flags: FILE_DISPOSITION_INFORMATION_EX_FLAGS,
}
impl ::core::marker::Copy for FILE_DISPOSITION_INFORMATION_EX {}
impl ::core::clone::Clone for FILE_DISPOSITION_INFORMATION_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_EA_INFORMATION {
    pub EaSize: u32,
}
impl ::core::marker::Copy for FILE_EA_INFORMATION {}
impl ::core::clone::Clone for FILE_EA_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_END_OF_FILE_INFORMATION_EX {
    pub EndOfFile: i64,
    pub PagingFileSizeInMM: i64,
    pub PagingFileMaxSize: i64,
    pub Flags: u32,
}
impl ::core::marker::Copy for FILE_END_OF_FILE_INFORMATION_EX {}
impl ::core::clone::Clone for FILE_END_OF_FILE_INFORMATION_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_FS_ATTRIBUTE_INFORMATION {
    pub FileSystemAttributes: u32,
    pub MaximumComponentNameLength: i32,
    pub FileSystemNameLength: u32,
    pub FileSystemName: [u16; 1],
}
impl ::core::marker::Copy for FILE_FS_ATTRIBUTE_INFORMATION {}
impl ::core::clone::Clone for FILE_FS_ATTRIBUTE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_FS_CONTROL_INFORMATION {
    pub FreeSpaceStartFiltering: i64,
    pub FreeSpaceThreshold: i64,
    pub FreeSpaceStopFiltering: i64,
    pub DefaultQuotaThreshold: i64,
    pub DefaultQuotaLimit: i64,
    pub FileSystemControlFlags: u32,
}
impl ::core::marker::Copy for FILE_FS_CONTROL_INFORMATION {}
impl ::core::clone::Clone for FILE_FS_CONTROL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_FS_DATA_COPY_INFORMATION {
    pub NumberOfCopies: u32,
}
impl ::core::marker::Copy for FILE_FS_DATA_COPY_INFORMATION {}
impl ::core::clone::Clone for FILE_FS_DATA_COPY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_FS_DRIVER_PATH_INFORMATION {
    pub DriverInPath: super::super::super::Win32::Foundation::BOOLEAN,
    pub DriverNameLength: u32,
    pub DriverName: [u16; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_FS_DRIVER_PATH_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_FS_DRIVER_PATH_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_FS_SECTOR_SIZE_INFORMATION {
    pub LogicalBytesPerSector: u32,
    pub PhysicalBytesPerSectorForAtomicity: u32,
    pub PhysicalBytesPerSectorForPerformance: u32,
    pub FileSystemEffectivePhysicalBytesPerSectorForAtomicity: u32,
    pub Flags: u32,
    pub ByteOffsetForSectorAlignment: u32,
    pub ByteOffsetForPartitionAlignment: u32,
}
impl ::core::marker::Copy for FILE_FS_SECTOR_SIZE_INFORMATION {}
impl ::core::clone::Clone for FILE_FS_SECTOR_SIZE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_FS_VOLUME_FLAGS_INFORMATION {
    pub Flags: u32,
}
impl ::core::marker::Copy for FILE_FS_VOLUME_FLAGS_INFORMATION {}
impl ::core::clone::Clone for FILE_FS_VOLUME_FLAGS_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_FULL_DIR_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub EaSize: u32,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_FULL_DIR_INFORMATION {}
impl ::core::clone::Clone for FILE_FULL_DIR_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_FULL_EA_INFORMATION {
    pub NextEntryOffset: u32,
    pub Flags: u8,
    pub EaNameLength: u8,
    pub EaValueLength: u16,
    pub EaName: [u8; 1],
}
impl ::core::marker::Copy for FILE_FULL_EA_INFORMATION {}
impl ::core::clone::Clone for FILE_FULL_EA_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_GET_EA_INFORMATION {
    pub NextEntryOffset: u32,
    pub EaNameLength: u8,
    pub EaName: [u8; 1],
}
impl ::core::marker::Copy for FILE_GET_EA_INFORMATION {}
impl ::core::clone::Clone for FILE_GET_EA_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub struct FILE_GET_QUOTA_INFORMATION {
    pub NextEntryOffset: u32,
    pub SidLength: u32,
    pub Sid: super::super::super::Win32::Security::SID,
}
#[cfg(feature = "Win32_Security")]
impl ::core::marker::Copy for FILE_GET_QUOTA_INFORMATION {}
#[cfg(feature = "Win32_Security")]
impl ::core::clone::Clone for FILE_GET_QUOTA_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_ID_BOTH_DIR_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub EaSize: u32,
    pub ShortNameLength: i8,
    pub ShortName: [u16; 12],
    pub FileId: i64,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_ID_BOTH_DIR_INFORMATION {}
impl ::core::clone::Clone for FILE_ID_BOTH_DIR_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct FILE_ID_EXTD_BOTH_DIR_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub EaSize: u32,
    pub ReparsePointTag: u32,
    pub FileId: super::super::super::Win32::Storage::FileSystem::FILE_ID_128,
    pub ShortNameLength: i8,
    pub ShortName: [u16; 12],
    pub FileName: [u16; 1],
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for FILE_ID_EXTD_BOTH_DIR_INFORMATION {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for FILE_ID_EXTD_BOTH_DIR_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct FILE_ID_EXTD_DIR_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub EaSize: u32,
    pub ReparsePointTag: u32,
    pub FileId: super::super::super::Win32::Storage::FileSystem::FILE_ID_128,
    pub FileName: [u16; 1],
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for FILE_ID_EXTD_DIR_INFORMATION {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for FILE_ID_EXTD_DIR_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_ID_FULL_DIR_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub EaSize: u32,
    pub FileId: i64,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_ID_FULL_DIR_INFORMATION {}
impl ::core::clone::Clone for FILE_ID_FULL_DIR_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_ID_GLOBAL_TX_DIR_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub EndOfFile: i64,
    pub AllocationSize: i64,
    pub FileAttributes: u32,
    pub FileNameLength: u32,
    pub FileId: i64,
    pub LockingTransactionId: ::windows_sys::core::GUID,
    pub TxInfoFlags: u32,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_ID_GLOBAL_TX_DIR_INFORMATION {}
impl ::core::clone::Clone for FILE_ID_GLOBAL_TX_DIR_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct FILE_ID_INFORMATION {
    pub VolumeSerialNumber: u64,
    pub FileId: super::super::super::Win32::Storage::FileSystem::FILE_ID_128,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for FILE_ID_INFORMATION {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for FILE_ID_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_WindowsProgramming\"`"]
#[cfg(feature = "Win32_System_WindowsProgramming")]
pub struct FILE_INFORMATION_DEFINITION {
    pub Class: super::super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
    pub NextEntryOffset: u32,
    pub FileNameLengthOffset: u32,
    pub FileNameOffset: u32,
}
#[cfg(feature = "Win32_System_WindowsProgramming")]
impl ::core::marker::Copy for FILE_INFORMATION_DEFINITION {}
#[cfg(feature = "Win32_System_WindowsProgramming")]
impl ::core::clone::Clone for FILE_INFORMATION_DEFINITION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_INTERNAL_INFORMATION {
    pub IndexNumber: i64,
}
impl ::core::marker::Copy for FILE_INTERNAL_INFORMATION {}
impl ::core::clone::Clone for FILE_INTERNAL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_KNOWN_FOLDER_INFORMATION {
    pub Type: FILE_KNOWN_FOLDER_TYPE,
}
impl ::core::marker::Copy for FILE_KNOWN_FOLDER_INFORMATION {}
impl ::core::clone::Clone for FILE_KNOWN_FOLDER_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct FILE_LINKS_FULL_ID_INFORMATION {
    pub BytesNeeded: u32,
    pub EntriesReturned: u32,
    pub Entry: FILE_LINK_ENTRY_FULL_ID_INFORMATION,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for FILE_LINKS_FULL_ID_INFORMATION {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for FILE_LINKS_FULL_ID_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_LINKS_INFORMATION {
    pub BytesNeeded: u32,
    pub EntriesReturned: u32,
    pub Entry: FILE_LINK_ENTRY_INFORMATION,
}
impl ::core::marker::Copy for FILE_LINKS_INFORMATION {}
impl ::core::clone::Clone for FILE_LINKS_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct FILE_LINK_ENTRY_FULL_ID_INFORMATION {
    pub NextEntryOffset: u32,
    pub ParentFileId: super::super::super::Win32::Storage::FileSystem::FILE_ID_128,
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for FILE_LINK_ENTRY_FULL_ID_INFORMATION {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for FILE_LINK_ENTRY_FULL_ID_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_LINK_ENTRY_INFORMATION {
    pub NextEntryOffset: u32,
    pub ParentFileId: i64,
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_LINK_ENTRY_INFORMATION {}
impl ::core::clone::Clone for FILE_LINK_ENTRY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_LINK_INFORMATION {
    pub Anonymous: FILE_LINK_INFORMATION_0,
    pub RootDirectory: super::super::super::Win32::Foundation::HANDLE,
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_LINK_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_LINK_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union FILE_LINK_INFORMATION_0 {
    pub ReplaceIfExists: super::super::super::Win32::Foundation::BOOLEAN,
    pub Flags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_LINK_INFORMATION_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_LINK_INFORMATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FILE_LOCK {
    pub CompleteLockIrpRoutine: PCOMPLETE_LOCK_IRP_ROUTINE,
    pub UnlockRoutine: PUNLOCK_ROUTINE,
    pub FastIoIsQuestionable: super::super::super::Win32::Foundation::BOOLEAN,
    pub SpareC: [super::super::super::Win32::Foundation::BOOLEAN; 3],
    pub LockInformation: *mut ::core::ffi::c_void,
    pub LastReturnedLockInfo: FILE_LOCK_INFO,
    pub LastReturnedLock: *mut ::core::ffi::c_void,
    pub LockRequestsInProgress: i32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FILE_LOCK {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FILE_LOCK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FILE_LOCK_INFO {
    pub StartingByte: i64,
    pub Length: i64,
    pub ExclusiveLock: super::super::super::Win32::Foundation::BOOLEAN,
    pub Key: u32,
    pub FileObject: *mut super::super::Foundation::FILE_OBJECT,
    pub ProcessId: *mut ::core::ffi::c_void,
    pub EndingByte: i64,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FILE_LOCK_INFO {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FILE_LOCK_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_MAILSLOT_QUERY_INFORMATION {
    pub MaximumMessageSize: u32,
    pub MailslotQuota: u32,
    pub NextMessageSize: u32,
    pub MessagesAvailable: u32,
    pub ReadTimeout: i64,
}
impl ::core::marker::Copy for FILE_MAILSLOT_QUERY_INFORMATION {}
impl ::core::clone::Clone for FILE_MAILSLOT_QUERY_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_MAILSLOT_SET_INFORMATION {
    pub ReadTimeout: *mut i64,
}
impl ::core::marker::Copy for FILE_MAILSLOT_SET_INFORMATION {}
impl ::core::clone::Clone for FILE_MAILSLOT_SET_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_MODE_INFORMATION {
    pub Mode: u32,
}
impl ::core::marker::Copy for FILE_MODE_INFORMATION {}
impl ::core::clone::Clone for FILE_MODE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_MOVE_CLUSTER_INFORMATION {
    pub ClusterCount: u32,
    pub RootDirectory: super::super::super::Win32::Foundation::HANDLE,
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_MOVE_CLUSTER_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_MOVE_CLUSTER_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_NAMES_INFORMATION {
    pub NextEntryOffset: u32,
    pub FileIndex: u32,
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_NAMES_INFORMATION {}
impl ::core::clone::Clone for FILE_NAMES_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_NAME_INFORMATION {
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_NAME_INFORMATION {}
impl ::core::clone::Clone for FILE_NAME_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_NETWORK_OPEN_INFORMATION {
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub AllocationSize: i64,
    pub EndOfFile: i64,
    pub FileAttributes: u32,
}
impl ::core::marker::Copy for FILE_NETWORK_OPEN_INFORMATION {}
impl ::core::clone::Clone for FILE_NETWORK_OPEN_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_NETWORK_PHYSICAL_NAME_INFORMATION {
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for FILE_NETWORK_PHYSICAL_NAME_INFORMATION {}
impl ::core::clone::Clone for FILE_NETWORK_PHYSICAL_NAME_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_OBJECTID_INFORMATION {
    pub FileReference: i64,
    pub ObjectId: [u8; 16],
    pub Anonymous: FILE_OBJECTID_INFORMATION_0,
}
impl ::core::marker::Copy for FILE_OBJECTID_INFORMATION {}
impl ::core::clone::Clone for FILE_OBJECTID_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union FILE_OBJECTID_INFORMATION_0 {
    pub Anonymous: FILE_OBJECTID_INFORMATION_0_0,
    pub ExtendedInfo: [u8; 48],
}
impl ::core::marker::Copy for FILE_OBJECTID_INFORMATION_0 {}
impl ::core::clone::Clone for FILE_OBJECTID_INFORMATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_OBJECTID_INFORMATION_0_0 {
    pub BirthVolumeId: [u8; 16],
    pub BirthObjectId: [u8; 16],
    pub DomainId: [u8; 16],
}
impl ::core::marker::Copy for FILE_OBJECTID_INFORMATION_0_0 {}
impl ::core::clone::Clone for FILE_OBJECTID_INFORMATION_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_PIPE_ASSIGN_EVENT_BUFFER {
    pub EventHandle: super::super::super::Win32::Foundation::HANDLE,
    pub KeyValue: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_PIPE_ASSIGN_EVENT_BUFFER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_PIPE_ASSIGN_EVENT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_CLIENT_PROCESS_BUFFER {
    pub ClientSession: *mut ::core::ffi::c_void,
    pub ClientProcess: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for FILE_PIPE_CLIENT_PROCESS_BUFFER {}
impl ::core::clone::Clone for FILE_PIPE_CLIENT_PROCESS_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_CLIENT_PROCESS_BUFFER_EX {
    pub ClientSession: *mut ::core::ffi::c_void,
    pub ClientProcess: *mut ::core::ffi::c_void,
    pub ClientComputerNameLength: u16,
    pub ClientComputerBuffer: [u16; 16],
}
impl ::core::marker::Copy for FILE_PIPE_CLIENT_PROCESS_BUFFER_EX {}
impl ::core::clone::Clone for FILE_PIPE_CLIENT_PROCESS_BUFFER_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_CLIENT_PROCESS_BUFFER_V2 {
    pub ClientSession: u64,
    pub ClientProcess: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for FILE_PIPE_CLIENT_PROCESS_BUFFER_V2 {}
impl ::core::clone::Clone for FILE_PIPE_CLIENT_PROCESS_BUFFER_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_CREATE_SYMLINK_INPUT {
    pub NameOffset: u16,
    pub NameLength: u16,
    pub SubstituteNameOffset: u16,
    pub SubstituteNameLength: u16,
    pub Flags: u32,
}
impl ::core::marker::Copy for FILE_PIPE_CREATE_SYMLINK_INPUT {}
impl ::core::clone::Clone for FILE_PIPE_CREATE_SYMLINK_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_DELETE_SYMLINK_INPUT {
    pub NameOffset: u16,
    pub NameLength: u16,
}
impl ::core::marker::Copy for FILE_PIPE_DELETE_SYMLINK_INPUT {}
impl ::core::clone::Clone for FILE_PIPE_DELETE_SYMLINK_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_EVENT_BUFFER {
    pub NamedPipeState: u32,
    pub EntryType: u32,
    pub ByteCount: u32,
    pub KeyValue: u32,
    pub NumberRequests: u32,
}
impl ::core::marker::Copy for FILE_PIPE_EVENT_BUFFER {}
impl ::core::clone::Clone for FILE_PIPE_EVENT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_INFORMATION {
    pub ReadMode: u32,
    pub CompletionMode: u32,
}
impl ::core::marker::Copy for FILE_PIPE_INFORMATION {}
impl ::core::clone::Clone for FILE_PIPE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_LOCAL_INFORMATION {
    pub NamedPipeType: u32,
    pub NamedPipeConfiguration: u32,
    pub MaximumInstances: u32,
    pub CurrentInstances: u32,
    pub InboundQuota: u32,
    pub ReadDataAvailable: u32,
    pub OutboundQuota: u32,
    pub WriteQuotaAvailable: u32,
    pub NamedPipeState: u32,
    pub NamedPipeEnd: u32,
}
impl ::core::marker::Copy for FILE_PIPE_LOCAL_INFORMATION {}
impl ::core::clone::Clone for FILE_PIPE_LOCAL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_PEEK_BUFFER {
    pub NamedPipeState: u32,
    pub ReadDataAvailable: u32,
    pub NumberOfMessages: u32,
    pub MessageLength: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for FILE_PIPE_PEEK_BUFFER {}
impl ::core::clone::Clone for FILE_PIPE_PEEK_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_PIPE_REMOTE_INFORMATION {
    pub CollectDataTime: i64,
    pub MaximumCollectionCount: u32,
}
impl ::core::marker::Copy for FILE_PIPE_REMOTE_INFORMATION {}
impl ::core::clone::Clone for FILE_PIPE_REMOTE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_PIPE_SILO_ARRIVAL_INPUT {
    pub JobHandle: super::super::super::Win32::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_PIPE_SILO_ARRIVAL_INPUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_PIPE_SILO_ARRIVAL_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_PIPE_WAIT_FOR_BUFFER {
    pub Timeout: i64,
    pub NameLength: u32,
    pub TimeoutSpecified: super::super::super::Win32::Foundation::BOOLEAN,
    pub Name: [u16; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_PIPE_WAIT_FOR_BUFFER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_PIPE_WAIT_FOR_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_POSITION_INFORMATION {
    pub CurrentByteOffset: i64,
}
impl ::core::marker::Copy for FILE_POSITION_INFORMATION {}
impl ::core::clone::Clone for FILE_POSITION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub struct FILE_QUOTA_INFORMATION {
    pub NextEntryOffset: u32,
    pub SidLength: u32,
    pub ChangeTime: i64,
    pub QuotaUsed: i64,
    pub QuotaThreshold: i64,
    pub QuotaLimit: i64,
    pub Sid: super::super::super::Win32::Security::SID,
}
#[cfg(feature = "Win32_Security")]
impl ::core::marker::Copy for FILE_QUOTA_INFORMATION {}
#[cfg(feature = "Win32_Security")]
impl ::core::clone::Clone for FILE_QUOTA_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_REMOTE_PROTOCOL_INFORMATION {
    pub StructureVersion: u16,
    pub StructureSize: u16,
    pub Protocol: u32,
    pub ProtocolMajorVersion: u16,
    pub ProtocolMinorVersion: u16,
    pub ProtocolRevision: u16,
    pub Reserved: u16,
    pub Flags: u32,
    pub GenericReserved: FILE_REMOTE_PROTOCOL_INFORMATION_0,
    pub ProtocolSpecific: FILE_REMOTE_PROTOCOL_INFORMATION_1,
}
impl ::core::marker::Copy for FILE_REMOTE_PROTOCOL_INFORMATION {}
impl ::core::clone::Clone for FILE_REMOTE_PROTOCOL_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_REMOTE_PROTOCOL_INFORMATION_0 {
    pub Reserved: [u32; 8],
}
impl ::core::marker::Copy for FILE_REMOTE_PROTOCOL_INFORMATION_0 {}
impl ::core::clone::Clone for FILE_REMOTE_PROTOCOL_INFORMATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union FILE_REMOTE_PROTOCOL_INFORMATION_1 {
    pub Smb2: FILE_REMOTE_PROTOCOL_INFORMATION_1_0,
    pub Reserved: [u32; 16],
}
impl ::core::marker::Copy for FILE_REMOTE_PROTOCOL_INFORMATION_1 {}
impl ::core::clone::Clone for FILE_REMOTE_PROTOCOL_INFORMATION_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_REMOTE_PROTOCOL_INFORMATION_1_0 {
    pub Server: FILE_REMOTE_PROTOCOL_INFORMATION_1_0_0,
    pub Share: FILE_REMOTE_PROTOCOL_INFORMATION_1_0_1,
}
impl ::core::marker::Copy for FILE_REMOTE_PROTOCOL_INFORMATION_1_0 {}
impl ::core::clone::Clone for FILE_REMOTE_PROTOCOL_INFORMATION_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_REMOTE_PROTOCOL_INFORMATION_1_0_0 {
    pub Capabilities: u32,
}
impl ::core::marker::Copy for FILE_REMOTE_PROTOCOL_INFORMATION_1_0_0 {}
impl ::core::clone::Clone for FILE_REMOTE_PROTOCOL_INFORMATION_1_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_REMOTE_PROTOCOL_INFORMATION_1_0_1 {
    pub Capabilities: u32,
    pub ShareFlags: u32,
    pub ShareType: u8,
    pub Reserved0: [u8; 3],
    pub Reserved1: u32,
}
impl ::core::marker::Copy for FILE_REMOTE_PROTOCOL_INFORMATION_1_0_1 {}
impl ::core::clone::Clone for FILE_REMOTE_PROTOCOL_INFORMATION_1_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_RENAME_INFORMATION {
    pub Anonymous: FILE_RENAME_INFORMATION_0,
    pub RootDirectory: super::super::super::Win32::Foundation::HANDLE,
    pub FileNameLength: u32,
    pub FileName: [u16; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_RENAME_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_RENAME_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union FILE_RENAME_INFORMATION_0 {
    pub ReplaceIfExists: super::super::super::Win32::Foundation::BOOLEAN,
    pub Flags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_RENAME_INFORMATION_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_RENAME_INFORMATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_REPARSE_POINT_INFORMATION {
    pub FileReference: i64,
    pub Tag: u32,
}
impl ::core::marker::Copy for FILE_REPARSE_POINT_INFORMATION {}
impl ::core::clone::Clone for FILE_REPARSE_POINT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_STANDARD_INFORMATION {
    pub AllocationSize: i64,
    pub EndOfFile: i64,
    pub NumberOfLinks: u32,
    pub DeletePending: super::super::super::Win32::Foundation::BOOLEAN,
    pub Directory: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_STANDARD_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_STANDARD_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_STANDARD_LINK_INFORMATION {
    pub NumberOfAccessibleLinks: u32,
    pub TotalNumberOfLinks: u32,
    pub DeletePending: super::super::super::Win32::Foundation::BOOLEAN,
    pub Directory: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_STANDARD_LINK_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_STANDARD_LINK_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_STAT_INFORMATION {
    pub FileId: i64,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub AllocationSize: i64,
    pub EndOfFile: i64,
    pub FileAttributes: u32,
    pub ReparseTag: u32,
    pub NumberOfLinks: u32,
    pub EffectiveAccess: u32,
}
impl ::core::marker::Copy for FILE_STAT_INFORMATION {}
impl ::core::clone::Clone for FILE_STAT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_STAT_LX_INFORMATION {
    pub FileId: i64,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub AllocationSize: i64,
    pub EndOfFile: i64,
    pub FileAttributes: u32,
    pub ReparseTag: u32,
    pub NumberOfLinks: u32,
    pub EffectiveAccess: u32,
    pub LxFlags: u32,
    pub LxUid: u32,
    pub LxGid: u32,
    pub LxMode: u32,
    pub LxDeviceIdMajor: u32,
    pub LxDeviceIdMinor: u32,
}
impl ::core::marker::Copy for FILE_STAT_LX_INFORMATION {}
impl ::core::clone::Clone for FILE_STAT_LX_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Ioctl\"`"]
#[cfg(feature = "Win32_System_Ioctl")]
pub struct FILE_STORAGE_RESERVE_ID_INFORMATION {
    pub StorageReserveId: super::super::super::Win32::System::Ioctl::STORAGE_RESERVE_ID,
}
#[cfg(feature = "Win32_System_Ioctl")]
impl ::core::marker::Copy for FILE_STORAGE_RESERVE_ID_INFORMATION {}
#[cfg(feature = "Win32_System_Ioctl")]
impl ::core::clone::Clone for FILE_STORAGE_RESERVE_ID_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_STREAM_INFORMATION {
    pub NextEntryOffset: u32,
    pub StreamNameLength: u32,
    pub StreamSize: i64,
    pub StreamAllocationSize: i64,
    pub StreamName: [u16; 1],
}
impl ::core::marker::Copy for FILE_STREAM_INFORMATION {}
impl ::core::clone::Clone for FILE_STREAM_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_TIMESTAMPS {
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
}
impl ::core::marker::Copy for FILE_TIMESTAMPS {}
impl ::core::clone::Clone for FILE_TIMESTAMPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FILE_TRACKING_INFORMATION {
    pub DestinationFile: super::super::super::Win32::Foundation::HANDLE,
    pub ObjectInformationLength: u32,
    pub ObjectInformation: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FILE_TRACKING_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FILE_TRACKING_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILE_VOLUME_NAME_INFORMATION {
    pub DeviceNameLength: u32,
    pub DeviceName: [u16; 1],
}
impl ::core::marker::Copy for FILE_VOLUME_NAME_INFORMATION {}
impl ::core::clone::Clone for FILE_VOLUME_NAME_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSCTL_GHOST_FILE_EXTENTS_INPUT_BUFFER {
    pub FileOffset: i64,
    pub ByteCount: i64,
    pub RecallOwnerGuid: ::windows_sys::core::GUID,
    pub RecallMetadataBufferSize: u32,
    pub RecallMetadataBuffer: [u8; 1],
}
impl ::core::marker::Copy for FSCTL_GHOST_FILE_EXTENTS_INPUT_BUFFER {}
impl ::core::clone::Clone for FSCTL_GHOST_FILE_EXTENTS_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSCTL_QUERY_GHOSTED_FILE_EXTENTS_INPUT_RANGE {
    pub FileOffset: i64,
    pub ByteCount: i64,
}
impl ::core::marker::Copy for FSCTL_QUERY_GHOSTED_FILE_EXTENTS_INPUT_RANGE {}
impl ::core::clone::Clone for FSCTL_QUERY_GHOSTED_FILE_EXTENTS_INPUT_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSCTL_QUERY_GHOSTED_FILE_EXTENTS_OUTPUT {
    pub ExtentCount: u32,
    pub TotalExtentCount: u32,
    pub Extents: [u8; 1],
}
impl ::core::marker::Copy for FSCTL_QUERY_GHOSTED_FILE_EXTENTS_OUTPUT {}
impl ::core::clone::Clone for FSCTL_QUERY_GHOSTED_FILE_EXTENTS_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSCTL_QUERY_VOLUME_NUMA_INFO_OUTPUT {
    pub NumaNode: u32,
}
impl ::core::marker::Copy for FSCTL_QUERY_VOLUME_NUMA_INFO_OUTPUT {}
impl ::core::clone::Clone for FSCTL_QUERY_VOLUME_NUMA_INFO_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSCTL_UNMAP_SPACE_INPUT_BUFFER {
    pub BytesToUnmap: i64,
}
impl ::core::marker::Copy for FSCTL_UNMAP_SPACE_INPUT_BUFFER {}
impl ::core::clone::Clone for FSCTL_UNMAP_SPACE_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSCTL_UNMAP_SPACE_OUTPUT {
    pub BytesUnmapped: i64,
}
impl ::core::marker::Copy for FSCTL_UNMAP_SPACE_OUTPUT {}
impl ::core::clone::Clone for FSCTL_UNMAP_SPACE_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct FSRTL_ADVANCED_FCB_HEADER {
    pub Base: FSRTL_COMMON_FCB_HEADER,
    pub FastMutex: *mut super::super::Foundation::FAST_MUTEX,
    pub FilterContexts: super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub PushLock: usize,
    pub FileContextSupportPointer: *mut *mut ::core::ffi::c_void,
    pub Anonymous: FSRTL_ADVANCED_FCB_HEADER_0,
    pub AePushLock: *mut ::core::ffi::c_void,
    pub BypassIoOpenCount: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for FSRTL_ADVANCED_FCB_HEADER {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for FSRTL_ADVANCED_FCB_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union FSRTL_ADVANCED_FCB_HEADER_0 {
    pub Oplock: *mut ::core::ffi::c_void,
    pub ReservedForRemote: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for FSRTL_ADVANCED_FCB_HEADER_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for FSRTL_ADVANCED_FCB_HEADER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`"]
#[cfg(feature = "Wdk_Foundation")]
pub struct FSRTL_AUXILIARY_BUFFER {
    pub Buffer: *mut ::core::ffi::c_void,
    pub Length: u32,
    pub Flags: u32,
    pub Mdl: *mut super::super::Foundation::MDL,
}
#[cfg(feature = "Wdk_Foundation")]
impl ::core::marker::Copy for FSRTL_AUXILIARY_BUFFER {}
#[cfg(feature = "Wdk_Foundation")]
impl ::core::clone::Clone for FSRTL_AUXILIARY_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
pub struct FSRTL_COMMON_FCB_HEADER {
    pub NodeTypeCode: i16,
    pub NodeByteSize: i16,
    pub Flags: u8,
    pub IsFastIoPossible: u8,
    pub Flags2: u8,
    pub _bitfield: u8,
    pub Resource: *mut super::super::Foundation::ERESOURCE,
    pub PagingIoResource: *mut super::super::Foundation::ERESOURCE,
    pub AllocationSize: i64,
    pub FileSize: i64,
    pub ValidDataLength: i64,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for FSRTL_COMMON_FCB_HEADER {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for FSRTL_COMMON_FCB_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSRTL_MUP_PROVIDER_INFO_LEVEL_1 {
    pub ProviderId: u32,
}
impl ::core::marker::Copy for FSRTL_MUP_PROVIDER_INFO_LEVEL_1 {}
impl ::core::clone::Clone for FSRTL_MUP_PROVIDER_INFO_LEVEL_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FSRTL_MUP_PROVIDER_INFO_LEVEL_2 {
    pub ProviderId: u32,
    pub ProviderName: super::super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FSRTL_MUP_PROVIDER_INFO_LEVEL_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FSRTL_MUP_PROVIDER_INFO_LEVEL_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct FSRTL_PER_FILEOBJECT_CONTEXT {
    pub Links: super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub OwnerId: *mut ::core::ffi::c_void,
    pub InstanceId: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for FSRTL_PER_FILEOBJECT_CONTEXT {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for FSRTL_PER_FILEOBJECT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
pub struct FSRTL_PER_FILE_CONTEXT {
    pub Links: super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub OwnerId: *mut ::core::ffi::c_void,
    pub InstanceId: *mut ::core::ffi::c_void,
    pub FreeCallback: super::super::Foundation::PFREE_FUNCTION,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for FSRTL_PER_FILE_CONTEXT {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for FSRTL_PER_FILE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
pub struct FSRTL_PER_STREAM_CONTEXT {
    pub Links: super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub OwnerId: *mut ::core::ffi::c_void,
    pub InstanceId: *mut ::core::ffi::c_void,
    pub FreeCallback: super::super::Foundation::PFREE_FUNCTION,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for FSRTL_PER_STREAM_CONTEXT {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for FSRTL_PER_STREAM_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSRTL_UNC_PROVIDER_REGISTRATION {
    pub Size: u16,
    pub Version: u16,
    pub Anonymous1: FSRTL_UNC_PROVIDER_REGISTRATION_0,
    pub Anonymous2: FSRTL_UNC_PROVIDER_REGISTRATION_1,
}
impl ::core::marker::Copy for FSRTL_UNC_PROVIDER_REGISTRATION {}
impl ::core::clone::Clone for FSRTL_UNC_PROVIDER_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union FSRTL_UNC_PROVIDER_REGISTRATION_0 {
    pub ProviderFlags: u32,
    pub Anonymous: FSRTL_UNC_PROVIDER_REGISTRATION_0_0,
}
impl ::core::marker::Copy for FSRTL_UNC_PROVIDER_REGISTRATION_0 {}
impl ::core::clone::Clone for FSRTL_UNC_PROVIDER_REGISTRATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSRTL_UNC_PROVIDER_REGISTRATION_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for FSRTL_UNC_PROVIDER_REGISTRATION_0_0 {}
impl ::core::clone::Clone for FSRTL_UNC_PROVIDER_REGISTRATION_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union FSRTL_UNC_PROVIDER_REGISTRATION_1 {
    pub HardeningCapabilities: u32,
    pub Anonymous: FSRTL_UNC_PROVIDER_REGISTRATION_1_0,
}
impl ::core::marker::Copy for FSRTL_UNC_PROVIDER_REGISTRATION_1 {}
impl ::core::clone::Clone for FSRTL_UNC_PROVIDER_REGISTRATION_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FSRTL_UNC_PROVIDER_REGISTRATION_1_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for FSRTL_UNC_PROVIDER_REGISTRATION_1_0 {}
impl ::core::clone::Clone for FSRTL_UNC_PROVIDER_REGISTRATION_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FS_BPIO_INFO {
    pub ActiveBypassIoCount: u32,
    pub StorageDriverNameLen: u16,
    pub StorageDriverName: [u16; 32],
}
impl ::core::marker::Copy for FS_BPIO_INFO {}
impl ::core::clone::Clone for FS_BPIO_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Ioctl\"`"]
#[cfg(feature = "Win32_System_Ioctl")]
pub struct FS_BPIO_INPUT {
    pub Operation: super::super::super::Win32::System::Ioctl::FS_BPIO_OPERATIONS,
    pub InFlags: super::super::super::Win32::System::Ioctl::FS_BPIO_INFLAGS,
    pub Reserved1: u64,
    pub Reserved2: u64,
}
#[cfg(feature = "Win32_System_Ioctl")]
impl ::core::marker::Copy for FS_BPIO_INPUT {}
#[cfg(feature = "Win32_System_Ioctl")]
impl ::core::clone::Clone for FS_BPIO_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FS_FILTER_CALLBACKS {
    pub SizeOfFsFilterCallbacks: u32,
    pub Reserved: u32,
    pub PreAcquireForSectionSynchronization: PFS_FILTER_CALLBACK,
    pub PostAcquireForSectionSynchronization: PFS_FILTER_COMPLETION_CALLBACK,
    pub PreReleaseForSectionSynchronization: PFS_FILTER_CALLBACK,
    pub PostReleaseForSectionSynchronization: PFS_FILTER_COMPLETION_CALLBACK,
    pub PreAcquireForCcFlush: PFS_FILTER_CALLBACK,
    pub PostAcquireForCcFlush: PFS_FILTER_COMPLETION_CALLBACK,
    pub PreReleaseForCcFlush: PFS_FILTER_CALLBACK,
    pub PostReleaseForCcFlush: PFS_FILTER_COMPLETION_CALLBACK,
    pub PreAcquireForModifiedPageWriter: PFS_FILTER_CALLBACK,
    pub PostAcquireForModifiedPageWriter: PFS_FILTER_COMPLETION_CALLBACK,
    pub PreReleaseForModifiedPageWriter: PFS_FILTER_CALLBACK,
    pub PostReleaseForModifiedPageWriter: PFS_FILTER_COMPLETION_CALLBACK,
    pub PreQueryOpen: PFS_FILTER_CALLBACK,
    pub PostQueryOpen: PFS_FILTER_COMPLETION_CALLBACK,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_CALLBACKS {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_CALLBACKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FS_FILTER_CALLBACK_DATA {
    pub SizeOfFsFilterCallbackData: u32,
    pub Operation: u8,
    pub Reserved: u8,
    pub DeviceObject: *mut super::super::Foundation::DEVICE_OBJECT,
    pub FileObject: *mut super::super::Foundation::FILE_OBJECT,
    pub Parameters: FS_FILTER_PARAMETERS,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_CALLBACK_DATA {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_CALLBACK_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FS_FILTER_PARAMETERS {
    pub AcquireForModifiedPageWriter: FS_FILTER_PARAMETERS_0,
    pub ReleaseForModifiedPageWriter: FS_FILTER_PARAMETERS_4,
    pub AcquireForSectionSynchronization: FS_FILTER_PARAMETERS_1,
    pub QueryOpen: FS_FILTER_PARAMETERS_3,
    pub Others: FS_FILTER_PARAMETERS_2,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_PARAMETERS {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FS_FILTER_PARAMETERS_0 {
    pub EndingOffset: *mut i64,
    pub ResourceToRelease: *mut *mut super::super::Foundation::ERESOURCE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_PARAMETERS_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_PARAMETERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FS_FILTER_PARAMETERS_1 {
    pub SyncType: FS_FILTER_SECTION_SYNC_TYPE,
    pub PageProtection: u32,
    pub OutputInformation: *mut FS_FILTER_SECTION_SYNC_OUTPUT,
    pub Flags: u32,
    pub AllocationAttributes: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_PARAMETERS_1 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_PARAMETERS_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FS_FILTER_PARAMETERS_2 {
    pub Argument1: *mut ::core::ffi::c_void,
    pub Argument2: *mut ::core::ffi::c_void,
    pub Argument3: *mut ::core::ffi::c_void,
    pub Argument4: *mut ::core::ffi::c_void,
    pub Argument5: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_PARAMETERS_2 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_PARAMETERS_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FS_FILTER_PARAMETERS_3 {
    pub Irp: *mut super::super::Foundation::IRP,
    pub FileInformation: *mut ::core::ffi::c_void,
    pub Length: *mut u32,
    pub FileInformationClass: super::super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
    pub CompletionStatus: super::super::super::Win32::Foundation::NTSTATUS,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_PARAMETERS_3 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_PARAMETERS_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FS_FILTER_PARAMETERS_4 {
    pub ResourceToRelease: *mut super::super::Foundation::ERESOURCE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FS_FILTER_PARAMETERS_4 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FS_FILTER_PARAMETERS_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FS_FILTER_SECTION_SYNC_OUTPUT {
    pub StructureSize: u32,
    pub SizeReturned: u32,
    pub Flags: u32,
    pub DesiredReadAlignment: u32,
}
impl ::core::marker::Copy for FS_FILTER_SECTION_SYNC_OUTPUT {}
impl ::core::clone::Clone for FS_FILTER_SECTION_SYNC_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct GENERATE_NAME_CONTEXT {
    pub Checksum: u16,
    pub ChecksumInserted: super::super::super::Win32::Foundation::BOOLEAN,
    pub NameLength: u8,
    pub NameBuffer: [u16; 8],
    pub ExtensionLength: u32,
    pub ExtensionBuffer: [u16; 4],
    pub LastIndexValue: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for GENERATE_NAME_CONTEXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for GENERATE_NAME_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GHOSTED_FILE_EXTENT {
    pub FileOffset: i64,
    pub ByteCount: i64,
    pub RecallOwnerGuid: ::windows_sys::core::GUID,
    pub NextEntryOffset: u32,
    pub RecallMetadataBufferSize: u32,
    pub RecallMetadataBuffer: [u8; 1],
}
impl ::core::marker::Copy for GHOSTED_FILE_EXTENT {}
impl ::core::clone::Clone for GHOSTED_FILE_EXTENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_CREATE_STREAM_FILE_OPTIONS {
    pub Size: u16,
    pub Flags: u16,
    pub TargetDeviceObject: *mut super::super::Foundation::DEVICE_OBJECT,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_CREATE_STREAM_FILE_OPTIONS {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_CREATE_STREAM_FILE_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct IO_DEVICE_HINT_ECP_CONTEXT {
    pub TargetDevice: *mut super::super::Foundation::DEVICE_OBJECT,
    pub RemainingName: super::super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for IO_DEVICE_HINT_ECP_CONTEXT {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for IO_DEVICE_HINT_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`"]
#[cfg(feature = "Wdk_Foundation")]
pub struct IO_PRIORITY_INFO {
    pub Size: u32,
    pub ThreadPriority: u32,
    pub PagePriority: u32,
    pub IoPriority: super::super::Foundation::IO_PRIORITY_HINT,
}
#[cfg(feature = "Wdk_Foundation")]
impl ::core::marker::Copy for IO_PRIORITY_INFO {}
#[cfg(feature = "Wdk_Foundation")]
impl ::core::clone::Clone for IO_PRIORITY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IO_STOP_ON_SYMLINK_FILTER_ECP_v0 {
    pub Out: IO_STOP_ON_SYMLINK_FILTER_ECP_v0_0,
}
impl ::core::marker::Copy for IO_STOP_ON_SYMLINK_FILTER_ECP_v0 {}
impl ::core::clone::Clone for IO_STOP_ON_SYMLINK_FILTER_ECP_v0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IO_STOP_ON_SYMLINK_FILTER_ECP_v0_0 {
    pub ReparseCount: u32,
    pub RemainingPathLength: u32,
}
impl ::core::marker::Copy for IO_STOP_ON_SYMLINK_FILTER_ECP_v0_0 {}
impl ::core::clone::Clone for IO_STOP_ON_SYMLINK_FILTER_ECP_v0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KAPC_STATE {
    pub ApcListHead: [super::super::super::Win32::System::Kernel::LIST_ENTRY; 2],
    pub Process: *mut isize,
    pub Anonymous1: KAPC_STATE_0,
    pub KernelApcPending: super::super::super::Win32::Foundation::BOOLEAN,
    pub Anonymous2: KAPC_STATE_1,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KAPC_STATE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KAPC_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union KAPC_STATE_0 {
    pub InProgressFlags: u8,
    pub Anonymous: KAPC_STATE_0_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KAPC_STATE_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KAPC_STATE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KAPC_STATE_0_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KAPC_STATE_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KAPC_STATE_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub union KAPC_STATE_1 {
    pub UserApcPendingAll: super::super::super::Win32::Foundation::BOOLEAN,
    pub Anonymous: KAPC_STATE_1_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KAPC_STATE_1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KAPC_STATE_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct KAPC_STATE_1_0 {
    pub _bitfield: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for KAPC_STATE_1_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for KAPC_STATE_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct LARGE_MCB {
    pub GuardedMutex: *mut super::super::Foundation::FAST_MUTEX,
    pub BaseMcb: BASE_MCB,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for LARGE_MCB {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for LARGE_MCB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LCN_WEAK_REFERENCE_BUFFER {
    pub Lcn: i64,
    pub LengthInClusters: i64,
    pub ReferenceCount: u32,
    pub State: u16,
}
impl ::core::marker::Copy for LCN_WEAK_REFERENCE_BUFFER {}
impl ::core::clone::Clone for LCN_WEAK_REFERENCE_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LCN_WEAK_REFERENCE_CREATE_INPUT_BUFFER {
    pub Offset: i64,
    pub Length: i64,
    pub Flags: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for LCN_WEAK_REFERENCE_CREATE_INPUT_BUFFER {}
impl ::core::clone::Clone for LCN_WEAK_REFERENCE_CREATE_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LINK_TRACKING_INFORMATION {
    pub Type: LINK_TRACKING_INFORMATION_TYPE,
    pub VolumeId: [u8; 16],
}
impl ::core::marker::Copy for LINK_TRACKING_INFORMATION {}
impl ::core::clone::Clone for LINK_TRACKING_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct MCB {
    pub DummyFieldThatSizesThisStructureCorrectly: LARGE_MCB,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MCB {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MCB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MEMORY_RANGE_ENTRY {
    pub VirtualAddress: *mut ::core::ffi::c_void,
    pub NumberOfBytes: usize,
}
impl ::core::marker::Copy for MEMORY_RANGE_ENTRY {}
impl ::core::clone::Clone for MEMORY_RANGE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MFT_ENUM_DATA {
    pub StartFileReferenceNumber: u64,
    pub LowUsn: i64,
    pub HighUsn: i64,
    pub MinMajorVersion: u16,
    pub MaxMajorVersion: u16,
}
impl ::core::marker::Copy for MFT_ENUM_DATA {}
impl ::core::clone::Clone for MFT_ENUM_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union MM_PREFETCH_FLAGS {
    pub Flags: MM_PREFETCH_FLAGS_0,
    pub AllFlags: u32,
}
impl ::core::marker::Copy for MM_PREFETCH_FLAGS {}
impl ::core::clone::Clone for MM_PREFETCH_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MM_PREFETCH_FLAGS_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for MM_PREFETCH_FLAGS_0 {}
impl ::core::clone::Clone for MM_PREFETCH_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(feature = "Win32_Security_Authentication_Identity")]
pub struct MSV1_0_ENUMUSERS_REQUEST {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::marker::Copy for MSV1_0_ENUMUSERS_REQUEST {}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::clone::Clone for MSV1_0_ENUMUSERS_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
pub struct MSV1_0_ENUMUSERS_RESPONSE {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
    pub NumberOfLoggedOnUsers: u32,
    pub LogonIds: *mut super::super::super::Win32::Foundation::LUID,
    pub EnumHandles: *mut u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::marker::Copy for MSV1_0_ENUMUSERS_RESPONSE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::clone::Clone for MSV1_0_ENUMUSERS_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
pub struct MSV1_0_GETCHALLENRESP_REQUEST {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
    pub ParameterControl: u32,
    pub LogonId: super::super::super::Win32::Foundation::LUID,
    pub Password: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub ChallengeToClient: [u8; 8],
    pub UserName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub LogonDomainName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub ServerName: super::super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::marker::Copy for MSV1_0_GETCHALLENRESP_REQUEST {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::clone::Clone for MSV1_0_GETCHALLENRESP_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
pub struct MSV1_0_GETCHALLENRESP_REQUEST_V1 {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
    pub ParameterControl: u32,
    pub LogonId: super::super::super::Win32::Foundation::LUID,
    pub Password: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub ChallengeToClient: [u8; 8],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::marker::Copy for MSV1_0_GETCHALLENRESP_REQUEST_V1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::clone::Clone for MSV1_0_GETCHALLENRESP_REQUEST_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity", feature = "Win32_System_Kernel"))]
pub struct MSV1_0_GETCHALLENRESP_RESPONSE {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
    pub CaseSensitiveChallengeResponse: super::super::super::Win32::System::Kernel::STRING,
    pub CaseInsensitiveChallengeResponse: super::super::super::Win32::System::Kernel::STRING,
    pub UserName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub LogonDomainName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub UserSessionKey: [u8; 16],
    pub LanmanSessionKey: [u8; 8],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MSV1_0_GETCHALLENRESP_RESPONSE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MSV1_0_GETCHALLENRESP_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
pub struct MSV1_0_GETUSERINFO_REQUEST {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
    pub LogonId: super::super::super::Win32::Foundation::LUID,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::marker::Copy for MSV1_0_GETUSERINFO_REQUEST {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::clone::Clone for MSV1_0_GETUSERINFO_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
pub struct MSV1_0_GETUSERINFO_RESPONSE {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
    pub UserSid: super::super::super::Win32::Foundation::PSID,
    pub UserName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub LogonDomainName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub LogonServer: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub LogonType: super::super::super::Win32::Security::Authentication::Identity::SECURITY_LOGON_TYPE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::marker::Copy for MSV1_0_GETUSERINFO_RESPONSE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
impl ::core::clone::Clone for MSV1_0_GETUSERINFO_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(feature = "Win32_Security_Authentication_Identity")]
pub struct MSV1_0_LM20_CHALLENGE_REQUEST {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::marker::Copy for MSV1_0_LM20_CHALLENGE_REQUEST {}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::clone::Clone for MSV1_0_LM20_CHALLENGE_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(feature = "Win32_Security_Authentication_Identity")]
pub struct MSV1_0_LM20_CHALLENGE_RESPONSE {
    pub MessageType: super::super::super::Win32::Security::Authentication::Identity::MSV1_0_PROTOCOL_MESSAGE_TYPE,
    pub ChallengeToClient: [u8; 8],
}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::marker::Copy for MSV1_0_LM20_CHALLENGE_RESPONSE {}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::clone::Clone for MSV1_0_LM20_CHALLENGE_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_APP_INSTANCE_ECP_CONTEXT {
    pub Size: u16,
    pub Reserved: u16,
    pub AppInstanceID: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for NETWORK_APP_INSTANCE_ECP_CONTEXT {}
impl ::core::clone::Clone for NETWORK_APP_INSTANCE_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_APP_INSTANCE_VERSION_ECP_CONTEXT {
    pub Size: u16,
    pub Reserved: u16,
    pub VersionHigh: u64,
    pub VersionLow: u64,
}
impl ::core::marker::Copy for NETWORK_APP_INSTANCE_VERSION_ECP_CONTEXT {}
impl ::core::clone::Clone for NETWORK_APP_INSTANCE_VERSION_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT {
    pub Size: u16,
    pub Reserved: u16,
    pub Anonymous: NETWORK_OPEN_ECP_CONTEXT_0,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT_0 {
    pub r#in: NETWORK_OPEN_ECP_CONTEXT_0_0,
    pub out: NETWORK_OPEN_ECP_CONTEXT_0_1,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT_0 {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT_0_0 {
    pub Location: NETWORK_OPEN_LOCATION_QUALIFIER,
    pub Integrity: NETWORK_OPEN_INTEGRITY_QUALIFIER,
    pub Flags: u32,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT_0_0 {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT_0_1 {
    pub Location: NETWORK_OPEN_LOCATION_QUALIFIER,
    pub Integrity: NETWORK_OPEN_INTEGRITY_QUALIFIER,
    pub Flags: u32,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT_0_1 {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT_V0 {
    pub Size: u16,
    pub Reserved: u16,
    pub Anonymous: NETWORK_OPEN_ECP_CONTEXT_V0_0,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT_V0 {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT_V0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT_V0_0 {
    pub r#in: NETWORK_OPEN_ECP_CONTEXT_V0_0_0,
    pub out: NETWORK_OPEN_ECP_CONTEXT_V0_0_1,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT_V0_0 {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT_V0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT_V0_0_0 {
    pub Location: NETWORK_OPEN_LOCATION_QUALIFIER,
    pub Integrity: NETWORK_OPEN_INTEGRITY_QUALIFIER,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT_V0_0_0 {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT_V0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETWORK_OPEN_ECP_CONTEXT_V0_0_1 {
    pub Location: NETWORK_OPEN_LOCATION_QUALIFIER,
    pub Integrity: NETWORK_OPEN_INTEGRITY_QUALIFIER,
}
impl ::core::marker::Copy for NETWORK_OPEN_ECP_CONTEXT_V0_0_1 {}
impl ::core::clone::Clone for NETWORK_OPEN_ECP_CONTEXT_V0_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct NFS_OPEN_ECP_CONTEXT {
    pub ExportAlias: *mut super::super::super::Win32::Foundation::UNICODE_STRING,
    pub ClientSocketAddress: *mut super::super::super::Win32::Networking::WinSock::SOCKADDR_STORAGE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for NFS_OPEN_ECP_CONTEXT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for NFS_OPEN_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NLSTABLEINFO {
    pub OemTableInfo: CPTABLEINFO,
    pub AnsiTableInfo: CPTABLEINFO,
    pub UpperCaseTable: *mut u16,
    pub LowerCaseTable: *mut u16,
}
impl ::core::marker::Copy for NLSTABLEINFO {}
impl ::core::clone::Clone for NLSTABLEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct OPEN_REPARSE_LIST {
    pub OpenReparseList: super::super::super::Win32::System::Kernel::LIST_ENTRY,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for OPEN_REPARSE_LIST {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for OPEN_REPARSE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct OPEN_REPARSE_LIST_ENTRY {
    pub OpenReparseListEntry: super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub ReparseTag: u32,
    pub Flags: u32,
    pub ReparseGuid: ::windows_sys::core::GUID,
    pub Size: u16,
    pub RemainingLength: u16,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for OPEN_REPARSE_LIST_ENTRY {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for OPEN_REPARSE_LIST_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OPLOCK_KEY_CONTEXT {
    pub Version: u16,
    pub Flags: u16,
    pub ParentOplockKey: ::windows_sys::core::GUID,
    pub TargetOplockKey: ::windows_sys::core::GUID,
    pub Reserved: u32,
}
impl ::core::marker::Copy for OPLOCK_KEY_CONTEXT {}
impl ::core::clone::Clone for OPLOCK_KEY_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OPLOCK_KEY_ECP_CONTEXT {
    pub OplockKey: ::windows_sys::core::GUID,
    pub Reserved: u32,
}
impl ::core::marker::Copy for OPLOCK_KEY_ECP_CONTEXT {}
impl ::core::clone::Clone for OPLOCK_KEY_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct OPLOCK_NOTIFY_PARAMS {
    pub NotifyReason: OPLOCK_NOTIFY_REASON,
    pub NotifyContext: *mut ::core::ffi::c_void,
    pub Irp: *mut super::super::Foundation::IRP,
    pub Status: super::super::super::Win32::Foundation::NTSTATUS,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for OPLOCK_NOTIFY_PARAMS {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for OPLOCK_NOTIFY_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_EXTENTS_DESCRIPTOR {
    pub NumberOfRuns: u32,
    pub NumberOfValidRuns: u32,
    pub Run: [PHYSICAL_MEMORY_RUN; 1],
}
impl ::core::marker::Copy for PHYSICAL_EXTENTS_DESCRIPTOR {}
impl ::core::clone::Clone for PHYSICAL_EXTENTS_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_MEMORY_DESCRIPTOR {
    pub NumberOfRuns: u32,
    pub NumberOfPages: u32,
    pub Run: [PHYSICAL_MEMORY_RUN; 1],
}
impl ::core::marker::Copy for PHYSICAL_MEMORY_DESCRIPTOR {}
impl ::core::clone::Clone for PHYSICAL_MEMORY_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_MEMORY_RUN {
    pub BasePage: u32,
    pub PageCount: u32,
}
impl ::core::marker::Copy for PHYSICAL_MEMORY_RUN {}
impl ::core::clone::Clone for PHYSICAL_MEMORY_RUN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PREFETCH_OPEN_ECP_CONTEXT {
    pub Context: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for PREFETCH_OPEN_ECP_CONTEXT {}
impl ::core::clone::Clone for PREFETCH_OPEN_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
pub struct PREFIX_TABLE {
    pub NodeTypeCode: i16,
    pub NameLength: i16,
    pub NextPrefixTree: *mut PREFIX_TABLE_ENTRY,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for PREFIX_TABLE {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for PREFIX_TABLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
pub struct PREFIX_TABLE_ENTRY {
    pub NodeTypeCode: i16,
    pub NameLength: i16,
    pub NextPrefixTree: *mut PREFIX_TABLE_ENTRY,
    pub Links: super::super::Foundation::RTL_SPLAY_LINKS,
    pub Prefix: *mut super::super::super::Win32::System::Kernel::STRING,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for PREFIX_TABLE_ENTRY {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for PREFIX_TABLE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PUBLIC_BCB {
    pub NodeTypeCode: i16,
    pub NodeByteSize: i16,
    pub MappedLength: u32,
    pub MappedFileOffset: i64,
}
impl ::core::marker::Copy for PUBLIC_BCB {}
impl ::core::clone::Clone for PUBLIC_BCB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Ioctl\"`"]
#[cfg(feature = "Win32_System_Ioctl")]
pub struct QUERY_BAD_RANGES_INPUT {
    pub Flags: u32,
    pub NumRanges: u32,
    pub Ranges: [super::super::super::Win32::System::Ioctl::QUERY_BAD_RANGES_INPUT_RANGE; 1],
}
#[cfg(feature = "Win32_System_Ioctl")]
impl ::core::marker::Copy for QUERY_BAD_RANGES_INPUT {}
#[cfg(feature = "Win32_System_Ioctl")]
impl ::core::clone::Clone for QUERY_BAD_RANGES_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QUERY_DIRECT_ACCESS_EXTENTS {
    pub FileOffset: i64,
    pub Length: i64,
    pub Flags: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for QUERY_DIRECT_ACCESS_EXTENTS {}
impl ::core::clone::Clone for QUERY_DIRECT_ACCESS_EXTENTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QUERY_ON_CREATE_EA_INFORMATION {
    pub EaBufferSize: u32,
    pub EaBuffer: *mut FILE_FULL_EA_INFORMATION,
}
impl ::core::marker::Copy for QUERY_ON_CREATE_EA_INFORMATION {}
impl ::core::clone::Clone for QUERY_ON_CREATE_EA_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QUERY_ON_CREATE_ECP_CONTEXT {
    pub RequestedClasses: u32,
    pub ClassesProcessed: u32,
    pub ClassesWithErrors: u32,
    pub ClassesWithNoData: u32,
    pub StatInformation: QUERY_ON_CREATE_FILE_STAT_INFORMATION,
    pub LxInformation: QUERY_ON_CREATE_FILE_LX_INFORMATION,
    pub EaInformation: QUERY_ON_CREATE_EA_INFORMATION,
}
impl ::core::marker::Copy for QUERY_ON_CREATE_ECP_CONTEXT {}
impl ::core::clone::Clone for QUERY_ON_CREATE_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QUERY_ON_CREATE_FILE_LX_INFORMATION {
    pub EffectiveAccess: u32,
    pub LxFlags: u32,
    pub LxUid: u32,
    pub LxGid: u32,
    pub LxMode: u32,
    pub LxDeviceIdMajor: u32,
    pub LxDeviceIdMinor: u32,
}
impl ::core::marker::Copy for QUERY_ON_CREATE_FILE_LX_INFORMATION {}
impl ::core::clone::Clone for QUERY_ON_CREATE_FILE_LX_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QUERY_ON_CREATE_FILE_STAT_INFORMATION {
    pub FileId: i64,
    pub CreationTime: i64,
    pub LastAccessTime: i64,
    pub LastWriteTime: i64,
    pub ChangeTime: i64,
    pub AllocationSize: i64,
    pub EndOfFile: i64,
    pub FileAttributes: u32,
    pub ReparseTag: u32,
    pub NumberOfLinks: u32,
}
impl ::core::marker::Copy for QUERY_ON_CREATE_FILE_STAT_INFORMATION {}
impl ::core::clone::Clone for QUERY_ON_CREATE_FILE_STAT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct QUERY_PATH_REQUEST {
    pub PathNameLength: u32,
    pub SecurityContext: *mut super::super::Foundation::IO_SECURITY_CONTEXT,
    pub FilePathName: [u16; 1],
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for QUERY_PATH_REQUEST {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for QUERY_PATH_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct QUERY_PATH_REQUEST_EX {
    pub pSecurityContext: *mut super::super::Foundation::IO_SECURITY_CONTEXT,
    pub EaLength: u32,
    pub pEaBuffer: *mut ::core::ffi::c_void,
    pub PathName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub DomainServiceName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub EcpList: *mut super::super::Foundation::ECP_LIST,
    pub Silo: super::super::Foundation::PESILO,
    pub Reserved: usize,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for QUERY_PATH_REQUEST_EX {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for QUERY_PATH_REQUEST_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QUERY_PATH_RESPONSE {
    pub LengthAccepted: u32,
}
impl ::core::marker::Copy for QUERY_PATH_RESPONSE {}
impl ::core::clone::Clone for QUERY_PATH_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct READ_AHEAD_PARAMETERS {
    pub NodeByteSize: i16,
    pub Granularity: u32,
    pub PipelinedRequestSize: u32,
    pub ReadAheadGrowthPercentage: u32,
}
impl ::core::marker::Copy for READ_AHEAD_PARAMETERS {}
impl ::core::clone::Clone for READ_AHEAD_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_FileSystem", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct READ_LIST {
    pub FileObject: *mut super::super::Foundation::FILE_OBJECT,
    pub NumberOfEntries: u32,
    pub IsImage: u32,
    pub List: [super::super::super::Win32::Storage::FileSystem::FILE_SEGMENT_ELEMENT; 1],
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_FileSystem", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for READ_LIST {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_FileSystem", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for READ_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct READ_USN_JOURNAL_DATA {
    pub StartUsn: i64,
    pub ReasonMask: u32,
    pub ReturnOnlyOnClose: u32,
    pub Timeout: u64,
    pub BytesToWaitFor: u64,
    pub UsnJournalID: u64,
    pub MinMajorVersion: u16,
    pub MaxMajorVersion: u16,
}
impl ::core::marker::Copy for READ_USN_JOURNAL_DATA {}
impl ::core::clone::Clone for READ_USN_JOURNAL_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_DEALLOCATE_RANGES_INPUT_BUFFER {
    pub RangeCount: u32,
    pub Ranges: [REFS_DEALLOCATE_RANGES_RANGE; 1],
}
impl ::core::marker::Copy for REFS_DEALLOCATE_RANGES_INPUT_BUFFER {}
impl ::core::clone::Clone for REFS_DEALLOCATE_RANGES_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_DEALLOCATE_RANGES_INPUT_BUFFER_EX {
    pub RangeCount: u32,
    pub Allocator: REFS_DEALLOCATE_RANGES_ALLOCATOR,
    pub StreamReserveUpdateCount: i64,
    pub OffsetToRanges: u32,
    pub OffsetToLeakCounts: u32,
    pub Reserved: [u64; 2],
}
impl ::core::marker::Copy for REFS_DEALLOCATE_RANGES_INPUT_BUFFER_EX {}
impl ::core::clone::Clone for REFS_DEALLOCATE_RANGES_INPUT_BUFFER_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_DEALLOCATE_RANGES_RANGE {
    pub StartOfRange: u64,
    pub CountOfRange: u64,
}
impl ::core::marker::Copy for REFS_DEALLOCATE_RANGES_RANGE {}
impl ::core::clone::Clone for REFS_DEALLOCATE_RANGES_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_QUERY_VOLUME_COMPRESSION_INFO_OUTPUT_BUFFER {
    pub DefaultCompressionFormat: REFS_COMPRESSION_FORMATS,
    pub DefaultCompressionLevel: i16,
    pub DefaultCompressionChunkSizeBytes: u32,
    pub VolumeClusterSizeBytes: u32,
    pub TotalVolumeClusters: u64,
    pub TotalAllocatedClusters: u64,
    pub TotalCompressibleClustersAllocated: u64,
    pub TotalCompressibleClustersInUse: u64,
    pub TotalCompressedClusters: u64,
    pub Reserved: [u64; 6],
}
impl ::core::marker::Copy for REFS_QUERY_VOLUME_COMPRESSION_INFO_OUTPUT_BUFFER {}
impl ::core::clone::Clone for REFS_QUERY_VOLUME_COMPRESSION_INFO_OUTPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct REFS_QUERY_VOLUME_DEDUP_INFO_OUTPUT_BUFFER {
    pub Enabled: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for REFS_QUERY_VOLUME_DEDUP_INFO_OUTPUT_BUFFER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for REFS_QUERY_VOLUME_DEDUP_INFO_OUTPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_REMOVE_HARDLINK_BACKPOINTER {
    pub ParentDirectory: u64,
    pub Reserved: u64,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for REFS_REMOVE_HARDLINK_BACKPOINTER {}
impl ::core::clone::Clone for REFS_REMOVE_HARDLINK_BACKPOINTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_SET_VOLUME_COMPRESSION_INFO_INPUT_BUFFER {
    pub CompressionFormat: REFS_COMPRESSION_FORMATS,
    pub CompressionLevel: i16,
    pub CompressionChunkSizeBytes: u32,
    pub Flags: REFS_SET_VOLUME_COMPRESSION_INFO_FLAGS,
    pub Reserved: [u64; 8],
}
impl ::core::marker::Copy for REFS_SET_VOLUME_COMPRESSION_INFO_INPUT_BUFFER {}
impl ::core::clone::Clone for REFS_SET_VOLUME_COMPRESSION_INFO_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct REFS_SET_VOLUME_DEDUP_INFO_INPUT_BUFFER {
    pub Enable: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for REFS_SET_VOLUME_DEDUP_INFO_INPUT_BUFFER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for REFS_SET_VOLUME_DEDUP_INFO_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_STREAM_EXTENT {
    pub Vcn: i64,
    pub Lcn: i64,
    pub Length: i64,
    pub Properties: u16,
}
impl ::core::marker::Copy for REFS_STREAM_EXTENT {}
impl ::core::clone::Clone for REFS_STREAM_EXTENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_STREAM_SNAPSHOT_LIST_OUTPUT_BUFFER {
    pub EntryCount: u32,
    pub BufferSizeRequiredForQuery: u32,
    pub Reserved: [u32; 2],
    pub Entries: [REFS_STREAM_SNAPSHOT_LIST_OUTPUT_BUFFER_ENTRY; 1],
}
impl ::core::marker::Copy for REFS_STREAM_SNAPSHOT_LIST_OUTPUT_BUFFER {}
impl ::core::clone::Clone for REFS_STREAM_SNAPSHOT_LIST_OUTPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_STREAM_SNAPSHOT_LIST_OUTPUT_BUFFER_ENTRY {
    pub NextEntryOffset: u32,
    pub SnapshotNameLength: u16,
    pub SnapshotCreationTime: u64,
    pub StreamSize: u64,
    pub StreamAllocationSize: u64,
    pub Reserved: [u64; 2],
    pub SnapshotName: [u16; 1],
}
impl ::core::marker::Copy for REFS_STREAM_SNAPSHOT_LIST_OUTPUT_BUFFER_ENTRY {}
impl ::core::clone::Clone for REFS_STREAM_SNAPSHOT_LIST_OUTPUT_BUFFER_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_STREAM_SNAPSHOT_MANAGEMENT_INPUT_BUFFER {
    pub Operation: REFS_STREAM_SNAPSHOT_OPERATION,
    pub SnapshotNameLength: u16,
    pub OperationInputBufferLength: u16,
    pub Reserved: [u64; 2],
    pub NameAndInputBuffer: [u16; 1],
}
impl ::core::marker::Copy for REFS_STREAM_SNAPSHOT_MANAGEMENT_INPUT_BUFFER {}
impl ::core::clone::Clone for REFS_STREAM_SNAPSHOT_MANAGEMENT_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_STREAM_SNAPSHOT_QUERY_DELTAS_INPUT_BUFFER {
    pub StartingVcn: i64,
    pub Flags: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for REFS_STREAM_SNAPSHOT_QUERY_DELTAS_INPUT_BUFFER {}
impl ::core::clone::Clone for REFS_STREAM_SNAPSHOT_QUERY_DELTAS_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_STREAM_SNAPSHOT_QUERY_DELTAS_OUTPUT_BUFFER {
    pub ExtentCount: u32,
    pub Reserved: [u32; 2],
    pub Extents: [REFS_STREAM_EXTENT; 1],
}
impl ::core::marker::Copy for REFS_STREAM_SNAPSHOT_QUERY_DELTAS_OUTPUT_BUFFER {}
impl ::core::clone::Clone for REFS_STREAM_SNAPSHOT_QUERY_DELTAS_OUTPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct REFS_VOLUME_COUNTER_INFO_INPUT_BUFFER {
    pub ResetCounters: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for REFS_VOLUME_COUNTER_INFO_INPUT_BUFFER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for REFS_VOLUME_COUNTER_INFO_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REFS_VOLUME_DATA_BUFFER {
    pub ByteCount: u32,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub BytesPerPhysicalSector: u32,
    pub VolumeSerialNumber: i64,
    pub NumberSectors: i64,
    pub TotalClusters: i64,
    pub FreeClusters: i64,
    pub TotalReserved: i64,
    pub BytesPerSector: u32,
    pub BytesPerCluster: u32,
    pub MaximumSizeOfResidentFile: i64,
    pub FastTierDataFillRatio: u16,
    pub SlowTierDataFillRatio: u16,
    pub DestagesFastTierToSlowTierRate: u32,
    pub Reserved: [i64; 9],
}
impl ::core::marker::Copy for REFS_VOLUME_DATA_BUFFER {}
impl ::core::clone::Clone for REFS_VOLUME_DATA_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REMOTE_LINK_TRACKING_INFORMATION {
    pub TargetFileObject: *mut ::core::ffi::c_void,
    pub TargetLinkTrackingInformationLength: u32,
    pub TargetLinkTrackingInformationBuffer: [u8; 1],
}
impl ::core::marker::Copy for REMOTE_LINK_TRACKING_INFORMATION {}
impl ::core::clone::Clone for REMOTE_LINK_TRACKING_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REPARSE_DATA_BUFFER {
    pub ReparseTag: u32,
    pub ReparseDataLength: u16,
    pub Reserved: u16,
    pub Anonymous: REPARSE_DATA_BUFFER_0,
}
impl ::core::marker::Copy for REPARSE_DATA_BUFFER {}
impl ::core::clone::Clone for REPARSE_DATA_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union REPARSE_DATA_BUFFER_0 {
    pub SymbolicLinkReparseBuffer: REPARSE_DATA_BUFFER_0_2,
    pub MountPointReparseBuffer: REPARSE_DATA_BUFFER_0_1,
    pub GenericReparseBuffer: REPARSE_DATA_BUFFER_0_0,
}
impl ::core::marker::Copy for REPARSE_DATA_BUFFER_0 {}
impl ::core::clone::Clone for REPARSE_DATA_BUFFER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REPARSE_DATA_BUFFER_0_0 {
    pub DataBuffer: [u8; 1],
}
impl ::core::marker::Copy for REPARSE_DATA_BUFFER_0_0 {}
impl ::core::clone::Clone for REPARSE_DATA_BUFFER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REPARSE_DATA_BUFFER_0_1 {
    pub SubstituteNameOffset: u16,
    pub SubstituteNameLength: u16,
    pub PrintNameOffset: u16,
    pub PrintNameLength: u16,
    pub PathBuffer: [u16; 1],
}
impl ::core::marker::Copy for REPARSE_DATA_BUFFER_0_1 {}
impl ::core::clone::Clone for REPARSE_DATA_BUFFER_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REPARSE_DATA_BUFFER_0_2 {
    pub SubstituteNameOffset: u16,
    pub SubstituteNameLength: u16,
    pub PrintNameOffset: u16,
    pub PrintNameLength: u16,
    pub Flags: u32,
    pub PathBuffer: [u16; 1],
}
impl ::core::marker::Copy for REPARSE_DATA_BUFFER_0_2 {}
impl ::core::clone::Clone for REPARSE_DATA_BUFFER_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct REPARSE_DATA_BUFFER_EX {
    pub Flags: u32,
    pub ExistingReparseTag: u32,
    pub ExistingReparseGuid: ::windows_sys::core::GUID,
    pub Reserved: u64,
    pub Anonymous: REPARSE_DATA_BUFFER_EX_0,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for REPARSE_DATA_BUFFER_EX {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for REPARSE_DATA_BUFFER_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub union REPARSE_DATA_BUFFER_EX_0 {
    pub ReparseDataBuffer: REPARSE_DATA_BUFFER,
    pub ReparseGuidDataBuffer: super::super::super::Win32::Storage::FileSystem::REPARSE_GUID_DATA_BUFFER,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for REPARSE_DATA_BUFFER_EX_0 {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for REPARSE_DATA_BUFFER_EX_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct REPARSE_INDEX_KEY {
    pub FileReparseTag: u32,
    pub FileId: i64,
}
impl ::core::marker::Copy for REPARSE_INDEX_KEY {}
impl ::core::clone::Clone for REPARSE_INDEX_KEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RETRIEVAL_POINTERS_AND_REFCOUNT_BUFFER {
    pub ExtentCount: u32,
    pub StartingVcn: i64,
    pub Extents: [RETRIEVAL_POINTERS_AND_REFCOUNT_BUFFER_0; 1],
}
impl ::core::marker::Copy for RETRIEVAL_POINTERS_AND_REFCOUNT_BUFFER {}
impl ::core::clone::Clone for RETRIEVAL_POINTERS_AND_REFCOUNT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RETRIEVAL_POINTERS_AND_REFCOUNT_BUFFER_0 {
    pub NextVcn: i64,
    pub Lcn: i64,
    pub ReferenceCount: u32,
}
impl ::core::marker::Copy for RETRIEVAL_POINTERS_AND_REFCOUNT_BUFFER_0 {}
impl ::core::clone::Clone for RETRIEVAL_POINTERS_AND_REFCOUNT_BUFFER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RKF_BYPASS_ECP_CONTEXT {
    pub Reserved: i32,
    pub Version: i32,
}
impl ::core::marker::Copy for RKF_BYPASS_ECP_CONTEXT {}
impl ::core::clone::Clone for RKF_BYPASS_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTL_HEAP_MEMORY_LIMIT_DATA {
    pub CommitLimitBytes: usize,
    pub CommitLimitFailureCode: usize,
    pub MaxAllocationSizeBytes: usize,
    pub AllocationLimitFailureCode: usize,
}
impl ::core::marker::Copy for RTL_HEAP_MEMORY_LIMIT_DATA {}
impl ::core::clone::Clone for RTL_HEAP_MEMORY_LIMIT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTL_HEAP_MEMORY_LIMIT_INFO {
    pub Version: u32,
    pub Data: RTL_HEAP_MEMORY_LIMIT_DATA,
}
impl ::core::marker::Copy for RTL_HEAP_MEMORY_LIMIT_INFO {}
impl ::core::clone::Clone for RTL_HEAP_MEMORY_LIMIT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RTL_HEAP_PARAMETERS {
    pub Length: u32,
    pub SegmentReserve: usize,
    pub SegmentCommit: usize,
    pub DeCommitFreeBlockThreshold: usize,
    pub DeCommitTotalFreeThreshold: usize,
    pub MaximumAllocationSize: usize,
    pub VirtualMemoryThreshold: usize,
    pub InitialCommit: usize,
    pub InitialReserve: usize,
    pub CommitRoutine: PRTL_HEAP_COMMIT_ROUTINE,
    pub Reserved: [usize; 2],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RTL_HEAP_PARAMETERS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RTL_HEAP_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTL_NLS_STATE {
    pub DefaultAcpTableInfo: CPTABLEINFO,
    pub DefaultOemTableInfo: CPTABLEINFO,
    pub ActiveCodePageData: *mut u16,
    pub OemCodePageData: *mut u16,
    pub LeadByteInfo: *mut u16,
    pub OemLeadByteInfo: *mut u16,
    pub CaseMappingData: *mut u16,
    pub UnicodeUpcaseTable844: *mut u16,
    pub UnicodeLowercaseTable844: *mut u16,
}
impl ::core::marker::Copy for RTL_NLS_STATE {}
impl ::core::clone::Clone for RTL_NLS_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RTL_SEGMENT_HEAP_MEMORY_SOURCE {
    pub Flags: u32,
    pub MemoryTypeMask: u32,
    pub NumaNode: u32,
    pub Anonymous: RTL_SEGMENT_HEAP_MEMORY_SOURCE_0,
    pub Reserved: [usize; 2],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RTL_SEGMENT_HEAP_MEMORY_SOURCE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RTL_SEGMENT_HEAP_MEMORY_SOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union RTL_SEGMENT_HEAP_MEMORY_SOURCE_0 {
    pub PartitionHandle: super::super::super::Win32::Foundation::HANDLE,
    pub Callbacks: *mut RTL_SEGMENT_HEAP_VA_CALLBACKS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RTL_SEGMENT_HEAP_MEMORY_SOURCE_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RTL_SEGMENT_HEAP_MEMORY_SOURCE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RTL_SEGMENT_HEAP_PARAMETERS {
    pub Version: u16,
    pub Size: u16,
    pub Flags: u32,
    pub MemorySource: RTL_SEGMENT_HEAP_MEMORY_SOURCE,
    pub Reserved: [usize; 4],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RTL_SEGMENT_HEAP_PARAMETERS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RTL_SEGMENT_HEAP_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RTL_SEGMENT_HEAP_VA_CALLBACKS {
    pub CallbackContext: super::super::super::Win32::Foundation::HANDLE,
    pub AllocateVirtualMemory: PALLOCATE_VIRTUAL_MEMORY_EX_CALLBACK,
    pub FreeVirtualMemory: PFREE_VIRTUAL_MEMORY_EX_CALLBACK,
    pub QueryVirtualMemory: PQUERY_VIRTUAL_MEMORY_CALLBACK,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RTL_SEGMENT_HEAP_VA_CALLBACKS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RTL_SEGMENT_HEAP_VA_CALLBACKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct SECURITY_CLIENT_CONTEXT {
    pub SecurityQos: super::super::super::Win32::Security::SECURITY_QUALITY_OF_SERVICE,
    pub ClientToken: *mut ::core::ffi::c_void,
    pub DirectlyAccessClientToken: super::super::super::Win32::Foundation::BOOLEAN,
    pub DirectAccessEffectiveOnly: super::super::super::Win32::Foundation::BOOLEAN,
    pub ServerIsRemote: super::super::super::Win32::Foundation::BOOLEAN,
    pub ClientTokenControl: super::super::super::Win32::Security::TOKEN_CONTROL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for SECURITY_CLIENT_CONTEXT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for SECURITY_CLIENT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Authentication_Identity\"`"]
#[cfg(feature = "Win32_Security_Authentication_Identity")]
pub struct SEC_APPLICATION_PROTOCOLS {
    pub ProtocolListsSize: u32,
    pub ProtocolLists: [super::super::super::Win32::Security::Authentication::Identity::SEC_APPLICATION_PROTOCOL_LIST; 1],
}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::marker::Copy for SEC_APPLICATION_PROTOCOLS {}
#[cfg(feature = "Win32_Security_Authentication_Identity")]
impl ::core::clone::Clone for SEC_APPLICATION_PROTOCOLS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEC_DTLS_MTU {
    pub PathMTU: u16,
}
impl ::core::marker::Copy for SEC_DTLS_MTU {}
impl ::core::clone::Clone for SEC_DTLS_MTU {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEC_FLAGS {
    pub Flags: u64,
}
impl ::core::marker::Copy for SEC_FLAGS {}
impl ::core::clone::Clone for SEC_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEC_NEGOTIATION_INFO {
    pub Size: u32,
    pub NameLength: u32,
    pub Name: *mut u16,
    pub Reserved: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for SEC_NEGOTIATION_INFO {}
impl ::core::clone::Clone for SEC_NEGOTIATION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEC_PRESHAREDKEY {
    pub KeySize: u16,
    pub Key: [u8; 1],
}
impl ::core::marker::Copy for SEC_PRESHAREDKEY {}
impl ::core::clone::Clone for SEC_PRESHAREDKEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEC_SRTP_MASTER_KEY_IDENTIFIER {
    pub MasterKeyIdentifierSize: u8,
    pub MasterKeyIdentifier: [u8; 1],
}
impl ::core::marker::Copy for SEC_SRTP_MASTER_KEY_IDENTIFIER {}
impl ::core::clone::Clone for SEC_SRTP_MASTER_KEY_IDENTIFIER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SET_CACHED_RUNS_STATE_INPUT_BUFFER {
    pub Enable: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SET_CACHED_RUNS_STATE_INPUT_BUFFER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SET_CACHED_RUNS_STATE_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct SE_AUDIT_INFO {
    pub Size: u32,
    pub AuditType: super::super::super::Win32::Security::AUDIT_EVENT_TYPE,
    pub AuditOperation: SE_AUDIT_OPERATION,
    pub AuditFlags: u32,
    pub SubsystemName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub ObjectTypeName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub ObjectName: super::super::super::Win32::Foundation::UNICODE_STRING,
    pub HandleId: *mut ::core::ffi::c_void,
    pub TransactionId: *mut ::windows_sys::core::GUID,
    pub OperationId: *mut super::super::super::Win32::Foundation::LUID,
    pub ObjectCreation: super::super::super::Win32::Foundation::BOOLEAN,
    pub GenerateOnClose: super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for SE_AUDIT_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for SE_AUDIT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SE_EXPORTS {
    pub SeCreateTokenPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeAssignPrimaryTokenPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeLockMemoryPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeIncreaseQuotaPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeUnsolicitedInputPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeTcbPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeSecurityPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeTakeOwnershipPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeLoadDriverPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeCreatePagefilePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeIncreaseBasePriorityPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeSystemProfilePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeSystemtimePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeProfileSingleProcessPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeCreatePermanentPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeBackupPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeRestorePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeShutdownPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeDebugPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeAuditPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeSystemEnvironmentPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeChangeNotifyPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeRemoteShutdownPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeNullSid: super::super::super::Win32::Foundation::PSID,
    pub SeWorldSid: super::super::super::Win32::Foundation::PSID,
    pub SeLocalSid: super::super::super::Win32::Foundation::PSID,
    pub SeCreatorOwnerSid: super::super::super::Win32::Foundation::PSID,
    pub SeCreatorGroupSid: super::super::super::Win32::Foundation::PSID,
    pub SeNtAuthoritySid: super::super::super::Win32::Foundation::PSID,
    pub SeDialupSid: super::super::super::Win32::Foundation::PSID,
    pub SeNetworkSid: super::super::super::Win32::Foundation::PSID,
    pub SeBatchSid: super::super::super::Win32::Foundation::PSID,
    pub SeInteractiveSid: super::super::super::Win32::Foundation::PSID,
    pub SeLocalSystemSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasAdminsSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasUsersSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasGuestsSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasPowerUsersSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasAccountOpsSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasSystemOpsSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasPrintOpsSid: super::super::super::Win32::Foundation::PSID,
    pub SeAliasBackupOpsSid: super::super::super::Win32::Foundation::PSID,
    pub SeAuthenticatedUsersSid: super::super::super::Win32::Foundation::PSID,
    pub SeRestrictedSid: super::super::super::Win32::Foundation::PSID,
    pub SeAnonymousLogonSid: super::super::super::Win32::Foundation::PSID,
    pub SeUndockPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeSyncAgentPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeEnableDelegationPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeLocalServiceSid: super::super::super::Win32::Foundation::PSID,
    pub SeNetworkServiceSid: super::super::super::Win32::Foundation::PSID,
    pub SeManageVolumePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeImpersonatePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeCreateGlobalPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeTrustedCredManAccessPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeRelabelPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeIncreaseWorkingSetPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeTimeZonePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeCreateSymbolicLinkPrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeIUserSid: super::super::super::Win32::Foundation::PSID,
    pub SeUntrustedMandatorySid: super::super::super::Win32::Foundation::PSID,
    pub SeLowMandatorySid: super::super::super::Win32::Foundation::PSID,
    pub SeMediumMandatorySid: super::super::super::Win32::Foundation::PSID,
    pub SeHighMandatorySid: super::super::super::Win32::Foundation::PSID,
    pub SeSystemMandatorySid: super::super::super::Win32::Foundation::PSID,
    pub SeOwnerRightsSid: super::super::super::Win32::Foundation::PSID,
    pub SeAllAppPackagesSid: super::super::super::Win32::Foundation::PSID,
    pub SeUserModeDriversSid: super::super::super::Win32::Foundation::PSID,
    pub SeProcTrustWinTcbSid: super::super::super::Win32::Foundation::PSID,
    pub SeTrustedInstallerSid: super::super::super::Win32::Foundation::PSID,
    pub SeDelegateSessionUserImpersonatePrivilege: super::super::super::Win32::Foundation::LUID,
    pub SeAppSiloSid: super::super::super::Win32::Foundation::PSID,
    pub SeAppSiloVolumeRootMinimalCapabilitySid: super::super::super::Win32::Foundation::PSID,
    pub SeAppSiloProfilesRootMinimalCapabilitySid: super::super::super::Win32::Foundation::PSID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SE_EXPORTS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SE_EXPORTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct SRV_OPEN_ECP_CONTEXT {
    pub ShareName: *mut super::super::super::Win32::Foundation::UNICODE_STRING,
    pub SocketAddress: *mut super::super::super::Win32::Networking::WinSock::SOCKADDR_STORAGE,
    pub OplockBlockState: super::super::super::Win32::Foundation::BOOLEAN,
    pub OplockAppState: super::super::super::Win32::Foundation::BOOLEAN,
    pub OplockFinalState: super::super::super::Win32::Foundation::BOOLEAN,
    pub Version: u16,
    pub InstanceType: SRV_INSTANCE_TYPE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for SRV_OPEN_ECP_CONTEXT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for SRV_OPEN_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYSTEM_PROCESS_TRUST_LABEL_ACE {
    pub Header: ACE_HEADER,
    pub Mask: u32,
    pub SidStart: u32,
}
impl ::core::marker::Copy for SYSTEM_PROCESS_TRUST_LABEL_ACE {}
impl ::core::clone::Clone for SYSTEM_PROCESS_TRUST_LABEL_ACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SecBuffer {
    pub cbBuffer: u32,
    pub BufferType: u32,
    pub pvBuffer: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for SecBuffer {}
impl ::core::clone::Clone for SecBuffer {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SecBufferDesc {
    pub ulVersion: u32,
    pub cBuffers: u32,
    pub pBuffers: *mut SecBuffer,
}
impl ::core::marker::Copy for SecBufferDesc {}
impl ::core::clone::Clone for SecBufferDesc {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SecHandle {
    pub dwLower: usize,
    pub dwUpper: usize,
}
impl ::core::marker::Copy for SecHandle {}
impl ::core::clone::Clone for SecHandle {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct TUNNEL {
    pub Mutex: super::super::Foundation::FAST_MUTEX,
    pub Cache: *mut super::super::Foundation::RTL_SPLAY_LINKS,
    pub TimerQueue: super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub NumEntries: u16,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for TUNNEL {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for TUNNEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
pub struct UNICODE_PREFIX_TABLE {
    pub NodeTypeCode: i16,
    pub NameLength: i16,
    pub NextPrefixTree: *mut UNICODE_PREFIX_TABLE_ENTRY,
    pub LastNextEntry: *mut UNICODE_PREFIX_TABLE_ENTRY,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
impl ::core::marker::Copy for UNICODE_PREFIX_TABLE {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
impl ::core::clone::Clone for UNICODE_PREFIX_TABLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
pub struct UNICODE_PREFIX_TABLE_ENTRY {
    pub NodeTypeCode: i16,
    pub NameLength: i16,
    pub NextPrefixTree: *mut UNICODE_PREFIX_TABLE_ENTRY,
    pub CaseMatch: *mut UNICODE_PREFIX_TABLE_ENTRY,
    pub Links: super::super::Foundation::RTL_SPLAY_LINKS,
    pub Prefix: *mut super::super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
impl ::core::marker::Copy for UNICODE_PREFIX_TABLE_ENTRY {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
impl ::core::clone::Clone for UNICODE_PREFIX_TABLE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct USN_JOURNAL_DATA {
    pub UsnJournalID: u64,
    pub FirstUsn: i64,
    pub NextUsn: i64,
    pub LowestValidUsn: i64,
    pub MaxUsn: i64,
    pub MaximumSize: u64,
    pub AllocationDelta: u64,
    pub MinSupportedMajorVersion: u16,
    pub MaxSupportedMajorVersion: u16,
}
impl ::core::marker::Copy for USN_JOURNAL_DATA {}
impl ::core::clone::Clone for USN_JOURNAL_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct USN_RECORD {
    pub RecordLength: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub FileReferenceNumber: u64,
    pub ParentFileReferenceNumber: u64,
    pub Usn: i64,
    pub TimeStamp: i64,
    pub Reason: u32,
    pub SourceInfo: u32,
    pub SecurityId: u32,
    pub FileAttributes: u32,
    pub FileNameLength: u16,
    pub FileNameOffset: u16,
    pub FileName: [u16; 1],
}
impl ::core::marker::Copy for USN_RECORD {}
impl ::core::clone::Clone for USN_RECORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VCN_RANGE_INPUT_BUFFER {
    pub StartingVcn: i64,
    pub ClusterCount: i64,
}
impl ::core::marker::Copy for VCN_RANGE_INPUT_BUFFER {}
impl ::core::clone::Clone for VCN_RANGE_INPUT_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VOLUME_REFS_INFO_BUFFER {
    pub CacheSizeInBytes: i64,
    pub AllocatedCacheInBytes: i64,
    pub PopulatedCacheInBytes: i64,
    pub InErrorCacheInBytes: i64,
    pub MemoryUsedForCacheMetadata: i64,
    pub CacheLineSize: u32,
    pub CacheTransactionsOutstanding: i32,
    pub CacheLinesFree: i32,
    pub CacheLinesInError: i32,
    pub CacheHitsInBytes: i64,
    pub CacheMissesInBytes: i64,
    pub CachePopulationUpdatesInBytes: i64,
    pub CacheWriteThroughUpdatesInBytes: i64,
    pub CacheInvalidationsInBytes: i64,
    pub CacheOverReadsInBytes: i64,
    pub MetadataWrittenBytes: i64,
    pub CacheHitCounter: i32,
    pub CacheMissCounter: i32,
    pub CacheLineAllocationCounter: i32,
    pub CacheInvalidationsCounter: i32,
    pub CachePopulationUpdatesCounter: i32,
    pub CacheWriteThroughUpdatesCounter: i32,
    pub MaxCacheTransactionsOutstanding: i32,
    pub DataWritesReallocationCount: i32,
    pub DataInPlaceWriteCount: i32,
    pub MetadataAllocationsFastTierCount: i32,
    pub MetadataAllocationsSlowTierCount: i32,
    pub DataAllocationsFastTierCount: i32,
    pub DataAllocationsSlowTierCount: i32,
    pub DestagesSlowTierToFastTier: i32,
    pub DestagesFastTierToSlowTier: i32,
    pub SlowTierDataFillRatio: i32,
    pub FastTierDataFillRatio: i32,
    pub SlowTierMetadataFillRatio: i32,
    pub FastTierMetadataFillRatio: i32,
    pub SlowToFastDestageReadLatency: i32,
    pub SlowToFastDestageReadLatencyBase: i32,
    pub SlowToFastDestageWriteLatency: i32,
    pub SlowToFastDestageWriteLatencyBase: i32,
    pub FastToSlowDestageReadLatency: i32,
    pub FastToSlowDestageReadLatencyBase: i32,
    pub FastToSlowDestageWriteLatency: i32,
    pub FastToSlowDestageWriteLatencyBase: i32,
    pub SlowTierContainerFillRatio: i32,
    pub SlowTierContainerFillRatioBase: i32,
    pub FastTierContainerFillRatio: i32,
    pub FastTierContainerFillRatioBase: i32,
    pub TreeUpdateLatency: i32,
    pub TreeUpdateLatencyBase: i32,
    pub CheckpointLatency: i32,
    pub CheckpointLatencyBase: i32,
    pub TreeUpdateCount: i32,
    pub CheckpointCount: i32,
    pub LogWriteCount: i32,
    pub LogFillRatio: i32,
    pub ReadCacheInvalidationsForOverwrite: i32,
    pub ReadCacheInvalidationsForReuse: i32,
    pub ReadCacheInvalidationsGeneral: i32,
    pub ReadCacheChecksOnMount: i32,
    pub ReadCacheIssuesOnMount: i32,
    pub TrimLatency: i32,
    pub TrimLatencyBase: i32,
    pub DataCompactionCount: i32,
    pub CompactionReadLatency: i32,
    pub CompactionReadLatencyBase: i32,
    pub CompactionWriteLatency: i32,
    pub CompactionWriteLatencyBase: i32,
    pub DataInPlaceWriteClusterCount: i64,
    pub CompactionFailedDueToIneligibleContainer: i32,
    pub CompactionFailedDueToMaxFragmentation: i32,
    pub CompactedContainerFillRatio: i32,
    pub CompactedContainerFillRatioBase: i32,
    pub ContainerMoveRetryCount: i32,
    pub ContainerMoveFailedDueToIneligibleContainer: i32,
    pub CompactionFailureCount: i32,
    pub ContainerMoveFailureCount: i32,
    pub NumberOfDirtyMetadataPages: i64,
    pub NumberOfDirtyTableListEntries: i32,
    pub NumberOfDeleteQueueEntries: i32,
}
impl ::core::marker::Copy for VOLUME_REFS_INFO_BUFFER {}
impl ::core::clone::Clone for VOLUME_REFS_INFO_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub type ALLOCATE_VIRTUAL_MEMORY_EX_CALLBACK = ::core::option::Option<unsafe extern "system" fn(callbackcontext: super::super::super::Win32::Foundation::HANDLE, processhandle: super::super::super::Win32::Foundation::HANDLE, baseaddress: *mut *mut ::core::ffi::c_void, regionsize: *mut usize, allocationtype: u32, pageprotection: u32, extendedparameters: *mut super::super::super::Win32::System::Memory::MEM_EXTENDED_PARAMETER, extendedparametercount: u32) -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type DRIVER_FS_NOTIFICATION = ::core::option::Option<unsafe extern "system" fn(deviceobject: *const super::super::Foundation::DEVICE_OBJECT, fsactive: super::super::super::Win32::Foundation::BOOLEAN) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type FREE_VIRTUAL_MEMORY_EX_CALLBACK = ::core::option::Option<unsafe extern "system" fn(callbackcontext: super::super::super::Win32::Foundation::HANDLE, processhandle: super::super::super::Win32::Foundation::HANDLE, baseaddress: *mut *mut ::core::ffi::c_void, regionsize: *mut usize, freetype: u32) -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PACQUIRE_FOR_LAZY_WRITE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, wait: super::super::super::Win32::Foundation::BOOLEAN) -> super::super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PACQUIRE_FOR_LAZY_WRITE_EX = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, inflags: u32, outflags: *mut u32) -> super::super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PACQUIRE_FOR_READ_AHEAD = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, wait: super::super::super::Win32::Foundation::BOOLEAN) -> super::super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PALLOCATE_VIRTUAL_MEMORY_EX_CALLBACK = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PASYNC_READ_COMPLETION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation::BOOLEAN>;
pub type PCC_POST_DEFERRED_WRITE = ::core::option::Option<unsafe extern "system" fn(context1: *const ::core::ffi::c_void, context2: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_Security"))]
pub type PCHECK_FOR_TRAVERSE_ACCESS = ::core::option::Option<unsafe extern "system" fn(notifycontext: *const ::core::ffi::c_void, targetcontext: *const ::core::ffi::c_void, subjectcontext: *const super::super::Foundation::SECURITY_SUBJECT_CONTEXT) -> super::super::super::Win32::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PCOMPLETE_LOCK_IRP_ROUTINE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, irp: *const super::super::Foundation::IRP) -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PDIRTY_PAGE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(fileobject: *const super::super::Foundation::FILE_OBJECT, fileoffset: *const i64, length: u32, oldestlsn: *const i64, newestlsn: *const i64, context1: *const ::core::ffi::c_void, context2: *const ::core::ffi::c_void) -> ()>;
pub type PDRIVER_FS_NOTIFICATION = ::core::option::Option<unsafe extern "system" fn() -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFILTER_REPORT_CHANGE = ::core::option::Option<unsafe extern "system" fn(notifycontext: *const ::core::ffi::c_void, filtercontext: *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation::BOOLEAN>;
pub type PFLUSH_TO_LSN = ::core::option::Option<unsafe extern "system" fn(loghandle: *const ::core::ffi::c_void, lsn: i64) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub type PFN_FSRTLTEARDOWNPERSTREAMCONTEXTS = ::core::option::Option<unsafe extern "system" fn(advancedheader: *const FSRTL_ADVANCED_FCB_HEADER) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFREE_VIRTUAL_MEMORY_EX_CALLBACK = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Win32::Foundation::NTSTATUS>;
pub type PFSRTL_EXTRA_CREATE_PARAMETER_CLEANUP_CALLBACK = ::core::option::Option<unsafe extern "system" fn(ecpcontext: *mut ::core::ffi::c_void, ecptype: *const ::windows_sys::core::GUID) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub type PFSRTL_STACK_OVERFLOW_ROUTINE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, event: *const super::super::Foundation::KEVENT) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFS_FILTER_CALLBACK = ::core::option::Option<unsafe extern "system" fn(data: *const FS_FILTER_CALLBACK_DATA, completioncontext: *mut *mut ::core::ffi::c_void) -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFS_FILTER_COMPLETION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(data: *const FS_FILTER_CALLBACK_DATA, operationstatus: super::super::super::Win32::Foundation::NTSTATUS, completioncontext: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type POPLOCK_FS_PREPOST_IRP = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, irp: *const super::super::Foundation::IRP) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type POPLOCK_NOTIFY_ROUTINE = ::core::option::Option<unsafe extern "system" fn(notifyparams: *const OPLOCK_NOTIFY_PARAMS) -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type POPLOCK_WAIT_COMPLETE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, irp: *const super::super::Foundation::IRP) -> ()>;
pub type PQUERY_LOG_USAGE = ::core::option::Option<unsafe extern "system" fn(loghandle: *const ::core::ffi::c_void, percentagefull: *mut u16) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PQUERY_VIRTUAL_MEMORY_CALLBACK = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Win32::Foundation::NTSTATUS>;
pub type PRELEASE_FROM_LAZY_WRITE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type PRELEASE_FROM_READ_AHEAD = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type PRTL_ALLOCATE_STRING_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> *mut ::core::ffi::c_void>;
pub type PRTL_FREE_STRING_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRTL_HEAP_COMMIT_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Win32::Foundation::NTSTATUS>;
pub type PRTL_REALLOCATE_STRING_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSE_LOGON_SESSION_TERMINATED_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSE_LOGON_SESSION_TERMINATED_ROUTINE_EX = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PUNLOCK_ROUTINE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, filelockinfo: *const FILE_LOCK_INFO) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type QUERY_VIRTUAL_MEMORY_CALLBACK = ::core::option::Option<unsafe extern "system" fn(callbackcontext: super::super::super::Win32::Foundation::HANDLE, processhandle: super::super::super::Win32::Foundation::HANDLE, baseaddress: *const ::core::ffi::c_void, memoryinformationclass: HEAP_MEMORY_INFO_CLASS, memoryinformation: *mut ::core::ffi::c_void, memoryinformationlength: usize, returnlength: *mut usize) -> super::super::super::Win32::Foundation::NTSTATUS>;
pub type RTL_ALLOCATE_STRING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(numberofbytes: usize) -> *mut ::core::ffi::c_void>;
pub type RTL_FREE_STRING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(buffer: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type RTL_HEAP_COMMIT_ROUTINE = ::core::option::Option<unsafe extern "system" fn(base: *const ::core::ffi::c_void, commitaddress: *mut *mut ::core::ffi::c_void, commitsize: *mut usize) -> super::super::super::Win32::Foundation::NTSTATUS>;
pub type RTL_REALLOCATE_STRING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(numberofbytes: usize, buffer: *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type SE_LOGON_SESSION_TERMINATED_ROUTINE = ::core::option::Option<unsafe extern "system" fn(logonid: *const super::super::super::Win32::Foundation::LUID) -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
pub type SE_LOGON_SESSION_TERMINATED_ROUTINE_EX = ::core::option::Option<unsafe extern "system" fn(logonid: *const super::super::super::Win32::Foundation::LUID, pserversilo: super::super::Foundation::PESILO, context: *const ::core::ffi::c_void) -> super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`"]
#[cfg(feature = "Wdk_Foundation")]
pub type SspiAsyncNotifyCallback = ::core::option::Option<unsafe extern "system" fn(handle: *const super::super::Foundation::SspiAsyncContext, callbackdata: *const ::core::ffi::c_void) -> ()>;
