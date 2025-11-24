::windows_targets::link!("mscoree.dll" "system" fn CLRCreateInstance(clsid : *const ::windows_sys::core::GUID, riid : *const ::windows_sys::core::GUID, ppinterface : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn CallFunctionShim(szdllname : ::windows_sys::core::PCWSTR, szfunctionname : ::windows_sys::core::PCSTR, lpvargument1 : *mut ::core::ffi::c_void, lpvargument2 : *mut ::core::ffi::c_void, szversion : ::windows_sys::core::PCWSTR, pvreserved : *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn ClrCreateManagedInstance(ptypename : ::windows_sys::core::PCWSTR, riid : *const ::windows_sys::core::GUID, ppobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn CorBindToCurrentRuntime(pwszfilename : ::windows_sys::core::PCWSTR, rclsid : *const ::windows_sys::core::GUID, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn CorBindToRuntime(pwszversion : ::windows_sys::core::PCWSTR, pwszbuildflavor : ::windows_sys::core::PCWSTR, rclsid : *const ::windows_sys::core::GUID, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("mscoree.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn CorBindToRuntimeByCfg(pcfgstream : super::Com:: IStream, reserved : u32, startupflags : u32, rclsid : *const ::windows_sys::core::GUID, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn CorBindToRuntimeEx(pwszversion : ::windows_sys::core::PCWSTR, pwszbuildflavor : ::windows_sys::core::PCWSTR, startupflags : u32, rclsid : *const ::windows_sys::core::GUID, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn CorBindToRuntimeHost(pwszversion : ::windows_sys::core::PCWSTR, pwszbuildflavor : ::windows_sys::core::PCWSTR, pwszhostconfigfile : ::windows_sys::core::PCWSTR, preserved : *mut ::core::ffi::c_void, startupflags : u32, rclsid : *const ::windows_sys::core::GUID, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn CorExitProcess(exitcode : i32) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
::windows_targets::link!("mscoree.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"] fn CorLaunchApplication(dwclickoncehost : HOST_TYPE, pwzappfullname : ::windows_sys::core::PCWSTR, dwmanifestpaths : u32, ppwzmanifestpaths : *const ::windows_sys::core::PCWSTR, dwactivationdata : u32, ppwzactivationdata : *const ::windows_sys::core::PCWSTR, lpprocessinformation : *mut super::Threading:: PROCESS_INFORMATION) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn CorMarkThreadInThreadPool() -> ());
::windows_targets::link!("mscoree.dll" "system" fn CreateDebuggingInterfaceFromVersion(idebuggerversion : i32, szdebuggeeversion : ::windows_sys::core::PCWSTR, ppcordb : *mut ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetCLRIdentityManager(riid : *const ::windows_sys::core::GUID, ppmanager : *mut ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetCORRequiredVersion(pbuffer : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetCORSystemDirectory(pbuffer : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetCORVersion(pbbuffer : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetFileVersion(szfilename : ::windows_sys::core::PCWSTR, szbuffer : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetRealProcAddress(pwszprocname : ::windows_sys::core::PCSTR, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetRequestedRuntimeInfo(pexe : ::windows_sys::core::PCWSTR, pwszversion : ::windows_sys::core::PCWSTR, pconfigurationfile : ::windows_sys::core::PCWSTR, startupflags : u32, runtimeinfoflags : u32, pdirectory : ::windows_sys::core::PWSTR, dwdirectory : u32, dwdirectorylength : *mut u32, pversion : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetRequestedRuntimeVersion(pexe : ::windows_sys::core::PCWSTR, pversion : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn GetRequestedRuntimeVersionForCLSID(rclsid : *const ::windows_sys::core::GUID, pversion : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32, dwresolutionflags : CLSID_RESOLUTION_FLAGS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mscoree.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetVersionFromProcess(hprocess : super::super::Foundation:: HANDLE, pversion : ::windows_sys::core::PWSTR, cchbuffer : u32, dwlength : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mscoree.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LoadLibraryShim(szdllname : ::windows_sys::core::PCWSTR, szversion : ::windows_sys::core::PCWSTR, pvreserved : *mut ::core::ffi::c_void, phmoddll : *mut super::super::Foundation:: HMODULE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn LoadStringRC(iresouceid : u32, szbuffer : ::windows_sys::core::PWSTR, imax : i32, bquiet : i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn LoadStringRCEx(lcid : u32, iresouceid : u32, szbuffer : ::windows_sys::core::PWSTR, imax : i32, bquiet : i32, pcwchused : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mscoree.dll" "system" fn LockClrVersion(hostcallback : FLockClrVersionCallback, pbeginhostsetup : *mut FLockClrVersionCallback, pendhostsetup : *mut FLockClrVersionCallback) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mscoree.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RunDll32ShimW(hwnd : super::super::Foundation:: HWND, hinst : super::super::Foundation:: HINSTANCE, lpszcmdline : ::windows_sys::core::PCWSTR, ncmdshow : i32) -> ::windows_sys::core::HRESULT);
pub type IActionOnCLREvent = *mut ::core::ffi::c_void;
pub type IApartmentCallback = *mut ::core::ffi::c_void;
pub type IAppDomainBinding = *mut ::core::ffi::c_void;
pub type ICLRAppDomainResourceMonitor = *mut ::core::ffi::c_void;
pub type ICLRAssemblyIdentityManager = *mut ::core::ffi::c_void;
pub type ICLRAssemblyReferenceList = *mut ::core::ffi::c_void;
pub type ICLRControl = *mut ::core::ffi::c_void;
pub type ICLRDebugManager = *mut ::core::ffi::c_void;
pub type ICLRDebugging = *mut ::core::ffi::c_void;
pub type ICLRDebuggingLibraryProvider = *mut ::core::ffi::c_void;
pub type ICLRDomainManager = *mut ::core::ffi::c_void;
pub type ICLRErrorReportingManager = *mut ::core::ffi::c_void;
pub type ICLRGCManager = *mut ::core::ffi::c_void;
pub type ICLRGCManager2 = *mut ::core::ffi::c_void;
pub type ICLRHostBindingPolicyManager = *mut ::core::ffi::c_void;
pub type ICLRHostProtectionManager = *mut ::core::ffi::c_void;
pub type ICLRIoCompletionManager = *mut ::core::ffi::c_void;
pub type ICLRMemoryNotificationCallback = *mut ::core::ffi::c_void;
pub type ICLRMetaHost = *mut ::core::ffi::c_void;
pub type ICLRMetaHostPolicy = *mut ::core::ffi::c_void;
pub type ICLROnEventManager = *mut ::core::ffi::c_void;
pub type ICLRPolicyManager = *mut ::core::ffi::c_void;
pub type ICLRProbingAssemblyEnum = *mut ::core::ffi::c_void;
pub type ICLRProfiling = *mut ::core::ffi::c_void;
pub type ICLRReferenceAssemblyEnum = *mut ::core::ffi::c_void;
pub type ICLRRuntimeHost = *mut ::core::ffi::c_void;
pub type ICLRRuntimeInfo = *mut ::core::ffi::c_void;
pub type ICLRStrongName = *mut ::core::ffi::c_void;
pub type ICLRStrongName2 = *mut ::core::ffi::c_void;
pub type ICLRStrongName3 = *mut ::core::ffi::c_void;
pub type ICLRSyncManager = *mut ::core::ffi::c_void;
pub type ICLRTask = *mut ::core::ffi::c_void;
pub type ICLRTask2 = *mut ::core::ffi::c_void;
pub type ICLRTaskManager = *mut ::core::ffi::c_void;
pub type ICatalogServices = *mut ::core::ffi::c_void;
pub type ICorConfiguration = *mut ::core::ffi::c_void;
pub type ICorRuntimeHost = *mut ::core::ffi::c_void;
pub type ICorThreadpool = *mut ::core::ffi::c_void;
pub type IDebuggerInfo = *mut ::core::ffi::c_void;
pub type IDebuggerThreadControl = *mut ::core::ffi::c_void;
pub type IGCHost = *mut ::core::ffi::c_void;
pub type IGCHost2 = *mut ::core::ffi::c_void;
pub type IGCHostControl = *mut ::core::ffi::c_void;
pub type IGCThreadControl = *mut ::core::ffi::c_void;
pub type IHostAssemblyManager = *mut ::core::ffi::c_void;
pub type IHostAssemblyStore = *mut ::core::ffi::c_void;
pub type IHostAutoEvent = *mut ::core::ffi::c_void;
pub type IHostControl = *mut ::core::ffi::c_void;
pub type IHostCrst = *mut ::core::ffi::c_void;
pub type IHostGCManager = *mut ::core::ffi::c_void;
pub type IHostIoCompletionManager = *mut ::core::ffi::c_void;
pub type IHostMalloc = *mut ::core::ffi::c_void;
pub type IHostManualEvent = *mut ::core::ffi::c_void;
pub type IHostMemoryManager = *mut ::core::ffi::c_void;
pub type IHostPolicyManager = *mut ::core::ffi::c_void;
pub type IHostSecurityContext = *mut ::core::ffi::c_void;
pub type IHostSecurityManager = *mut ::core::ffi::c_void;
pub type IHostSemaphore = *mut ::core::ffi::c_void;
pub type IHostSyncManager = *mut ::core::ffi::c_void;
pub type IHostTask = *mut ::core::ffi::c_void;
pub type IHostTaskManager = *mut ::core::ffi::c_void;
pub type IHostThreadpoolManager = *mut ::core::ffi::c_void;
pub type IManagedObject = *mut ::core::ffi::c_void;
pub type IObjectHandle = *mut ::core::ffi::c_void;
pub type ITypeName = *mut ::core::ffi::c_void;
pub type ITypeNameBuilder = *mut ::core::ffi::c_void;
pub type ITypeNameFactory = *mut ::core::ffi::c_void;
pub const APPDOMAIN_FORCE_TRIVIAL_WAIT_OPERATIONS: APPDOMAIN_SECURITY_FLAGS = 8i32;
pub const APPDOMAIN_SECURITY_DEFAULT: APPDOMAIN_SECURITY_FLAGS = 0i32;
pub const APPDOMAIN_SECURITY_FORBID_CROSSAD_REVERSE_PINVOKE: APPDOMAIN_SECURITY_FLAGS = 2i32;
pub const APPDOMAIN_SECURITY_SANDBOXED: APPDOMAIN_SECURITY_FLAGS = 1i32;
pub const BucketParamLength: u32 = 255u32;
pub const BucketParamsCount: u32 = 10u32;
pub const CLRRuntimeHost: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x90f1a06e_7712_4762_86b5_7a5eba6bdb02);
pub const CLR_ASSEMBLY_BUILD_VERSION: u32 = 0u32;
pub const CLR_ASSEMBLY_IDENTITY_FLAGS_DEFAULT: ECLRAssemblyIdentityFlags = 0i32;
pub const CLR_ASSEMBLY_MAJOR_VERSION: u32 = 4u32;
pub const CLR_ASSEMBLY_MINOR_VERSION: u32 = 0u32;
pub const CLR_BUILD_VERSION: u32 = 22220u32;
pub const CLR_DEBUGGING_MANAGED_EVENT_DEBUGGER_LAUNCH: CLR_DEBUGGING_PROCESS_FLAGS = 2i32;
pub const CLR_DEBUGGING_MANAGED_EVENT_PENDING: CLR_DEBUGGING_PROCESS_FLAGS = 1i32;
pub const CLR_MAJOR_VERSION: u32 = 4u32;
pub const CLR_MINOR_VERSION: u32 = 0u32;
pub const CLSID_CLRDebugging: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbacc578d_fbdd_48a4_969f_02d932b74634);
pub const CLSID_CLRDebuggingLegacy: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdf8395b5_a4ba_450b_a77c_a9a47762c520);
pub const CLSID_CLRMetaHost: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9280188d_0e8e_4867_b30c_7fa83884e8de);
pub const CLSID_CLRMetaHostPolicy: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2ebcd49a_1b47_4a61_b13a_4a03701e594b);
pub const CLSID_CLRProfiling: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbd097ed8_733e_43fe_8ed7_a95ff9a8448c);
pub const CLSID_CLRStrongName: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb79b0acd_f5cd_409b_b5a5_a16244610b92);
pub const CLSID_RESOLUTION_DEFAULT: CLSID_RESOLUTION_FLAGS = 0i32;
pub const CLSID_RESOLUTION_REGISTERED: CLSID_RESOLUTION_FLAGS = 1i32;
pub const COR_GC_COUNTS: COR_GC_STAT_TYPES = 1i32;
pub const COR_GC_MEMORYUSAGE: COR_GC_STAT_TYPES = 2i32;
pub const COR_GC_THREAD_HAS_PROMOTED_BYTES: COR_GC_THREAD_STATS_TYPES = 1i32;
pub const ComCallUnmarshal: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3f281000_e95a_11d2_886b_00c04f869f04);
pub const ComCallUnmarshalV4: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x45fb4600_e6e8_4928_b25e_50476ff79425);
pub const CorRuntimeHost: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb2f6723_ab3a_11d2_9c40_00c04fa30a3e);
pub const DEPRECATED_CLR_API_MESG: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("This API has been deprecated. Refer to https://go.microsoft.com/fwlink/?LinkId=143720 for more details.");
pub const DUMP_FLAVOR_CriticalCLRState: ECustomDumpFlavor = 1i32;
pub const DUMP_FLAVOR_Default: ECustomDumpFlavor = 0i32;
pub const DUMP_FLAVOR_Mini: ECustomDumpFlavor = 0i32;
pub const DUMP_FLAVOR_NonHeapCLRState: ECustomDumpFlavor = 2i32;
pub const DUMP_ITEM_None: ECustomDumpItemKind = 0i32;
pub const Event_ClrDisabled: EClrEvent = 1i32;
pub const Event_DomainUnload: EClrEvent = 0i32;
pub const Event_MDAFired: EClrEvent = 2i32;
pub const Event_StackOverflow: EClrEvent = 3i32;
pub const FAIL_AccessViolation: EClrFailure = 5i32;
pub const FAIL_CodeContract: EClrFailure = 6i32;
pub const FAIL_CriticalResource: EClrFailure = 1i32;
pub const FAIL_FatalRuntime: EClrFailure = 2i32;
pub const FAIL_NonCriticalResource: EClrFailure = 0i32;
pub const FAIL_OrphanedLock: EClrFailure = 3i32;
pub const FAIL_StackOverflow: EClrFailure = 4i32;
pub const HOST_APPLICATION_BINDING_POLICY: EHostApplicationPolicy = 1i32;
pub const HOST_BINDING_POLICY_MODIFY_CHAIN: EHostBindingPolicyModifyFlags = 1i32;
pub const HOST_BINDING_POLICY_MODIFY_DEFAULT: EHostBindingPolicyModifyFlags = 0i32;
pub const HOST_BINDING_POLICY_MODIFY_MAX: EHostBindingPolicyModifyFlags = 3i32;
pub const HOST_BINDING_POLICY_MODIFY_REMOVE: EHostBindingPolicyModifyFlags = 2i32;
pub const HOST_TYPE_APPLAUNCH: HOST_TYPE = 1i32;
pub const HOST_TYPE_CORFLAG: HOST_TYPE = 2i32;
pub const HOST_TYPE_DEFAULT: HOST_TYPE = 0i32;
pub const InvalidBucketParamIndex: BucketParameterIndex = 9i32;
pub const LIBID_mscoree: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5477469e_83b1_11d2_8b49_00a0c9b7c9c4);
pub const MALLOC_EXECUTABLE: MALLOC_TYPE = 2i32;
pub const MALLOC_THREADSAFE: MALLOC_TYPE = 1i32;
pub const METAHOST_CONFIG_FLAGS_LEGACY_V2_ACTIVATION_POLICY_FALSE: METAHOST_CONFIG_FLAGS = 2i32;
pub const METAHOST_CONFIG_FLAGS_LEGACY_V2_ACTIVATION_POLICY_MASK: METAHOST_CONFIG_FLAGS = 3i32;
pub const METAHOST_CONFIG_FLAGS_LEGACY_V2_ACTIVATION_POLICY_TRUE: METAHOST_CONFIG_FLAGS = 1i32;
pub const METAHOST_CONFIG_FLAGS_LEGACY_V2_ACTIVATION_POLICY_UNSET: METAHOST_CONFIG_FLAGS = 0i32;
pub const METAHOST_POLICY_APPLY_UPGRADE_POLICY: METAHOST_POLICY_FLAGS = 8i32;
pub const METAHOST_POLICY_EMULATE_EXE_LAUNCH: METAHOST_POLICY_FLAGS = 16i32;
pub const METAHOST_POLICY_ENSURE_SKU_SUPPORTED: METAHOST_POLICY_FLAGS = 128i32;
pub const METAHOST_POLICY_HIGHCOMPAT: METAHOST_POLICY_FLAGS = 0i32;
pub const METAHOST_POLICY_IGNORE_ERROR_MODE: METAHOST_POLICY_FLAGS = 4096i32;
pub const METAHOST_POLICY_SHOW_ERROR_DIALOG: METAHOST_POLICY_FLAGS = 32i32;
pub const METAHOST_POLICY_USE_PROCESS_IMAGE_PATH: METAHOST_POLICY_FLAGS = 64i32;
pub const MaxClrEvent: EClrEvent = 4i32;
pub const MaxClrFailure: EClrFailure = 7i32;
pub const MaxClrOperation: EClrOperation = 7i32;
pub const MaxPolicyAction: EPolicyAction = 10i32;
pub const OPR_AppDomainRudeUnload: EClrOperation = 4i32;
pub const OPR_AppDomainUnload: EClrOperation = 3i32;
pub const OPR_FinalizerRun: EClrOperation = 6i32;
pub const OPR_ProcessExit: EClrOperation = 5i32;
pub const OPR_ThreadAbort: EClrOperation = 0i32;
pub const OPR_ThreadRudeAbortInCriticalRegion: EClrOperation = 2i32;
pub const OPR_ThreadRudeAbortInNonCriticalRegion: EClrOperation = 1i32;
pub const Parameter1: BucketParameterIndex = 0i32;
pub const Parameter2: BucketParameterIndex = 1i32;
pub const Parameter3: BucketParameterIndex = 2i32;
pub const Parameter4: BucketParameterIndex = 3i32;
pub const Parameter5: BucketParameterIndex = 4i32;
pub const Parameter6: BucketParameterIndex = 5i32;
pub const Parameter7: BucketParameterIndex = 6i32;
pub const Parameter8: BucketParameterIndex = 7i32;
pub const Parameter9: BucketParameterIndex = 8i32;
pub const RUNTIME_INFO_DONT_RETURN_DIRECTORY: RUNTIME_INFO_FLAGS = 16i32;
pub const RUNTIME_INFO_DONT_RETURN_VERSION: RUNTIME_INFO_FLAGS = 32i32;
pub const RUNTIME_INFO_DONT_SHOW_ERROR_DIALOG: RUNTIME_INFO_FLAGS = 64i32;
pub const RUNTIME_INFO_IGNORE_ERROR_MODE: RUNTIME_INFO_FLAGS = 4096i32;
pub const RUNTIME_INFO_REQUEST_AMD64: RUNTIME_INFO_FLAGS = 4i32;
pub const RUNTIME_INFO_REQUEST_ARM64: RUNTIME_INFO_FLAGS = 8192i32;
pub const RUNTIME_INFO_REQUEST_IA64: RUNTIME_INFO_FLAGS = 2i32;
pub const RUNTIME_INFO_REQUEST_X86: RUNTIME_INFO_FLAGS = 8i32;
pub const RUNTIME_INFO_UPGRADE_VERSION: RUNTIME_INFO_FLAGS = 1i32;
pub const SO_ClrEngine: StackOverflowType = 1i32;
pub const SO_Managed: StackOverflowType = 0i32;
pub const SO_Other: StackOverflowType = 2i32;
pub const STARTUP_ALWAYSFLOW_IMPERSONATION: STARTUP_FLAGS = 262144i32;
pub const STARTUP_ARM: STARTUP_FLAGS = 4194304i32;
pub const STARTUP_CONCURRENT_GC: STARTUP_FLAGS = 1i32;
pub const STARTUP_DISABLE_COMMITTHREADSTACK: STARTUP_FLAGS = 131072i32;
pub const STARTUP_ETW: STARTUP_FLAGS = 1048576i32;
pub const STARTUP_HOARD_GC_VM: STARTUP_FLAGS = 8192i32;
pub const STARTUP_LEGACY_IMPERSONATION: STARTUP_FLAGS = 65536i32;
pub const STARTUP_LOADER_OPTIMIZATION_MASK: STARTUP_FLAGS = 6i32;
pub const STARTUP_LOADER_OPTIMIZATION_MULTI_DOMAIN: STARTUP_FLAGS = 4i32;
pub const STARTUP_LOADER_OPTIMIZATION_MULTI_DOMAIN_HOST: STARTUP_FLAGS = 6i32;
pub const STARTUP_LOADER_OPTIMIZATION_SINGLE_DOMAIN: STARTUP_FLAGS = 2i32;
pub const STARTUP_LOADER_SAFEMODE: STARTUP_FLAGS = 16i32;
pub const STARTUP_LOADER_SETPREFERENCE: STARTUP_FLAGS = 256i32;
pub const STARTUP_SERVER_GC: STARTUP_FLAGS = 4096i32;
pub const STARTUP_SINGLE_VERSION_HOSTING_INTERFACE: STARTUP_FLAGS = 16384i32;
pub const STARTUP_TRIM_GC_COMMIT: STARTUP_FLAGS = 524288i32;
pub const TT_ADUNLOAD: ETaskType = 128i32;
pub const TT_DEBUGGERHELPER: ETaskType = 1i32;
pub const TT_FINALIZER: ETaskType = 4i32;
pub const TT_GC: ETaskType = 2i32;
pub const TT_THREADPOOL_GATE: ETaskType = 16i32;
pub const TT_THREADPOOL_IOCOMPLETION: ETaskType = 64i32;
pub const TT_THREADPOOL_TIMER: ETaskType = 8i32;
pub const TT_THREADPOOL_WAIT: ETaskType = 512i32;
pub const TT_THREADPOOL_WORKER: ETaskType = 32i32;
pub const TT_UNKNOWN: ETaskType = -2147483648i32;
pub const TT_USER: ETaskType = 256i32;
pub const TypeNameFactory: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb81ff171_20f3_11d2_8dcc_00a0c9b00525);
pub const WAIT_ALERTABLE: WAIT_OPTION = 2i32;
pub const WAIT_MSGPUMP: WAIT_OPTION = 1i32;
pub const WAIT_NOTINDEADLOCK: WAIT_OPTION = 4i32;
pub const eAbortThread: EPolicyAction = 2i32;
pub const eAll: EApiCategories = 511i32;
pub const eAppDomainCritical: EMemoryCriticalLevel = 1i32;
pub const eCurrentContext: EContextType = 0i32;
pub const eDisableRuntime: EPolicyAction = 9i32;
pub const eExitProcess: EPolicyAction = 6i32;
pub const eExternalProcessMgmt: EApiCategories = 4i32;
pub const eExternalThreading: EApiCategories = 16i32;
pub const eFastExitProcess: EPolicyAction = 7i32;
pub const eHostDeterminedPolicy: EClrUnhandledException = 1i32;
pub const eInitializeNewDomainFlags_NoSecurityChanges: EInitializeNewDomainFlags = 2i32;
pub const eInitializeNewDomainFlags_None: EInitializeNewDomainFlags = 0i32;
pub const eMayLeakOnAbort: EApiCategories = 256i32;
pub const eMemoryAvailableHigh: EMemoryAvailable = 3i32;
pub const eMemoryAvailableLow: EMemoryAvailable = 1i32;
pub const eMemoryAvailableNeutral: EMemoryAvailable = 2i32;
pub const eNoAction: EPolicyAction = 0i32;
pub const eNoChecks: EApiCategories = 0i32;
pub const ePolicyLevelAdmin: EBindPolicyLevels = 32i32;
pub const ePolicyLevelApp: EBindPolicyLevels = 4i32;
pub const ePolicyLevelHost: EBindPolicyLevels = 16i32;
pub const ePolicyLevelNone: EBindPolicyLevels = 0i32;
pub const ePolicyLevelPublisher: EBindPolicyLevels = 8i32;
pub const ePolicyLevelRetargetable: EBindPolicyLevels = 1i32;
pub const ePolicyPortability: EBindPolicyLevels = 64i32;
pub const ePolicyUnifiedToCLR: EBindPolicyLevels = 2i32;
pub const eProcessCritical: EMemoryCriticalLevel = 2i32;
pub const eRestrictedContext: EContextType = 1i32;
pub const eRudeAbortThread: EPolicyAction = 3i32;
pub const eRudeExitProcess: EPolicyAction = 8i32;
pub const eRudeUnloadAppDomain: EPolicyAction = 5i32;
pub const eRuntimeDeterminedPolicy: EClrUnhandledException = 0i32;
pub const eSecurityInfrastructure: EApiCategories = 64i32;
pub const eSelfAffectingProcessMgmt: EApiCategories = 8i32;
pub const eSelfAffectingThreading: EApiCategories = 32i32;
pub const eSharedState: EApiCategories = 2i32;
pub const eSymbolReadingAlways: ESymbolReadingPolicy = 1i32;
pub const eSymbolReadingFullTrustOnly: ESymbolReadingPolicy = 2i32;
pub const eSymbolReadingNever: ESymbolReadingPolicy = 0i32;
pub const eSynchronization: EApiCategories = 1i32;
pub const eTaskCritical: EMemoryCriticalLevel = 0i32;
pub const eThrowException: EPolicyAction = 1i32;
pub const eUI: EApiCategories = 128i32;
pub const eUnloadAppDomain: EPolicyAction = 4i32;
pub type APPDOMAIN_SECURITY_FLAGS = i32;
pub type BucketParameterIndex = i32;
pub type CLR_DEBUGGING_PROCESS_FLAGS = i32;
pub type CLSID_RESOLUTION_FLAGS = i32;
pub type COR_GC_STAT_TYPES = i32;
pub type COR_GC_THREAD_STATS_TYPES = i32;
pub type EApiCategories = i32;
pub type EBindPolicyLevels = i32;
pub type ECLRAssemblyIdentityFlags = i32;
pub type EClrEvent = i32;
pub type EClrFailure = i32;
pub type EClrOperation = i32;
pub type EClrUnhandledException = i32;
pub type EContextType = i32;
pub type ECustomDumpFlavor = i32;
pub type ECustomDumpItemKind = i32;
pub type EHostApplicationPolicy = i32;
pub type EHostBindingPolicyModifyFlags = i32;
pub type EInitializeNewDomainFlags = i32;
pub type EMemoryAvailable = i32;
pub type EMemoryCriticalLevel = i32;
pub type EPolicyAction = i32;
pub type ESymbolReadingPolicy = i32;
pub type ETaskType = i32;
pub type HOST_TYPE = i32;
pub type MALLOC_TYPE = i32;
pub type METAHOST_CONFIG_FLAGS = i32;
pub type METAHOST_POLICY_FLAGS = i32;
pub type RUNTIME_INFO_FLAGS = i32;
pub type STARTUP_FLAGS = i32;
pub type StackOverflowType = i32;
pub type WAIT_OPTION = i32;
#[repr(C)]
pub struct AssemblyBindInfo {
    pub dwAppDomainId: u32,
    pub lpReferencedIdentity: ::windows_sys::core::PCWSTR,
    pub lpPostPolicyIdentity: ::windows_sys::core::PCWSTR,
    pub ePolicyLevel: u32,
}
impl ::core::marker::Copy for AssemblyBindInfo {}
impl ::core::clone::Clone for AssemblyBindInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct BucketParameters {
    pub fInited: super::super::Foundation::BOOL,
    pub pszEventTypeName: [u16; 255],
    pub pszParams: [u16; 2550],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for BucketParameters {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for BucketParameters {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLR_DEBUGGING_VERSION {
    pub wStructVersion: u16,
    pub wMajor: u16,
    pub wMinor: u16,
    pub wBuild: u16,
    pub wRevision: u16,
}
impl ::core::marker::Copy for CLR_DEBUGGING_VERSION {}
impl ::core::clone::Clone for CLR_DEBUGGING_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COR_GC_STATS {
    pub Flags: u32,
    pub ExplicitGCCount: usize,
    pub GenCollectionsTaken: [usize; 3],
    pub CommittedKBytes: usize,
    pub ReservedKBytes: usize,
    pub Gen0HeapSizeKBytes: usize,
    pub Gen1HeapSizeKBytes: usize,
    pub Gen2HeapSizeKBytes: usize,
    pub LargeObjectHeapSizeKBytes: usize,
    pub KBytesPromotedFromGen0: usize,
    pub KBytesPromotedFromGen1: usize,
}
impl ::core::marker::Copy for COR_GC_STATS {}
impl ::core::clone::Clone for COR_GC_STATS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COR_GC_THREAD_STATS {
    pub PerThreadAllocation: u64,
    pub Flags: u32,
}
impl ::core::marker::Copy for COR_GC_THREAD_STATS {}
impl ::core::clone::Clone for COR_GC_THREAD_STATS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CustomDumpItem {
    pub itemKind: ECustomDumpItemKind,
    pub Anonymous: CustomDumpItem_0,
}
impl ::core::marker::Copy for CustomDumpItem {}
impl ::core::clone::Clone for CustomDumpItem {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CustomDumpItem_0 {
    pub pReserved: usize,
}
impl ::core::marker::Copy for CustomDumpItem_0 {}
impl ::core::clone::Clone for CustomDumpItem_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MDAInfo {
    pub lpMDACaption: ::windows_sys::core::PCWSTR,
    pub lpMDAMessage: ::windows_sys::core::PCWSTR,
    pub lpStackTrace: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for MDAInfo {}
impl ::core::clone::Clone for MDAInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ModuleBindInfo {
    pub dwAppDomainId: u32,
    pub lpAssemblyIdentity: ::windows_sys::core::PCWSTR,
    pub lpModuleName: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for ModuleBindInfo {}
impl ::core::clone::Clone for ModuleBindInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
pub struct StackOverflowInfo {
    pub soType: StackOverflowType,
    pub pExceptionInfo: *mut super::Diagnostics::Debug::EXCEPTION_POINTERS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for StackOverflowInfo {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for StackOverflowInfo {
    fn clone(&self) -> Self {
        *self
    }
}
pub type CLRCreateInstanceFnPtr = ::core::option::Option<unsafe extern "system" fn(clsid: *const ::windows_sys::core::GUID, riid: *const ::windows_sys::core::GUID, ppinterface: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type CallbackThreadSetFnPtr = ::core::option::Option<unsafe extern "system" fn() -> ::windows_sys::core::HRESULT>;
pub type CallbackThreadUnsetFnPtr = ::core::option::Option<unsafe extern "system" fn() -> ::windows_sys::core::HRESULT>;
pub type CreateInterfaceFnPtr = ::core::option::Option<unsafe extern "system" fn(clsid: *const ::windows_sys::core::GUID, riid: *const ::windows_sys::core::GUID, ppinterface: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type FExecuteInAppDomainCallback = ::core::option::Option<unsafe extern "system" fn(cookie: *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type FLockClrVersionCallback = ::core::option::Option<unsafe extern "system" fn() -> ::windows_sys::core::HRESULT>;
pub type PTLS_CALLBACK_FUNCTION = ::core::option::Option<unsafe extern "system" fn(__midl____midl_itf_mscoree_0000_00040005: *mut ::core::ffi::c_void) -> ()>;
pub type RuntimeLoadedCallbackFnPtr = ::core::option::Option<unsafe extern "system" fn(pruntimeinfo: ICLRRuntimeInfo, pfncallbackthreadset: CallbackThreadSetFnPtr, pfncallbackthreadunset: CallbackThreadUnsetFnPtr) -> ()>;
