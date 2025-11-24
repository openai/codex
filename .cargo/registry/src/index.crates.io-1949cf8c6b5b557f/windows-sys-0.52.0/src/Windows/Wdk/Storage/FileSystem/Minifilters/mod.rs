::windows_targets::link!("fltmgr.sys" "system" fn FltAcknowledgeEcp(filter : PFLT_FILTER, ecpcontext : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltAcquirePushLockExclusive(pushlock : *mut usize) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltAcquirePushLockExclusiveEx(pushlock : *mut usize, flags : u32) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltAcquirePushLockShared(pushlock : *mut usize) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltAcquirePushLockSharedEx(pushlock : *mut usize, flags : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FltAcquireResourceExclusive(resource : *mut super::super::super::Foundation:: ERESOURCE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FltAcquireResourceShared(resource : *mut super::super::super::Foundation:: ERESOURCE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltAddOpenReparseEntry(filter : PFLT_FILTER, data : *const FLT_CALLBACK_DATA, openreparseentry : *const super:: OPEN_REPARSE_LIST_ENTRY) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltAdjustDeviceStackSizeForIoRedirection(sourceinstance : PFLT_INSTANCE, targetinstance : PFLT_INSTANCE, sourcedevicestacksizemodified : *mut super::super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltAllocateCallbackData(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, retnewcallbackdata : *mut *mut FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltAllocateCallbackDataEx(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, flags : u32, retnewcallbackdata : *mut *mut FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltAllocateContext(filter : PFLT_FILTER, contexttype : u16, contextsize : usize, pooltype : super::super::super::Foundation:: POOL_TYPE, returnedcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltAllocateDeferredIoWorkItem() -> PFLT_DEFERRED_IO_WORKITEM);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltAllocateExtraCreateParameter(filter : PFLT_FILTER, ecptype : *const ::windows_sys::core::GUID, sizeofcontext : u32, flags : u32, cleanupcallback : super:: PFSRTL_EXTRA_CREATE_PARAMETER_CLEANUP_CALLBACK, pooltag : u32, ecpcontext : *mut *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltAllocateExtraCreateParameterFromLookasideList(filter : PFLT_FILTER, ecptype : *const ::windows_sys::core::GUID, sizeofcontext : u32, flags : u32, cleanupcallback : super:: PFSRTL_EXTRA_CREATE_PARAMETER_CLEANUP_CALLBACK, lookasidelist : *mut ::core::ffi::c_void, ecpcontext : *mut *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltAllocateExtraCreateParameterList(filter : PFLT_FILTER, flags : u32, ecplist : *mut *mut super::super::super::Foundation:: ECP_LIST) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltAllocateFileLock(completelockcallbackdataroutine : PFLT_COMPLETE_LOCK_CALLBACK_DATA_ROUTINE, unlockroutine : super:: PUNLOCK_ROUTINE) -> *mut super:: FILE_LOCK);
::windows_targets::link!("fltmgr.sys" "system" fn FltAllocateGenericWorkItem() -> PFLT_GENERIC_WORKITEM);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FltAllocatePoolAlignedWithTag(instance : PFLT_INSTANCE, pooltype : super::super::super::Foundation:: POOL_TYPE, numberofbytes : usize, tag : u32) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltApplyPriorityInfoThread(inputpriorityinfo : *const super:: IO_PRIORITY_INFO, outputpriorityinfo : *mut super:: IO_PRIORITY_INFO, thread : super::super::super::Foundation:: PETHREAD) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltAttachVolume(filter : PFLT_FILTER, volume : PFLT_VOLUME, instancename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, retinstance : *mut PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltAttachVolumeAtAltitude(filter : PFLT_FILTER, volume : PFLT_VOLUME, altitude : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, instancename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, retinstance : *mut PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn FltBuildDefaultSecurityDescriptor(securitydescriptor : *mut super::super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, desiredaccess : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCancelFileOpen(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCancelIo(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCancellableWaitForMultipleObjects(count : u32, objectarray : *const *const ::core::ffi::c_void, waittype : super::super::super::super::Win32::System::Kernel:: WAIT_TYPE, timeout : *const i64, waitblockarray : *const super::super::super::Foundation:: KWAIT_BLOCK, callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCancellableWaitForSingleObject(object : *const ::core::ffi::c_void, timeout : *const i64, callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCbdqDisable(cbdq : *mut FLT_CALLBACK_DATA_QUEUE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCbdqEnable(cbdq : *mut FLT_CALLBACK_DATA_QUEUE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCbdqInitialize(instance : PFLT_INSTANCE, cbdq : *mut FLT_CALLBACK_DATA_QUEUE, cbdqinsertio : PFLT_CALLBACK_DATA_QUEUE_INSERT_IO, cbdqremoveio : PFLT_CALLBACK_DATA_QUEUE_REMOVE_IO, cbdqpeeknextio : PFLT_CALLBACK_DATA_QUEUE_PEEK_NEXT_IO, cbdqacquire : PFLT_CALLBACK_DATA_QUEUE_ACQUIRE, cbdqrelease : PFLT_CALLBACK_DATA_QUEUE_RELEASE, cbdqcompletecanceledio : PFLT_CALLBACK_DATA_QUEUE_COMPLETE_CANCELED_IO) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCbdqInsertIo(cbdq : *mut FLT_CALLBACK_DATA_QUEUE, cbd : *const FLT_CALLBACK_DATA, context : *const super::super::super::System::SystemServices:: IO_CSQ_IRP_CONTEXT, insertcontext : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCbdqRemoveIo(cbdq : *mut FLT_CALLBACK_DATA_QUEUE, context : *const super::super::super::System::SystemServices:: IO_CSQ_IRP_CONTEXT) -> *mut FLT_CALLBACK_DATA);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCbdqRemoveNextIo(cbdq : *mut FLT_CALLBACK_DATA_QUEUE, peekcontext : *const ::core::ffi::c_void) -> *mut FLT_CALLBACK_DATA);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltCheckAndGrowNameControl(namectrl : *mut FLT_NAME_CONTROL, newsize : u16) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCheckLockForReadAccess(filelock : *const super:: FILE_LOCK, callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCheckLockForWriteAccess(filelock : *const super:: FILE_LOCK, callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCheckOplock(oplock : *const *const ::core::ffi::c_void, callbackdata : *const FLT_CALLBACK_DATA, context : *const ::core::ffi::c_void, waitcompletionroutine : PFLTOPLOCK_WAIT_COMPLETE_ROUTINE, prepostcallbackdataroutine : PFLTOPLOCK_PREPOST_CALLBACKDATA_ROUTINE) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCheckOplockEx(oplock : *const *const ::core::ffi::c_void, callbackdata : *const FLT_CALLBACK_DATA, flags : u32, context : *const ::core::ffi::c_void, waitcompletionroutine : PFLTOPLOCK_WAIT_COMPLETE_ROUTINE, prepostcallbackdataroutine : PFLTOPLOCK_PREPOST_CALLBACKDATA_ROUTINE) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltClearCallbackDataDirty(data : *mut FLT_CALLBACK_DATA) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltClearCancelCompletion(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltClose(filehandle : super::super::super::super::Win32::Foundation:: HANDLE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltCloseClientPort(filter : PFLT_FILTER, clientport : *mut PFLT_PORT) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltCloseCommunicationPort(serverport : PFLT_PORT) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltCloseSectionForDataScan(sectioncontext : PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltCommitComplete(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, transactioncontext : PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltCommitFinalizeComplete(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, transactioncontext : PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltCompareInstanceAltitudes(instance1 : PFLT_INSTANCE, instance2 : PFLT_INSTANCE) -> i32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCompletePendedPostOperation(callbackdata : *const FLT_CALLBACK_DATA) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCompletePendedPreOperation(callbackdata : *const FLT_CALLBACK_DATA, callbackstatus : FLT_PREOP_CALLBACK_STATUS, context : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCopyOpenReparseList(filter : PFLT_FILTER, data : *const FLT_CALLBACK_DATA, ecplist : *mut super::super::super::Foundation:: ECP_LIST) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltCreateCommunicationPort(filter : PFLT_FILTER, serverport : *mut PFLT_PORT, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, serverportcookie : *const ::core::ffi::c_void, connectnotifycallback : PFLT_CONNECT_NOTIFY, disconnectnotifycallback : PFLT_DISCONNECT_NOTIFY, messagenotifycallback : PFLT_MESSAGE_NOTIFY, maxconnections : i32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn FltCreateFile(filter : PFLT_FILTER, instance : PFLT_INSTANCE, filehandle : *mut super::super::super::super::Win32::Foundation:: HANDLE, desiredaccess : u32, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, allocationsize : *const i64, fileattributes : u32, shareaccess : u32, createdisposition : u32, createoptions : u32, eabuffer : *const ::core::ffi::c_void, ealength : u32, flags : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCreateFileEx(filter : PFLT_FILTER, instance : PFLT_INSTANCE, filehandle : *mut super::super::super::super::Win32::Foundation:: HANDLE, fileobject : *mut *mut super::super::super::Foundation:: FILE_OBJECT, desiredaccess : u32, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, allocationsize : *const i64, fileattributes : u32, shareaccess : u32, createdisposition : u32, createoptions : u32, eabuffer : *const ::core::ffi::c_void, ealength : u32, flags : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCreateFileEx2(filter : PFLT_FILTER, instance : PFLT_INSTANCE, filehandle : *mut super::super::super::super::Win32::Foundation:: HANDLE, fileobject : *mut *mut super::super::super::Foundation:: FILE_OBJECT, desiredaccess : u32, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, allocationsize : *const i64, fileattributes : u32, shareaccess : u32, createdisposition : u32, createoptions : u32, eabuffer : *const ::core::ffi::c_void, ealength : u32, flags : u32, drivercontext : *const super::super::super::System::SystemServices:: IO_DRIVER_CREATE_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCreateMailslotFile(filter : PFLT_FILTER, instance : PFLT_INSTANCE, filehandle : *mut super::super::super::super::Win32::Foundation:: HANDLE, fileobject : *mut *mut super::super::super::Foundation:: FILE_OBJECT, desiredaccess : u32, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, createoptions : u32, mailslotquota : u32, maximummessagesize : u32, readtimeout : *const i64, drivercontext : *const super::super::super::System::SystemServices:: IO_DRIVER_CREATE_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCreateNamedPipeFile(filter : PFLT_FILTER, instance : PFLT_INSTANCE, filehandle : *mut super::super::super::super::Win32::Foundation:: HANDLE, fileobject : *mut *mut super::super::super::Foundation:: FILE_OBJECT, desiredaccess : u32, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, shareaccess : u32, createdisposition : u32, createoptions : u32, namedpipetype : u32, readmode : u32, completionmode : u32, maximuminstances : u32, inboundquota : u32, outboundquota : u32, defaulttimeout : *const i64, drivercontext : *const super::super::super::System::SystemServices:: IO_DRIVER_CREATE_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltCreateSectionForDataScan(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, sectioncontext : PFLT_CONTEXT, desiredaccess : u32, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, maximumsize : *const i64, sectionpageprotection : u32, allocationattributes : u32, flags : u32, sectionhandle : *mut super::super::super::super::Win32::Foundation:: HANDLE, sectionobject : *mut *mut ::core::ffi::c_void, sectionfilesize : *mut i64) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltCreateSystemVolumeInformationFolder(instance : PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltCurrentBatchOplock(oplock : *const *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltCurrentOplock(oplock : *const *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltCurrentOplockH(oplock : *const *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltDecodeParameters(callbackdata : *const FLT_CALLBACK_DATA, mdladdresspointer : *mut *mut *mut super::super::super::Foundation:: MDL, buffer : *mut *mut *mut ::core::ffi::c_void, length : *mut *mut u32, desiredaccess : *mut super::super::super::System::SystemServices:: LOCK_OPERATION) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltDeleteContext(context : PFLT_CONTEXT) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltDeleteExtraCreateParameterLookasideList(filter : PFLT_FILTER, lookaside : *mut ::core::ffi::c_void, flags : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltDeleteFileContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltDeleteInstanceContext(instance : PFLT_INSTANCE, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltDeletePushLock(pushlock : *const usize) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltDeleteStreamContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltDeleteStreamHandleContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltDeleteTransactionContext(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltDeleteVolumeContext(filter : PFLT_FILTER, volume : PFLT_VOLUME, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltDetachVolume(filter : PFLT_FILTER, volume : PFLT_VOLUME, instancename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltDeviceIoControlFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, iocontrolcode : u32, inputbuffer : *const ::core::ffi::c_void, inputbufferlength : u32, outputbuffer : *mut ::core::ffi::c_void, outputbufferlength : u32, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltDoCompletionProcessingWhenSafe(data : *const FLT_CALLBACK_DATA, fltobjects : *const FLT_RELATED_OBJECTS, completioncontext : *const ::core::ffi::c_void, flags : u32, safepostcallback : PFLT_POST_OPERATION_CALLBACK, retpostoperationstatus : *mut FLT_POSTOP_CALLBACK_STATUS) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltEnlistInTransaction(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, transactioncontext : PFLT_CONTEXT, notificationmask : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltEnumerateFilterInformation(index : u32, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: FILTER_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltEnumerateFilters(filterlist : *mut PFLT_FILTER, filterlistsize : u32, numberfiltersreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_InstallableFileSystems", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_Storage_InstallableFileSystems\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltEnumerateInstanceInformationByDeviceObject(deviceobject : *const super::super::super::Foundation:: DEVICE_OBJECT, index : u32, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: INSTANCE_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltEnumerateInstanceInformationByFilter(filter : PFLT_FILTER, index : u32, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: INSTANCE_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltEnumerateInstanceInformationByVolume(volume : PFLT_VOLUME, index : u32, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: INSTANCE_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltEnumerateInstanceInformationByVolumeName(volumename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, index : u32, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: INSTANCE_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltEnumerateInstances(volume : PFLT_VOLUME, filter : PFLT_FILTER, instancelist : *mut PFLT_INSTANCE, instancelistsize : u32, numberinstancesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltEnumerateVolumeInformation(filter : PFLT_FILTER, index : u32, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: FILTER_VOLUME_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltEnumerateVolumes(filter : PFLT_FILTER, volumelist : *mut PFLT_VOLUME, volumelistsize : u32, numbervolumesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFastIoMdlRead(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, lockkey : u32, mdlchain : *mut *mut super::super::super::Foundation:: MDL, iostatus : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFastIoMdlReadComplete(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, mdlchain : *const super::super::super::Foundation:: MDL) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFastIoMdlWriteComplete(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, mdlchain : *const super::super::super::Foundation:: MDL) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFastIoPrepareMdlWrite(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fileoffset : *const i64, length : u32, lockkey : u32, mdlchain : *mut *mut super::super::super::Foundation:: MDL, iostatus : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltFindExtraCreateParameter(filter : PFLT_FILTER, ecplist : *const super::super::super::Foundation:: ECP_LIST, ecptype : *const ::windows_sys::core::GUID, ecpcontext : *mut *mut ::core::ffi::c_void, ecpcontextsize : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFlushBuffers(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFlushBuffers2(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, flushtype : u32, callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFreeCallbackData(callbackdata : *const FLT_CALLBACK_DATA) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltFreeDeferredIoWorkItem(fltworkitem : PFLT_DEFERRED_IO_WORKITEM) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltFreeExtraCreateParameter(filter : PFLT_FILTER, ecpcontext : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FltFreeExtraCreateParameterList(filter : PFLT_FILTER, ecplist : *const super::super::super::Foundation:: ECP_LIST) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFreeFileLock(filelock : *const super:: FILE_LOCK) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltFreeGenericWorkItem(fltworkitem : PFLT_GENERIC_WORKITEM) -> ());
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FltFreeOpenReparseList(filter : PFLT_FILTER, ecplist : *const super::super::super::Foundation:: ECP_LIST) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltFreePoolAlignedWithTag(instance : PFLT_INSTANCE, buffer : *const ::core::ffi::c_void, tag : u32) -> ());
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn FltFreeSecurityDescriptor(securitydescriptor : super::super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltFsControlFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fscontrolcode : u32, inputbuffer : *const ::core::ffi::c_void, inputbufferlength : u32, outputbuffer : *mut ::core::ffi::c_void, outputbufferlength : u32, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetActivityIdCallbackData(callbackdata : *const FLT_CALLBACK_DATA, guid : *mut ::windows_sys::core::GUID) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetBottomInstance(volume : PFLT_VOLUME, instance : *mut PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetContexts(fltobjects : *const FLT_RELATED_OBJECTS, desiredcontexts : u16, contexts : *mut FLT_RELATED_CONTEXTS) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetContextsEx(fltobjects : *const FLT_RELATED_OBJECTS, desiredcontexts : u16, contextssize : usize, contexts : *mut FLT_RELATED_CONTEXTS_EX) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetDestinationFileNameInformation(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, rootdirectory : super::super::super::super::Win32::Foundation:: HANDLE, filename : ::windows_sys::core::PCWSTR, filenamelength : u32, nameoptions : u32, retfilenameinformation : *mut *mut FLT_FILE_NAME_INFORMATION) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetDeviceObject(volume : PFLT_VOLUME, deviceobject : *mut *mut super::super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetDiskDeviceObject(volume : PFLT_VOLUME, diskdeviceobject : *mut *mut super::super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetEcpListFromCallbackData(filter : PFLT_FILTER, callbackdata : *const FLT_CALLBACK_DATA, ecplist : *mut *mut super::super::super::Foundation:: ECP_LIST) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetFileContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, context : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetFileNameInformation(callbackdata : *const FLT_CALLBACK_DATA, nameoptions : u32, filenameinformation : *mut *mut FLT_FILE_NAME_INFORMATION) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetFileNameInformationUnsafe(fileobject : *const super::super::super::Foundation:: FILE_OBJECT, instance : PFLT_INSTANCE, nameoptions : u32, filenameinformation : *mut *mut FLT_FILE_NAME_INFORMATION) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltGetFileSystemType(fltobject : *const ::core::ffi::c_void, filesystemtype : *mut super::super::super::super::Win32::Storage::InstallableFileSystems:: FLT_FILESYSTEM_TYPE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetFilterFromInstance(instance : PFLT_INSTANCE, retfilter : *mut PFLT_FILTER) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetFilterFromName(filtername : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, retfilter : *mut PFLT_FILTER) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltGetFilterInformation(filter : PFLT_FILTER, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: FILTER_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetFsZeroingOffset(data : *const FLT_CALLBACK_DATA, zeroingoffset : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetInstanceContext(instance : PFLT_INSTANCE, context : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltGetInstanceInformation(instance : PFLT_INSTANCE, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: INSTANCE_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetIoAttributionHandleFromCallbackData(data : *const FLT_CALLBACK_DATA) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetIoPriorityHint(data : *const FLT_CALLBACK_DATA) -> super::super::super::Foundation:: IO_PRIORITY_HINT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetIoPriorityHintFromCallbackData(data : *const FLT_CALLBACK_DATA) -> super::super::super::Foundation:: IO_PRIORITY_HINT);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetIoPriorityHintFromFileObject(fileobject : *const super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::Foundation:: IO_PRIORITY_HINT);
#[cfg(feature = "Wdk_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`"] fn FltGetIoPriorityHintFromThread(thread : super::super::super::Foundation:: PETHREAD) -> super::super::super::Foundation:: IO_PRIORITY_HINT);
::windows_targets::link!("fltmgr.sys" "system" fn FltGetIrpName(irpmajorcode : u8) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetLowerInstance(currentinstance : PFLT_INSTANCE, lowerinstance : *mut PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetNewSystemBufferAddress(callbackdata : *const FLT_CALLBACK_DATA) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltGetNextExtraCreateParameter(filter : PFLT_FILTER, ecplist : *const super::super::super::Foundation:: ECP_LIST, currentecpcontext : *const ::core::ffi::c_void, nextecptype : *mut ::windows_sys::core::GUID, nextecpcontext : *mut *mut ::core::ffi::c_void, nextecpcontextsize : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetRequestorProcess(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::Foundation:: PEPROCESS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetRequestorProcessId(callbackdata : *const FLT_CALLBACK_DATA) -> u32);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetRequestorProcessIdEx(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: HANDLE);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetRequestorSessionId(callbackdata : *const FLT_CALLBACK_DATA, sessionid : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltGetRoutineAddress(fltmgrroutinename : ::windows_sys::core::PCSTR) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetSectionContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, context : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetStreamContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, context : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetStreamHandleContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, context : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetSwappedBufferMdlAddress(callbackdata : *const FLT_CALLBACK_DATA) -> *mut super::super::super::Foundation:: MDL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetTopInstance(volume : PFLT_VOLUME, instance : *mut PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltGetTransactionContext(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, context : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetTunneledName(callbackdata : *const FLT_CALLBACK_DATA, filenameinformation : *const FLT_FILE_NAME_INFORMATION, rettunneledfilenameinformation : *mut *mut FLT_FILE_NAME_INFORMATION) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetUpperInstance(currentinstance : PFLT_INSTANCE, upperinstance : *mut PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetVolumeContext(filter : PFLT_FILTER, volume : PFLT_VOLUME, context : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetVolumeFromDeviceObject(filter : PFLT_FILTER, deviceobject : *const super::super::super::Foundation:: DEVICE_OBJECT, retvolume : *mut PFLT_VOLUME) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltGetVolumeFromFileObject(filter : PFLT_FILTER, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, retvolume : *mut PFLT_VOLUME) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetVolumeFromInstance(instance : PFLT_INSTANCE, retvolume : *mut PFLT_VOLUME) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetVolumeFromName(filter : PFLT_FILTER, volumename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, retvolume : *mut PFLT_VOLUME) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetVolumeGuidName(volume : PFLT_VOLUME, volumeguidname : *mut super::super::super::super::Win32::Foundation:: UNICODE_STRING, buffersizeneeded : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_InstallableFileSystems"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_InstallableFileSystems\"`"] fn FltGetVolumeInformation(volume : PFLT_VOLUME, informationclass : super::super::super::super::Win32::Storage::InstallableFileSystems:: FILTER_VOLUME_INFORMATION_CLASS, buffer : *mut ::core::ffi::c_void, buffersize : u32, bytesreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetVolumeInstanceFromName(filter : PFLT_FILTER, volume : PFLT_VOLUME, instancename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, retinstance : *mut PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetVolumeName(volume : PFLT_VOLUME, volumename : *mut super::super::super::super::Win32::Foundation:: UNICODE_STRING, buffersizeneeded : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltGetVolumeProperties(volume : PFLT_VOLUME, volumeproperties : *mut FLT_VOLUME_PROPERTIES, volumepropertieslength : u32, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltInitExtraCreateParameterLookasideList(filter : PFLT_FILTER, lookaside : *mut ::core::ffi::c_void, flags : u32, size : usize, tag : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltInitializeFileLock(filelock : *mut super:: FILE_LOCK) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltInitializeOplock(oplock : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltInitializePushLock(pushlock : *mut usize) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltInsertExtraCreateParameter(filter : PFLT_FILTER, ecplist : *mut super::super::super::Foundation:: ECP_LIST, ecpcontext : *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltIs32bitProcess(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltIsCallbackDataDirty(data : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltIsDirectory(fileobject : *const super::super::super::Foundation:: FILE_OBJECT, instance : PFLT_INSTANCE, isdirectory : *mut super::super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltIsEcpAcknowledged(filter : PFLT_FILTER, ecpcontext : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltIsEcpFromUserMode(filter : PFLT_FILTER, ecpcontext : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltIsFltMgrVolumeDeviceObject(deviceobject : *const super::super::super::Foundation:: DEVICE_OBJECT) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltIsIoCanceled(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltIsIoRedirectionAllowed(sourceinstance : PFLT_INSTANCE, targetinstance : PFLT_INSTANCE, redirectionallowed : *mut super::super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltIsIoRedirectionAllowedForOperation(data : *const FLT_CALLBACK_DATA, targetinstance : PFLT_INSTANCE, redirectionallowedthisio : *mut super::super::super::super::Win32::Foundation:: BOOLEAN, redirectionallowedallio : *mut super::super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltIsOperationSynchronous(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltIsVolumeSnapshot(fltobject : *const ::core::ffi::c_void, issnapshotvolume : *mut super::super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltIsVolumeWritable(fltobject : *const ::core::ffi::c_void, iswritable : *mut super::super::super::super::Win32::Foundation:: BOOLEAN) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltLoadFilter(filtername : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltLockUserBuffer(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltNotifyFilterChangeDirectory(notifysync : super::super::super::Foundation:: PNOTIFY_SYNC, notifylist : *mut super::super::super::super::Win32::System::Kernel:: LIST_ENTRY, fscontext : *const ::core::ffi::c_void, fulldirectoryname : *const super::super::super::super::Win32::System::Kernel:: STRING, watchtree : super::super::super::super::Win32::Foundation:: BOOLEAN, ignorebuffer : super::super::super::super::Win32::Foundation:: BOOLEAN, completionfilter : u32, notifycallbackdata : *const FLT_CALLBACK_DATA, traversecallback : super:: PCHECK_FOR_TRAVERSE_ACCESS, subjectcontext : *const super::super::super::Foundation:: SECURITY_SUBJECT_CONTEXT, filtercallback : super:: PFILTER_REPORT_CHANGE) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltObjectDereference(fltobject : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltObjectReference(fltobject : *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOpenVolume(instance : PFLT_INSTANCE, volumehandle : *mut super::super::super::super::Win32::Foundation:: HANDLE, volumefileobject : *mut *mut super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOplockBreakH(oplock : *const *const ::core::ffi::c_void, callbackdata : *const FLT_CALLBACK_DATA, flags : u32, context : *const ::core::ffi::c_void, waitcompletionroutine : PFLTOPLOCK_WAIT_COMPLETE_ROUTINE, prepostcallbackdataroutine : PFLTOPLOCK_PREPOST_CALLBACKDATA_ROUTINE) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOplockBreakToNone(oplock : *const *const ::core::ffi::c_void, callbackdata : *const FLT_CALLBACK_DATA, context : *const ::core::ffi::c_void, waitcompletionroutine : PFLTOPLOCK_WAIT_COMPLETE_ROUTINE, prepostcallbackdataroutine : PFLTOPLOCK_PREPOST_CALLBACKDATA_ROUTINE) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOplockBreakToNoneEx(oplock : *const *const ::core::ffi::c_void, callbackdata : *const FLT_CALLBACK_DATA, flags : u32, context : *const ::core::ffi::c_void, waitcompletionroutine : PFLTOPLOCK_WAIT_COMPLETE_ROUTINE, prepostcallbackdataroutine : PFLTOPLOCK_PREPOST_CALLBACKDATA_ROUTINE) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOplockFsctrl(oplock : *const *const ::core::ffi::c_void, callbackdata : *const FLT_CALLBACK_DATA, opencount : u32) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOplockFsctrlEx(oplock : *const *const ::core::ffi::c_void, callbackdata : *const FLT_CALLBACK_DATA, opencount : u32, flags : u32) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltOplockIsFastIoPossible(oplock : *const *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOplockIsSharedRequest(callbackdata : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltOplockKeysEqual(fo1 : *const super::super::super::Foundation:: FILE_OBJECT, fo2 : *const super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltParseFileName(filename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, extension : *mut super::super::super::super::Win32::Foundation:: UNICODE_STRING, stream : *mut super::super::super::super::Win32::Foundation:: UNICODE_STRING, finalcomponent : *mut super::super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltParseFileNameInformation(filenameinformation : *mut FLT_FILE_NAME_INFORMATION) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltPerformAsynchronousIo(callbackdata : *mut FLT_CALLBACK_DATA, callbackroutine : PFLT_COMPLETED_ASYNC_IO_CALLBACK, callbackcontext : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltPerformSynchronousIo(callbackdata : *mut FLT_CALLBACK_DATA) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltPrePrepareComplete(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, transactioncontext : PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltPrepareComplete(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, transactioncontext : PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltPrepareToReuseEcp(filter : PFLT_FILTER, ecpcontext : *const ::core::ffi::c_void) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltProcessFileLock(filelock : *const super:: FILE_LOCK, callbackdata : *const FLT_CALLBACK_DATA, context : *const ::core::ffi::c_void) -> FLT_PREOP_CALLBACK_STATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltPropagateActivityIdToThread(callbackdata : *const FLT_CALLBACK_DATA, propagateid : *mut ::windows_sys::core::GUID, originalid : *mut *mut ::windows_sys::core::GUID) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltPropagateIrpExtension(sourcedata : *const FLT_CALLBACK_DATA, targetdata : *mut FLT_CALLBACK_DATA, flags : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltPurgeFileNameInformationCache(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueryDirectoryFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, returnsingleentry : super::super::super::super::Win32::Foundation:: BOOLEAN, filename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, restartscan : super::super::super::super::Win32::Foundation:: BOOLEAN, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueryDirectoryFileEx(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, queryflags : u32, filename : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueryEaFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, returnedeadata : *mut ::core::ffi::c_void, length : u32, returnsingleentry : super::super::super::super::Win32::Foundation:: BOOLEAN, ealist : *const ::core::ffi::c_void, ealistlength : u32, eaindex : *const u32, restartscan : super::super::super::super::Win32::Foundation:: BOOLEAN, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_System_IO", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_System_IO\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueryInformationByName(filter : PFLT_FILTER, instance : PFLT_INSTANCE, objectattributes : *const super::super::super::Foundation:: OBJECT_ATTRIBUTES, iostatusblock : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, drivercontext : *const super::super::super::System::SystemServices:: IO_DRIVER_CREATE_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueryInformationFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fileinformation : *mut ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueryQuotaInformationFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, iostatusblock : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, buffer : *mut ::core::ffi::c_void, length : u32, returnsingleentry : super::super::super::super::Win32::Foundation:: BOOLEAN, sidlist : *const ::core::ffi::c_void, sidlistlength : u32, startsid : *const u32, restartscan : super::super::super::super::Win32::Foundation:: BOOLEAN, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQuerySecurityObject(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, securityinformation : u32, securitydescriptor : super::super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR, length : u32, lengthneeded : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn FltQueryVolumeInformation(instance : PFLT_INSTANCE, iosb : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fsinformation : *mut ::core::ffi::c_void, length : u32, fsinformationclass : super:: FS_INFORMATION_CLASS) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueryVolumeInformationFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fsinformation : *mut ::core::ffi::c_void, length : u32, fsinformationclass : super:: FS_INFORMATION_CLASS, lengthreturned : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltQueueDeferredIoWorkItem(fltworkitem : PFLT_DEFERRED_IO_WORKITEM, data : *const FLT_CALLBACK_DATA, workerroutine : PFLT_DEFERRED_IO_WORKITEM_ROUTINE, queuetype : super::super::super::System::SystemServices:: WORK_QUEUE_TYPE, context : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_System_SystemServices", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`"] fn FltQueueGenericWorkItem(fltworkitem : PFLT_GENERIC_WORKITEM, fltobject : *const ::core::ffi::c_void, workerroutine : PFLT_GENERIC_WORKITEM_ROUTINE, queuetype : super::super::super::System::SystemServices:: WORK_QUEUE_TYPE, context : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltReadFile(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, byteoffset : *const i64, length : u32, buffer : *mut ::core::ffi::c_void, flags : u32, bytesread : *mut u32, callbackroutine : PFLT_COMPLETED_ASYNC_IO_CALLBACK, callbackcontext : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltReadFileEx(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, byteoffset : *const i64, length : u32, buffer : *mut ::core::ffi::c_void, flags : u32, bytesread : *mut u32, callbackroutine : PFLT_COMPLETED_ASYNC_IO_CALLBACK, callbackcontext : *const ::core::ffi::c_void, key : *const u32, mdl : *const super::super::super::Foundation:: MDL) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltReferenceContext(context : PFLT_CONTEXT) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltReferenceFileNameInformation(filenameinformation : *const FLT_FILE_NAME_INFORMATION) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_InstallableFileSystems", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_Storage_InstallableFileSystems\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRegisterFilter(driver : *const super::super::super::Foundation:: DRIVER_OBJECT, registration : *const FLT_REGISTRATION, retfilter : *mut PFLT_FILTER) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltRegisterForDataScan(instance : PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltReissueSynchronousIo(initiatinginstance : PFLT_INSTANCE, callbackdata : *const FLT_CALLBACK_DATA) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltReleaseContext(context : PFLT_CONTEXT) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltReleaseContexts(contexts : *const FLT_RELATED_CONTEXTS) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltReleaseContextsEx(contextssize : usize, contexts : *const FLT_RELATED_CONTEXTS_EX) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltReleaseFileNameInformation(filenameinformation : *const FLT_FILE_NAME_INFORMATION) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltReleasePushLock(pushlock : *mut usize) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltReleasePushLockEx(pushlock : *mut usize, flags : u32) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_System_Kernel\"`"] fn FltReleaseResource(resource : *mut super::super::super::Foundation:: ERESOURCE) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltRemoveExtraCreateParameter(filter : PFLT_FILTER, ecplist : *mut super::super::super::Foundation:: ECP_LIST, ecptype : *const ::windows_sys::core::GUID, ecpcontext : *mut *mut ::core::ffi::c_void, ecpcontextsize : *mut u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRemoveOpenReparseEntry(filter : PFLT_FILTER, data : *const FLT_CALLBACK_DATA, openreparseentry : *const super:: OPEN_REPARSE_LIST_ENTRY) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRequestFileInfoOnCreateCompletion(filter : PFLT_FILTER, data : *const FLT_CALLBACK_DATA, infoclassflags : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRequestOperationStatusCallback(data : *const FLT_CALLBACK_DATA, callbackroutine : PFLT_GET_OPERATION_STATUS_CALLBACK, requestercontext : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRetainSwappedBufferMdlAddress(callbackdata : *const FLT_CALLBACK_DATA) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRetrieveFileInfoOnCreateCompletion(filter : PFLT_FILTER, data : *const FLT_CALLBACK_DATA, infoclass : u32, size : *mut u32) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRetrieveFileInfoOnCreateCompletionEx(filter : PFLT_FILTER, data : *const FLT_CALLBACK_DATA, infoclass : u32, retinfosize : *mut u32, retinfobuffer : *mut *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltRetrieveIoPriorityInfo(data : *const FLT_CALLBACK_DATA, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, thread : super::super::super::Foundation:: PETHREAD, priorityinfo : *mut super:: IO_PRIORITY_INFO) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltReuseCallbackData(callbackdata : *mut FLT_CALLBACK_DATA) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltRollbackComplete(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, transactioncontext : PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltRollbackEnlistment(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, transactioncontext : PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltSendMessage(filter : PFLT_FILTER, clientport : *const PFLT_PORT, senderbuffer : *const ::core::ffi::c_void, senderbufferlength : u32, replybuffer : *mut ::core::ffi::c_void, replylength : *mut u32, timeout : *const i64) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetActivityIdCallbackData(callbackdata : *mut FLT_CALLBACK_DATA, guid : *const ::windows_sys::core::GUID) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetCallbackDataDirty(data : *mut FLT_CALLBACK_DATA) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetCancelCompletion(callbackdata : *const FLT_CALLBACK_DATA, canceledcallback : PFLT_COMPLETE_CANCELED_CALLBACK) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetEaFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, eabuffer : *const ::core::ffi::c_void, length : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetEcpListIntoCallbackData(filter : PFLT_FILTER, callbackdata : *const FLT_CALLBACK_DATA, ecplist : *const super::super::super::Foundation:: ECP_LIST) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetFileContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, operation : FLT_SET_CONTEXT_OPERATION, newcontext : PFLT_CONTEXT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetFsZeroingOffset(data : *const FLT_CALLBACK_DATA, zeroingoffset : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetFsZeroingOffsetRequired(data : *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetInformationFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, fileinformation : *const ::core::ffi::c_void, length : u32, fileinformationclass : super::super::super::super::Win32::System::WindowsProgramming:: FILE_INFORMATION_CLASS) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltSetInstanceContext(instance : PFLT_INSTANCE, operation : FLT_SET_CONTEXT_OPERATION, newcontext : PFLT_CONTEXT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetIoPriorityHintIntoCallbackData(data : *const FLT_CALLBACK_DATA, priorityhint : super::super::super::Foundation:: IO_PRIORITY_HINT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetIoPriorityHintIntoFileObject(fileobject : *const super::super::super::Foundation:: FILE_OBJECT, priorityhint : super::super::super::Foundation:: IO_PRIORITY_HINT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltSetIoPriorityHintIntoThread(thread : super::super::super::Foundation:: PETHREAD, priorityhint : super::super::super::Foundation:: IO_PRIORITY_HINT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetQuotaInformationFile(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, buffer : *const ::core::ffi::c_void, length : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetSecurityObject(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, securityinformation : u32, securitydescriptor : super::super::super::super::Win32::Security:: PSECURITY_DESCRIPTOR) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetStreamContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, operation : FLT_SET_CONTEXT_OPERATION, newcontext : PFLT_CONTEXT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSetStreamHandleContext(instance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, operation : FLT_SET_CONTEXT_OPERATION, newcontext : PFLT_CONTEXT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Win32_Foundation"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Win32_Foundation\"`"] fn FltSetTransactionContext(instance : PFLT_INSTANCE, transaction : *const super::super::super::Foundation:: KTRANSACTION, operation : FLT_SET_CONTEXT_OPERATION, newcontext : PFLT_CONTEXT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltSetVolumeContext(volume : PFLT_VOLUME, operation : FLT_SET_CONTEXT_OPERATION, newcontext : PFLT_CONTEXT, oldcontext : *mut PFLT_CONTEXT) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn FltSetVolumeInformation(instance : PFLT_INSTANCE, iosb : *mut super::super::super::super::Win32::System::IO:: IO_STATUS_BLOCK, fsinformation : *mut ::core::ffi::c_void, length : u32, fsinformationclass : super:: FS_INFORMATION_CLASS) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltStartFiltering(filter : PFLT_FILTER) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSupportsFileContexts(fileobject : *const super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSupportsFileContextsEx(fileobject : *const super::super::super::Foundation:: FILE_OBJECT, instance : PFLT_INSTANCE) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSupportsStreamContexts(fileobject : *const super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltSupportsStreamHandleContexts(fileobject : *const super::super::super::Foundation:: FILE_OBJECT) -> super::super::super::super::Win32::Foundation:: BOOLEAN);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltTagFile(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, filetag : u32, guid : *const ::windows_sys::core::GUID, databuffer : *const ::core::ffi::c_void, databufferlength : u16) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltTagFileEx(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, filetag : u32, guid : *const ::windows_sys::core::GUID, databuffer : *const ::core::ffi::c_void, databufferlength : u16, existingfiletag : u32, existingguid : *const ::windows_sys::core::GUID, flags : u32) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltUninitializeFileLock(filelock : *const super:: FILE_LOCK) -> ());
::windows_targets::link!("fltmgr.sys" "system" fn FltUninitializeOplock(oplock : *const *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FltUnloadFilter(filtername : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
::windows_targets::link!("fltmgr.sys" "system" fn FltUnregisterFilter(filter : PFLT_FILTER) -> ());
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltUntagFile(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, filetag : u32, guid : *const ::windows_sys::core::GUID) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltVetoBypassIo(callbackdata : *const FLT_CALLBACK_DATA, fltobjects : *const FLT_RELATED_OBJECTS, operationstatus : super::super::super::super::Win32::Foundation:: NTSTATUS, failurereason : *const super::super::super::super::Win32::Foundation:: UNICODE_STRING) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltWriteFile(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, byteoffset : *const i64, length : u32, buffer : *const ::core::ffi::c_void, flags : u32, byteswritten : *mut u32, callbackroutine : PFLT_COMPLETED_ASYNC_IO_CALLBACK, callbackcontext : *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltWriteFileEx(initiatinginstance : PFLT_INSTANCE, fileobject : *const super::super::super::Foundation:: FILE_OBJECT, byteoffset : *const i64, length : u32, buffer : *const ::core::ffi::c_void, flags : u32, byteswritten : *mut u32, callbackroutine : PFLT_COMPLETED_ASYNC_IO_CALLBACK, callbackcontext : *const ::core::ffi::c_void, key : *const u32, mdl : *const super::super::super::Foundation:: MDL) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
::windows_targets::link!("fltmgr.sys" "system" #[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"] fn FltpTraceRedirectedFileIo(originatingfileobject : *const super::super::super::Foundation:: FILE_OBJECT, childcallbackdata : *mut FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation:: NTSTATUS);
pub const FLTFL_CALLBACK_DATA_DIRTY: u32 = 2147483648u32;
pub const FLTFL_CALLBACK_DATA_DRAINING_IO: u32 = 262144u32;
pub const FLTFL_CALLBACK_DATA_FAST_IO_OPERATION: u32 = 2u32;
pub const FLTFL_CALLBACK_DATA_FS_FILTER_OPERATION: u32 = 4u32;
pub const FLTFL_CALLBACK_DATA_GENERATED_IO: u32 = 65536u32;
pub const FLTFL_CALLBACK_DATA_IRP_OPERATION: u32 = 1u32;
pub const FLTFL_CALLBACK_DATA_NEW_SYSTEM_BUFFER: u32 = 1048576u32;
pub const FLTFL_CALLBACK_DATA_POST_OPERATION: u32 = 524288u32;
pub const FLTFL_CALLBACK_DATA_REISSUED_IO: u32 = 131072u32;
pub const FLTFL_CALLBACK_DATA_REISSUE_MASK: u32 = 65535u32;
pub const FLTFL_CALLBACK_DATA_SYSTEM_BUFFER: u32 = 8u32;
pub const FLTFL_CONTEXT_REGISTRATION_NO_EXACT_SIZE_MATCH: u32 = 1u32;
pub const FLTFL_FILE_NAME_PARSED_EXTENSION: u32 = 2u32;
pub const FLTFL_FILE_NAME_PARSED_FINAL_COMPONENT: u32 = 1u32;
pub const FLTFL_FILE_NAME_PARSED_PARENT_DIR: u32 = 8u32;
pub const FLTFL_FILE_NAME_PARSED_STREAM: u32 = 4u32;
pub const FLTFL_FILTER_UNLOAD_MANDATORY: u32 = 1u32;
pub const FLTFL_INSTANCE_SETUP_AUTOMATIC_ATTACHMENT: u32 = 1u32;
pub const FLTFL_INSTANCE_SETUP_DETACHED_VOLUME: u32 = 8u32;
pub const FLTFL_INSTANCE_SETUP_MANUAL_ATTACHMENT: u32 = 2u32;
pub const FLTFL_INSTANCE_SETUP_NEWLY_MOUNTED_VOLUME: u32 = 4u32;
pub const FLTFL_INSTANCE_TEARDOWN_FILTER_UNLOAD: u32 = 2u32;
pub const FLTFL_INSTANCE_TEARDOWN_INTERNAL_ERROR: u32 = 16u32;
pub const FLTFL_INSTANCE_TEARDOWN_MANDATORY_FILTER_UNLOAD: u32 = 4u32;
pub const FLTFL_INSTANCE_TEARDOWN_MANUAL: u32 = 1u32;
pub const FLTFL_INSTANCE_TEARDOWN_VOLUME_DISMOUNT: u32 = 8u32;
pub const FLTFL_IO_OPERATION_DO_NOT_UPDATE_BYTE_OFFSET: u32 = 4u32;
pub const FLTFL_IO_OPERATION_NON_CACHED: u32 = 1u32;
pub const FLTFL_IO_OPERATION_PAGING: u32 = 2u32;
pub const FLTFL_IO_OPERATION_SYNCHRONOUS_PAGING: u32 = 8u32;
pub const FLTFL_NORMALIZE_NAME_CASE_SENSITIVE: u32 = 1u32;
pub const FLTFL_NORMALIZE_NAME_DESTINATION_FILE_NAME: u32 = 2u32;
pub const FLTFL_OPERATION_REGISTRATION_SKIP_CACHED_IO: u32 = 2u32;
pub const FLTFL_OPERATION_REGISTRATION_SKIP_NON_CACHED_NON_PAGING_IO: u32 = 8u32;
pub const FLTFL_OPERATION_REGISTRATION_SKIP_NON_DASD_IO: u32 = 4u32;
pub const FLTFL_OPERATION_REGISTRATION_SKIP_PAGING_IO: u32 = 1u32;
pub const FLTFL_POST_OPERATION_DRAINING: u32 = 1u32;
pub const FLTFL_REGISTRATION_DO_NOT_SUPPORT_SERVICE_STOP: u32 = 1u32;
pub const FLTFL_REGISTRATION_SUPPORT_DAX_VOLUME: u32 = 4u32;
pub const FLTFL_REGISTRATION_SUPPORT_NPFS_MSFS: u32 = 2u32;
pub const FLTFL_REGISTRATION_SUPPORT_WCOS: u32 = 8u32;
pub const FLTTCFL_AUTO_REPARSE: u32 = 1u32;
pub const FLT_ALLOCATE_CALLBACK_DATA_PREALLOCATE_ALL_MEMORY: u32 = 1u32;
pub const FLT_CONTEXT_END: u32 = 65535u32;
pub const FLT_FILE_CONTEXT: u32 = 4u32;
pub const FLT_FILE_NAME_ALLOW_QUERY_ON_REPARSE: u32 = 67108864u32;
pub const FLT_FILE_NAME_DO_NOT_CACHE: u32 = 33554432u32;
pub const FLT_FILE_NAME_NORMALIZED: u32 = 1u32;
pub const FLT_FILE_NAME_OPENED: u32 = 2u32;
pub const FLT_FILE_NAME_QUERY_ALWAYS_ALLOW_CACHE_LOOKUP: u32 = 1024u32;
pub const FLT_FILE_NAME_QUERY_CACHE_ONLY: u32 = 512u32;
pub const FLT_FILE_NAME_QUERY_DEFAULT: u32 = 256u32;
pub const FLT_FILE_NAME_QUERY_FILESYSTEM_ONLY: u32 = 768u32;
pub const FLT_FILE_NAME_REQUEST_FROM_CURRENT_PROVIDER: u32 = 16777216u32;
pub const FLT_FILE_NAME_SHORT: u32 = 3u32;
pub const FLT_FLUSH_TYPE_DATA_SYNC_ONLY: u32 = 8u32;
pub const FLT_FLUSH_TYPE_FILE_DATA_ONLY: u32 = 2u32;
pub const FLT_FLUSH_TYPE_FLUSH_AND_PURGE: u32 = 1u32;
pub const FLT_FLUSH_TYPE_NO_SYNC: u32 = 4u32;
pub const FLT_INSTANCE_CONTEXT: u32 = 2u32;
pub const FLT_INTERNAL_OPERATION_COUNT: u32 = 22u32;
pub const FLT_MAX_DEVICE_REPARSE_ATTEMPTS: u32 = 64u32;
pub const FLT_PORT_CONNECT: u32 = 1u32;
pub const FLT_POSTOP_DISALLOW_FSFILTER_IO: FLT_POSTOP_CALLBACK_STATUS = 2i32;
pub const FLT_POSTOP_FINISHED_PROCESSING: FLT_POSTOP_CALLBACK_STATUS = 0i32;
pub const FLT_POSTOP_MORE_PROCESSING_REQUIRED: FLT_POSTOP_CALLBACK_STATUS = 1i32;
pub const FLT_PREOP_COMPLETE: FLT_PREOP_CALLBACK_STATUS = 4i32;
pub const FLT_PREOP_DISALLOW_FASTIO: FLT_PREOP_CALLBACK_STATUS = 3i32;
pub const FLT_PREOP_DISALLOW_FSFILTER_IO: FLT_PREOP_CALLBACK_STATUS = 6i32;
pub const FLT_PREOP_PENDING: FLT_PREOP_CALLBACK_STATUS = 2i32;
pub const FLT_PREOP_SUCCESS_NO_CALLBACK: FLT_PREOP_CALLBACK_STATUS = 1i32;
pub const FLT_PREOP_SUCCESS_WITH_CALLBACK: FLT_PREOP_CALLBACK_STATUS = 0i32;
pub const FLT_PREOP_SYNCHRONIZE: FLT_PREOP_CALLBACK_STATUS = 5i32;
pub const FLT_PUSH_LOCK_DISABLE_AUTO_BOOST: u32 = 2u32;
pub const FLT_PUSH_LOCK_ENABLE_AUTO_BOOST: u32 = 1u32;
pub const FLT_PUSH_LOCK_VALID_FLAGS: u32 = 3u32;
pub const FLT_REGISTRATION_VERSION: u32 = 515u32;
pub const FLT_REGISTRATION_VERSION_0200: u32 = 512u32;
pub const FLT_REGISTRATION_VERSION_0201: u32 = 513u32;
pub const FLT_REGISTRATION_VERSION_0202: u32 = 514u32;
pub const FLT_REGISTRATION_VERSION_0203: u32 = 515u32;
pub const FLT_SECTION_CONTEXT: u32 = 64u32;
pub const FLT_SET_CONTEXT_KEEP_IF_EXISTS: FLT_SET_CONTEXT_OPERATION = 1i32;
pub const FLT_SET_CONTEXT_REPLACE_IF_EXISTS: FLT_SET_CONTEXT_OPERATION = 0i32;
pub const FLT_STREAMHANDLE_CONTEXT: u32 = 16u32;
pub const FLT_STREAM_CONTEXT: u32 = 8u32;
pub const FLT_TRANSACTION_CONTEXT: u32 = 32u32;
pub const FLT_VALID_FILE_NAME_FLAGS: u32 = 4278190080u32;
pub const FLT_VALID_FILE_NAME_FORMATS: u32 = 255u32;
pub const FLT_VALID_FILE_NAME_QUERY_METHODS: u32 = 65280u32;
pub const FLT_VOLUME_CONTEXT: u32 = 1u32;
pub const GUID_ECP_FLT_CREATEFILE_TARGET: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xce08041d_f411_447f_b70d_ccee45c23fac);
pub const VOL_PROP_FL_DAX_VOLUME: u32 = 1u32;
pub type FLT_CALLBACK_DATA_QUEUE_FLAGS = i32;
pub type FLT_POSTOP_CALLBACK_STATUS = i32;
pub type FLT_PREOP_CALLBACK_STATUS = i32;
pub type FLT_SET_CONTEXT_OPERATION = i32;
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_CALLBACK_DATA {
    pub Flags: u32,
    pub Thread: super::super::super::Foundation::PETHREAD,
    pub Iopb: *const FLT_IO_PARAMETER_BLOCK,
    pub IoStatus: super::super::super::super::Win32::System::IO::IO_STATUS_BLOCK,
    pub TagData: *mut FLT_TAG_DATA_BUFFER,
    pub Anonymous: FLT_CALLBACK_DATA_0,
    pub RequestorMode: i8,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_CALLBACK_DATA {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_CALLBACK_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FLT_CALLBACK_DATA_0 {
    pub Anonymous: FLT_CALLBACK_DATA_0_0,
    pub FilterContext: [*mut ::core::ffi::c_void; 4],
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_CALLBACK_DATA_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_CALLBACK_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_CALLBACK_DATA_0_0 {
    pub QueueLinks: super::super::super::super::Win32::System::Kernel::LIST_ENTRY,
    pub QueueContext: [*mut ::core::ffi::c_void; 2],
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_CALLBACK_DATA_0_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_CALLBACK_DATA_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_CALLBACK_DATA_QUEUE {
    pub Csq: super::super::super::System::SystemServices::IO_CSQ,
    pub Flags: FLT_CALLBACK_DATA_QUEUE_FLAGS,
    pub Instance: PFLT_INSTANCE,
    pub InsertIo: PFLT_CALLBACK_DATA_QUEUE_INSERT_IO,
    pub RemoveIo: PFLT_CALLBACK_DATA_QUEUE_REMOVE_IO,
    pub PeekNextIo: PFLT_CALLBACK_DATA_QUEUE_PEEK_NEXT_IO,
    pub Acquire: PFLT_CALLBACK_DATA_QUEUE_ACQUIRE,
    pub Release: PFLT_CALLBACK_DATA_QUEUE_RELEASE,
    pub CompleteCanceledIo: PFLT_CALLBACK_DATA_QUEUE_COMPLETE_CANCELED_IO,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_CALLBACK_DATA_QUEUE {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_CALLBACK_DATA_QUEUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`"]
#[cfg(feature = "Wdk_Foundation")]
pub struct FLT_CONTEXT_REGISTRATION {
    pub ContextType: u16,
    pub Flags: u16,
    pub ContextCleanupCallback: PFLT_CONTEXT_CLEANUP_CALLBACK,
    pub Size: usize,
    pub PoolTag: u32,
    pub ContextAllocateCallback: PFLT_CONTEXT_ALLOCATE_CALLBACK,
    pub ContextFreeCallback: PFLT_CONTEXT_FREE_CALLBACK,
    pub Reserved1: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Wdk_Foundation")]
impl ::core::marker::Copy for FLT_CONTEXT_REGISTRATION {}
#[cfg(feature = "Wdk_Foundation")]
impl ::core::clone::Clone for FLT_CONTEXT_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FLT_CREATEFILE_TARGET_ECP_CONTEXT {
    pub Instance: PFLT_INSTANCE,
    pub Volume: PFLT_VOLUME,
    pub FileNameInformation: *mut FLT_FILE_NAME_INFORMATION,
    pub Flags: u16,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FLT_CREATEFILE_TARGET_ECP_CONTEXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FLT_CREATEFILE_TARGET_ECP_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FLT_FILE_NAME_INFORMATION {
    pub Size: u16,
    pub NamesParsed: u16,
    pub Format: u32,
    pub Name: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub Volume: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub Share: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub Extension: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub Stream: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub FinalComponent: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub ParentDir: super::super::super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FLT_FILE_NAME_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FLT_FILE_NAME_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_IO_PARAMETER_BLOCK {
    pub IrpFlags: u32,
    pub MajorFunction: u8,
    pub MinorFunction: u8,
    pub OperationFlags: u8,
    pub Reserved: u8,
    pub TargetFileObject: *mut super::super::super::Foundation::FILE_OBJECT,
    pub TargetInstance: PFLT_INSTANCE,
    pub Parameters: FLT_PARAMETERS,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_IO_PARAMETER_BLOCK {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_IO_PARAMETER_BLOCK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FLT_NAME_CONTROL {
    pub Name: super::super::super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FLT_NAME_CONTROL {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FLT_NAME_CONTROL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_OPERATION_REGISTRATION {
    pub MajorFunction: u8,
    pub Flags: u32,
    pub PreOperation: PFLT_PRE_OPERATION_CALLBACK,
    pub PostOperation: PFLT_POST_OPERATION_CALLBACK,
    pub Reserved1: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_OPERATION_REGISTRATION {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_OPERATION_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FLT_PARAMETERS {
    pub Create: FLT_PARAMETERS_4,
    pub CreatePipe: FLT_PARAMETERS_3,
    pub CreateMailslot: FLT_PARAMETERS_2,
    pub Read: FLT_PARAMETERS_24,
    pub Write: FLT_PARAMETERS_32,
    pub QueryFileInformation: FLT_PARAMETERS_19,
    pub SetFileInformation: FLT_PARAMETERS_27,
    pub QueryEa: FLT_PARAMETERS_18,
    pub SetEa: FLT_PARAMETERS_26,
    pub QueryVolumeInformation: FLT_PARAMETERS_23,
    pub SetVolumeInformation: FLT_PARAMETERS_30,
    pub DirectoryControl: FLT_PARAMETERS_6,
    pub FileSystemControl: FLT_PARAMETERS_8,
    pub DeviceIoControl: FLT_PARAMETERS_5,
    pub LockControl: FLT_PARAMETERS_9,
    pub QuerySecurity: FLT_PARAMETERS_22,
    pub SetSecurity: FLT_PARAMETERS_29,
    pub WMI: FLT_PARAMETERS_31,
    pub QueryQuota: FLT_PARAMETERS_21,
    pub SetQuota: FLT_PARAMETERS_28,
    pub Pnp: FLT_PARAMETERS_16,
    pub AcquireForSectionSynchronization: FLT_PARAMETERS_1,
    pub AcquireForModifiedPageWriter: FLT_PARAMETERS_0,
    pub ReleaseForModifiedPageWriter: FLT_PARAMETERS_25,
    pub QueryOpen: FLT_PARAMETERS_20,
    pub FastIoCheckIfPossible: FLT_PARAMETERS_7,
    pub NetworkQueryOpen: FLT_PARAMETERS_14,
    pub MdlRead: FLT_PARAMETERS_11,
    pub MdlReadComplete: FLT_PARAMETERS_10,
    pub PrepareMdlWrite: FLT_PARAMETERS_17,
    pub MdlWriteComplete: FLT_PARAMETERS_12,
    pub MountVolume: FLT_PARAMETERS_13,
    pub Others: FLT_PARAMETERS_15,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_0 {
    pub EndingOffset: *mut i64,
    pub ResourceToRelease: *mut *mut super::super::super::Foundation::ERESOURCE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_1 {
    pub SyncType: super::FS_FILTER_SECTION_SYNC_TYPE,
    pub PageProtection: u32,
    pub OutputInformation: *mut super::FS_FILTER_SECTION_SYNC_OUTPUT,
    pub Flags: u32,
    pub AllocationAttributes: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_1 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_2 {
    pub SecurityContext: *mut super::super::super::Foundation::IO_SECURITY_CONTEXT,
    pub Options: u32,
    pub Reserved: u16,
    pub ShareAccess: u16,
    pub Parameters: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_2 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_3 {
    pub SecurityContext: *mut super::super::super::Foundation::IO_SECURITY_CONTEXT,
    pub Options: u32,
    pub Reserved: u16,
    pub ShareAccess: u16,
    pub Parameters: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_3 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_4 {
    pub SecurityContext: *mut super::super::super::Foundation::IO_SECURITY_CONTEXT,
    pub Options: u32,
    pub FileAttributes: u16,
    pub ShareAccess: u16,
    pub EaLength: u32,
    pub EaBuffer: *mut ::core::ffi::c_void,
    pub AllocationSize: i64,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_4 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FLT_PARAMETERS_5 {
    pub Common: FLT_PARAMETERS_5_1,
    pub Neither: FLT_PARAMETERS_5_4,
    pub Buffered: FLT_PARAMETERS_5_0,
    pub Direct: FLT_PARAMETERS_5_2,
    pub FastIo: FLT_PARAMETERS_5_3,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_5 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_5_0 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub IoControlCode: u32,
    pub SystemBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_5_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_5_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_5_1 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub IoControlCode: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_5_1 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_5_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_5_2 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub IoControlCode: u32,
    pub InputSystemBuffer: *mut ::core::ffi::c_void,
    pub OutputBuffer: *mut ::core::ffi::c_void,
    pub OutputMdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_5_2 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_5_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_5_3 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub IoControlCode: u32,
    pub InputBuffer: *mut ::core::ffi::c_void,
    pub OutputBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_5_3 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_5_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_5_4 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub IoControlCode: u32,
    pub InputBuffer: *mut ::core::ffi::c_void,
    pub OutputBuffer: *mut ::core::ffi::c_void,
    pub OutputMdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_5_4 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_5_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FLT_PARAMETERS_6 {
    pub QueryDirectory: FLT_PARAMETERS_6_2,
    pub NotifyDirectory: FLT_PARAMETERS_6_1,
    pub NotifyDirectoryEx: FLT_PARAMETERS_6_0,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_6 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_6_0 {
    pub Length: u32,
    pub CompletionFilter: u32,
    pub DirectoryNotifyInformationClass: super::super::super::System::SystemServices::DIRECTORY_NOTIFY_INFORMATION_CLASS,
    pub Spare2: u32,
    pub DirectoryBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_6_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_6_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_6_1 {
    pub Length: u32,
    pub CompletionFilter: u32,
    pub Spare1: u32,
    pub Spare2: u32,
    pub DirectoryBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_6_1 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_6_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_6_2 {
    pub Length: u32,
    pub FileName: *mut super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub FileInformationClass: super::super::super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
    pub FileIndex: u32,
    pub DirectoryBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_6_2 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_6_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_7 {
    pub FileOffset: i64,
    pub Length: u32,
    pub LockKey: u32,
    pub CheckForReadOperation: super::super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_7 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FLT_PARAMETERS_8 {
    pub VerifyVolume: FLT_PARAMETERS_8_4,
    pub Common: FLT_PARAMETERS_8_1,
    pub Neither: FLT_PARAMETERS_8_3,
    pub Buffered: FLT_PARAMETERS_8_0,
    pub Direct: FLT_PARAMETERS_8_2,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_8 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_8 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_8_0 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub FsControlCode: u32,
    pub SystemBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_8_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_8_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_8_1 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub FsControlCode: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_8_1 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_8_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_8_2 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub FsControlCode: u32,
    pub InputSystemBuffer: *mut ::core::ffi::c_void,
    pub OutputBuffer: *mut ::core::ffi::c_void,
    pub OutputMdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_8_2 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_8_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_8_3 {
    pub OutputBufferLength: u32,
    pub InputBufferLength: u32,
    pub FsControlCode: u32,
    pub InputBuffer: *mut ::core::ffi::c_void,
    pub OutputBuffer: *mut ::core::ffi::c_void,
    pub OutputMdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_8_3 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_8_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_8_4 {
    pub Vpb: *mut super::super::super::Foundation::VPB,
    pub DeviceObject: *mut super::super::super::Foundation::DEVICE_OBJECT,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_8_4 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_8_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_9 {
    pub Length: *mut i64,
    pub Key: u32,
    pub ByteOffset: i64,
    pub ProcessId: super::super::super::Foundation::PEPROCESS,
    pub FailImmediately: super::super::super::super::Win32::Foundation::BOOLEAN,
    pub ExclusiveLock: super::super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_9 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_9 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_10 {
    pub MdlChain: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_10 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_10 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_11 {
    pub FileOffset: i64,
    pub Length: u32,
    pub Key: u32,
    pub MdlChain: *mut *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_11 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_11 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_12 {
    pub FileOffset: i64,
    pub MdlChain: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_12 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_12 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_13 {
    pub DeviceType: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_13 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_13 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_14 {
    pub Irp: *mut super::super::super::Foundation::IRP,
    pub NetworkInformation: *mut super::FILE_NETWORK_OPEN_INFORMATION,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_14 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_14 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_15 {
    pub Argument1: *mut ::core::ffi::c_void,
    pub Argument2: *mut ::core::ffi::c_void,
    pub Argument3: *mut ::core::ffi::c_void,
    pub Argument4: *mut ::core::ffi::c_void,
    pub Argument5: *mut ::core::ffi::c_void,
    pub Argument6: i64,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_15 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_15 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FLT_PARAMETERS_16 {
    pub StartDevice: FLT_PARAMETERS_16_8,
    pub QueryDeviceRelations: FLT_PARAMETERS_16_2,
    pub QueryInterface: FLT_PARAMETERS_16_5,
    pub DeviceCapabilities: FLT_PARAMETERS_16_0,
    pub FilterResourceRequirements: FLT_PARAMETERS_16_1,
    pub ReadWriteConfig: FLT_PARAMETERS_16_6,
    pub SetLock: FLT_PARAMETERS_16_7,
    pub QueryId: FLT_PARAMETERS_16_4,
    pub QueryDeviceText: FLT_PARAMETERS_16_3,
    pub UsageNotification: FLT_PARAMETERS_16_9,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_0 {
    pub Capabilities: *mut super::super::super::System::SystemServices::DEVICE_CAPABILITIES,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_1 {
    pub IoResourceRequirementList: *mut super::super::super::System::SystemServices::IO_RESOURCE_REQUIREMENTS_LIST,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_1 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_2 {
    pub Type: super::super::super::System::SystemServices::DEVICE_RELATION_TYPE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_2 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_3 {
    pub DeviceTextType: super::super::super::System::SystemServices::DEVICE_TEXT_TYPE,
    pub LocaleId: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_3 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_4 {
    pub IdType: super::super::super::System::SystemServices::BUS_QUERY_ID_TYPE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_4 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_5 {
    pub InterfaceType: *const ::windows_sys::core::GUID,
    pub Size: u16,
    pub Version: u16,
    pub Interface: *mut super::super::super::System::SystemServices::INTERFACE,
    pub InterfaceSpecificData: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_5 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_6 {
    pub WhichSpace: u32,
    pub Buffer: *mut ::core::ffi::c_void,
    pub Offset: u32,
    pub Length: u32,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_6 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_7 {
    pub Lock: super::super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_7 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_8 {
    pub AllocatedResources: *mut super::super::super::System::SystemServices::CM_RESOURCE_LIST,
    pub AllocatedResourcesTranslated: *mut super::super::super::System::SystemServices::CM_RESOURCE_LIST,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_8 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_8 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_16_9 {
    pub InPath: super::super::super::super::Win32::Foundation::BOOLEAN,
    pub Reserved: [super::super::super::super::Win32::Foundation::BOOLEAN; 3],
    pub Type: super::super::super::System::SystemServices::DEVICE_USAGE_NOTIFICATION_TYPE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_16_9 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_16_9 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_17 {
    pub FileOffset: i64,
    pub Length: u32,
    pub Key: u32,
    pub MdlChain: *mut *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_17 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_17 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_18 {
    pub Length: u32,
    pub EaList: *mut ::core::ffi::c_void,
    pub EaListLength: u32,
    pub EaIndex: u32,
    pub EaBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_18 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_18 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_19 {
    pub Length: u32,
    pub FileInformationClass: super::super::super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
    pub InfoBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_19 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_19 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_20 {
    pub Irp: *mut super::super::super::Foundation::IRP,
    pub FileInformation: *mut ::core::ffi::c_void,
    pub Length: *mut u32,
    pub FileInformationClass: super::super::super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_20 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_20 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_21 {
    pub Length: u32,
    pub StartSid: super::super::super::super::Win32::Foundation::PSID,
    pub SidList: *mut super::FILE_GET_QUOTA_INFORMATION,
    pub SidListLength: u32,
    pub QuotaBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_21 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_21 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_22 {
    pub SecurityInformation: u32,
    pub Length: u32,
    pub SecurityBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_22 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_22 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_23 {
    pub Length: u32,
    pub FsInformationClass: super::FS_INFORMATION_CLASS,
    pub VolumeBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_23 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_23 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_24 {
    pub Length: u32,
    pub Key: u32,
    pub ByteOffset: i64,
    pub ReadBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_24 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_24 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_25 {
    pub ResourceToRelease: *mut super::super::super::Foundation::ERESOURCE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_25 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_25 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_26 {
    pub Length: u32,
    pub EaBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_26 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_26 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_27 {
    pub Length: u32,
    pub FileInformationClass: super::super::super::super::Win32::System::WindowsProgramming::FILE_INFORMATION_CLASS,
    pub ParentOfTarget: *mut super::super::super::Foundation::FILE_OBJECT,
    pub Anonymous: FLT_PARAMETERS_27_0,
    pub InfoBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_27 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_27 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub union FLT_PARAMETERS_27_0 {
    pub Anonymous: FLT_PARAMETERS_27_0_0,
    pub ClusterCount: u32,
    pub DeleteHandle: super::super::super::super::Win32::Foundation::HANDLE,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_27_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_27_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_27_0_0 {
    pub ReplaceIfExists: super::super::super::super::Win32::Foundation::BOOLEAN,
    pub AdvanceOnly: super::super::super::super::Win32::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_27_0_0 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_27_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_28 {
    pub Length: u32,
    pub QuotaBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_28 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_28 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_29 {
    pub SecurityInformation: u32,
    pub SecurityDescriptor: super::super::super::super::Win32::Security::PSECURITY_DESCRIPTOR,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_29 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_29 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_30 {
    pub Length: u32,
    pub FsInformationClass: super::FS_INFORMATION_CLASS,
    pub VolumeBuffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_30 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_30 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_31 {
    pub ProviderId: usize,
    pub DataPath: *mut ::core::ffi::c_void,
    pub BufferSize: u32,
    pub Buffer: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_31 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_31 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_PARAMETERS_32 {
    pub Length: u32,
    pub Key: u32,
    pub ByteOffset: i64,
    pub WriteBuffer: *mut ::core::ffi::c_void,
    pub MdlAddress: *mut super::super::super::Foundation::MDL,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_PARAMETERS_32 {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_PARAMETERS_32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_Storage_InstallableFileSystems\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_InstallableFileSystems", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_REGISTRATION {
    pub Size: u16,
    pub Version: u16,
    pub Flags: u32,
    pub ContextRegistration: *const FLT_CONTEXT_REGISTRATION,
    pub OperationRegistration: *const FLT_OPERATION_REGISTRATION,
    pub FilterUnloadCallback: PFLT_FILTER_UNLOAD_CALLBACK,
    pub InstanceSetupCallback: PFLT_INSTANCE_SETUP_CALLBACK,
    pub InstanceQueryTeardownCallback: PFLT_INSTANCE_QUERY_TEARDOWN_CALLBACK,
    pub InstanceTeardownStartCallback: PFLT_INSTANCE_TEARDOWN_CALLBACK,
    pub InstanceTeardownCompleteCallback: PFLT_INSTANCE_TEARDOWN_CALLBACK,
    pub GenerateFileNameCallback: PFLT_GENERATE_FILE_NAME,
    pub NormalizeNameComponentCallback: PFLT_NORMALIZE_NAME_COMPONENT,
    pub NormalizeContextCleanupCallback: PFLT_NORMALIZE_CONTEXT_CLEANUP,
    pub TransactionNotificationCallback: PFLT_TRANSACTION_NOTIFICATION_CALLBACK,
    pub NormalizeNameComponentExCallback: PFLT_NORMALIZE_NAME_COMPONENT_EX,
    pub SectionNotificationCallback: PFLT_SECTION_CONFLICT_NOTIFICATION_CALLBACK,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_InstallableFileSystems", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_REGISTRATION {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_InstallableFileSystems", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLT_RELATED_CONTEXTS {
    pub VolumeContext: PFLT_CONTEXT,
    pub InstanceContext: PFLT_CONTEXT,
    pub FileContext: PFLT_CONTEXT,
    pub StreamContext: PFLT_CONTEXT,
    pub StreamHandleContext: PFLT_CONTEXT,
    pub TransactionContext: PFLT_CONTEXT,
}
impl ::core::marker::Copy for FLT_RELATED_CONTEXTS {}
impl ::core::clone::Clone for FLT_RELATED_CONTEXTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLT_RELATED_CONTEXTS_EX {
    pub VolumeContext: PFLT_CONTEXT,
    pub InstanceContext: PFLT_CONTEXT,
    pub FileContext: PFLT_CONTEXT,
    pub StreamContext: PFLT_CONTEXT,
    pub StreamHandleContext: PFLT_CONTEXT,
    pub TransactionContext: PFLT_CONTEXT,
    pub SectionContext: PFLT_CONTEXT,
}
impl ::core::marker::Copy for FLT_RELATED_CONTEXTS_EX {}
impl ::core::clone::Clone for FLT_RELATED_CONTEXTS_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub struct FLT_RELATED_OBJECTS {
    pub Size: u16,
    pub TransactionContext: u16,
    pub Filter: PFLT_FILTER,
    pub Volume: PFLT_VOLUME,
    pub Instance: PFLT_INSTANCE,
    pub FileObject: *const super::super::super::Foundation::FILE_OBJECT,
    pub Transaction: *const super::super::super::Foundation::KTRANSACTION,
}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::marker::Copy for FLT_RELATED_OBJECTS {}
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
impl ::core::clone::Clone for FLT_RELATED_OBJECTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLT_TAG_DATA_BUFFER {
    pub FileTag: u32,
    pub TagDataLength: u16,
    pub UnparsedNameLength: u16,
    pub Anonymous: FLT_TAG_DATA_BUFFER_0,
}
impl ::core::marker::Copy for FLT_TAG_DATA_BUFFER {}
impl ::core::clone::Clone for FLT_TAG_DATA_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union FLT_TAG_DATA_BUFFER_0 {
    pub SymbolicLinkReparseBuffer: FLT_TAG_DATA_BUFFER_0_3,
    pub MountPointReparseBuffer: FLT_TAG_DATA_BUFFER_0_2,
    pub GenericReparseBuffer: FLT_TAG_DATA_BUFFER_0_1,
    pub GenericGUIDReparseBuffer: FLT_TAG_DATA_BUFFER_0_0,
}
impl ::core::marker::Copy for FLT_TAG_DATA_BUFFER_0 {}
impl ::core::clone::Clone for FLT_TAG_DATA_BUFFER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLT_TAG_DATA_BUFFER_0_0 {
    pub TagGuid: ::windows_sys::core::GUID,
    pub DataBuffer: [u8; 1],
}
impl ::core::marker::Copy for FLT_TAG_DATA_BUFFER_0_0 {}
impl ::core::clone::Clone for FLT_TAG_DATA_BUFFER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLT_TAG_DATA_BUFFER_0_1 {
    pub DataBuffer: [u8; 1],
}
impl ::core::marker::Copy for FLT_TAG_DATA_BUFFER_0_1 {}
impl ::core::clone::Clone for FLT_TAG_DATA_BUFFER_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLT_TAG_DATA_BUFFER_0_2 {
    pub SubstituteNameOffset: u16,
    pub SubstituteNameLength: u16,
    pub PrintNameOffset: u16,
    pub PrintNameLength: u16,
    pub PathBuffer: [u16; 1],
}
impl ::core::marker::Copy for FLT_TAG_DATA_BUFFER_0_2 {}
impl ::core::clone::Clone for FLT_TAG_DATA_BUFFER_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLT_TAG_DATA_BUFFER_0_3 {
    pub SubstituteNameOffset: u16,
    pub SubstituteNameLength: u16,
    pub PrintNameOffset: u16,
    pub PrintNameLength: u16,
    pub Flags: u32,
    pub PathBuffer: [u16; 1],
}
impl ::core::marker::Copy for FLT_TAG_DATA_BUFFER_0_3 {}
impl ::core::clone::Clone for FLT_TAG_DATA_BUFFER_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct FLT_VOLUME_PROPERTIES {
    pub DeviceType: u32,
    pub DeviceCharacteristics: u32,
    pub DeviceObjectFlags: u32,
    pub AlignmentRequirement: u32,
    pub SectorSize: u16,
    pub Flags: u16,
    pub FileSystemDriverName: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub FileSystemDeviceName: super::super::super::super::Win32::Foundation::UNICODE_STRING,
    pub RealDeviceName: super::super::super::super::Win32::Foundation::UNICODE_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for FLT_VOLUME_PROPERTIES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for FLT_VOLUME_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PFLT_CONTEXT = *mut ::core::ffi::c_void;
pub type PFLT_DEFERRED_IO_WORKITEM = isize;
pub type PFLT_FILTER = isize;
pub type PFLT_GENERIC_WORKITEM = isize;
pub type PFLT_INSTANCE = isize;
pub type PFLT_PORT = isize;
pub type PFLT_VOLUME = isize;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLTOPLOCK_PREPOST_CALLBACKDATA_ROUTINE = ::core::option::Option<unsafe extern "system" fn(callbackdata: *const FLT_CALLBACK_DATA, context: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLTOPLOCK_WAIT_COMPLETE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(callbackdata: *const FLT_CALLBACK_DATA, context: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_CALLBACK_DATA_QUEUE_ACQUIRE = ::core::option::Option<unsafe extern "system" fn(cbdq: *mut FLT_CALLBACK_DATA_QUEUE, irql: *mut u8) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_CALLBACK_DATA_QUEUE_COMPLETE_CANCELED_IO = ::core::option::Option<unsafe extern "system" fn(cbdq: *mut FLT_CALLBACK_DATA_QUEUE, cbd: *mut FLT_CALLBACK_DATA) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_CALLBACK_DATA_QUEUE_INSERT_IO = ::core::option::Option<unsafe extern "system" fn(cbdq: *mut FLT_CALLBACK_DATA_QUEUE, cbd: *const FLT_CALLBACK_DATA, insertcontext: *const ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_CALLBACK_DATA_QUEUE_PEEK_NEXT_IO = ::core::option::Option<unsafe extern "system" fn(cbdq: *const FLT_CALLBACK_DATA_QUEUE, cbd: *const FLT_CALLBACK_DATA, peekcontext: *const ::core::ffi::c_void) -> *mut FLT_CALLBACK_DATA>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_CALLBACK_DATA_QUEUE_RELEASE = ::core::option::Option<unsafe extern "system" fn(cbdq: *mut FLT_CALLBACK_DATA_QUEUE, irql: u8) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_CALLBACK_DATA_QUEUE_REMOVE_IO = ::core::option::Option<unsafe extern "system" fn(cbdq: *mut FLT_CALLBACK_DATA_QUEUE, cbd: *const FLT_CALLBACK_DATA) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_COMPLETED_ASYNC_IO_CALLBACK = ::core::option::Option<unsafe extern "system" fn(callbackdata: *const FLT_CALLBACK_DATA, context: PFLT_CONTEXT) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_COMPLETE_CANCELED_CALLBACK = ::core::option::Option<unsafe extern "system" fn(callbackdata: *const FLT_CALLBACK_DATA) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_COMPLETE_LOCK_CALLBACK_DATA_ROUTINE = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, callbackdata: *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFLT_CONNECT_NOTIFY = ::core::option::Option<unsafe extern "system" fn(clientport: PFLT_PORT, serverportcookie: *const ::core::ffi::c_void, connectioncontext: *const ::core::ffi::c_void, sizeofcontext: u32, connectionportcookie: *mut *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`"]
#[cfg(feature = "Wdk_Foundation")]
pub type PFLT_CONTEXT_ALLOCATE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pooltype: super::super::super::Foundation::POOL_TYPE, size: usize, contexttype: u16) -> *mut ::core::ffi::c_void>;
pub type PFLT_CONTEXT_CLEANUP_CALLBACK = ::core::option::Option<unsafe extern "system" fn(context: PFLT_CONTEXT, contexttype: u16) -> ()>;
pub type PFLT_CONTEXT_FREE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pool: *const ::core::ffi::c_void, contexttype: u16) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_DEFERRED_IO_WORKITEM_ROUTINE = ::core::option::Option<unsafe extern "system" fn(fltworkitem: PFLT_DEFERRED_IO_WORKITEM, callbackdata: *const FLT_CALLBACK_DATA, context: *const ::core::ffi::c_void) -> ()>;
pub type PFLT_DISCONNECT_NOTIFY = ::core::option::Option<unsafe extern "system" fn(connectioncookie: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFLT_FILTER_UNLOAD_CALLBACK = ::core::option::Option<unsafe extern "system" fn(flags: u32) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_GENERATE_FILE_NAME = ::core::option::Option<unsafe extern "system" fn(instance: PFLT_INSTANCE, fileobject: *const super::super::super::Foundation::FILE_OBJECT, callbackdata: *const FLT_CALLBACK_DATA, nameoptions: u32, cachefilenameinformation: *mut super::super::super::super::Win32::Foundation::BOOLEAN, filename: *mut FLT_NAME_CONTROL) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
pub type PFLT_GENERIC_WORKITEM_ROUTINE = ::core::option::Option<unsafe extern "system" fn(fltworkitem: PFLT_GENERIC_WORKITEM, fltobject: *const ::core::ffi::c_void, context: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_GET_OPERATION_STATUS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(fltobjects: *const FLT_RELATED_OBJECTS, iopbsnapshot: *const FLT_IO_PARAMETER_BLOCK, operationstatus: super::super::super::super::Win32::Foundation::NTSTATUS, requestercontext: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_INSTANCE_QUERY_TEARDOWN_CALLBACK = ::core::option::Option<unsafe extern "system" fn(fltobjects: *const FLT_RELATED_OBJECTS, flags: u32) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_Storage_InstallableFileSystems\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_Storage_InstallableFileSystems", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_INSTANCE_SETUP_CALLBACK = ::core::option::Option<unsafe extern "system" fn(fltobjects: *const FLT_RELATED_OBJECTS, flags: u32, volumedevicetype: u32, volumefilesystemtype: super::super::super::super::Win32::Storage::InstallableFileSystems::FLT_FILESYSTEM_TYPE) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_INSTANCE_TEARDOWN_CALLBACK = ::core::option::Option<unsafe extern "system" fn(fltobjects: *const FLT_RELATED_OBJECTS, reason: u32) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFLT_MESSAGE_NOTIFY = ::core::option::Option<unsafe extern "system" fn(portcookie: *const ::core::ffi::c_void, inputbuffer: *const ::core::ffi::c_void, inputbufferlength: u32, outputbuffer: *mut ::core::ffi::c_void, outputbufferlength: u32, returnoutputbufferlength: *mut u32) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
pub type PFLT_NORMALIZE_CONTEXT_CLEANUP = ::core::option::Option<unsafe extern "system" fn(normalizationcontext: *const *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFLT_NORMALIZE_NAME_COMPONENT = ::core::option::Option<unsafe extern "system" fn(instance: PFLT_INSTANCE, parentdirectory: *const super::super::super::super::Win32::Foundation::UNICODE_STRING, volumenamelength: u16, component: *const super::super::super::super::Win32::Foundation::UNICODE_STRING, expandcomponentname: *mut super::FILE_NAMES_INFORMATION, expandcomponentnamelength: u32, flags: u32, normalizationcontext: *mut *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_NORMALIZE_NAME_COMPONENT_EX = ::core::option::Option<unsafe extern "system" fn(instance: PFLT_INSTANCE, fileobject: *const super::super::super::Foundation::FILE_OBJECT, parentdirectory: *const super::super::super::super::Win32::Foundation::UNICODE_STRING, volumenamelength: u16, component: *const super::super::super::super::Win32::Foundation::UNICODE_STRING, expandcomponentname: *mut super::FILE_NAMES_INFORMATION, expandcomponentnamelength: u32, flags: u32, normalizationcontext: *mut *mut ::core::ffi::c_void) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_POST_OPERATION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(data: *mut FLT_CALLBACK_DATA, fltobjects: *const FLT_RELATED_OBJECTS, completioncontext: *const ::core::ffi::c_void, flags: u32) -> FLT_POSTOP_CALLBACK_STATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_PRE_OPERATION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(data: *mut FLT_CALLBACK_DATA, fltobjects: *const FLT_RELATED_OBJECTS, completioncontext: *mut *mut ::core::ffi::c_void) -> FLT_PREOP_CALLBACK_STATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_SECTION_CONFLICT_NOTIFICATION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(instance: PFLT_INSTANCE, sectioncontext: PFLT_CONTEXT, data: *const FLT_CALLBACK_DATA) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Wdk_Foundation\"`, `\"Wdk_System_SystemServices\"`, `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_IO\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Power\"`, `\"Win32_System_WindowsProgramming\"`"]
#[cfg(all(feature = "Wdk_Foundation", feature = "Wdk_System_SystemServices", feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_IO", feature = "Win32_System_Kernel", feature = "Win32_System_Power", feature = "Win32_System_WindowsProgramming"))]
pub type PFLT_TRANSACTION_NOTIFICATION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(fltobjects: *const FLT_RELATED_OBJECTS, transactioncontext: PFLT_CONTEXT, notificationmask: u32) -> super::super::super::super::Win32::Foundation::NTSTATUS>;
