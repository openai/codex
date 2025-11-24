#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("faultrep.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AddERExcludedApplicationA(szapplication : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("faultrep.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AddERExcludedApplicationW(wszapplication : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
::windows_targets::link!("faultrep.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"] fn ReportFault(pep : *const super::Diagnostics::Debug:: EXCEPTION_POINTERS, dwopt : u32) -> EFaultRepRetVal);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wer.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerAddExcludedApplication(pwzexename : ::windows_sys::core::PCWSTR, ballusers : super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerFreeString(pwszstr : ::windows_sys::core::PCWSTR) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerGetFlags(hprocess : super::super::Foundation:: HANDLE, pdwflags : *mut WER_FAULT_REPORTING) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerRegisterAdditionalProcess(processid : u32, captureextrainfoforthreadid : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerRegisterAppLocalDump(localappdatarelativepath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerRegisterCustomMetadata(key : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerRegisterExcludedMemoryBlock(address : *const ::core::ffi::c_void, size : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerRegisterFile(pwzfile : ::windows_sys::core::PCWSTR, regfiletype : WER_REGISTER_FILE_TYPE, dwflags : WER_FILE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerRegisterMemoryBlock(pvaddress : *const ::core::ffi::c_void, dwsize : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerRegisterRuntimeExceptionModule(pwszoutofprocesscallbackdll : ::windows_sys::core::PCWSTR, pcontext : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wer.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerRemoveExcludedApplication(pwzexename : ::windows_sys::core::PCWSTR, ballusers : super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
::windows_targets::link!("wer.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"] fn WerReportAddDump(hreporthandle : HREPORT, hprocess : super::super::Foundation:: HANDLE, hthread : super::super::Foundation:: HANDLE, dumptype : WER_DUMP_TYPE, pexceptionparam : *const WER_EXCEPTION_INFORMATION, pdumpcustomoptions : *const WER_DUMP_CUSTOM_OPTIONS, dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerReportAddFile(hreporthandle : HREPORT, pwzpath : ::windows_sys::core::PCWSTR, repfiletype : WER_FILE_TYPE, dwfileflags : WER_FILE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerReportCloseHandle(hreporthandle : HREPORT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wer.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerReportCreate(pwzeventtype : ::windows_sys::core::PCWSTR, reptype : WER_REPORT_TYPE, preportinformation : *const WER_REPORT_INFORMATION, phreporthandle : *mut HREPORT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("faultrep.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerReportHang(hwndhungapp : super::super::Foundation:: HWND, pwzhungapplicationname : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerReportSetParameter(hreporthandle : HREPORT, dwparamid : u32, pwzname : ::windows_sys::core::PCWSTR, pwzvalue : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerReportSetUIOption(hreporthandle : HREPORT, repuitypeid : WER_REPORT_UI, pwzvalue : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerReportSubmit(hreporthandle : HREPORT, consent : WER_CONSENT, dwflags : WER_SUBMIT_FLAGS, psubmitresult : *mut WER_SUBMIT_RESULT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerSetFlags(dwflags : WER_FAULT_REPORTING) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerStoreClose(hreportstore : HREPORTSTORE) -> ());
::windows_targets::link!("wer.dll" "system" fn WerStoreGetFirstReportKey(hreportstore : HREPORTSTORE, ppszreportkey : *mut ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerStoreGetNextReportKey(hreportstore : HREPORTSTORE, ppszreportkey : *mut ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerStoreGetReportCount(hreportstore : HREPORTSTORE, pdwreportcount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerStoreGetSizeOnDisk(hreportstore : HREPORTSTORE, pqwsizeinbytes : *mut u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerStoreOpen(repstoretype : REPORT_STORE_TYPES, phreportstore : *mut HREPORTSTORE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerStorePurge() -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wer.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerStoreQueryReportMetadataV1(hreportstore : HREPORTSTORE, pszreportkey : ::windows_sys::core::PCWSTR, preportmetadata : *mut WER_REPORT_METADATA_V1) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wer.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerStoreQueryReportMetadataV2(hreportstore : HREPORTSTORE, pszreportkey : ::windows_sys::core::PCWSTR, preportmetadata : *mut WER_REPORT_METADATA_V2) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wer.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WerStoreQueryReportMetadataV3(hreportstore : HREPORTSTORE, pszreportkey : ::windows_sys::core::PCWSTR, preportmetadata : *mut WER_REPORT_METADATA_V3) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wer.dll" "system" fn WerStoreUploadReport(hreportstore : HREPORTSTORE, pszreportkey : ::windows_sys::core::PCWSTR, dwflags : u32, psubmitresult : *mut WER_SUBMIT_RESULT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerUnregisterAdditionalProcess(processid : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerUnregisterAppLocalDump() -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerUnregisterCustomMetadata(key : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerUnregisterExcludedMemoryBlock(address : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerUnregisterFile(pwzfilepath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerUnregisterMemoryBlock(pvaddress : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn WerUnregisterRuntimeExceptionModule(pwszoutofprocesscallbackdll : ::windows_sys::core::PCWSTR, pcontext : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
pub const APPCRASH_EVENT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("APPCRASH");
pub const E_STORE_INVALID: REPORT_STORE_TYPES = 4i32;
pub const E_STORE_MACHINE_ARCHIVE: REPORT_STORE_TYPES = 2i32;
pub const E_STORE_MACHINE_QUEUE: REPORT_STORE_TYPES = 3i32;
pub const E_STORE_USER_ARCHIVE: REPORT_STORE_TYPES = 0i32;
pub const E_STORE_USER_QUEUE: REPORT_STORE_TYPES = 1i32;
pub const PACKAGED_APPCRASH_EVENT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MoAppCrash");
pub const WER_DUMP_AUXILIARY: u32 = 2u32;
pub const WER_DUMP_MASK_START: u32 = 1u32;
pub const WER_DUMP_NOHEAP_ONQUEUE: u32 = 1u32;
pub const WER_FAULT_REPORTING_ALWAYS_SHOW_UI: WER_FAULT_REPORTING = 16u32;
pub const WER_FAULT_REPORTING_CRITICAL: u32 = 512u32;
pub const WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH: u32 = 128u32;
pub const WER_FAULT_REPORTING_DISABLE_SNAPSHOT_HANG: u32 = 256u32;
pub const WER_FAULT_REPORTING_DURABLE: u32 = 1024u32;
pub const WER_FAULT_REPORTING_FLAG_DISABLE_THREAD_SUSPENSION: WER_FAULT_REPORTING = 4u32;
pub const WER_FAULT_REPORTING_FLAG_NOHEAP: WER_FAULT_REPORTING = 1u32;
pub const WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE: u32 = 64u32;
pub const WER_FAULT_REPORTING_FLAG_QUEUE: WER_FAULT_REPORTING = 2u32;
pub const WER_FAULT_REPORTING_FLAG_QUEUE_UPLOAD: WER_FAULT_REPORTING = 8u32;
pub const WER_FAULT_REPORTING_NO_UI: u32 = 32u32;
pub const WER_FILE_ANONYMOUS_DATA: WER_FILE = 2u32;
pub const WER_FILE_COMPRESSED: u32 = 4u32;
pub const WER_FILE_DELETE_WHEN_DONE: WER_FILE = 1u32;
pub const WER_MAX_APPLICATION_NAME_LENGTH: u32 = 128u32;
pub const WER_MAX_BUCKET_ID_STRING_LENGTH: u32 = 260u32;
pub const WER_MAX_DESCRIPTION_LENGTH: u32 = 512u32;
pub const WER_MAX_EVENT_NAME_LENGTH: u32 = 64u32;
pub const WER_MAX_FRIENDLY_EVENT_NAME_LENGTH: u32 = 128u32;
pub const WER_MAX_LOCAL_DUMP_SUBPATH_LENGTH: u32 = 64u32;
pub const WER_MAX_PARAM_COUNT: u32 = 10u32;
pub const WER_MAX_PARAM_LENGTH: u32 = 260u32;
pub const WER_MAX_PREFERRED_MODULES: u32 = 128u32;
pub const WER_MAX_PREFERRED_MODULES_BUFFER: u32 = 256u32;
pub const WER_MAX_REGISTERED_DUMPCOLLECTION: u32 = 4u32;
pub const WER_MAX_REGISTERED_ENTRIES: u32 = 512u32;
pub const WER_MAX_REGISTERED_METADATA: u32 = 8u32;
pub const WER_MAX_REGISTERED_RUNTIME_EXCEPTION_MODULES: u32 = 16u32;
pub const WER_MAX_SIGNATURE_NAME_LENGTH: u32 = 128u32;
pub const WER_MAX_TOTAL_PARAM_LENGTH: u32 = 1720u32;
pub const WER_METADATA_KEY_MAX_LENGTH: u32 = 64u32;
pub const WER_METADATA_VALUE_MAX_LENGTH: u32 = 128u32;
pub const WER_P0: u32 = 0u32;
pub const WER_P1: u32 = 1u32;
pub const WER_P2: u32 = 2u32;
pub const WER_P3: u32 = 3u32;
pub const WER_P4: u32 = 4u32;
pub const WER_P5: u32 = 5u32;
pub const WER_P6: u32 = 6u32;
pub const WER_P7: u32 = 7u32;
pub const WER_P8: u32 = 8u32;
pub const WER_P9: u32 = 9u32;
pub const WER_RUNTIME_EXCEPTION_DEBUGGER_LAUNCH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("OutOfProcessExceptionEventDebuggerLaunchCallback");
pub const WER_RUNTIME_EXCEPTION_EVENT_FUNCTION: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("OutOfProcessExceptionEventCallback");
pub const WER_RUNTIME_EXCEPTION_EVENT_SIGNATURE_FUNCTION: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("OutOfProcessExceptionEventSignatureCallback");
pub const WER_SUBMIT_ADD_REGISTERED_DATA: WER_SUBMIT_FLAGS = 16u32;
pub const WER_SUBMIT_ARCHIVE_PARAMETERS_ONLY: WER_SUBMIT_FLAGS = 4096u32;
pub const WER_SUBMIT_BYPASS_DATA_THROTTLING: WER_SUBMIT_FLAGS = 2048u32;
pub const WER_SUBMIT_BYPASS_NETWORK_COST_THROTTLING: u32 = 32768u32;
pub const WER_SUBMIT_BYPASS_POWER_THROTTLING: u32 = 16384u32;
pub const WER_SUBMIT_HONOR_RECOVERY: WER_SUBMIT_FLAGS = 1u32;
pub const WER_SUBMIT_HONOR_RESTART: WER_SUBMIT_FLAGS = 2u32;
pub const WER_SUBMIT_NO_ARCHIVE: WER_SUBMIT_FLAGS = 256u32;
pub const WER_SUBMIT_NO_CLOSE_UI: WER_SUBMIT_FLAGS = 64u32;
pub const WER_SUBMIT_NO_QUEUE: WER_SUBMIT_FLAGS = 128u32;
pub const WER_SUBMIT_OUTOFPROCESS: WER_SUBMIT_FLAGS = 32u32;
pub const WER_SUBMIT_OUTOFPROCESS_ASYNC: WER_SUBMIT_FLAGS = 1024u32;
pub const WER_SUBMIT_QUEUE: WER_SUBMIT_FLAGS = 4u32;
pub const WER_SUBMIT_REPORT_MACHINE_ID: WER_SUBMIT_FLAGS = 8192u32;
pub const WER_SUBMIT_SHOW_DEBUG: WER_SUBMIT_FLAGS = 8u32;
pub const WER_SUBMIT_START_MINIMIZED: WER_SUBMIT_FLAGS = 512u32;
pub const WerConsentAlwaysPrompt: WER_CONSENT = 4i32;
pub const WerConsentApproved: WER_CONSENT = 2i32;
pub const WerConsentDenied: WER_CONSENT = 3i32;
pub const WerConsentMax: WER_CONSENT = 5i32;
pub const WerConsentNotAsked: WER_CONSENT = 1i32;
pub const WerCustomAction: WER_SUBMIT_RESULT = 9i32;
pub const WerDisabled: WER_SUBMIT_RESULT = 5i32;
pub const WerDisabledQueue: WER_SUBMIT_RESULT = 7i32;
pub const WerDumpTypeHeapDump: WER_DUMP_TYPE = 3i32;
pub const WerDumpTypeMax: WER_DUMP_TYPE = 5i32;
pub const WerDumpTypeMicroDump: WER_DUMP_TYPE = 1i32;
pub const WerDumpTypeMiniDump: WER_DUMP_TYPE = 2i32;
pub const WerDumpTypeNone: WER_DUMP_TYPE = 0i32;
pub const WerDumpTypeTriageDump: WER_DUMP_TYPE = 4i32;
pub const WerFileTypeAuxiliaryDump: WER_FILE_TYPE = 8i32;
pub const WerFileTypeCustomDump: WER_FILE_TYPE = 7i32;
pub const WerFileTypeEtlTrace: WER_FILE_TYPE = 9i32;
pub const WerFileTypeHeapdump: WER_FILE_TYPE = 3i32;
pub const WerFileTypeMax: WER_FILE_TYPE = 10i32;
pub const WerFileTypeMicrodump: WER_FILE_TYPE = 1i32;
pub const WerFileTypeMinidump: WER_FILE_TYPE = 2i32;
pub const WerFileTypeOther: WER_FILE_TYPE = 5i32;
pub const WerFileTypeTriagedump: WER_FILE_TYPE = 6i32;
pub const WerFileTypeUserDocument: WER_FILE_TYPE = 4i32;
pub const WerRegFileTypeMax: WER_REGISTER_FILE_TYPE = 3i32;
pub const WerRegFileTypeOther: WER_REGISTER_FILE_TYPE = 2i32;
pub const WerRegFileTypeUserDocument: WER_REGISTER_FILE_TYPE = 1i32;
pub const WerReportApplicationCrash: WER_REPORT_TYPE = 2i32;
pub const WerReportApplicationHang: WER_REPORT_TYPE = 3i32;
pub const WerReportAsync: WER_SUBMIT_RESULT = 8i32;
pub const WerReportCancelled: WER_SUBMIT_RESULT = 6i32;
pub const WerReportCritical: WER_REPORT_TYPE = 1i32;
pub const WerReportDebug: WER_SUBMIT_RESULT = 3i32;
pub const WerReportFailed: WER_SUBMIT_RESULT = 4i32;
pub const WerReportInvalid: WER_REPORT_TYPE = 5i32;
pub const WerReportKernel: WER_REPORT_TYPE = 4i32;
pub const WerReportNonCritical: WER_REPORT_TYPE = 0i32;
pub const WerReportQueued: WER_SUBMIT_RESULT = 1i32;
pub const WerReportUploaded: WER_SUBMIT_RESULT = 2i32;
pub const WerReportUploadedCab: WER_SUBMIT_RESULT = 11i32;
pub const WerStorageLocationNotFound: WER_SUBMIT_RESULT = 12i32;
pub const WerSubmitResultMax: WER_SUBMIT_RESULT = 13i32;
pub const WerThrottled: WER_SUBMIT_RESULT = 10i32;
pub const WerUIAdditionalDataDlgHeader: WER_REPORT_UI = 1i32;
pub const WerUICloseDlgBody: WER_REPORT_UI = 9i32;
pub const WerUICloseDlgButtonText: WER_REPORT_UI = 10i32;
pub const WerUICloseDlgHeader: WER_REPORT_UI = 8i32;
pub const WerUICloseText: WER_REPORT_UI = 7i32;
pub const WerUIConsentDlgBody: WER_REPORT_UI = 4i32;
pub const WerUIConsentDlgHeader: WER_REPORT_UI = 3i32;
pub const WerUIIconFilePath: WER_REPORT_UI = 2i32;
pub const WerUIMax: WER_REPORT_UI = 11i32;
pub const WerUIOfflineSolutionCheckText: WER_REPORT_UI = 6i32;
pub const WerUIOnlineSolutionCheckText: WER_REPORT_UI = 5i32;
pub const frrvErr: EFaultRepRetVal = 3i32;
pub const frrvErrAnotherInstance: EFaultRepRetVal = 8i32;
pub const frrvErrDoubleFault: EFaultRepRetVal = 10i32;
pub const frrvErrNoDW: EFaultRepRetVal = 4i32;
pub const frrvErrNoMemory: EFaultRepRetVal = 9i32;
pub const frrvErrTimeout: EFaultRepRetVal = 5i32;
pub const frrvLaunchDebugger: EFaultRepRetVal = 6i32;
pub const frrvOk: EFaultRepRetVal = 0i32;
pub const frrvOkHeadless: EFaultRepRetVal = 7i32;
pub const frrvOkManifest: EFaultRepRetVal = 1i32;
pub const frrvOkQueued: EFaultRepRetVal = 2i32;
pub type EFaultRepRetVal = i32;
pub type REPORT_STORE_TYPES = i32;
pub type WER_CONSENT = i32;
pub type WER_DUMP_TYPE = i32;
pub type WER_FAULT_REPORTING = u32;
pub type WER_FILE = u32;
pub type WER_FILE_TYPE = i32;
pub type WER_REGISTER_FILE_TYPE = i32;
pub type WER_REPORT_TYPE = i32;
pub type WER_REPORT_UI = i32;
pub type WER_SUBMIT_FLAGS = u32;
pub type WER_SUBMIT_RESULT = i32;
pub type HREPORT = isize;
pub type HREPORTSTORE = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_DUMP_CUSTOM_OPTIONS {
    pub dwSize: u32,
    pub dwMask: u32,
    pub dwDumpFlags: u32,
    pub bOnlyThisThread: super::super::Foundation::BOOL,
    pub dwExceptionThreadFlags: u32,
    pub dwOtherThreadFlags: u32,
    pub dwExceptionThreadExFlags: u32,
    pub dwOtherThreadExFlags: u32,
    pub dwPreferredModuleFlags: u32,
    pub dwOtherModuleFlags: u32,
    pub wzPreferredModuleList: [u16; 256],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_DUMP_CUSTOM_OPTIONS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_DUMP_CUSTOM_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_DUMP_CUSTOM_OPTIONS_V2 {
    pub dwSize: u32,
    pub dwMask: u32,
    pub dwDumpFlags: u32,
    pub bOnlyThisThread: super::super::Foundation::BOOL,
    pub dwExceptionThreadFlags: u32,
    pub dwOtherThreadFlags: u32,
    pub dwExceptionThreadExFlags: u32,
    pub dwOtherThreadExFlags: u32,
    pub dwPreferredModuleFlags: u32,
    pub dwOtherModuleFlags: u32,
    pub wzPreferredModuleList: [u16; 256],
    pub dwPreferredModuleResetFlags: u32,
    pub dwOtherModuleResetFlags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_DUMP_CUSTOM_OPTIONS_V2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_DUMP_CUSTOM_OPTIONS_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_DUMP_CUSTOM_OPTIONS_V3 {
    pub dwSize: u32,
    pub dwMask: u32,
    pub dwDumpFlags: u32,
    pub bOnlyThisThread: super::super::Foundation::BOOL,
    pub dwExceptionThreadFlags: u32,
    pub dwOtherThreadFlags: u32,
    pub dwExceptionThreadExFlags: u32,
    pub dwOtherThreadExFlags: u32,
    pub dwPreferredModuleFlags: u32,
    pub dwOtherModuleFlags: u32,
    pub wzPreferredModuleList: [u16; 256],
    pub dwPreferredModuleResetFlags: u32,
    pub dwOtherModuleResetFlags: u32,
    pub pvDumpKey: *mut ::core::ffi::c_void,
    pub hSnapshot: super::super::Foundation::HANDLE,
    pub dwThreadID: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_DUMP_CUSTOM_OPTIONS_V3 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_DUMP_CUSTOM_OPTIONS_V3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
pub struct WER_EXCEPTION_INFORMATION {
    pub pExceptionPointers: *mut super::Diagnostics::Debug::EXCEPTION_POINTERS,
    pub bClientPointers: super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for WER_EXCEPTION_INFORMATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for WER_EXCEPTION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_REPORT_INFORMATION {
    pub dwSize: u32,
    pub hProcess: super::super::Foundation::HANDLE,
    pub wzConsentKey: [u16; 64],
    pub wzFriendlyEventName: [u16; 128],
    pub wzApplicationName: [u16; 128],
    pub wzApplicationPath: [u16; 260],
    pub wzDescription: [u16; 512],
    pub hwndParent: super::super::Foundation::HWND,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_REPORT_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_REPORT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_REPORT_INFORMATION_V3 {
    pub dwSize: u32,
    pub hProcess: super::super::Foundation::HANDLE,
    pub wzConsentKey: [u16; 64],
    pub wzFriendlyEventName: [u16; 128],
    pub wzApplicationName: [u16; 128],
    pub wzApplicationPath: [u16; 260],
    pub wzDescription: [u16; 512],
    pub hwndParent: super::super::Foundation::HWND,
    pub wzNamespacePartner: [u16; 64],
    pub wzNamespaceGroup: [u16; 64],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_REPORT_INFORMATION_V3 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_REPORT_INFORMATION_V3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_REPORT_INFORMATION_V4 {
    pub dwSize: u32,
    pub hProcess: super::super::Foundation::HANDLE,
    pub wzConsentKey: [u16; 64],
    pub wzFriendlyEventName: [u16; 128],
    pub wzApplicationName: [u16; 128],
    pub wzApplicationPath: [u16; 260],
    pub wzDescription: [u16; 512],
    pub hwndParent: super::super::Foundation::HWND,
    pub wzNamespacePartner: [u16; 64],
    pub wzNamespaceGroup: [u16; 64],
    pub rgbApplicationIdentity: [u8; 16],
    pub hSnapshot: super::super::Foundation::HANDLE,
    pub hDeleteFilesImpersonationToken: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_REPORT_INFORMATION_V4 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_REPORT_INFORMATION_V4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_REPORT_INFORMATION_V5 {
    pub dwSize: u32,
    pub hProcess: super::super::Foundation::HANDLE,
    pub wzConsentKey: [u16; 64],
    pub wzFriendlyEventName: [u16; 128],
    pub wzApplicationName: [u16; 128],
    pub wzApplicationPath: [u16; 260],
    pub wzDescription: [u16; 512],
    pub hwndParent: super::super::Foundation::HWND,
    pub wzNamespacePartner: [u16; 64],
    pub wzNamespaceGroup: [u16; 64],
    pub rgbApplicationIdentity: [u8; 16],
    pub hSnapshot: super::super::Foundation::HANDLE,
    pub hDeleteFilesImpersonationToken: super::super::Foundation::HANDLE,
    pub submitResultMax: WER_SUBMIT_RESULT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_REPORT_INFORMATION_V5 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_REPORT_INFORMATION_V5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_REPORT_METADATA_V1 {
    pub Signature: WER_REPORT_SIGNATURE,
    pub BucketId: ::windows_sys::core::GUID,
    pub ReportId: ::windows_sys::core::GUID,
    pub CreationTime: super::super::Foundation::FILETIME,
    pub SizeInBytes: u64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_REPORT_METADATA_V1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_REPORT_METADATA_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_REPORT_METADATA_V2 {
    pub Signature: WER_REPORT_SIGNATURE,
    pub BucketId: ::windows_sys::core::GUID,
    pub ReportId: ::windows_sys::core::GUID,
    pub CreationTime: super::super::Foundation::FILETIME,
    pub SizeInBytes: u64,
    pub CabId: [u16; 260],
    pub ReportStatus: u32,
    pub ReportIntegratorId: ::windows_sys::core::GUID,
    pub NumberOfFiles: u32,
    pub SizeOfFileNames: u32,
    pub FileNames: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_REPORT_METADATA_V2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_REPORT_METADATA_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WER_REPORT_METADATA_V3 {
    pub Signature: WER_REPORT_SIGNATURE,
    pub BucketId: ::windows_sys::core::GUID,
    pub ReportId: ::windows_sys::core::GUID,
    pub CreationTime: super::super::Foundation::FILETIME,
    pub SizeInBytes: u64,
    pub CabId: [u16; 260],
    pub ReportStatus: u32,
    pub ReportIntegratorId: ::windows_sys::core::GUID,
    pub NumberOfFiles: u32,
    pub SizeOfFileNames: u32,
    pub FileNames: ::windows_sys::core::PWSTR,
    pub FriendlyEventName: [u16; 128],
    pub ApplicationName: [u16; 128],
    pub ApplicationPath: [u16; 260],
    pub Description: [u16; 512],
    pub BucketIdString: [u16; 260],
    pub LegacyBucketId: u64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WER_REPORT_METADATA_V3 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WER_REPORT_METADATA_V3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WER_REPORT_PARAMETER {
    pub Name: [u16; 129],
    pub Value: [u16; 260],
}
impl ::core::marker::Copy for WER_REPORT_PARAMETER {}
impl ::core::clone::Clone for WER_REPORT_PARAMETER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WER_REPORT_SIGNATURE {
    pub EventName: [u16; 65],
    pub Parameters: [WER_REPORT_PARAMETER; 10],
}
impl ::core::marker::Copy for WER_REPORT_SIGNATURE {}
impl ::core::clone::Clone for WER_REPORT_SIGNATURE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
pub struct WER_RUNTIME_EXCEPTION_INFORMATION {
    pub dwSize: u32,
    pub hProcess: super::super::Foundation::HANDLE,
    pub hThread: super::super::Foundation::HANDLE,
    pub exceptionRecord: super::Diagnostics::Debug::EXCEPTION_RECORD,
    pub context: super::Diagnostics::Debug::CONTEXT,
    pub pwszReportId: ::windows_sys::core::PCWSTR,
    pub bIsFatal: super::super::Foundation::BOOL,
    pub dwReserved: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for WER_RUNTIME_EXCEPTION_INFORMATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for WER_RUNTIME_EXCEPTION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
pub type PFN_WER_RUNTIME_EXCEPTION_DEBUGGER_LAUNCH = ::core::option::Option<unsafe extern "system" fn(pcontext: *const ::core::ffi::c_void, pexceptioninformation: *const WER_RUNTIME_EXCEPTION_INFORMATION, pbiscustomdebugger: *mut super::super::Foundation::BOOL, pwszdebuggerlaunch: ::windows_sys::core::PWSTR, pchdebuggerlaunch: *mut u32, pbisdebuggerautolaunch: *mut super::super::Foundation::BOOL) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
pub type PFN_WER_RUNTIME_EXCEPTION_EVENT = ::core::option::Option<unsafe extern "system" fn(pcontext: *const ::core::ffi::c_void, pexceptioninformation: *const WER_RUNTIME_EXCEPTION_INFORMATION, pbownershipclaimed: *mut super::super::Foundation::BOOL, pwszeventname: ::windows_sys::core::PWSTR, pchsize: *mut u32, pdwsignaturecount: *mut u32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
pub type PFN_WER_RUNTIME_EXCEPTION_EVENT_SIGNATURE = ::core::option::Option<unsafe extern "system" fn(pcontext: *const ::core::ffi::c_void, pexceptioninformation: *const WER_RUNTIME_EXCEPTION_INFORMATION, dwindex: u32, pwszname: ::windows_sys::core::PWSTR, pchname: *mut u32, pwszvalue: ::windows_sys::core::PWSTR, pchvalue: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type pfn_ADDEREXCLUDEDAPPLICATIONA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> EFaultRepRetVal>;
pub type pfn_ADDEREXCLUDEDAPPLICATIONW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> EFaultRepRetVal>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
pub type pfn_REPORTFAULT = ::core::option::Option<unsafe extern "system" fn(param0: *const super::Diagnostics::Debug::EXCEPTION_POINTERS, param1: u32) -> EFaultRepRetVal>;
