::windows_targets::link!("comsvcs.dll" "system" fn CoCreateActivity(piunknown : ::windows_sys::core::IUnknown, riid : *const ::windows_sys::core::GUID, ppobj : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("comsvcs.dll" "system" fn CoEnterServiceDomain(pconfigobject : ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("ole32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn CoGetDefaultContext(apttype : super::Com:: APTTYPE, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("comsvcs.dll" "system" fn CoLeaveServiceDomain(punkstatus : ::windows_sys::core::IUnknown) -> ());
::windows_targets::link!("mtxdm.dll" "cdecl" fn GetDispenserManager(param0 : *mut IDispenserManager) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("comsvcs.dll" "system" fn GetManagedExtensions(dwexts : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("comsvcs.dll" "system" fn MTSCreateActivity(riid : *const ::windows_sys::core::GUID, ppobj : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("comsvcs.dll" "cdecl" fn RecycleSurrogate(lreasoncode : i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("comsvcs.dll" "cdecl" fn SafeRef(rid : *const ::windows_sys::core::GUID, punk : ::windows_sys::core::IUnknown) -> *mut ::core::ffi::c_void);
pub type ContextInfo = *mut ::core::ffi::c_void;
pub type ContextInfo2 = *mut ::core::ffi::c_void;
pub type IAppDomainHelper = *mut ::core::ffi::c_void;
pub type IAssemblyLocator = *mut ::core::ffi::c_void;
pub type IAsyncErrorNotify = *mut ::core::ffi::c_void;
pub type ICOMAdminCatalog = *mut ::core::ffi::c_void;
pub type ICOMAdminCatalog2 = *mut ::core::ffi::c_void;
pub type ICOMLBArguments = *mut ::core::ffi::c_void;
pub type ICatalogCollection = *mut ::core::ffi::c_void;
pub type ICatalogObject = *mut ::core::ffi::c_void;
pub type ICheckSxsConfig = *mut ::core::ffi::c_void;
pub type IComActivityEvents = *mut ::core::ffi::c_void;
pub type IComApp2Events = *mut ::core::ffi::c_void;
pub type IComAppEvents = *mut ::core::ffi::c_void;
pub type IComCRMEvents = *mut ::core::ffi::c_void;
pub type IComExceptionEvents = *mut ::core::ffi::c_void;
pub type IComIdentityEvents = *mut ::core::ffi::c_void;
pub type IComInstance2Events = *mut ::core::ffi::c_void;
pub type IComInstanceEvents = *mut ::core::ffi::c_void;
pub type IComLTxEvents = *mut ::core::ffi::c_void;
pub type IComMethod2Events = *mut ::core::ffi::c_void;
pub type IComMethodEvents = *mut ::core::ffi::c_void;
pub type IComMtaThreadPoolKnobs = *mut ::core::ffi::c_void;
pub type IComObjectConstruction2Events = *mut ::core::ffi::c_void;
pub type IComObjectConstructionEvents = *mut ::core::ffi::c_void;
pub type IComObjectEvents = *mut ::core::ffi::c_void;
pub type IComObjectPool2Events = *mut ::core::ffi::c_void;
pub type IComObjectPoolEvents = *mut ::core::ffi::c_void;
pub type IComObjectPoolEvents2 = *mut ::core::ffi::c_void;
pub type IComQCEvents = *mut ::core::ffi::c_void;
pub type IComResourceEvents = *mut ::core::ffi::c_void;
pub type IComSecurityEvents = *mut ::core::ffi::c_void;
pub type IComStaThreadPoolKnobs = *mut ::core::ffi::c_void;
pub type IComStaThreadPoolKnobs2 = *mut ::core::ffi::c_void;
pub type IComThreadEvents = *mut ::core::ffi::c_void;
pub type IComTrackingInfoCollection = *mut ::core::ffi::c_void;
pub type IComTrackingInfoEvents = *mut ::core::ffi::c_void;
pub type IComTrackingInfoObject = *mut ::core::ffi::c_void;
pub type IComTrackingInfoProperties = *mut ::core::ffi::c_void;
pub type IComTransaction2Events = *mut ::core::ffi::c_void;
pub type IComTransactionEvents = *mut ::core::ffi::c_void;
pub type IComUserEvent = *mut ::core::ffi::c_void;
pub type IContextProperties = *mut ::core::ffi::c_void;
pub type IContextSecurityPerimeter = *mut ::core::ffi::c_void;
pub type IContextState = *mut ::core::ffi::c_void;
pub type ICreateWithLocalTransaction = *mut ::core::ffi::c_void;
pub type ICreateWithTipTransactionEx = *mut ::core::ffi::c_void;
pub type ICreateWithTransactionEx = *mut ::core::ffi::c_void;
pub type ICrmCompensator = *mut ::core::ffi::c_void;
pub type ICrmCompensatorVariants = *mut ::core::ffi::c_void;
pub type ICrmFormatLogRecords = *mut ::core::ffi::c_void;
pub type ICrmLogControl = *mut ::core::ffi::c_void;
pub type ICrmMonitor = *mut ::core::ffi::c_void;
pub type ICrmMonitorClerks = *mut ::core::ffi::c_void;
pub type ICrmMonitorLogRecords = *mut ::core::ffi::c_void;
pub type IDispenserDriver = *mut ::core::ffi::c_void;
pub type IDispenserManager = *mut ::core::ffi::c_void;
pub type IEnumNames = *mut ::core::ffi::c_void;
pub type IEventServerTrace = *mut ::core::ffi::c_void;
pub type IGetAppTrackerData = *mut ::core::ffi::c_void;
pub type IGetContextProperties = *mut ::core::ffi::c_void;
pub type IGetSecurityCallContext = *mut ::core::ffi::c_void;
pub type IHolder = *mut ::core::ffi::c_void;
pub type ILBEvents = *mut ::core::ffi::c_void;
pub type IMTSActivity = *mut ::core::ffi::c_void;
pub type IMTSCall = *mut ::core::ffi::c_void;
pub type IMTSLocator = *mut ::core::ffi::c_void;
pub type IManagedActivationEvents = *mut ::core::ffi::c_void;
pub type IManagedObjectInfo = *mut ::core::ffi::c_void;
pub type IManagedPoolAction = *mut ::core::ffi::c_void;
pub type IManagedPooledObj = *mut ::core::ffi::c_void;
pub type IMessageMover = *mut ::core::ffi::c_void;
pub type IMtsEventInfo = *mut ::core::ffi::c_void;
pub type IMtsEvents = *mut ::core::ffi::c_void;
pub type IMtsGrp = *mut ::core::ffi::c_void;
pub type IObjPool = *mut ::core::ffi::c_void;
pub type IObjectConstruct = *mut ::core::ffi::c_void;
pub type IObjectConstructString = *mut ::core::ffi::c_void;
pub type IObjectContext = *mut ::core::ffi::c_void;
pub type IObjectContextActivity = *mut ::core::ffi::c_void;
pub type IObjectContextInfo = *mut ::core::ffi::c_void;
pub type IObjectContextInfo2 = *mut ::core::ffi::c_void;
pub type IObjectContextTip = *mut ::core::ffi::c_void;
pub type IObjectControl = *mut ::core::ffi::c_void;
pub type IPlaybackControl = *mut ::core::ffi::c_void;
pub type IPoolManager = *mut ::core::ffi::c_void;
pub type IProcessInitializer = *mut ::core::ffi::c_void;
pub type ISecurityCallContext = *mut ::core::ffi::c_void;
pub type ISecurityCallersColl = *mut ::core::ffi::c_void;
pub type ISecurityIdentityColl = *mut ::core::ffi::c_void;
pub type ISecurityProperty = *mut ::core::ffi::c_void;
pub type ISelectCOMLBServer = *mut ::core::ffi::c_void;
pub type ISendMethodEvents = *mut ::core::ffi::c_void;
pub type IServiceActivity = *mut ::core::ffi::c_void;
pub type IServiceCall = *mut ::core::ffi::c_void;
pub type IServiceComTIIntrinsicsConfig = *mut ::core::ffi::c_void;
pub type IServiceIISIntrinsicsConfig = *mut ::core::ffi::c_void;
pub type IServiceInheritanceConfig = *mut ::core::ffi::c_void;
pub type IServicePartitionConfig = *mut ::core::ffi::c_void;
pub type IServicePool = *mut ::core::ffi::c_void;
pub type IServicePoolConfig = *mut ::core::ffi::c_void;
pub type IServiceSxsConfig = *mut ::core::ffi::c_void;
pub type IServiceSynchronizationConfig = *mut ::core::ffi::c_void;
pub type IServiceSysTxnConfig = *mut ::core::ffi::c_void;
pub type IServiceThreadPoolConfig = *mut ::core::ffi::c_void;
pub type IServiceTrackerConfig = *mut ::core::ffi::c_void;
pub type IServiceTransactionConfig = *mut ::core::ffi::c_void;
pub type IServiceTransactionConfigBase = *mut ::core::ffi::c_void;
pub type ISharedProperty = *mut ::core::ffi::c_void;
pub type ISharedPropertyGroup = *mut ::core::ffi::c_void;
pub type ISharedPropertyGroupManager = *mut ::core::ffi::c_void;
pub type ISystemAppEventData = *mut ::core::ffi::c_void;
pub type IThreadPoolKnobs = *mut ::core::ffi::c_void;
pub type ITransactionContext = *mut ::core::ffi::c_void;
pub type ITransactionContextEx = *mut ::core::ffi::c_void;
pub type ITransactionProperty = *mut ::core::ffi::c_void;
pub type ITransactionProxy = *mut ::core::ffi::c_void;
pub type ITransactionResourcePool = *mut ::core::ffi::c_void;
pub type ITransactionStatus = *mut ::core::ffi::c_void;
pub type ITxProxyHolder = *mut ::core::ffi::c_void;
pub type ObjectContext = *mut ::core::ffi::c_void;
pub type ObjectControl = *mut ::core::ffi::c_void;
pub type SecurityProperty = *mut ::core::ffi::c_void;
pub const APPTYPE_LIBRARY: COMPLUS_APPTYPE = 0i32;
pub const APPTYPE_SERVER: COMPLUS_APPTYPE = 1i32;
pub const APPTYPE_SWC: COMPLUS_APPTYPE = 2i32;
pub const APPTYPE_UNKNOWN: COMPLUS_APPTYPE = -1i32;
pub const AppDomainHelper: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xef24f689_14f8_4d92_b4af_d7b1f0e70fd4);
pub const ByotServerEx: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0aa_7f19_11d2_978e_0000f8757e2a);
pub const COMAdmin32BitComponent: COMAdminComponentType = 1i32;
pub const COMAdmin64BitComponent: COMAdminComponentType = 2i32;
pub const COMAdminAccessChecksApplicationComponentLevel: COMAdminAccessChecksLevelOptions = 1i32;
pub const COMAdminAccessChecksApplicationLevel: COMAdminAccessChecksLevelOptions = 0i32;
pub const COMAdminActivationInproc: COMAdminActivationOptions = 0i32;
pub const COMAdminActivationLocal: COMAdminActivationOptions = 1i32;
pub const COMAdminAuthenticationCall: COMAdminAuthenticationLevelOptions = 3i32;
pub const COMAdminAuthenticationCapabilitiesDynamicCloaking: COMAdminAuthenticationCapabilitiesOptions = 64i32;
pub const COMAdminAuthenticationCapabilitiesNone: COMAdminAuthenticationCapabilitiesOptions = 0i32;
pub const COMAdminAuthenticationCapabilitiesSecureReference: COMAdminAuthenticationCapabilitiesOptions = 2i32;
pub const COMAdminAuthenticationCapabilitiesStaticCloaking: COMAdminAuthenticationCapabilitiesOptions = 32i32;
pub const COMAdminAuthenticationConnect: COMAdminAuthenticationLevelOptions = 2i32;
pub const COMAdminAuthenticationDefault: COMAdminAuthenticationLevelOptions = 0i32;
pub const COMAdminAuthenticationIntegrity: COMAdminAuthenticationLevelOptions = 5i32;
pub const COMAdminAuthenticationNone: COMAdminAuthenticationLevelOptions = 1i32;
pub const COMAdminAuthenticationPacket: COMAdminAuthenticationLevelOptions = 4i32;
pub const COMAdminAuthenticationPrivacy: COMAdminAuthenticationLevelOptions = 6i32;
pub const COMAdminCatalog: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf618c514_dfb8_11d1_a2cf_00805fc79235);
pub const COMAdminCatalogCollection: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf618c516_dfb8_11d1_a2cf_00805fc79235);
pub const COMAdminCatalogObject: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf618c515_dfb8_11d1_a2cf_00805fc79235);
pub const COMAdminCompFlagAlreadyInstalled: COMAdminComponentFlags = 16i32;
pub const COMAdminCompFlagCOMPlusPropertiesFound: COMAdminComponentFlags = 2i32;
pub const COMAdminCompFlagInterfacesFound: COMAdminComponentFlags = 8i32;
pub const COMAdminCompFlagNotInApplication: COMAdminComponentFlags = 32i32;
pub const COMAdminCompFlagProxyFound: COMAdminComponentFlags = 4i32;
pub const COMAdminCompFlagTypeInfoFound: COMAdminComponentFlags = 1i32;
pub const COMAdminErrAlreadyInstalled: COMAdminErrorCodes = -2146368508i32;
pub const COMAdminErrAppDirNotFound: COMAdminErrorCodes = -2146368481i32;
pub const COMAdminErrAppFileReadFail: COMAdminErrorCodes = -2146368504i32;
pub const COMAdminErrAppFileVersion: COMAdminErrorCodes = -2146368503i32;
pub const COMAdminErrAppFileWriteFail: COMAdminErrorCodes = -2146368505i32;
pub const COMAdminErrAppNotRunning: COMAdminErrorCodes = -2146367478i32;
pub const COMAdminErrApplicationExists: COMAdminErrorCodes = -2146368501i32;
pub const COMAdminErrApplidMatchesClsid: COMAdminErrorCodes = -2146368442i32;
pub const COMAdminErrAuthenticationLevel: COMAdminErrorCodes = -2146368493i32;
pub const COMAdminErrBadPath: COMAdminErrorCodes = -2146368502i32;
pub const COMAdminErrBadRegistryLibID: COMAdminErrorCodes = -2146368482i32;
pub const COMAdminErrBadRegistryProgID: COMAdminErrorCodes = -2146368494i32;
pub const COMAdminErrBasePartitionOnly: COMAdminErrorCodes = -2146368432i32;
pub const COMAdminErrCLSIDOrIIDMismatch: COMAdminErrorCodes = -2146368488i32;
pub const COMAdminErrCanNotExportAppProxy: COMAdminErrorCodes = -2146368438i32;
pub const COMAdminErrCanNotExportSystemApp: COMAdminErrorCodes = -2146368436i32;
pub const COMAdminErrCanNotStartApp: COMAdminErrorCodes = -2146368437i32;
pub const COMAdminErrCanNotSubscribeToComponent: COMAdminErrorCodes = -2146368435i32;
pub const COMAdminErrCannotCopyEventClass: COMAdminErrorCodes = -2146367456i32;
pub const COMAdminErrCantCopyFile: COMAdminErrorCodes = -2146368499i32;
pub const COMAdminErrCantRecycleLibraryApps: COMAdminErrorCodes = -2146367473i32;
pub const COMAdminErrCantRecycleServiceApps: COMAdminErrorCodes = -2146367471i32;
pub const COMAdminErrCatBitnessMismatch: COMAdminErrorCodes = -2146368382i32;
pub const COMAdminErrCatPauseResumeNotSupported: COMAdminErrorCodes = -2146368379i32;
pub const COMAdminErrCatServerFault: COMAdminErrorCodes = -2146368378i32;
pub const COMAdminErrCatUnacceptableBitness: COMAdminErrorCodes = -2146368381i32;
pub const COMAdminErrCatWrongAppBitnessBitness: COMAdminErrorCodes = -2146368380i32;
pub const COMAdminErrCoReqCompInstalled: COMAdminErrorCodes = -2146368459i32;
pub const COMAdminErrCompFileBadTLB: COMAdminErrorCodes = -2146368472i32;
pub const COMAdminErrCompFileClassNotAvail: COMAdminErrorCodes = -2146368473i32;
pub const COMAdminErrCompFileDoesNotExist: COMAdminErrorCodes = -2146368476i32;
pub const COMAdminErrCompFileGetClassObj: COMAdminErrorCodes = -2146368474i32;
pub const COMAdminErrCompFileLoadDLLFail: COMAdminErrorCodes = -2146368475i32;
pub const COMAdminErrCompFileNoRegistrar: COMAdminErrorCodes = -2146368460i32;
pub const COMAdminErrCompFileNotInstallable: COMAdminErrorCodes = -2146368471i32;
pub const COMAdminErrCompMoveBadDest: COMAdminErrorCodes = -2146368466i32;
pub const COMAdminErrCompMoveDest: COMAdminErrorCodes = -2146367459i32;
pub const COMAdminErrCompMoveLocked: COMAdminErrorCodes = -2146368467i32;
pub const COMAdminErrCompMovePrivate: COMAdminErrorCodes = -2146367458i32;
pub const COMAdminErrCompMoveSource: COMAdminErrorCodes = -2146367460i32;
pub const COMAdminErrComponentExists: COMAdminErrorCodes = -2146368455i32;
pub const COMAdminErrDllLoadFailed: COMAdminErrorCodes = -2146368483i32;
pub const COMAdminErrDllRegisterServer: COMAdminErrorCodes = -2146368486i32;
pub const COMAdminErrDuplicatePartitionName: COMAdminErrorCodes = -2146368425i32;
pub const COMAdminErrEventClassCannotBeSubscriber: COMAdminErrorCodes = -2146368434i32;
pub const COMAdminErrImportedComponentsNotAllowed: COMAdminErrorCodes = -2146368421i32;
pub const COMAdminErrInvalidPartition: COMAdminErrorCodes = -2146367477i32;
pub const COMAdminErrInvalidUserids: COMAdminErrorCodes = -2146368496i32;
pub const COMAdminErrKeyMissing: COMAdminErrorCodes = -2146368509i32;
pub const COMAdminErrLibAppProxyIncompatible: COMAdminErrorCodes = -2146368433i32;
pub const COMAdminErrMigSchemaNotFound: COMAdminErrorCodes = -2146368383i32;
pub const COMAdminErrMigVersionNotSupported: COMAdminErrorCodes = -2146368384i32;
pub const COMAdminErrNoRegistryCLSID: COMAdminErrorCodes = -2146368495i32;
pub const COMAdminErrNoServerShare: COMAdminErrorCodes = -2146368485i32;
pub const COMAdminErrNoUser: COMAdminErrorCodes = -2146368497i32;
pub const COMAdminErrNotChangeable: COMAdminErrorCodes = -2146368470i32;
pub const COMAdminErrNotDeletable: COMAdminErrorCodes = -2146368469i32;
pub const COMAdminErrNotInRegistry: COMAdminErrorCodes = -2146368450i32;
pub const COMAdminErrObjectDoesNotExist: COMAdminErrorCodes = -2146367479i32;
pub const COMAdminErrObjectErrors: COMAdminErrorCodes = -2146368511i32;
pub const COMAdminErrObjectExists: COMAdminErrorCodes = -2146368456i32;
pub const COMAdminErrObjectInvalid: COMAdminErrorCodes = -2146368510i32;
pub const COMAdminErrObjectNotPoolable: COMAdminErrorCodes = -2146368449i32;
pub const COMAdminErrObjectParentMissing: COMAdminErrorCodes = -2146367480i32;
pub const COMAdminErrPartitionInUse: COMAdminErrorCodes = -2146368423i32;
pub const COMAdminErrPartitionMsiOnly: COMAdminErrorCodes = -2146367463i32;
pub const COMAdminErrPausedProcessMayNotBeRecycled: COMAdminErrorCodes = -2146367469i32;
pub const COMAdminErrProcessAlreadyRecycled: COMAdminErrorCodes = -2146367470i32;
pub const COMAdminErrPropertyOverflow: COMAdminErrorCodes = -2146368452i32;
pub const COMAdminErrPropertySaveFailed: COMAdminErrorCodes = -2146368457i32;
pub const COMAdminErrQueuingServiceNotAvailable: COMAdminErrorCodes = -2146367998i32;
pub const COMAdminErrRegFileCorrupt: COMAdminErrorCodes = -2146368453i32;
pub const COMAdminErrRegdbAlreadyRunning: COMAdminErrorCodes = -2146368395i32;
pub const COMAdminErrRegdbNotInitialized: COMAdminErrorCodes = -2146368398i32;
pub const COMAdminErrRegdbNotOpen: COMAdminErrorCodes = -2146368397i32;
pub const COMAdminErrRegdbSystemErr: COMAdminErrorCodes = -2146368396i32;
pub const COMAdminErrRegisterTLB: COMAdminErrorCodes = -2146368464i32;
pub const COMAdminErrRegistrarFailed: COMAdminErrorCodes = -2146368477i32;
pub const COMAdminErrRemoteInterface: COMAdminErrorCodes = -2146368487i32;
pub const COMAdminErrRequiresDifferentPlatform: COMAdminErrorCodes = -2146368439i32;
pub const COMAdminErrRoleDoesNotExist: COMAdminErrorCodes = -2146368441i32;
pub const COMAdminErrRoleExists: COMAdminErrorCodes = -2146368500i32;
pub const COMAdminErrServiceNotInstalled: COMAdminErrorCodes = -2146368458i32;
pub const COMAdminErrSession: COMAdminErrorCodes = -2146368468i32;
pub const COMAdminErrStartAppDisabled: COMAdminErrorCodes = -2146368431i32;
pub const COMAdminErrStartAppNeedsComponents: COMAdminErrorCodes = -2146368440i32;
pub const COMAdminErrSystemApp: COMAdminErrorCodes = -2146368461i32;
pub const COMAdminErrUserPasswdNotValid: COMAdminErrorCodes = -2146368492i32;
pub const COMAdminExportApplicationProxy: COMAdminApplicationExportOptions = 2i32;
pub const COMAdminExportForceOverwriteOfFiles: COMAdminApplicationExportOptions = 4i32;
pub const COMAdminExportIn10Format: COMAdminApplicationExportOptions = 16i32;
pub const COMAdminExportNoUsers: COMAdminApplicationExportOptions = 0i32;
pub const COMAdminExportUsers: COMAdminApplicationExportOptions = 1i32;
pub const COMAdminFileFlagAlreadyInstalled: COMAdminFileFlags = 512i32;
pub const COMAdminFileFlagBadTLB: COMAdminFileFlags = 1024i32;
pub const COMAdminFileFlagCOM: COMAdminFileFlags = 2i32;
pub const COMAdminFileFlagClassNotAvailable: COMAdminFileFlags = 4096i32;
pub const COMAdminFileFlagContainsComp: COMAdminFileFlags = 8i32;
pub const COMAdminFileFlagContainsPS: COMAdminFileFlags = 4i32;
pub const COMAdminFileFlagContainsTLB: COMAdminFileFlags = 16i32;
pub const COMAdminFileFlagDLLRegsvrFailed: COMAdminFileFlags = 32768i32;
pub const COMAdminFileFlagDoesNotExist: COMAdminFileFlags = 256i32;
pub const COMAdminFileFlagError: COMAdminFileFlags = 262144i32;
pub const COMAdminFileFlagGetClassObjFailed: COMAdminFileFlags = 2048i32;
pub const COMAdminFileFlagLoadable: COMAdminFileFlags = 1i32;
pub const COMAdminFileFlagNoRegistrar: COMAdminFileFlags = 16384i32;
pub const COMAdminFileFlagRegTLBFailed: COMAdminFileFlags = 65536i32;
pub const COMAdminFileFlagRegistrar: COMAdminFileFlags = 8192i32;
pub const COMAdminFileFlagRegistrarFailed: COMAdminFileFlags = 131072i32;
pub const COMAdminFileFlagSelfReg: COMAdminFileFlags = 32i32;
pub const COMAdminFileFlagSelfUnReg: COMAdminFileFlags = 64i32;
pub const COMAdminFileFlagUnloadableDLL: COMAdminFileFlags = 128i32;
pub const COMAdminImpersonationAnonymous: COMAdminImpersonationLevelOptions = 1i32;
pub const COMAdminImpersonationDelegate: COMAdminImpersonationLevelOptions = 4i32;
pub const COMAdminImpersonationIdentify: COMAdminImpersonationLevelOptions = 2i32;
pub const COMAdminImpersonationImpersonate: COMAdminImpersonationLevelOptions = 3i32;
pub const COMAdminInUseByCatalog: COMAdminInUse = 1i32;
pub const COMAdminInUseByRegistryClsid: COMAdminInUse = 5i32;
pub const COMAdminInUseByRegistryProxyStub: COMAdminInUse = 3i32;
pub const COMAdminInUseByRegistryTypeLib: COMAdminInUse = 4i32;
pub const COMAdminInUseByRegistryUnknown: COMAdminInUse = 2i32;
pub const COMAdminInstallForceOverwriteOfFiles: COMAdminApplicationInstallOptions = 2i32;
pub const COMAdminInstallNoUsers: COMAdminApplicationInstallOptions = 0i32;
pub const COMAdminInstallUsers: COMAdminApplicationInstallOptions = 1i32;
pub const COMAdminNotInUse: COMAdminInUse = 0i32;
pub const COMAdminOSNotInitialized: COMAdminOS = 0i32;
pub const COMAdminOSUnknown: COMAdminOS = 6i32;
pub const COMAdminOSWindows2000: COMAdminOS = 3i32;
pub const COMAdminOSWindows2000AdvancedServer: COMAdminOS = 4i32;
pub const COMAdminOSWindows2000Unknown: COMAdminOS = 5i32;
pub const COMAdminOSWindows3_1: COMAdminOS = 1i32;
pub const COMAdminOSWindows7DatacenterServer: COMAdminOS = 27i32;
pub const COMAdminOSWindows7EnterpriseServer: COMAdminOS = 26i32;
pub const COMAdminOSWindows7Personal: COMAdminOS = 23i32;
pub const COMAdminOSWindows7Professional: COMAdminOS = 24i32;
pub const COMAdminOSWindows7StandardServer: COMAdminOS = 25i32;
pub const COMAdminOSWindows7WebServer: COMAdminOS = 28i32;
pub const COMAdminOSWindows8DatacenterServer: COMAdminOS = 33i32;
pub const COMAdminOSWindows8EnterpriseServer: COMAdminOS = 32i32;
pub const COMAdminOSWindows8Personal: COMAdminOS = 29i32;
pub const COMAdminOSWindows8Professional: COMAdminOS = 30i32;
pub const COMAdminOSWindows8StandardServer: COMAdminOS = 31i32;
pub const COMAdminOSWindows8WebServer: COMAdminOS = 34i32;
pub const COMAdminOSWindows9x: COMAdminOS = 2i32;
pub const COMAdminOSWindowsBlueDatacenterServer: COMAdminOS = 39i32;
pub const COMAdminOSWindowsBlueEnterpriseServer: COMAdminOS = 38i32;
pub const COMAdminOSWindowsBluePersonal: COMAdminOS = 35i32;
pub const COMAdminOSWindowsBlueProfessional: COMAdminOS = 36i32;
pub const COMAdminOSWindowsBlueStandardServer: COMAdminOS = 37i32;
pub const COMAdminOSWindowsBlueWebServer: COMAdminOS = 40i32;
pub const COMAdminOSWindowsLonghornDatacenterServer: COMAdminOS = 21i32;
pub const COMAdminOSWindowsLonghornEnterpriseServer: COMAdminOS = 20i32;
pub const COMAdminOSWindowsLonghornPersonal: COMAdminOS = 17i32;
pub const COMAdminOSWindowsLonghornProfessional: COMAdminOS = 18i32;
pub const COMAdminOSWindowsLonghornStandardServer: COMAdminOS = 19i32;
pub const COMAdminOSWindowsLonghornWebServer: COMAdminOS = 22i32;
pub const COMAdminOSWindowsNETDatacenterServer: COMAdminOS = 15i32;
pub const COMAdminOSWindowsNETEnterpriseServer: COMAdminOS = 14i32;
pub const COMAdminOSWindowsNETStandardServer: COMAdminOS = 13i32;
pub const COMAdminOSWindowsNETWebServer: COMAdminOS = 16i32;
pub const COMAdminOSWindowsXPPersonal: COMAdminOS = 11i32;
pub const COMAdminOSWindowsXPProfessional: COMAdminOS = 12i32;
pub const COMAdminQCMessageAuthenticateOff: COMAdminQCMessageAuthenticateOptions = 1i32;
pub const COMAdminQCMessageAuthenticateOn: COMAdminQCMessageAuthenticateOptions = 2i32;
pub const COMAdminQCMessageAuthenticateSecureApps: COMAdminQCMessageAuthenticateOptions = 0i32;
pub const COMAdminServiceContinuePending: COMAdminServiceStatusOptions = 4i32;
pub const COMAdminServiceLoadBalanceRouter: COMAdminServiceOptions = 1i32;
pub const COMAdminServicePausePending: COMAdminServiceStatusOptions = 5i32;
pub const COMAdminServicePaused: COMAdminServiceStatusOptions = 6i32;
pub const COMAdminServiceRunning: COMAdminServiceStatusOptions = 3i32;
pub const COMAdminServiceStartPending: COMAdminServiceStatusOptions = 1i32;
pub const COMAdminServiceStopPending: COMAdminServiceStatusOptions = 2i32;
pub const COMAdminServiceStopped: COMAdminServiceStatusOptions = 0i32;
pub const COMAdminServiceUnknownState: COMAdminServiceStatusOptions = 7i32;
pub const COMAdminSynchronizationIgnored: COMAdminSynchronizationOptions = 0i32;
pub const COMAdminSynchronizationNone: COMAdminSynchronizationOptions = 1i32;
pub const COMAdminSynchronizationRequired: COMAdminSynchronizationOptions = 3i32;
pub const COMAdminSynchronizationRequiresNew: COMAdminSynchronizationOptions = 4i32;
pub const COMAdminSynchronizationSupported: COMAdminSynchronizationOptions = 2i32;
pub const COMAdminThreadingModelApartment: COMAdminThreadingModels = 0i32;
pub const COMAdminThreadingModelBoth: COMAdminThreadingModels = 3i32;
pub const COMAdminThreadingModelFree: COMAdminThreadingModels = 1i32;
pub const COMAdminThreadingModelMain: COMAdminThreadingModels = 2i32;
pub const COMAdminThreadingModelNeutral: COMAdminThreadingModels = 4i32;
pub const COMAdminThreadingModelNotSpecified: COMAdminThreadingModels = 5i32;
pub const COMAdminTransactionIgnored: COMAdminTransactionOptions = 0i32;
pub const COMAdminTransactionNone: COMAdminTransactionOptions = 1i32;
pub const COMAdminTransactionRequired: COMAdminTransactionOptions = 3i32;
pub const COMAdminTransactionRequiresNew: COMAdminTransactionOptions = 4i32;
pub const COMAdminTransactionSupported: COMAdminTransactionOptions = 2i32;
pub const COMAdminTxIsolationLevelAny: COMAdminTxIsolationLevelOptions = 0i32;
pub const COMAdminTxIsolationLevelReadCommitted: COMAdminTxIsolationLevelOptions = 2i32;
pub const COMAdminTxIsolationLevelReadUnCommitted: COMAdminTxIsolationLevelOptions = 1i32;
pub const COMAdminTxIsolationLevelRepeatableRead: COMAdminTxIsolationLevelOptions = 3i32;
pub const COMAdminTxIsolationLevelSerializable: COMAdminTxIsolationLevelOptions = 4i32;
pub const COMEvents: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0ab_7f19_11d2_978e_0000f8757e2a);
pub const CRMClerk: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0bd_7f19_11d2_978e_0000f8757e2a);
pub const CRMFLAG_FORGETTARGET: CRMFLAGS = 1i32;
pub const CRMFLAG_REPLAYINPROGRESS: CRMFLAGS = 64i32;
pub const CRMFLAG_WRITTENDURINGABORT: CRMFLAGS = 8i32;
pub const CRMFLAG_WRITTENDURINGCOMMIT: CRMFLAGS = 4i32;
pub const CRMFLAG_WRITTENDURINGPREPARE: CRMFLAGS = 2i32;
pub const CRMFLAG_WRITTENDURINGRECOVERY: CRMFLAGS = 16i32;
pub const CRMFLAG_WRITTENDURINGREPLAY: CRMFLAGS = 32i32;
pub const CRMREGFLAG_ABORTPHASE: CRMREGFLAGS = 4i32;
pub const CRMREGFLAG_ALLPHASES: CRMREGFLAGS = 7i32;
pub const CRMREGFLAG_COMMITPHASE: CRMREGFLAGS = 2i32;
pub const CRMREGFLAG_FAILIFINDOUBTSREMAIN: CRMREGFLAGS = 16i32;
pub const CRMREGFLAG_PREPAREPHASE: CRMREGFLAGS = 1i32;
pub const CRMRecoveryClerk: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0be_7f19_11d2_978e_0000f8757e2a);
pub const CRR_ACTIVATION_LIMIT: u32 = 4294967294u32;
pub const CRR_CALL_LIMIT: u32 = 4294967293u32;
pub const CRR_LIFETIME_LIMIT: u32 = 4294967295u32;
pub const CRR_MEMORY_LIMIT: u32 = 4294967292u32;
pub const CRR_NO_REASON_SUPPLIED: u32 = 0u32;
pub const CRR_RECYCLED_FROM_UI: u32 = 4294967291u32;
pub const CSC_BindToPoolThread: CSC_Binding = 1i32;
pub const CSC_CreateTransactionIfNecessary: CSC_TransactionConfig = 2i32;
pub const CSC_DontUseTracker: CSC_TrackerConfig = 0i32;
pub const CSC_IfContainerIsSynchronized: CSC_SynchronizationConfig = 1i32;
pub const CSC_IfContainerIsTransactional: CSC_TransactionConfig = 1i32;
pub const CSC_Ignore: CSC_InheritanceConfig = 1i32;
pub const CSC_Inherit: CSC_InheritanceConfig = 0i32;
pub const CSC_InheritCOMTIIntrinsics: CSC_COMTIIntrinsicsConfig = 1i32;
pub const CSC_InheritIISIntrinsics: CSC_IISIntrinsicsConfig = 1i32;
pub const CSC_InheritPartition: CSC_PartitionConfig = 1i32;
pub const CSC_InheritSxs: CSC_SxsConfig = 1i32;
pub const CSC_MTAThreadPool: CSC_ThreadPool = 3i32;
pub const CSC_NewPartition: CSC_PartitionConfig = 2i32;
pub const CSC_NewSxs: CSC_SxsConfig = 2i32;
pub const CSC_NewSynchronization: CSC_SynchronizationConfig = 3i32;
pub const CSC_NewSynchronizationIfNecessary: CSC_SynchronizationConfig = 2i32;
pub const CSC_NewTransaction: CSC_TransactionConfig = 3i32;
pub const CSC_NoBinding: CSC_Binding = 0i32;
pub const CSC_NoCOMTIIntrinsics: CSC_COMTIIntrinsicsConfig = 0i32;
pub const CSC_NoIISIntrinsics: CSC_IISIntrinsicsConfig = 0i32;
pub const CSC_NoPartition: CSC_PartitionConfig = 0i32;
pub const CSC_NoSxs: CSC_SxsConfig = 0i32;
pub const CSC_NoSynchronization: CSC_SynchronizationConfig = 0i32;
pub const CSC_NoTransaction: CSC_TransactionConfig = 0i32;
pub const CSC_STAThreadPool: CSC_ThreadPool = 2i32;
pub const CSC_ThreadPoolInherit: CSC_ThreadPool = 1i32;
pub const CSC_ThreadPoolNone: CSC_ThreadPool = 0i32;
pub const CSC_UseTracker: CSC_TrackerConfig = 1i32;
pub const CServiceConfig: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0c8_7f19_11d2_978e_0000f8757e2a);
pub const ClrAssemblyLocator: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x458aa3b5_265a_4b75_bc05_9bea4630cf18);
pub const CoMTSLocator: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0ac_7f19_11d2_978e_0000f8757e2a);
pub const ComServiceEvents: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0c3_7f19_11d2_978e_0000f8757e2a);
pub const ComSystemAppEventData: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0c6_7f19_11d2_978e_0000f8757e2a);
pub const DATA_NOT_AVAILABLE: u32 = 4294967295u32;
pub const DUMPTYPE_FULL: DUMPTYPE = 0i32;
pub const DUMPTYPE_MINI: DUMPTYPE = 1i32;
pub const DUMPTYPE_NONE: DUMPTYPE = 2i32;
pub const DispenserManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0c0_7f19_11d2_978e_0000f8757e2a);
pub const Dummy30040732: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0a9_7f19_11d2_978e_0000f8757e2a);
pub const EventServer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabafbc_7f19_11d2_978e_0000f8757e2a);
pub const GATD_INCLUDE_APPLICATION_NAME: GetAppTrackerDataFlags = 16i32;
pub const GATD_INCLUDE_CLASS_NAME: GetAppTrackerDataFlags = 8i32;
pub const GATD_INCLUDE_LIBRARY_APPS: GetAppTrackerDataFlags = 2i32;
pub const GATD_INCLUDE_PROCESS_EXE_NAME: GetAppTrackerDataFlags = 1i32;
pub const GATD_INCLUDE_SWC: GetAppTrackerDataFlags = 4i32;
pub const GUID_STRING_SIZE: u32 = 40u32;
pub const GetSecurityCallContextAppObject: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0a8_7f19_11d2_978e_0000f8757e2a);
pub const LBEvents: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0c1_7f19_11d2_978e_0000f8757e2a);
pub const LockMethod: LockModes = 1i32;
pub const LockSetGet: LockModes = 0i32;
pub const MTXDM_E_ENLISTRESOURCEFAILED: u32 = 2147803392u32;
pub const MessageMover: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0bf_7f19_11d2_978e_0000f8757e2a);
pub const MtsGrp: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4b2e958d_0393_11d1_b1ab_00aa00ba3258);
pub const PoolMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabafb5_7f19_11d2_978e_0000f8757e2a);
pub const Process: ReleaseModes = 1i32;
pub const SecurityCallContext: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0a7_7f19_11d2_978e_0000f8757e2a);
pub const SecurityCallers: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0a6_7f19_11d2_978e_0000f8757e2a);
pub const SecurityIdentity: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0a5_7f19_11d2_978e_0000f8757e2a);
pub const ServicePool: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0c9_7f19_11d2_978e_0000f8757e2a);
pub const ServicePoolConfig: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabb0ca_7f19_11d2_978e_0000f8757e2a);
pub const SharedProperty: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2a005c05_a5de_11cf_9e66_00aa00a3f464);
pub const SharedPropertyGroup: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2a005c0b_a5de_11cf_9e66_00aa00a3f464);
pub const SharedPropertyGroupManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2a005c11_a5de_11cf_9e66_00aa00a3f464);
pub const Standard: ReleaseModes = 0i32;
pub const TRACKER_INIT_EVENT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Global\\COM+ Tracker Init Event");
pub const TRACKER_STARTSTOP_EVENT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Global\\COM+ Tracker Push Event");
pub const TRKCOLL_APPLICATIONS: TRACKING_COLL_TYPE = 1i32;
pub const TRKCOLL_COMPONENTS: TRACKING_COLL_TYPE = 2i32;
pub const TRKCOLL_PROCESSES: TRACKING_COLL_TYPE = 0i32;
pub const TrackerServer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecabafb9_7f19_11d2_978e_0000f8757e2a);
pub const TransactionContext: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7999fc25_d3c6_11cf_acab_00a024a55aef);
pub const TransactionContextEx: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5cb66670_d3d4_11cf_acab_00a024a55aef);
pub const TxAbort: TransactionVote = 1i32;
pub const TxCommit: TransactionVote = 0i32;
pub const TxState_Aborted: CrmTransactionState = 2i32;
pub const TxState_Active: CrmTransactionState = 0i32;
pub const TxState_Committed: CrmTransactionState = 1i32;
pub const TxState_Indoubt: CrmTransactionState = 3i32;
pub const comQCErrApplicationNotQueued: AutoSvcs_Error_Constants = 2148599296u32;
pub const comQCErrNoQueueableInterfaces: AutoSvcs_Error_Constants = 2148599297u32;
pub const comQCErrQueueTransactMismatch: AutoSvcs_Error_Constants = 2148599299u32;
pub const comQCErrQueuingServiceNotAvailable: AutoSvcs_Error_Constants = 2148599298u32;
pub const comqcErrBadMarshaledObject: AutoSvcs_Error_Constants = 2148599382u32;
pub const comqcErrInvalidMessage: AutoSvcs_Error_Constants = 2148599376u32;
pub const comqcErrMarshaledObjSameTxn: AutoSvcs_Error_Constants = 2148599304u32;
pub const comqcErrMsgNotAuthenticated: AutoSvcs_Error_Constants = 2148599380u32;
pub const comqcErrMsmqConnectorUsed: AutoSvcs_Error_Constants = 2148599381u32;
pub const comqcErrMsmqServiceUnavailable: AutoSvcs_Error_Constants = 2148599379u32;
pub const comqcErrMsmqSidUnavailable: AutoSvcs_Error_Constants = 2148599377u32;
pub const comqcErrOutParam: AutoSvcs_Error_Constants = 2148599301u32;
pub const comqcErrPSLoad: AutoSvcs_Error_Constants = 2148599303u32;
pub const comqcErrRecorderMarshalled: AutoSvcs_Error_Constants = 2148599300u32;
pub const comqcErrRecorderNotTrusted: AutoSvcs_Error_Constants = 2148599302u32;
pub const comqcErrWrongMsgExtension: AutoSvcs_Error_Constants = 2148599378u32;
pub const mtsErrCtxAborted: AutoSvcs_Error_Constants = 2147803138u32;
pub const mtsErrCtxAborting: AutoSvcs_Error_Constants = 2147803139u32;
pub const mtsErrCtxNoContext: AutoSvcs_Error_Constants = 2147803140u32;
pub const mtsErrCtxNoSecurity: AutoSvcs_Error_Constants = 2147803149u32;
pub const mtsErrCtxNotRegistered: AutoSvcs_Error_Constants = 2147803141u32;
pub const mtsErrCtxOldReference: AutoSvcs_Error_Constants = 2147803143u32;
pub const mtsErrCtxRoleNotFound: AutoSvcs_Error_Constants = 2147803148u32;
pub const mtsErrCtxSynchTimeout: AutoSvcs_Error_Constants = 2147803142u32;
pub const mtsErrCtxTMNotAvailable: AutoSvcs_Error_Constants = 2147803151u32;
pub const mtsErrCtxWrongThread: AutoSvcs_Error_Constants = 2147803150u32;
pub type AutoSvcs_Error_Constants = u32;
pub type COMAdminAccessChecksLevelOptions = i32;
pub type COMAdminActivationOptions = i32;
pub type COMAdminApplicationExportOptions = i32;
pub type COMAdminApplicationInstallOptions = i32;
pub type COMAdminAuthenticationCapabilitiesOptions = i32;
pub type COMAdminAuthenticationLevelOptions = i32;
pub type COMAdminComponentFlags = i32;
pub type COMAdminComponentType = i32;
pub type COMAdminErrorCodes = i32;
pub type COMAdminFileFlags = i32;
pub type COMAdminImpersonationLevelOptions = i32;
pub type COMAdminInUse = i32;
pub type COMAdminOS = i32;
pub type COMAdminQCMessageAuthenticateOptions = i32;
pub type COMAdminServiceOptions = i32;
pub type COMAdminServiceStatusOptions = i32;
pub type COMAdminSynchronizationOptions = i32;
pub type COMAdminThreadingModels = i32;
pub type COMAdminTransactionOptions = i32;
pub type COMAdminTxIsolationLevelOptions = i32;
pub type COMPLUS_APPTYPE = i32;
pub type CRMFLAGS = i32;
pub type CRMREGFLAGS = i32;
pub type CSC_Binding = i32;
pub type CSC_COMTIIntrinsicsConfig = i32;
pub type CSC_IISIntrinsicsConfig = i32;
pub type CSC_InheritanceConfig = i32;
pub type CSC_PartitionConfig = i32;
pub type CSC_SxsConfig = i32;
pub type CSC_SynchronizationConfig = i32;
pub type CSC_ThreadPool = i32;
pub type CSC_TrackerConfig = i32;
pub type CSC_TransactionConfig = i32;
pub type CrmTransactionState = i32;
pub type DUMPTYPE = i32;
pub type GetAppTrackerDataFlags = i32;
pub type LockModes = i32;
pub type ReleaseModes = i32;
pub type TRACKING_COLL_TYPE = i32;
pub type TransactionVote = i32;
#[repr(C)]
pub struct APPDATA {
    pub m_idApp: u32,
    pub m_szAppGuid: [u16; 40],
    pub m_dwAppProcessId: u32,
    pub m_AppStatistics: APPSTATISTICS,
}
impl ::core::marker::Copy for APPDATA {}
impl ::core::clone::Clone for APPDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct APPSTATISTICS {
    pub m_cTotalCalls: u32,
    pub m_cTotalInstances: u32,
    pub m_cTotalClasses: u32,
    pub m_cCallsPerSecond: u32,
}
impl ::core::marker::Copy for APPSTATISTICS {}
impl ::core::clone::Clone for APPSTATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ApplicationProcessRecycleInfo {
    pub IsRecyclable: super::super::Foundation::BOOL,
    pub IsRecycled: super::super::Foundation::BOOL,
    pub TimeRecycled: super::super::Foundation::FILETIME,
    pub TimeToTerminate: super::super::Foundation::FILETIME,
    pub RecycleReasonCode: i32,
    pub IsPendingRecycle: super::super::Foundation::BOOL,
    pub HasAutomaticLifetimeRecycling: super::super::Foundation::BOOL,
    pub TimeForAutomaticRecycling: super::super::Foundation::FILETIME,
    pub MemoryLimitInKB: u32,
    pub MemoryUsageInKBLastCheck: u32,
    pub ActivationLimit: u32,
    pub NumActivationsLastReported: u32,
    pub CallLimit: u32,
    pub NumCallsLastReported: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ApplicationProcessRecycleInfo {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ApplicationProcessRecycleInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ApplicationProcessStatistics {
    pub NumCallsOutstanding: u32,
    pub NumTrackedComponents: u32,
    pub NumComponentInstances: u32,
    pub AvgCallsPerSecond: u32,
    pub Reserved1: u32,
    pub Reserved2: u32,
    pub Reserved3: u32,
    pub Reserved4: u32,
}
impl ::core::marker::Copy for ApplicationProcessStatistics {}
impl ::core::clone::Clone for ApplicationProcessStatistics {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ApplicationProcessSummary {
    pub PartitionIdPrimaryApplication: ::windows_sys::core::GUID,
    pub ApplicationIdPrimaryApplication: ::windows_sys::core::GUID,
    pub ApplicationInstanceId: ::windows_sys::core::GUID,
    pub ProcessId: u32,
    pub Type: COMPLUS_APPTYPE,
    pub ProcessExeName: ::windows_sys::core::PWSTR,
    pub IsService: super::super::Foundation::BOOL,
    pub IsPaused: super::super::Foundation::BOOL,
    pub IsRecycled: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ApplicationProcessSummary {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ApplicationProcessSummary {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ApplicationSummary {
    pub ApplicationInstanceId: ::windows_sys::core::GUID,
    pub PartitionId: ::windows_sys::core::GUID,
    pub ApplicationId: ::windows_sys::core::GUID,
    pub Type: COMPLUS_APPTYPE,
    pub ApplicationName: ::windows_sys::core::PWSTR,
    pub NumTrackedComponents: u32,
    pub NumComponentInstances: u32,
}
impl ::core::marker::Copy for ApplicationSummary {}
impl ::core::clone::Clone for ApplicationSummary {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLSIDDATA {
    pub m_clsid: ::windows_sys::core::GUID,
    pub m_cReferences: u32,
    pub m_cBound: u32,
    pub m_cPooled: u32,
    pub m_cInCall: u32,
    pub m_dwRespTime: u32,
    pub m_cCallsCompleted: u32,
    pub m_cCallsFailed: u32,
}
impl ::core::marker::Copy for CLSIDDATA {}
impl ::core::clone::Clone for CLSIDDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLSIDDATA2 {
    pub m_clsid: ::windows_sys::core::GUID,
    pub m_appid: ::windows_sys::core::GUID,
    pub m_partid: ::windows_sys::core::GUID,
    pub m_pwszAppName: ::windows_sys::core::PWSTR,
    pub m_pwszCtxName: ::windows_sys::core::PWSTR,
    pub m_eAppType: COMPLUS_APPTYPE,
    pub m_cReferences: u32,
    pub m_cBound: u32,
    pub m_cPooled: u32,
    pub m_cInCall: u32,
    pub m_dwRespTime: u32,
    pub m_cCallsCompleted: u32,
    pub m_cCallsFailed: u32,
}
impl ::core::marker::Copy for CLSIDDATA2 {}
impl ::core::clone::Clone for CLSIDDATA2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COMSVCSEVENTINFO {
    pub cbSize: u32,
    pub dwPid: u32,
    pub lTime: i64,
    pub lMicroTime: i32,
    pub perfCount: i64,
    pub guidApp: ::windows_sys::core::GUID,
    pub sMachineName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for COMSVCSEVENTINFO {}
impl ::core::clone::Clone for COMSVCSEVENTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ComponentHangMonitorInfo {
    pub IsMonitored: super::super::Foundation::BOOL,
    pub TerminateOnHang: super::super::Foundation::BOOL,
    pub AvgCallThresholdInMs: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ComponentHangMonitorInfo {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ComponentHangMonitorInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ComponentStatistics {
    pub NumInstances: u32,
    pub NumBoundReferences: u32,
    pub NumPooledObjects: u32,
    pub NumObjectsInCall: u32,
    pub AvgResponseTimeInMs: u32,
    pub NumCallsCompletedRecent: u32,
    pub NumCallsFailedRecent: u32,
    pub NumCallsCompletedTotal: u32,
    pub NumCallsFailedTotal: u32,
    pub Reserved1: u32,
    pub Reserved2: u32,
    pub Reserved3: u32,
    pub Reserved4: u32,
}
impl ::core::marker::Copy for ComponentStatistics {}
impl ::core::clone::Clone for ComponentStatistics {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ComponentSummary {
    pub ApplicationInstanceId: ::windows_sys::core::GUID,
    pub PartitionId: ::windows_sys::core::GUID,
    pub ApplicationId: ::windows_sys::core::GUID,
    pub Clsid: ::windows_sys::core::GUID,
    pub ClassName: ::windows_sys::core::PWSTR,
    pub ApplicationName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ComponentSummary {}
impl ::core::clone::Clone for ComponentSummary {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct CrmLogRecordRead {
    pub dwCrmFlags: u32,
    pub dwSequenceNumber: u32,
    pub blobUserData: super::Com::BLOB,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for CrmLogRecordRead {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for CrmLogRecordRead {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HANG_INFO {
    pub fAppHangMonitorEnabled: super::super::Foundation::BOOL,
    pub fTerminateOnHang: super::super::Foundation::BOOL,
    pub DumpType: DUMPTYPE,
    pub dwHangTimeout: u32,
    pub dwDumpCount: u32,
    pub dwInfoMsgCount: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HANG_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HANG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RECYCLE_INFO {
    pub guidCombaseProcessIdentifier: ::windows_sys::core::GUID,
    pub ProcessStartTime: i64,
    pub dwRecycleLifetimeLimit: u32,
    pub dwRecycleMemoryLimit: u32,
    pub dwRecycleExpirationTimeout: u32,
}
impl ::core::marker::Copy for RECYCLE_INFO {}
impl ::core::clone::Clone for RECYCLE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
