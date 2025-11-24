::windows_targets::link!("dbgmodel.dll" "system" fn CreateDataModelManager(debughost : IDebugHost, manager : *mut IDataModelManager) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("dbgeng.dll" "system" fn DebugConnect(remoteoptions : ::windows_sys::core::PCSTR, interfaceid : *const ::windows_sys::core::GUID, interface : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("dbgeng.dll" "system" fn DebugConnectWide(remoteoptions : ::windows_sys::core::PCWSTR, interfaceid : *const ::windows_sys::core::GUID, interface : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("dbgeng.dll" "system" fn DebugCreate(interfaceid : *const ::windows_sys::core::GUID, interface : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("dbgeng.dll" "system" fn DebugCreateEx(interfaceid : *const ::windows_sys::core::GUID, dbgengoptions : u32, interface : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
pub type DebugBaseEventCallbacks = *mut ::core::ffi::c_void;
pub type DebugBaseEventCallbacksWide = *mut ::core::ffi::c_void;
pub type ICodeAddressConcept = *mut ::core::ffi::c_void;
pub type IComparableConcept = *mut ::core::ffi::c_void;
pub type IDataModelConcept = *mut ::core::ffi::c_void;
pub type IDataModelManager = *mut ::core::ffi::c_void;
pub type IDataModelManager2 = *mut ::core::ffi::c_void;
pub type IDataModelNameBinder = *mut ::core::ffi::c_void;
pub type IDataModelScript = *mut ::core::ffi::c_void;
pub type IDataModelScriptClient = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebug = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebug2 = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebugBreakpoint = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebugBreakpointEnumerator = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebugClient = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebugStack = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebugStackFrame = *mut ::core::ffi::c_void;
pub type IDataModelScriptDebugVariableSetEnumerator = *mut ::core::ffi::c_void;
pub type IDataModelScriptHostContext = *mut ::core::ffi::c_void;
pub type IDataModelScriptManager = *mut ::core::ffi::c_void;
pub type IDataModelScriptProvider = *mut ::core::ffi::c_void;
pub type IDataModelScriptProviderEnumerator = *mut ::core::ffi::c_void;
pub type IDataModelScriptTemplate = *mut ::core::ffi::c_void;
pub type IDataModelScriptTemplateEnumerator = *mut ::core::ffi::c_void;
pub type IDebugAdvanced = *mut ::core::ffi::c_void;
pub type IDebugAdvanced2 = *mut ::core::ffi::c_void;
pub type IDebugAdvanced3 = *mut ::core::ffi::c_void;
pub type IDebugAdvanced4 = *mut ::core::ffi::c_void;
pub type IDebugBreakpoint = *mut ::core::ffi::c_void;
pub type IDebugBreakpoint2 = *mut ::core::ffi::c_void;
pub type IDebugBreakpoint3 = *mut ::core::ffi::c_void;
pub type IDebugClient = *mut ::core::ffi::c_void;
pub type IDebugClient2 = *mut ::core::ffi::c_void;
pub type IDebugClient3 = *mut ::core::ffi::c_void;
pub type IDebugClient4 = *mut ::core::ffi::c_void;
pub type IDebugClient5 = *mut ::core::ffi::c_void;
pub type IDebugClient6 = *mut ::core::ffi::c_void;
pub type IDebugClient7 = *mut ::core::ffi::c_void;
pub type IDebugClient8 = *mut ::core::ffi::c_void;
pub type IDebugControl = *mut ::core::ffi::c_void;
pub type IDebugControl2 = *mut ::core::ffi::c_void;
pub type IDebugControl3 = *mut ::core::ffi::c_void;
pub type IDebugControl4 = *mut ::core::ffi::c_void;
pub type IDebugControl5 = *mut ::core::ffi::c_void;
pub type IDebugControl6 = *mut ::core::ffi::c_void;
pub type IDebugControl7 = *mut ::core::ffi::c_void;
pub type IDebugDataSpaces = *mut ::core::ffi::c_void;
pub type IDebugDataSpaces2 = *mut ::core::ffi::c_void;
pub type IDebugDataSpaces3 = *mut ::core::ffi::c_void;
pub type IDebugDataSpaces4 = *mut ::core::ffi::c_void;
pub type IDebugEventCallbacks = *mut ::core::ffi::c_void;
pub type IDebugEventCallbacksWide = *mut ::core::ffi::c_void;
pub type IDebugEventContextCallbacks = *mut ::core::ffi::c_void;
pub type IDebugFAEntryTags = *mut ::core::ffi::c_void;
pub type IDebugFailureAnalysis = *mut ::core::ffi::c_void;
pub type IDebugFailureAnalysis2 = *mut ::core::ffi::c_void;
pub type IDebugFailureAnalysis3 = *mut ::core::ffi::c_void;
pub type IDebugHost = *mut ::core::ffi::c_void;
pub type IDebugHostBaseClass = *mut ::core::ffi::c_void;
pub type IDebugHostConstant = *mut ::core::ffi::c_void;
pub type IDebugHostContext = *mut ::core::ffi::c_void;
pub type IDebugHostData = *mut ::core::ffi::c_void;
pub type IDebugHostErrorSink = *mut ::core::ffi::c_void;
pub type IDebugHostEvaluator = *mut ::core::ffi::c_void;
pub type IDebugHostEvaluator2 = *mut ::core::ffi::c_void;
pub type IDebugHostExtensibility = *mut ::core::ffi::c_void;
pub type IDebugHostField = *mut ::core::ffi::c_void;
pub type IDebugHostMemory = *mut ::core::ffi::c_void;
pub type IDebugHostMemory2 = *mut ::core::ffi::c_void;
pub type IDebugHostModule = *mut ::core::ffi::c_void;
pub type IDebugHostModule2 = *mut ::core::ffi::c_void;
pub type IDebugHostModuleSignature = *mut ::core::ffi::c_void;
pub type IDebugHostPublic = *mut ::core::ffi::c_void;
pub type IDebugHostScriptHost = *mut ::core::ffi::c_void;
pub type IDebugHostStatus = *mut ::core::ffi::c_void;
pub type IDebugHostSymbol = *mut ::core::ffi::c_void;
pub type IDebugHostSymbol2 = *mut ::core::ffi::c_void;
pub type IDebugHostSymbolEnumerator = *mut ::core::ffi::c_void;
pub type IDebugHostSymbols = *mut ::core::ffi::c_void;
pub type IDebugHostType = *mut ::core::ffi::c_void;
pub type IDebugHostType2 = *mut ::core::ffi::c_void;
pub type IDebugHostTypeSignature = *mut ::core::ffi::c_void;
pub type IDebugInputCallbacks = *mut ::core::ffi::c_void;
pub type IDebugOutputCallbacks = *mut ::core::ffi::c_void;
pub type IDebugOutputCallbacks2 = *mut ::core::ffi::c_void;
pub type IDebugOutputCallbacksWide = *mut ::core::ffi::c_void;
pub type IDebugOutputStream = *mut ::core::ffi::c_void;
pub type IDebugPlmClient = *mut ::core::ffi::c_void;
pub type IDebugPlmClient2 = *mut ::core::ffi::c_void;
pub type IDebugPlmClient3 = *mut ::core::ffi::c_void;
pub type IDebugRegisters = *mut ::core::ffi::c_void;
pub type IDebugRegisters2 = *mut ::core::ffi::c_void;
pub type IDebugSymbolGroup = *mut ::core::ffi::c_void;
pub type IDebugSymbolGroup2 = *mut ::core::ffi::c_void;
pub type IDebugSymbols = *mut ::core::ffi::c_void;
pub type IDebugSymbols2 = *mut ::core::ffi::c_void;
pub type IDebugSymbols3 = *mut ::core::ffi::c_void;
pub type IDebugSymbols4 = *mut ::core::ffi::c_void;
pub type IDebugSymbols5 = *mut ::core::ffi::c_void;
pub type IDebugSystemObjects = *mut ::core::ffi::c_void;
pub type IDebugSystemObjects2 = *mut ::core::ffi::c_void;
pub type IDebugSystemObjects3 = *mut ::core::ffi::c_void;
pub type IDebugSystemObjects4 = *mut ::core::ffi::c_void;
pub type IDynamicConceptProviderConcept = *mut ::core::ffi::c_void;
pub type IDynamicKeyProviderConcept = *mut ::core::ffi::c_void;
pub type IEquatableConcept = *mut ::core::ffi::c_void;
pub type IHostDataModelAccess = *mut ::core::ffi::c_void;
pub type IIndexableConcept = *mut ::core::ffi::c_void;
pub type IIterableConcept = *mut ::core::ffi::c_void;
pub type IKeyEnumerator = *mut ::core::ffi::c_void;
pub type IKeyStore = *mut ::core::ffi::c_void;
pub type IModelIterator = *mut ::core::ffi::c_void;
pub type IModelKeyReference = *mut ::core::ffi::c_void;
pub type IModelKeyReference2 = *mut ::core::ffi::c_void;
pub type IModelMethod = *mut ::core::ffi::c_void;
pub type IModelObject = *mut ::core::ffi::c_void;
pub type IModelPropertyAccessor = *mut ::core::ffi::c_void;
pub type IPreferredRuntimeTypeConcept = *mut ::core::ffi::c_void;
pub type IRawEnumerator = *mut ::core::ffi::c_void;
pub type IStringDisplayableConcept = *mut ::core::ffi::c_void;
pub const ADDRESS_TYPE_INDEX_NOT_FOUND: u32 = 11u32;
pub const Ambiguous: SignatureComparison = 1i32;
pub const CANNOT_ALLOCATE_MEMORY: u32 = 9u32;
pub const CLSID_DebugFailureAnalysisBasic: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb74eed7f_1c7d_4c1b_959f_b96dd9175aa4);
pub const CLSID_DebugFailureAnalysisKernel: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xee433078_64af_4c33_ab2f_ecad7f2a002d);
pub const CLSID_DebugFailureAnalysisTarget: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xba9bfb05_ef75_4bbd_a745_a6b5529458b8);
pub const CLSID_DebugFailureAnalysisUser: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe60b0c93_cf49_4a32_8147_0362202dc56b);
pub const CLSID_DebugFailureAnalysisWinCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x67d5e86f_f5e2_462a_9233_1bd616fcc7e8);
pub const CLSID_DebugFailureAnalysisXBox360: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x901625bb_95f1_4318_ac80_9d733cee8c8b);
pub const CROSS_PLATFORM_MAXIMUM_PROCESSORS: u32 = 2048u32;
pub const CURRENT_KD_SECONDARY_VERSION: u32 = 2u32;
pub const CallingConventionCDecl: CallingConventionKind = 1i32;
pub const CallingConventionFastCall: CallingConventionKind = 2i32;
pub const CallingConventionStdCall: CallingConventionKind = 3i32;
pub const CallingConventionSysCall: CallingConventionKind = 4i32;
pub const CallingConventionThisCall: CallingConventionKind = 5i32;
pub const CallingConventionUnknown: CallingConventionKind = 0i32;
pub const DBGKD_MAJOR_BIG: DBGKD_MAJOR_TYPES = 2i32;
pub const DBGKD_MAJOR_CE: DBGKD_MAJOR_TYPES = 10i32;
pub const DBGKD_MAJOR_COUNT: DBGKD_MAJOR_TYPES = 11i32;
pub const DBGKD_MAJOR_EFI: DBGKD_MAJOR_TYPES = 5i32;
pub const DBGKD_MAJOR_EXDI: DBGKD_MAJOR_TYPES = 3i32;
pub const DBGKD_MAJOR_HYPERVISOR: DBGKD_MAJOR_TYPES = 8i32;
pub const DBGKD_MAJOR_MIDORI: DBGKD_MAJOR_TYPES = 9i32;
pub const DBGKD_MAJOR_NT: DBGKD_MAJOR_TYPES = 0i32;
pub const DBGKD_MAJOR_NTBD: DBGKD_MAJOR_TYPES = 4i32;
pub const DBGKD_MAJOR_SINGULARITY: DBGKD_MAJOR_TYPES = 7i32;
pub const DBGKD_MAJOR_TNT: DBGKD_MAJOR_TYPES = 6i32;
pub const DBGKD_MAJOR_XBOX: DBGKD_MAJOR_TYPES = 1i32;
pub const DBGKD_SIMULATION_EXDI: i32 = 1i32;
pub const DBGKD_SIMULATION_NONE: i32 = 0i32;
pub const DBGKD_VERS_FLAG_DATA: u32 = 2u32;
pub const DBGKD_VERS_FLAG_HAL_IN_NTOS: u32 = 64u32;
pub const DBGKD_VERS_FLAG_HSS: u32 = 16u32;
pub const DBGKD_VERS_FLAG_MP: u32 = 1u32;
pub const DBGKD_VERS_FLAG_NOMM: u32 = 8u32;
pub const DBGKD_VERS_FLAG_PARTITIONS: u32 = 32u32;
pub const DBGKD_VERS_FLAG_PTR64: u32 = 4u32;
pub const DBG_DUMP_ADDRESS_AT_END: u32 = 131072u32;
pub const DBG_DUMP_ADDRESS_OF_FIELD: u32 = 65536u32;
pub const DBG_DUMP_ARRAY: u32 = 32768u32;
pub const DBG_DUMP_BLOCK_RECURSE: u32 = 2097152u32;
pub const DBG_DUMP_CALL_FOR_EACH: u32 = 8u32;
pub const DBG_DUMP_COMPACT_OUT: u32 = 8192u32;
pub const DBG_DUMP_COPY_TYPE_DATA: u32 = 262144u32;
pub const DBG_DUMP_FIELD_ARRAY: u32 = 16u32;
pub const DBG_DUMP_FIELD_CALL_BEFORE_PRINT: u32 = 1u32;
pub const DBG_DUMP_FIELD_COPY_FIELD_DATA: u32 = 32u32;
pub const DBG_DUMP_FIELD_DEFAULT_STRING: u32 = 65536u32;
pub const DBG_DUMP_FIELD_FULL_NAME: u32 = 8u32;
pub const DBG_DUMP_FIELD_GUID_STRING: u32 = 524288u32;
pub const DBG_DUMP_FIELD_MULTI_STRING: u32 = 262144u32;
pub const DBG_DUMP_FIELD_NO_CALLBACK_REQ: u32 = 2u32;
pub const DBG_DUMP_FIELD_NO_PRINT: u32 = 16384u32;
pub const DBG_DUMP_FIELD_RECUR_ON_THIS: u32 = 4u32;
pub const DBG_DUMP_FIELD_RETURN_ADDRESS: u32 = 4096u32;
pub const DBG_DUMP_FIELD_SIZE_IN_BITS: u32 = 8192u32;
pub const DBG_DUMP_FIELD_UTF32_STRING: u32 = 1048576u32;
pub const DBG_DUMP_FIELD_WCHAR_STRING: u32 = 131072u32;
pub const DBG_DUMP_FUNCTION_FORMAT: u32 = 1048576u32;
pub const DBG_DUMP_GET_SIZE_ONLY: u32 = 128u32;
pub const DBG_DUMP_LIST: u32 = 32u32;
pub const DBG_DUMP_MATCH_SIZE: u32 = 4194304u32;
pub const DBG_DUMP_NO_INDENT: u32 = 1u32;
pub const DBG_DUMP_NO_OFFSET: u32 = 2u32;
pub const DBG_DUMP_NO_PRINT: u32 = 64u32;
pub const DBG_DUMP_READ_PHYSICAL: u32 = 524288u32;
pub const DBG_DUMP_VERBOSE: u32 = 4u32;
pub const DBG_FRAME_DEFAULT: u32 = 0u32;
pub const DBG_FRAME_IGNORE_INLINE: u32 = 4294967295u32;
pub const DBG_RETURN_SUBTYPES: u32 = 0u32;
pub const DBG_RETURN_TYPE: u32 = 0u32;
pub const DBG_RETURN_TYPE_VALUES: u32 = 0u32;
pub const DEBUG_ADDSYNTHMOD_DEFAULT: u32 = 0u32;
pub const DEBUG_ADDSYNTHMOD_ZEROBASE: u32 = 1u32;
pub const DEBUG_ADDSYNTHSYM_DEFAULT: u32 = 0u32;
pub const DEBUG_ANY_ID: u32 = 4294967295u32;
pub const DEBUG_ASMOPT_DEFAULT: u32 = 0u32;
pub const DEBUG_ASMOPT_IGNORE_OUTPUT_WIDTH: u32 = 4u32;
pub const DEBUG_ASMOPT_NO_CODE_BYTES: u32 = 2u32;
pub const DEBUG_ASMOPT_SOURCE_LINE_NUMBER: u32 = 8u32;
pub const DEBUG_ASMOPT_VERBOSE: u32 = 1u32;
pub const DEBUG_ATTACH_DEFAULT: u32 = 0u32;
pub const DEBUG_ATTACH_EXDI_DRIVER: u32 = 2u32;
pub const DEBUG_ATTACH_EXISTING: u32 = 2u32;
pub const DEBUG_ATTACH_INSTALL_DRIVER: u32 = 4u32;
pub const DEBUG_ATTACH_INVASIVE_NO_INITIAL_BREAK: u32 = 8u32;
pub const DEBUG_ATTACH_INVASIVE_RESUME_PROCESS: u32 = 16u32;
pub const DEBUG_ATTACH_KERNEL_CONNECTION: u32 = 0u32;
pub const DEBUG_ATTACH_LOCAL_KERNEL: u32 = 1u32;
pub const DEBUG_ATTACH_NONINVASIVE: u32 = 1u32;
pub const DEBUG_ATTACH_NONINVASIVE_ALLOW_PARTIAL: u32 = 32u32;
pub const DEBUG_ATTACH_NONINVASIVE_NO_SUSPEND: u32 = 4u32;
pub const DEBUG_BREAKPOINT_ADDER_ONLY: u32 = 8u32;
pub const DEBUG_BREAKPOINT_CODE: u32 = 0u32;
pub const DEBUG_BREAKPOINT_DATA: u32 = 1u32;
pub const DEBUG_BREAKPOINT_DEFERRED: u32 = 2u32;
pub const DEBUG_BREAKPOINT_ENABLED: u32 = 4u32;
pub const DEBUG_BREAKPOINT_GO_ONLY: u32 = 1u32;
pub const DEBUG_BREAKPOINT_INLINE: u32 = 3u32;
pub const DEBUG_BREAKPOINT_ONE_SHOT: u32 = 16u32;
pub const DEBUG_BREAKPOINT_TIME: u32 = 2u32;
pub const DEBUG_BREAK_EXECUTE: u32 = 4u32;
pub const DEBUG_BREAK_IO: u32 = 8u32;
pub const DEBUG_BREAK_READ: u32 = 1u32;
pub const DEBUG_BREAK_WRITE: u32 = 2u32;
pub const DEBUG_CDS_ALL: u32 = 4294967295u32;
pub const DEBUG_CDS_DATA: u32 = 2u32;
pub const DEBUG_CDS_REFRESH: u32 = 4u32;
pub const DEBUG_CDS_REFRESH_ADDBREAKPOINT: u32 = 4u32;
pub const DEBUG_CDS_REFRESH_EVALUATE: u32 = 1u32;
pub const DEBUG_CDS_REFRESH_EXECUTE: u32 = 2u32;
pub const DEBUG_CDS_REFRESH_EXECUTECOMMANDFILE: u32 = 3u32;
pub const DEBUG_CDS_REFRESH_INLINESTEP: u32 = 16u32;
pub const DEBUG_CDS_REFRESH_INLINESTEP_PSEUDO: u32 = 17u32;
pub const DEBUG_CDS_REFRESH_REMOVEBREAKPOINT: u32 = 5u32;
pub const DEBUG_CDS_REFRESH_SETSCOPE: u32 = 12u32;
pub const DEBUG_CDS_REFRESH_SETSCOPEFRAMEBYINDEX: u32 = 13u32;
pub const DEBUG_CDS_REFRESH_SETSCOPEFROMJITDEBUGINFO: u32 = 14u32;
pub const DEBUG_CDS_REFRESH_SETSCOPEFROMSTOREDEVENT: u32 = 15u32;
pub const DEBUG_CDS_REFRESH_SETVALUE: u32 = 10u32;
pub const DEBUG_CDS_REFRESH_SETVALUE2: u32 = 11u32;
pub const DEBUG_CDS_REFRESH_WRITEPHYSICAL: u32 = 8u32;
pub const DEBUG_CDS_REFRESH_WRITEPHYSICAL2: u32 = 9u32;
pub const DEBUG_CDS_REFRESH_WRITEVIRTUAL: u32 = 6u32;
pub const DEBUG_CDS_REFRESH_WRITEVIRTUALUNCACHED: u32 = 7u32;
pub const DEBUG_CDS_REGISTERS: u32 = 1u32;
pub const DEBUG_CES_ALL: u32 = 4294967295u32;
pub const DEBUG_CES_ASSEMBLY_OPTIONS: u32 = 4096u32;
pub const DEBUG_CES_BREAKPOINTS: u32 = 4u32;
pub const DEBUG_CES_CODE_LEVEL: u32 = 8u32;
pub const DEBUG_CES_CURRENT_THREAD: u32 = 1u32;
pub const DEBUG_CES_EFFECTIVE_PROCESSOR: u32 = 2u32;
pub const DEBUG_CES_ENGINE_OPTIONS: u32 = 32u32;
pub const DEBUG_CES_EVENT_FILTERS: u32 = 256u32;
pub const DEBUG_CES_EXECUTION_STATUS: u32 = 16u32;
pub const DEBUG_CES_EXPRESSION_SYNTAX: u32 = 8192u32;
pub const DEBUG_CES_EXTENSIONS: u32 = 1024u32;
pub const DEBUG_CES_LOG_FILE: u32 = 64u32;
pub const DEBUG_CES_PROCESS_OPTIONS: u32 = 512u32;
pub const DEBUG_CES_RADIX: u32 = 128u32;
pub const DEBUG_CES_SYSTEMS: u32 = 2048u32;
pub const DEBUG_CES_TEXT_REPLACEMENTS: u32 = 16384u32;
pub const DEBUG_CLASS_IMAGE_FILE: u32 = 3u32;
pub const DEBUG_CLASS_KERNEL: u32 = 1u32;
pub const DEBUG_CLASS_UNINITIALIZED: u32 = 0u32;
pub const DEBUG_CLASS_USER_WINDOWS: u32 = 2u32;
pub const DEBUG_CLIENT_CDB: u32 = 4u32;
pub const DEBUG_CLIENT_KD: u32 = 5u32;
pub const DEBUG_CLIENT_NTKD: u32 = 3u32;
pub const DEBUG_CLIENT_NTSD: u32 = 2u32;
pub const DEBUG_CLIENT_UNKNOWN: u32 = 0u32;
pub const DEBUG_CLIENT_VSINT: u32 = 1u32;
pub const DEBUG_CLIENT_WINDBG: u32 = 6u32;
pub const DEBUG_CLIENT_WINIDE: u32 = 7u32;
pub const DEBUG_CMDEX_ADD_EVENT_STRING: u32 = 1u32;
pub const DEBUG_CMDEX_INVALID: u32 = 0u32;
pub const DEBUG_CMDEX_RESET_EVENT_STRINGS: u32 = 2u32;
pub const DEBUG_COMMAND_EXCEPTION_ID: u32 = 3688893886u32;
pub const DEBUG_CONNECT_SESSION_DEFAULT: u32 = 0u32;
pub const DEBUG_CONNECT_SESSION_NO_ANNOUNCE: u32 = 2u32;
pub const DEBUG_CONNECT_SESSION_NO_VERSION: u32 = 1u32;
pub const DEBUG_CSS_ALL: u32 = 4294967295u32;
pub const DEBUG_CSS_COLLAPSE_CHILDREN: u32 = 64u32;
pub const DEBUG_CSS_LOADS: u32 = 1u32;
pub const DEBUG_CSS_PATHS: u32 = 8u32;
pub const DEBUG_CSS_SCOPE: u32 = 4u32;
pub const DEBUG_CSS_SYMBOL_OPTIONS: u32 = 16u32;
pub const DEBUG_CSS_TYPE_OPTIONS: u32 = 32u32;
pub const DEBUG_CSS_UNLOADS: u32 = 2u32;
pub const DEBUG_CURRENT_DEFAULT: u32 = 15u32;
pub const DEBUG_CURRENT_DISASM: u32 = 2u32;
pub const DEBUG_CURRENT_REGISTERS: u32 = 4u32;
pub const DEBUG_CURRENT_SOURCE_LINE: u32 = 8u32;
pub const DEBUG_CURRENT_SYMBOL: u32 = 1u32;
pub const DEBUG_DATA_BASE_TRANSLATION_VIRTUAL_OFFSET: u32 = 3u32;
pub const DEBUG_DATA_BreakpointWithStatusAddr: u32 = 32u32;
pub const DEBUG_DATA_CmNtCSDVersionAddr: u32 = 616u32;
pub const DEBUG_DATA_DumpAttributes: u32 = 100072u32;
pub const DEBUG_DATA_DumpFormatVersion: u32 = 100040u32;
pub const DEBUG_DATA_DumpMmStorage: u32 = 100064u32;
pub const DEBUG_DATA_DumpPowerState: u32 = 100056u32;
pub const DEBUG_DATA_DumpWriterStatus: u32 = 100032u32;
pub const DEBUG_DATA_DumpWriterVersion: u32 = 100048u32;
pub const DEBUG_DATA_EtwpDebuggerData: u32 = 816u32;
pub const DEBUG_DATA_ExpNumberOfPagedPoolsAddr: u32 = 112u32;
pub const DEBUG_DATA_ExpPagedPoolDescriptorAddr: u32 = 104u32;
pub const DEBUG_DATA_ExpSystemResourcesListAddr: u32 = 96u32;
pub const DEBUG_DATA_IopErrorLogListHeadAddr: u32 = 144u32;
pub const DEBUG_DATA_KPCR_OFFSET: u32 = 0u32;
pub const DEBUG_DATA_KPRCB_OFFSET: u32 = 1u32;
pub const DEBUG_DATA_KTHREAD_OFFSET: u32 = 2u32;
pub const DEBUG_DATA_KdPrintBufferSizeAddr: u32 = 720u32;
pub const DEBUG_DATA_KdPrintCircularBufferAddr: u32 = 480u32;
pub const DEBUG_DATA_KdPrintCircularBufferEndAddr: u32 = 488u32;
pub const DEBUG_DATA_KdPrintCircularBufferPtrAddr: u32 = 712u32;
pub const DEBUG_DATA_KdPrintRolloverCountAddr: u32 = 504u32;
pub const DEBUG_DATA_KdPrintWritePointerAddr: u32 = 496u32;
pub const DEBUG_DATA_KeBugCheckCallbackListHeadAddr: u32 = 128u32;
pub const DEBUG_DATA_KeTimeIncrementAddr: u32 = 120u32;
pub const DEBUG_DATA_KeUserCallbackDispatcherAddr: u32 = 64u32;
pub const DEBUG_DATA_KernBase: u32 = 24u32;
pub const DEBUG_DATA_KernelVerifierAddr: u32 = 576u32;
pub const DEBUG_DATA_KiBugcheckDataAddr: u32 = 136u32;
pub const DEBUG_DATA_KiCallUserModeAddr: u32 = 56u32;
pub const DEBUG_DATA_KiNormalSystemCall: u32 = 528u32;
pub const DEBUG_DATA_KiProcessorBlockAddr: u32 = 536u32;
pub const DEBUG_DATA_MmAllocatedNonPagedPoolAddr: u32 = 592u32;
pub const DEBUG_DATA_MmAvailablePagesAddr: u32 = 424u32;
pub const DEBUG_DATA_MmBadPagesDetected: u32 = 800u32;
pub const DEBUG_DATA_MmDriverCommitAddr: u32 = 352u32;
pub const DEBUG_DATA_MmExtendedCommitAddr: u32 = 376u32;
pub const DEBUG_DATA_MmFreePageListHeadAddr: u32 = 392u32;
pub const DEBUG_DATA_MmHighestPhysicalPageAddr: u32 = 240u32;
pub const DEBUG_DATA_MmHighestUserAddressAddr: u32 = 456u32;
pub const DEBUG_DATA_MmLastUnloadedDriverAddr: u32 = 552u32;
pub const DEBUG_DATA_MmLoadedUserImageListAddr: u32 = 512u32;
pub const DEBUG_DATA_MmLowestPhysicalPageAddr: u32 = 232u32;
pub const DEBUG_DATA_MmMaximumNonPagedPoolInBytesAddr: u32 = 256u32;
pub const DEBUG_DATA_MmModifiedNoWritePageListHeadAddr: u32 = 416u32;
pub const DEBUG_DATA_MmModifiedPageListHeadAddr: u32 = 408u32;
pub const DEBUG_DATA_MmNonPagedPoolEndAddr: u32 = 280u32;
pub const DEBUG_DATA_MmNonPagedPoolStartAddr: u32 = 272u32;
pub const DEBUG_DATA_MmNonPagedSystemStartAddr: u32 = 264u32;
pub const DEBUG_DATA_MmNumberOfPagingFilesAddr: u32 = 224u32;
pub const DEBUG_DATA_MmNumberOfPhysicalPagesAddr: u32 = 248u32;
pub const DEBUG_DATA_MmPageSize: u32 = 312u32;
pub const DEBUG_DATA_MmPagedPoolCommitAddr: u32 = 368u32;
pub const DEBUG_DATA_MmPagedPoolEndAddr: u32 = 296u32;
pub const DEBUG_DATA_MmPagedPoolInformationAddr: u32 = 304u32;
pub const DEBUG_DATA_MmPagedPoolStartAddr: u32 = 288u32;
pub const DEBUG_DATA_MmPeakCommitmentAddr: u32 = 600u32;
pub const DEBUG_DATA_MmPfnDatabaseAddr: u32 = 192u32;
pub const DEBUG_DATA_MmPhysicalMemoryBlockAddr: u32 = 624u32;
pub const DEBUG_DATA_MmProcessCommitAddr: u32 = 360u32;
pub const DEBUG_DATA_MmResidentAvailablePagesAddr: u32 = 432u32;
pub const DEBUG_DATA_MmSessionBase: u32 = 632u32;
pub const DEBUG_DATA_MmSessionSize: u32 = 640u32;
pub const DEBUG_DATA_MmSharedCommitAddr: u32 = 344u32;
pub const DEBUG_DATA_MmSizeOfPagedPoolInBytesAddr: u32 = 320u32;
pub const DEBUG_DATA_MmSpecialPoolTagAddr: u32 = 568u32;
pub const DEBUG_DATA_MmStandbyPageListHeadAddr: u32 = 400u32;
pub const DEBUG_DATA_MmSubsectionBaseAddr: u32 = 216u32;
pub const DEBUG_DATA_MmSystemCacheEndAddr: u32 = 176u32;
pub const DEBUG_DATA_MmSystemCacheStartAddr: u32 = 168u32;
pub const DEBUG_DATA_MmSystemCacheWsAddr: u32 = 184u32;
pub const DEBUG_DATA_MmSystemParentTablePage: u32 = 648u32;
pub const DEBUG_DATA_MmSystemPtesEndAddr: u32 = 208u32;
pub const DEBUG_DATA_MmSystemPtesStartAddr: u32 = 200u32;
pub const DEBUG_DATA_MmSystemRangeStartAddr: u32 = 464u32;
pub const DEBUG_DATA_MmTotalCommitLimitAddr: u32 = 328u32;
pub const DEBUG_DATA_MmTotalCommitLimitMaximumAddr: u32 = 608u32;
pub const DEBUG_DATA_MmTotalCommittedPagesAddr: u32 = 336u32;
pub const DEBUG_DATA_MmTriageActionTakenAddr: u32 = 560u32;
pub const DEBUG_DATA_MmUnloadedDriversAddr: u32 = 544u32;
pub const DEBUG_DATA_MmUserProbeAddressAddr: u32 = 472u32;
pub const DEBUG_DATA_MmVerifierDataAddr: u32 = 584u32;
pub const DEBUG_DATA_MmVirtualTranslationBase: u32 = 656u32;
pub const DEBUG_DATA_MmZeroedPageListHeadAddr: u32 = 384u32;
pub const DEBUG_DATA_NonPagedPoolDescriptorAddr: u32 = 448u32;
pub const DEBUG_DATA_NtBuildLabAddr: u32 = 520u32;
pub const DEBUG_DATA_ObpRootDirectoryObjectAddr: u32 = 152u32;
pub const DEBUG_DATA_ObpTypeObjectTypeAddr: u32 = 160u32;
pub const DEBUG_DATA_OffsetEprocessDirectoryTableBase: u32 = 686u32;
pub const DEBUG_DATA_OffsetEprocessParentCID: u32 = 684u32;
pub const DEBUG_DATA_OffsetEprocessPeb: u32 = 682u32;
pub const DEBUG_DATA_OffsetKThreadApcProcess: u32 = 672u32;
pub const DEBUG_DATA_OffsetKThreadBStore: u32 = 676u32;
pub const DEBUG_DATA_OffsetKThreadBStoreLimit: u32 = 678u32;
pub const DEBUG_DATA_OffsetKThreadInitialStack: u32 = 670u32;
pub const DEBUG_DATA_OffsetKThreadKernelStack: u32 = 668u32;
pub const DEBUG_DATA_OffsetKThreadNextProcessor: u32 = 664u32;
pub const DEBUG_DATA_OffsetKThreadState: u32 = 674u32;
pub const DEBUG_DATA_OffsetKThreadTeb: u32 = 666u32;
pub const DEBUG_DATA_OffsetPrcbCpuType: u32 = 696u32;
pub const DEBUG_DATA_OffsetPrcbCurrentThread: u32 = 692u32;
pub const DEBUG_DATA_OffsetPrcbDpcRoutine: u32 = 690u32;
pub const DEBUG_DATA_OffsetPrcbMhz: u32 = 694u32;
pub const DEBUG_DATA_OffsetPrcbNumber: u32 = 702u32;
pub const DEBUG_DATA_OffsetPrcbProcessorState: u32 = 700u32;
pub const DEBUG_DATA_OffsetPrcbVendorString: u32 = 698u32;
pub const DEBUG_DATA_PROCESSOR_IDENTIFICATION: u32 = 4u32;
pub const DEBUG_DATA_PROCESSOR_SPEED: u32 = 5u32;
pub const DEBUG_DATA_PaeEnabled: u32 = 100000u32;
pub const DEBUG_DATA_PagingLevels: u32 = 100080u32;
pub const DEBUG_DATA_PoolTrackTableAddr: u32 = 440u32;
pub const DEBUG_DATA_ProductType: u32 = 100016u32;
pub const DEBUG_DATA_PsActiveProcessHeadAddr: u32 = 80u32;
pub const DEBUG_DATA_PsLoadedModuleListAddr: u32 = 72u32;
pub const DEBUG_DATA_PspCidTableAddr: u32 = 88u32;
pub const DEBUG_DATA_PteBase: u32 = 864u32;
pub const DEBUG_DATA_SPACE_BUS_DATA: u32 = 5u32;
pub const DEBUG_DATA_SPACE_CONTROL: u32 = 2u32;
pub const DEBUG_DATA_SPACE_COUNT: u32 = 7u32;
pub const DEBUG_DATA_SPACE_DEBUGGER_DATA: u32 = 6u32;
pub const DEBUG_DATA_SPACE_IO: u32 = 3u32;
pub const DEBUG_DATA_SPACE_MSR: u32 = 4u32;
pub const DEBUG_DATA_SPACE_PHYSICAL: u32 = 1u32;
pub const DEBUG_DATA_SPACE_VIRTUAL: u32 = 0u32;
pub const DEBUG_DATA_SavedContextAddr: u32 = 40u32;
pub const DEBUG_DATA_SharedUserData: u32 = 100008u32;
pub const DEBUG_DATA_SizeEProcess: u32 = 680u32;
pub const DEBUG_DATA_SizeEThread: u32 = 704u32;
pub const DEBUG_DATA_SizePrcb: u32 = 688u32;
pub const DEBUG_DATA_SuiteMask: u32 = 100024u32;
pub const DEBUG_DISASM_EFFECTIVE_ADDRESS: u32 = 1u32;
pub const DEBUG_DISASM_MATCHING_SYMBOLS: u32 = 2u32;
pub const DEBUG_DISASM_SOURCE_FILE_NAME: u32 = 8u32;
pub const DEBUG_DISASM_SOURCE_LINE_NUMBER: u32 = 4u32;
pub const DEBUG_DUMP_ACTIVE: u32 = 1030u32;
pub const DEBUG_DUMP_DEFAULT: u32 = 1025u32;
pub const DEBUG_DUMP_FILE_BASE: u32 = 4294967295u32;
pub const DEBUG_DUMP_FILE_LOAD_FAILED_INDEX: u32 = 4294967295u32;
pub const DEBUG_DUMP_FILE_ORIGINAL_CAB_INDEX: u32 = 4294967294u32;
pub const DEBUG_DUMP_FILE_PAGE_FILE_DUMP: u32 = 0u32;
pub const DEBUG_DUMP_FULL: u32 = 1026u32;
pub const DEBUG_DUMP_IMAGE_FILE: u32 = 1027u32;
pub const DEBUG_DUMP_SMALL: u32 = 1024u32;
pub const DEBUG_DUMP_TRACE_LOG: u32 = 1028u32;
pub const DEBUG_DUMP_WINDOWS_CE: u32 = 1029u32;
pub const DEBUG_ECREATE_PROCESS_DEFAULT: u32 = 0u32;
pub const DEBUG_ECREATE_PROCESS_INHERIT_HANDLES: u32 = 1u32;
pub const DEBUG_ECREATE_PROCESS_USE_IMPLICIT_COMMAND_LINE: u32 = 4u32;
pub const DEBUG_ECREATE_PROCESS_USE_VERIFIER_FLAGS: u32 = 2u32;
pub const DEBUG_EINDEX_FROM_CURRENT: u32 = 2u32;
pub const DEBUG_EINDEX_FROM_END: u32 = 1u32;
pub const DEBUG_EINDEX_FROM_START: u32 = 0u32;
pub const DEBUG_EINDEX_NAME: u32 = 0u32;
pub const DEBUG_END_ACTIVE_DETACH: u32 = 2u32;
pub const DEBUG_END_ACTIVE_TERMINATE: u32 = 1u32;
pub const DEBUG_END_DISCONNECT: u32 = 4u32;
pub const DEBUG_END_PASSIVE: u32 = 0u32;
pub const DEBUG_END_REENTRANT: u32 = 3u32;
pub const DEBUG_ENGOPT_ALL: u32 = 32505855u32;
pub const DEBUG_ENGOPT_ALLOW_NETWORK_PATHS: u32 = 4u32;
pub const DEBUG_ENGOPT_ALLOW_READ_ONLY_BREAKPOINTS: u32 = 1024u32;
pub const DEBUG_ENGOPT_DEBUGGING_SENSITIVE_DATA: u32 = 4194304u32;
pub const DEBUG_ENGOPT_DISABLESQM: u32 = 524288u32;
pub const DEBUG_ENGOPT_DISABLE_EXECUTION_COMMANDS: u32 = 65536u32;
pub const DEBUG_ENGOPT_DISABLE_MANAGED_SUPPORT: u32 = 16384u32;
pub const DEBUG_ENGOPT_DISABLE_MODULE_SYMBOL_LOAD: u32 = 32768u32;
pub const DEBUG_ENGOPT_DISABLE_STEPLINES_OPTIONS: u32 = 2097152u32;
pub const DEBUG_ENGOPT_DISALLOW_IMAGE_FILE_MAPPING: u32 = 131072u32;
pub const DEBUG_ENGOPT_DISALLOW_NETWORK_PATHS: u32 = 8u32;
pub const DEBUG_ENGOPT_DISALLOW_SHELL_COMMANDS: u32 = 4096u32;
pub const DEBUG_ENGOPT_FAIL_INCOMPLETE_INFORMATION: u32 = 512u32;
pub const DEBUG_ENGOPT_FINAL_BREAK: u32 = 128u32;
pub const DEBUG_ENGOPT_IGNORE_DBGHELP_VERSION: u32 = 1u32;
pub const DEBUG_ENGOPT_IGNORE_EXTENSION_VERSIONS: u32 = 2u32;
pub const DEBUG_ENGOPT_IGNORE_LOADER_EXCEPTIONS: u32 = 16u32;
pub const DEBUG_ENGOPT_INITIAL_BREAK: u32 = 32u32;
pub const DEBUG_ENGOPT_INITIAL_MODULE_BREAK: u32 = 64u32;
pub const DEBUG_ENGOPT_KD_QUIET_MODE: u32 = 8192u32;
pub const DEBUG_ENGOPT_NO_EXECUTE_REPEAT: u32 = 256u32;
pub const DEBUG_ENGOPT_PREFER_DML: u32 = 262144u32;
pub const DEBUG_ENGOPT_PREFER_TRACE_FILES: u32 = 8388608u32;
pub const DEBUG_ENGOPT_RESOLVE_SHADOWED_VARIABLES: u32 = 16777216u32;
pub const DEBUG_ENGOPT_SYNCHRONIZE_BREAKPOINTS: u32 = 2048u32;
pub const DEBUG_EVENT_BREAKPOINT: u32 = 1u32;
pub const DEBUG_EVENT_CHANGE_DEBUGGEE_STATE: u32 = 1024u32;
pub const DEBUG_EVENT_CHANGE_ENGINE_STATE: u32 = 2048u32;
pub const DEBUG_EVENT_CHANGE_SYMBOL_STATE: u32 = 4096u32;
pub const DEBUG_EVENT_CREATE_PROCESS: u32 = 16u32;
pub const DEBUG_EVENT_CREATE_THREAD: u32 = 4u32;
pub const DEBUG_EVENT_EXCEPTION: u32 = 2u32;
pub const DEBUG_EVENT_EXIT_PROCESS: u32 = 32u32;
pub const DEBUG_EVENT_EXIT_THREAD: u32 = 8u32;
pub const DEBUG_EVENT_LOAD_MODULE: u32 = 64u32;
pub const DEBUG_EVENT_SERVICE_EXCEPTION: u32 = 8192u32;
pub const DEBUG_EVENT_SESSION_STATUS: u32 = 512u32;
pub const DEBUG_EVENT_SYSTEM_ERROR: u32 = 256u32;
pub const DEBUG_EVENT_UNLOAD_MODULE: u32 = 128u32;
pub const DEBUG_EXECUTE_DEFAULT: u32 = 0u32;
pub const DEBUG_EXECUTE_ECHO: u32 = 1u32;
pub const DEBUG_EXECUTE_EVENT: u32 = 2048u32;
pub const DEBUG_EXECUTE_EXTENSION: u32 = 32u32;
pub const DEBUG_EXECUTE_HOTKEY: u32 = 1024u32;
pub const DEBUG_EXECUTE_INTERNAL: u32 = 64u32;
pub const DEBUG_EXECUTE_MENU: u32 = 512u32;
pub const DEBUG_EXECUTE_NOT_LOGGED: u32 = 2u32;
pub const DEBUG_EXECUTE_NO_REPEAT: u32 = 4u32;
pub const DEBUG_EXECUTE_SCRIPT: u32 = 128u32;
pub const DEBUG_EXECUTE_TOOLBAR: u32 = 256u32;
pub const DEBUG_EXECUTE_USER_CLICKED: u32 = 16u32;
pub const DEBUG_EXECUTE_USER_TYPED: u32 = 8u32;
pub const DEBUG_EXEC_FLAGS_NONBLOCK: u32 = 1u32;
pub const DEBUG_EXPR_CPLUSPLUS: u32 = 1u32;
pub const DEBUG_EXPR_MASM: u32 = 0u32;
pub const DEBUG_EXTENSION_AT_ENGINE: u32 = 0u32;
pub const DEBUG_EXTINIT_HAS_COMMAND_HELP: u32 = 1u32;
pub const DEBUG_EXT_PVALUE_DEFAULT: u32 = 0u32;
pub const DEBUG_EXT_PVTYPE_IS_POINTER: u32 = 1u32;
pub const DEBUG_EXT_PVTYPE_IS_VALUE: u32 = 0u32;
pub const DEBUG_EXT_QVALUE_DEFAULT: u32 = 0u32;
pub const DEBUG_FA_ENTRY_ANSI_STRING: FA_ENTRY_TYPE = 5i32;
pub const DEBUG_FA_ENTRY_ANSI_STRINGs: FA_ENTRY_TYPE = 6i32;
pub const DEBUG_FA_ENTRY_ARRAY: FA_ENTRY_TYPE = 32768i32;
pub const DEBUG_FA_ENTRY_EXTENSION_CMD: FA_ENTRY_TYPE = 7i32;
pub const DEBUG_FA_ENTRY_INSTRUCTION_OFFSET: FA_ENTRY_TYPE = 3i32;
pub const DEBUG_FA_ENTRY_NO_TYPE: FA_ENTRY_TYPE = 0i32;
pub const DEBUG_FA_ENTRY_POINTER: FA_ENTRY_TYPE = 4i32;
pub const DEBUG_FA_ENTRY_STRUCTURED_DATA: FA_ENTRY_TYPE = 8i32;
pub const DEBUG_FA_ENTRY_ULONG: FA_ENTRY_TYPE = 1i32;
pub const DEBUG_FA_ENTRY_ULONG64: FA_ENTRY_TYPE = 2i32;
pub const DEBUG_FA_ENTRY_UNICODE_STRING: FA_ENTRY_TYPE = 9i32;
pub const DEBUG_FILTER_BREAK: u32 = 0u32;
pub const DEBUG_FILTER_CREATE_PROCESS: u32 = 2u32;
pub const DEBUG_FILTER_CREATE_THREAD: u32 = 0u32;
pub const DEBUG_FILTER_DEBUGGEE_OUTPUT: u32 = 9u32;
pub const DEBUG_FILTER_EXIT_PROCESS: u32 = 3u32;
pub const DEBUG_FILTER_EXIT_THREAD: u32 = 1u32;
pub const DEBUG_FILTER_GO_HANDLED: u32 = 0u32;
pub const DEBUG_FILTER_GO_NOT_HANDLED: u32 = 1u32;
pub const DEBUG_FILTER_IGNORE: u32 = 3u32;
pub const DEBUG_FILTER_INITIAL_BREAKPOINT: u32 = 7u32;
pub const DEBUG_FILTER_INITIAL_MODULE_LOAD: u32 = 8u32;
pub const DEBUG_FILTER_LOAD_MODULE: u32 = 4u32;
pub const DEBUG_FILTER_OUTPUT: u32 = 2u32;
pub const DEBUG_FILTER_REMOVE: u32 = 4u32;
pub const DEBUG_FILTER_SECOND_CHANCE_BREAK: u32 = 1u32;
pub const DEBUG_FILTER_SYSTEM_ERROR: u32 = 6u32;
pub const DEBUG_FILTER_UNLOAD_MODULE: u32 = 5u32;
pub const DEBUG_FIND_SOURCE_BEST_MATCH: u32 = 2u32;
pub const DEBUG_FIND_SOURCE_DEFAULT: u32 = 0u32;
pub const DEBUG_FIND_SOURCE_FULL_PATH: u32 = 1u32;
pub const DEBUG_FIND_SOURCE_NO_SRCSRV: u32 = 4u32;
pub const DEBUG_FIND_SOURCE_TOKEN_LOOKUP: u32 = 8u32;
pub const DEBUG_FIND_SOURCE_WITH_CHECKSUM: u32 = 16u32;
pub const DEBUG_FIND_SOURCE_WITH_CHECKSUM_STRICT: u32 = 32u32;
pub const DEBUG_FLR_ACPI: DEBUG_FLR_PARAM_TYPE = 24576i32;
pub const DEBUG_FLR_ACPI_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 24832i32;
pub const DEBUG_FLR_ACPI_EXTENSION: DEBUG_FLR_PARAM_TYPE = 11i32;
pub const DEBUG_FLR_ACPI_OBJECT: DEBUG_FLR_PARAM_TYPE = 13i32;
pub const DEBUG_FLR_ACPI_RESCONFLICT: DEBUG_FLR_PARAM_TYPE = 12i32;
pub const DEBUG_FLR_ADDITIONAL_DEBUGTEXT: DEBUG_FLR_PARAM_TYPE = 65546i32;
pub const DEBUG_FLR_ADDITIONAL_XML: DEBUG_FLR_PARAM_TYPE = 1150976i32;
pub const DEBUG_FLR_ADD_PROCESS_IN_BUCKET: DEBUG_FLR_PARAM_TYPE = 8219i32;
pub const DEBUG_FLR_ALUREON: DEBUG_FLR_PARAM_TYPE = 12372i32;
pub const DEBUG_FLR_ANALYSIS_REPROCESS: DEBUG_FLR_PARAM_TYPE = 1052705i32;
pub const DEBUG_FLR_ANALYSIS_SESSION_ELAPSED_TIME: DEBUG_FLR_PARAM_TYPE = 1052701i32;
pub const DEBUG_FLR_ANALYSIS_SESSION_HOST: DEBUG_FLR_PARAM_TYPE = 1052700i32;
pub const DEBUG_FLR_ANALYSIS_SESSION_TIME: DEBUG_FLR_PARAM_TYPE = 1052699i32;
pub const DEBUG_FLR_ANALYSIS_VERSION: DEBUG_FLR_PARAM_TYPE = 1052702i32;
pub const DEBUG_FLR_ANALYZABLE_POOL_CORRUPTION: DEBUG_FLR_PARAM_TYPE = 8202i32;
pub const DEBUG_FLR_APPKILL: DEBUG_FLR_PARAM_TYPE = 8212i32;
pub const DEBUG_FLR_APPLICATION_VERIFIER_LOADED: DEBUG_FLR_PARAM_TYPE = 1048626i32;
pub const DEBUG_FLR_APPS_NOT_TERMINATED: DEBUG_FLR_PARAM_TYPE = 8258i32;
pub const DEBUG_FLR_APPVERIFERFLAGS: DEBUG_FLR_PARAM_TYPE = 1048600i32;
pub const DEBUG_FLR_ARM_WRITE_AV_CAVEAT: DEBUG_FLR_PARAM_TYPE = 8241i32;
pub const DEBUG_FLR_ASSERT_DATA: DEBUG_FLR_PARAM_TYPE = 768i32;
pub const DEBUG_FLR_ASSERT_FILE: DEBUG_FLR_PARAM_TYPE = 769i32;
pub const DEBUG_FLR_ASSERT_INSTRUCTION: DEBUG_FLR_PARAM_TYPE = 778i32;
pub const DEBUG_FLR_BADPAGES_DETECTED: DEBUG_FLR_PARAM_TYPE = 4109i32;
pub const DEBUG_FLR_BAD_HANDLE: DEBUG_FLR_PARAM_TYPE = 17i32;
pub const DEBUG_FLR_BAD_MEMORY_REFERENCE: DEBUG_FLR_PARAM_TYPE = 8210i32;
pub const DEBUG_FLR_BAD_OBJECT_REFERENCE: DEBUG_FLR_PARAM_TYPE = 8211i32;
pub const DEBUG_FLR_BAD_STACK: DEBUG_FLR_PARAM_TYPE = 8193i32;
pub const DEBUG_FLR_BLOCKED_THREAD0: DEBUG_FLR_PARAM_TYPE = -1073741818i32;
pub const DEBUG_FLR_BLOCKED_THREAD1: DEBUG_FLR_PARAM_TYPE = -1073741817i32;
pub const DEBUG_FLR_BLOCKED_THREAD2: DEBUG_FLR_PARAM_TYPE = -1073741816i32;
pub const DEBUG_FLR_BLOCKING_PROCESSID: DEBUG_FLR_PARAM_TYPE = -1073741815i32;
pub const DEBUG_FLR_BLOCKING_THREAD: DEBUG_FLR_PARAM_TYPE = -1073741820i32;
pub const DEBUG_FLR_BOOST_FOLLOWUP_TO_SPECIFIC: DEBUG_FLR_PARAM_TYPE = 8222i32;
pub const DEBUG_FLR_BOOTSTAT: DEBUG_FLR_PARAM_TYPE = 28672i32;
pub const DEBUG_FLR_BOOTSTAT_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 28928i32;
pub const DEBUG_FLR_BUCKET_ID: DEBUG_FLR_PARAM_TYPE = 65536i32;
pub const DEBUG_FLR_BUCKET_ID_CHECKSUM: DEBUG_FLR_PARAM_TYPE = 1052684i32;
pub const DEBUG_FLR_BUCKET_ID_FLAVOR_STR: DEBUG_FLR_PARAM_TYPE = 1052686i32;
pub const DEBUG_FLR_BUCKET_ID_FUNCTION_STR: DEBUG_FLR_PARAM_TYPE = 1052676i32;
pub const DEBUG_FLR_BUCKET_ID_FUNC_OFFSET: DEBUG_FLR_PARAM_TYPE = 65589i32;
pub const DEBUG_FLR_BUCKET_ID_IMAGE_STR: DEBUG_FLR_PARAM_TYPE = 1052703i32;
pub const DEBUG_FLR_BUCKET_ID_MODULE_STR: DEBUG_FLR_PARAM_TYPE = 1052674i32;
pub const DEBUG_FLR_BUCKET_ID_MODVER_STR: DEBUG_FLR_PARAM_TYPE = 1052675i32;
pub const DEBUG_FLR_BUCKET_ID_OFFSET: DEBUG_FLR_PARAM_TYPE = 1052677i32;
pub const DEBUG_FLR_BUCKET_ID_PREFIX_STR: DEBUG_FLR_PARAM_TYPE = 1052673i32;
pub const DEBUG_FLR_BUCKET_ID_PRIVATE: DEBUG_FLR_PARAM_TYPE = 1052704i32;
pub const DEBUG_FLR_BUCKET_ID_TIMEDATESTAMP: DEBUG_FLR_PARAM_TYPE = 1052683i32;
pub const DEBUG_FLR_BUGCHECKING_DRIVER: DEBUG_FLR_PARAM_TYPE = 12292i32;
pub const DEBUG_FLR_BUGCHECKING_DRIVER_IDTAG: DEBUG_FLR_PARAM_TYPE = 65559i32;
pub const DEBUG_FLR_BUGCHECK_CODE: DEBUG_FLR_PARAM_TYPE = 4108i32;
pub const DEBUG_FLR_BUGCHECK_DESC: DEBUG_FLR_PARAM_TYPE = 1538i32;
pub const DEBUG_FLR_BUGCHECK_P1: DEBUG_FLR_PARAM_TYPE = 4115i32;
pub const DEBUG_FLR_BUGCHECK_P2: DEBUG_FLR_PARAM_TYPE = 4116i32;
pub const DEBUG_FLR_BUGCHECK_P3: DEBUG_FLR_PARAM_TYPE = 4117i32;
pub const DEBUG_FLR_BUGCHECK_P4: DEBUG_FLR_PARAM_TYPE = 4118i32;
pub const DEBUG_FLR_BUGCHECK_SPECIFIER: DEBUG_FLR_PARAM_TYPE = 1537i32;
pub const DEBUG_FLR_BUGCHECK_STR: DEBUG_FLR_PARAM_TYPE = 1536i32;
pub const DEBUG_FLR_BUILDNAME_IN_BUCKET: DEBUG_FLR_PARAM_TYPE = 12349i32;
pub const DEBUG_FLR_BUILDOSVER_STR_deprecated: DEBUG_FLR_PARAM_TYPE = 1052929i32;
pub const DEBUG_FLR_BUILD_OS_FULL_VERSION_STRING: DEBUG_FLR_PARAM_TYPE = 65567i32;
pub const DEBUG_FLR_BUILD_VERSION_STRING: DEBUG_FLR_PARAM_TYPE = 65566i32;
pub const DEBUG_FLR_CANCELLATION_NOT_SUPPORTED: DEBUG_FLR_PARAM_TYPE = 12350i32;
pub const DEBUG_FLR_CHKIMG_EXTENSION: DEBUG_FLR_PARAM_TYPE = 19i32;
pub const DEBUG_FLR_CHPE_PROCESS: DEBUG_FLR_PARAM_TYPE = -268435433i32;
pub const DEBUG_FLR_CLIENT_DRIVER: DEBUG_FLR_PARAM_TYPE = 1031i32;
pub const DEBUG_FLR_COLLECT_DATA_FOR_BUCKET: DEBUG_FLR_PARAM_TYPE = 65577i32;
pub const DEBUG_FLR_COMPUTER_NAME: DEBUG_FLR_PARAM_TYPE = 65578i32;
pub const DEBUG_FLR_CONTEXT: DEBUG_FLR_PARAM_TYPE = -1073741823i32;
pub const DEBUG_FLR_CONTEXT_COMMAND: DEBUG_FLR_PARAM_TYPE = 2097164i32;
pub const DEBUG_FLR_CONTEXT_FLAGS: DEBUG_FLR_PARAM_TYPE = 2097165i32;
pub const DEBUG_FLR_CONTEXT_FOLLOWUP_INDEX: DEBUG_FLR_PARAM_TYPE = 2097191i32;
pub const DEBUG_FLR_CONTEXT_ID: DEBUG_FLR_PARAM_TYPE = 2097168i32;
pub const DEBUG_FLR_CONTEXT_METADATA: DEBUG_FLR_PARAM_TYPE = 2097211i32;
pub const DEBUG_FLR_CONTEXT_ORDER: DEBUG_FLR_PARAM_TYPE = 2097166i32;
pub const DEBUG_FLR_CONTEXT_RESTORE_COMMAND: DEBUG_FLR_PARAM_TYPE = 65551i32;
pub const DEBUG_FLR_CONTEXT_SYSTEM: DEBUG_FLR_PARAM_TYPE = 2097167i32;
pub const DEBUG_FLR_CORRUPTING_POOL_ADDRESS: DEBUG_FLR_PARAM_TYPE = 1026i32;
pub const DEBUG_FLR_CORRUPTING_POOL_TAG: DEBUG_FLR_PARAM_TYPE = 1027i32;
pub const DEBUG_FLR_CORRUPT_MODULE_LIST: DEBUG_FLR_PARAM_TYPE = 8192i32;
pub const DEBUG_FLR_CORRUPT_SERVICE_TABLE: DEBUG_FLR_PARAM_TYPE = 12308i32;
pub const DEBUG_FLR_COVERAGE_BUILD: DEBUG_FLR_PARAM_TYPE = 8244i32;
pub const DEBUG_FLR_CPU_COUNT: DEBUG_FLR_PARAM_TYPE = 12330i32;
pub const DEBUG_FLR_CPU_FAMILY: DEBUG_FLR_PARAM_TYPE = 12333i32;
pub const DEBUG_FLR_CPU_MICROCODE_VERSION: DEBUG_FLR_PARAM_TYPE = 12329i32;
pub const DEBUG_FLR_CPU_MICROCODE_ZERO_INTEL: DEBUG_FLR_PARAM_TYPE = 8228i32;
pub const DEBUG_FLR_CPU_MODEL: DEBUG_FLR_PARAM_TYPE = 12334i32;
pub const DEBUG_FLR_CPU_OVERCLOCKED: DEBUG_FLR_PARAM_TYPE = 8198i32;
pub const DEBUG_FLR_CPU_SPEED: DEBUG_FLR_PARAM_TYPE = 12331i32;
pub const DEBUG_FLR_CPU_STEPPING: DEBUG_FLR_PARAM_TYPE = 12335i32;
pub const DEBUG_FLR_CPU_VENDOR: DEBUG_FLR_PARAM_TYPE = 12332i32;
pub const DEBUG_FLR_CRITICAL_PROCESS: DEBUG_FLR_PARAM_TYPE = 4119i32;
pub const DEBUG_FLR_CRITICAL_PROCESS_REPORTGUID: DEBUG_FLR_PARAM_TYPE = 65628i32;
pub const DEBUG_FLR_CRITICAL_SECTION: DEBUG_FLR_PARAM_TYPE = 16i32;
pub const DEBUG_FLR_CURRENT_IRQL: DEBUG_FLR_PARAM_TYPE = 512i32;
pub const DEBUG_FLR_CUSTOMER_CRASH_COUNT: DEBUG_FLR_PARAM_TYPE = 12299i32;
pub const DEBUG_FLR_CUSTOMREPORTTAG: DEBUG_FLR_PARAM_TYPE = -268435454i32;
pub const DEBUG_FLR_CUSTOM_ANALYSIS_TAG_MAX: DEBUG_FLR_PARAM_TYPE = -1342177280i32;
pub const DEBUG_FLR_CUSTOM_ANALYSIS_TAG_MIN: DEBUG_FLR_PARAM_TYPE = -1610612736i32;
pub const DEBUG_FLR_CUSTOM_COMMAND: DEBUG_FLR_PARAM_TYPE = -268435431i32;
pub const DEBUG_FLR_CUSTOM_COMMAND_OUTPUT: DEBUG_FLR_PARAM_TYPE = -268435430i32;
pub const DEBUG_FLR_DEADLOCK_INPROC: DEBUG_FLR_PARAM_TYPE = 1048589i32;
pub const DEBUG_FLR_DEADLOCK_XPROC: DEBUG_FLR_PARAM_TYPE = 1048590i32;
pub const DEBUG_FLR_DEBUG_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1118208i32;
pub const DEBUG_FLR_DEFAULT_BUCKET_ID: DEBUG_FLR_PARAM_TYPE = 65544i32;
pub const DEBUG_FLR_DEFAULT_SOLUTION_ID: DEBUG_FLR_PARAM_TYPE = 12294i32;
pub const DEBUG_FLR_DERIVED_WAIT_CHAIN: DEBUG_FLR_PARAM_TYPE = 1048583i32;
pub const DEBUG_FLR_DESKTOP_HEAP_MISSING: DEBUG_FLR_PARAM_TYPE = 1048593i32;
pub const DEBUG_FLR_DETOURED_IMAGE: DEBUG_FLR_PARAM_TYPE = 12351i32;
pub const DEBUG_FLR_DEVICE_NODE: DEBUG_FLR_PARAM_TYPE = 28i32;
pub const DEBUG_FLR_DEVICE_OBJECT: DEBUG_FLR_PARAM_TYPE = 3i32;
pub const DEBUG_FLR_DISKIO_READ_FAILURE: DEBUG_FLR_PARAM_TYPE = 12353i32;
pub const DEBUG_FLR_DISKIO_WRITE_FAILURE: DEBUG_FLR_PARAM_TYPE = 12354i32;
pub const DEBUG_FLR_DISKSEC_ISSUEDESCSTRING_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435450i32;
pub const DEBUG_FLR_DISKSEC_MFGID_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435451i32;
pub const DEBUG_FLR_DISKSEC_MODEL_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435452i32;
pub const DEBUG_FLR_DISKSEC_ORGID_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435453i32;
pub const DEBUG_FLR_DISKSEC_PRIVATE_DATASIZE_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435444i32;
pub const DEBUG_FLR_DISKSEC_PRIVATE_OFFSET_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435445i32;
pub const DEBUG_FLR_DISKSEC_PRIVATE_TOTSIZE_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435446i32;
pub const DEBUG_FLR_DISKSEC_PUBLIC_DATASIZE_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435447i32;
pub const DEBUG_FLR_DISKSEC_PUBLIC_OFFSET_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435448i32;
pub const DEBUG_FLR_DISKSEC_PUBLIC_TOTSIZE_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435449i32;
pub const DEBUG_FLR_DISKSEC_REASON_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435442i32;
pub const DEBUG_FLR_DISKSEC_TOTALSIZE_DEPRECATED: DEBUG_FLR_PARAM_TYPE = -268435443i32;
pub const DEBUG_FLR_DISK_HARDWARE_ERROR: DEBUG_FLR_PARAM_TYPE = 8206i32;
pub const DEBUG_FLR_DPC_RUNTIME: DEBUG_FLR_PARAM_TYPE = 4111i32;
pub const DEBUG_FLR_DPC_STACK_BASE: DEBUG_FLR_PARAM_TYPE = -1073741809i32;
pub const DEBUG_FLR_DPC_TIMELIMIT: DEBUG_FLR_PARAM_TYPE = 4112i32;
pub const DEBUG_FLR_DPC_TIMEOUT_TYPE: DEBUG_FLR_PARAM_TYPE = 4110i32;
pub const DEBUG_FLR_DRIVER_HARDWAREID: DEBUG_FLR_PARAM_TYPE = 65552i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_DEVICE_ID: DEBUG_FLR_PARAM_TYPE = 65554i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_DEVICE_NAME: DEBUG_FLR_PARAM_TYPE = 65633i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_ID_BUS_TYPE: DEBUG_FLR_PARAM_TYPE = 65557i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_REV_ID: DEBUG_FLR_PARAM_TYPE = 65556i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_SUBSYS_ID: DEBUG_FLR_PARAM_TYPE = 65555i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_SUBVENDOR_NAME: DEBUG_FLR_PARAM_TYPE = 65632i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_VENDOR_ID: DEBUG_FLR_PARAM_TYPE = 65553i32;
pub const DEBUG_FLR_DRIVER_HARDWARE_VENDOR_NAME: DEBUG_FLR_PARAM_TYPE = 65631i32;
pub const DEBUG_FLR_DRIVER_OBJECT: DEBUG_FLR_PARAM_TYPE = 2i32;
pub const DEBUG_FLR_DRIVER_VERIFIER_IO_VIOLATION_TYPE: DEBUG_FLR_PARAM_TYPE = 4096i32;
pub const DEBUG_FLR_DRIVER_XML_DESCRIPTION: DEBUG_FLR_PARAM_TYPE = 65562i32;
pub const DEBUG_FLR_DRIVER_XML_MANUFACTURER: DEBUG_FLR_PARAM_TYPE = 65564i32;
pub const DEBUG_FLR_DRIVER_XML_PRODUCTNAME: DEBUG_FLR_PARAM_TYPE = 65563i32;
pub const DEBUG_FLR_DRIVER_XML_VERSION: DEBUG_FLR_PARAM_TYPE = 65565i32;
pub const DEBUG_FLR_DRVPOWERSTATE_SUBCODE: DEBUG_FLR_PARAM_TYPE = 4101i32;
pub const DEBUG_FLR_DUMPSTREAM_COMMENTA: DEBUG_FLR_PARAM_TYPE = -268435435i32;
pub const DEBUG_FLR_DUMPSTREAM_COMMENTW: DEBUG_FLR_PARAM_TYPE = -268435434i32;
pub const DEBUG_FLR_DUMP_CLASS: DEBUG_FLR_PARAM_TYPE = 1048627i32;
pub const DEBUG_FLR_DUMP_FILE_ATTRIBUTES: DEBUG_FLR_PARAM_TYPE = 4113i32;
pub const DEBUG_FLR_DUMP_FLAGS: DEBUG_FLR_PARAM_TYPE = 1048625i32;
pub const DEBUG_FLR_DUMP_QUALIFIER: DEBUG_FLR_PARAM_TYPE = 1048628i32;
pub const DEBUG_FLR_DUMP_TYPE: DEBUG_FLR_PARAM_TYPE = 1048602i32;
pub const DEBUG_FLR_END_MESSAGE: DEBUG_FLR_PARAM_TYPE = 65612i32;
pub const DEBUG_FLR_ERESOURCE_ADDRESS: DEBUG_FLR_PARAM_TYPE = 22i32;
pub const DEBUG_FLR_EVENT_CODE_DATA_MISMATCH: DEBUG_FLR_PARAM_TYPE = 12338i32;
pub const DEBUG_FLR_EXCEPTION_CODE: DEBUG_FLR_PARAM_TYPE = 4097i32;
pub const DEBUG_FLR_EXCEPTION_CODE_STR: DEBUG_FLR_PARAM_TYPE = 4098i32;
pub const DEBUG_FLR_EXCEPTION_CODE_STR_deprecated: DEBUG_FLR_PARAM_TYPE = 1052672i32;
pub const DEBUG_FLR_EXCEPTION_CONTEXT_RECURSION: DEBUG_FLR_PARAM_TYPE = 12352i32;
pub const DEBUG_FLR_EXCEPTION_DOESNOT_MATCH_CODE: DEBUG_FLR_PARAM_TYPE = 777i32;
pub const DEBUG_FLR_EXCEPTION_MODULE_INFO: DEBUG_FLR_PARAM_TYPE = 2097190i32;
pub const DEBUG_FLR_EXCEPTION_PARAMETER1: DEBUG_FLR_PARAM_TYPE = 770i32;
pub const DEBUG_FLR_EXCEPTION_PARAMETER2: DEBUG_FLR_PARAM_TYPE = 771i32;
pub const DEBUG_FLR_EXCEPTION_PARAMETER3: DEBUG_FLR_PARAM_TYPE = 772i32;
pub const DEBUG_FLR_EXCEPTION_PARAMETER4: DEBUG_FLR_PARAM_TYPE = 773i32;
pub const DEBUG_FLR_EXCEPTION_RECORD: DEBUG_FLR_PARAM_TYPE = 774i32;
pub const DEBUG_FLR_EXCEPTION_STR: DEBUG_FLR_PARAM_TYPE = 776i32;
pub const DEBUG_FLR_EXECUTE_ADDRESS: DEBUG_FLR_PARAM_TYPE = 30i32;
pub const DEBUG_FLR_FAILED_INSTRUCTION_ADDRESS: DEBUG_FLR_PARAM_TYPE = 9i32;
pub const DEBUG_FLR_FAILURE_ANALYSIS_SOURCE: DEBUG_FLR_PARAM_TYPE = 65591i32;
pub const DEBUG_FLR_FAILURE_BUCKET_ID: DEBUG_FLR_PARAM_TYPE = 65561i32;
pub const DEBUG_FLR_FAILURE_DISPLAY_NAME: DEBUG_FLR_PARAM_TYPE = 2097239i32;
pub const DEBUG_FLR_FAILURE_EXCEPTION_CODE: DEBUG_FLR_PARAM_TYPE = 65607i32;
pub const DEBUG_FLR_FAILURE_FUNCTION_NAME: DEBUG_FLR_PARAM_TYPE = 65609i32;
pub const DEBUG_FLR_FAILURE_ID_HASH: DEBUG_FLR_PARAM_TYPE = 65592i32;
pub const DEBUG_FLR_FAILURE_ID_HASH_STRING: DEBUG_FLR_PARAM_TYPE = 65593i32;
pub const DEBUG_FLR_FAILURE_ID_REPORT_LINK: DEBUG_FLR_PARAM_TYPE = 65594i32;
pub const DEBUG_FLR_FAILURE_IMAGE_NAME: DEBUG_FLR_PARAM_TYPE = 65608i32;
pub const DEBUG_FLR_FAILURE_LIST: DEBUG_FLR_PARAM_TYPE = 2097238i32;
pub const DEBUG_FLR_FAILURE_MODULE_NAME: DEBUG_FLR_PARAM_TYPE = 65629i32;
pub const DEBUG_FLR_FAILURE_PROBLEM_CLASS: DEBUG_FLR_PARAM_TYPE = 65606i32;
pub const DEBUG_FLR_FAILURE_SYMBOL_NAME: DEBUG_FLR_PARAM_TYPE = 65610i32;
pub const DEBUG_FLR_FAULTING_INSTR_CODE: DEBUG_FLR_PARAM_TYPE = 12297i32;
pub const DEBUG_FLR_FAULTING_IP: DEBUG_FLR_PARAM_TYPE = -2147483648i32;
pub const DEBUG_FLR_FAULTING_LOCAL_VARIABLE_NAME: DEBUG_FLR_PARAM_TYPE = 1048623i32;
pub const DEBUG_FLR_FAULTING_MODULE: DEBUG_FLR_PARAM_TYPE = -2147483647i32;
pub const DEBUG_FLR_FAULTING_SERVICE_NAME: DEBUG_FLR_PARAM_TYPE = 65570i32;
pub const DEBUG_FLR_FAULTING_SOURCE_CODE: DEBUG_FLR_PARAM_TYPE = 65569i32;
pub const DEBUG_FLR_FAULTING_SOURCE_COMMIT_ID: DEBUG_FLR_PARAM_TYPE = 65634i32;
pub const DEBUG_FLR_FAULTING_SOURCE_CONTROL_TYPE: DEBUG_FLR_PARAM_TYPE = 65635i32;
pub const DEBUG_FLR_FAULTING_SOURCE_FILE: DEBUG_FLR_PARAM_TYPE = 65586i32;
pub const DEBUG_FLR_FAULTING_SOURCE_LINE: DEBUG_FLR_PARAM_TYPE = 65585i32;
pub const DEBUG_FLR_FAULTING_SOURCE_LINE_NUMBER: DEBUG_FLR_PARAM_TYPE = 65587i32;
pub const DEBUG_FLR_FAULTING_SOURCE_PROJECT: DEBUG_FLR_PARAM_TYPE = 65636i32;
pub const DEBUG_FLR_FAULTING_SOURCE_REPO_ID: DEBUG_FLR_PARAM_TYPE = 65637i32;
pub const DEBUG_FLR_FAULTING_SOURCE_REPO_URL: DEBUG_FLR_PARAM_TYPE = 65638i32;
pub const DEBUG_FLR_FAULTING_SOURCE_SRV_COMMAND: DEBUG_FLR_PARAM_TYPE = 65639i32;
pub const DEBUG_FLR_FAULTING_THREAD: DEBUG_FLR_PARAM_TYPE = -1073741824i32;
pub const DEBUG_FLR_FAULT_THREAD_SHA1_HASH_M: DEBUG_FLR_PARAM_TYPE = 1048597i32;
pub const DEBUG_FLR_FAULT_THREAD_SHA1_HASH_MF: DEBUG_FLR_PARAM_TYPE = 1048595i32;
pub const DEBUG_FLR_FAULT_THREAD_SHA1_HASH_MFO: DEBUG_FLR_PARAM_TYPE = 1048596i32;
pub const DEBUG_FLR_FA_ADHOC_ANALYSIS_ITEMS: DEBUG_FLR_PARAM_TYPE = 2097230i32;
pub const DEBUG_FLR_FA_PERF_DATA: DEBUG_FLR_PARAM_TYPE = 2097214i32;
pub const DEBUG_FLR_FA_PERF_ELAPSED_MS: DEBUG_FLR_PARAM_TYPE = 2097218i32;
pub const DEBUG_FLR_FA_PERF_ITEM: DEBUG_FLR_PARAM_TYPE = 2097215i32;
pub const DEBUG_FLR_FA_PERF_ITEM_NAME: DEBUG_FLR_PARAM_TYPE = 2097216i32;
pub const DEBUG_FLR_FA_PERF_ITERATIONS: DEBUG_FLR_PARAM_TYPE = 2097217i32;
pub const DEBUG_FLR_FEATURE_PATH: DEBUG_FLR_PARAM_TYPE = 65613i32;
pub const DEBUG_FLR_FILESYSTEMS_NTFS: DEBUG_FLR_PARAM_TYPE = 30208i32;
pub const DEBUG_FLR_FILESYSTEMS_NTFS_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 30448i32;
pub const DEBUG_FLR_FILESYSTEMS_REFS: DEBUG_FLR_PARAM_TYPE = 30720i32;
pub const DEBUG_FLR_FILESYSTEMS_REFS_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 30960i32;
pub const DEBUG_FLR_FILE_ID: DEBUG_FLR_PARAM_TYPE = 1280i32;
pub const DEBUG_FLR_FILE_IN_CAB: DEBUG_FLR_PARAM_TYPE = 65571i32;
pub const DEBUG_FLR_FILE_LINE: DEBUG_FLR_PARAM_TYPE = 1281i32;
pub const DEBUG_FLR_FIXED_IN_OSVERSION: DEBUG_FLR_PARAM_TYPE = 65543i32;
pub const DEBUG_FLR_FOLLOWUP_BEFORE_RETRACER: DEBUG_FLR_PARAM_TYPE = 65611i32;
pub const DEBUG_FLR_FOLLOWUP_BUCKET_ID: DEBUG_FLR_PARAM_TYPE = -2147483641i32;
pub const DEBUG_FLR_FOLLOWUP_CONTEXT: DEBUG_FLR_PARAM_TYPE = 2097153i32;
pub const DEBUG_FLR_FOLLOWUP_DRIVER_ONLY: DEBUG_FLR_PARAM_TYPE = 8196i32;
pub const DEBUG_FLR_FOLLOWUP_IP: DEBUG_FLR_PARAM_TYPE = -2147483645i32;
pub const DEBUG_FLR_FOLLOWUP_NAME: DEBUG_FLR_PARAM_TYPE = 65539i32;
pub const DEBUG_FLR_FRAME_ONE_INVALID: DEBUG_FLR_PARAM_TYPE = -2147483644i32;
pub const DEBUG_FLR_FRAME_SOURCE_FILE_NAME: DEBUG_FLR_PARAM_TYPE = 2097240i32;
pub const DEBUG_FLR_FRAME_SOURCE_FILE_PATH: DEBUG_FLR_PARAM_TYPE = 2097241i32;
pub const DEBUG_FLR_FRAME_SOURCE_LINE_NUMBER: DEBUG_FLR_PARAM_TYPE = 2097242i32;
pub const DEBUG_FLR_FREED_POOL_TAG: DEBUG_FLR_PARAM_TYPE = 1028i32;
pub const DEBUG_FLR_GSFAILURE_ANALYSIS_TEXT: DEBUG_FLR_PARAM_TYPE = 12323i32;
pub const DEBUG_FLR_GSFAILURE_COOKIES_MATCH_EXH: DEBUG_FLR_PARAM_TYPE = 12356i32;
pub const DEBUG_FLR_GSFAILURE_CORRUPTED_COOKIE: DEBUG_FLR_PARAM_TYPE = 12314i32;
pub const DEBUG_FLR_GSFAILURE_CORRUPTED_EBP: DEBUG_FLR_PARAM_TYPE = 12315i32;
pub const DEBUG_FLR_GSFAILURE_CORRUPTED_EBPESP: DEBUG_FLR_PARAM_TYPE = 12318i32;
pub const DEBUG_FLR_GSFAILURE_FALSE_POSITIVE: DEBUG_FLR_PARAM_TYPE = 8236i32;
pub const DEBUG_FLR_GSFAILURE_FRAME_COOKIE: DEBUG_FLR_PARAM_TYPE = 12312i32;
pub const DEBUG_FLR_GSFAILURE_FRAME_COOKIE_COMPLEMENT: DEBUG_FLR_PARAM_TYPE = 12313i32;
pub const DEBUG_FLR_GSFAILURE_FUNCTION: DEBUG_FLR_PARAM_TYPE = 12310i32;
pub const DEBUG_FLR_GSFAILURE_MANAGED: DEBUG_FLR_PARAM_TYPE = 12357i32;
pub const DEBUG_FLR_GSFAILURE_MANAGED_FRAMEID: DEBUG_FLR_PARAM_TYPE = 12360i32;
pub const DEBUG_FLR_GSFAILURE_MANAGED_THREADID: DEBUG_FLR_PARAM_TYPE = 12359i32;
pub const DEBUG_FLR_GSFAILURE_MEMORY_READ_ERROR: DEBUG_FLR_PARAM_TYPE = 12320i32;
pub const DEBUG_FLR_GSFAILURE_MISSING_ESTABLISHER_FRAME: DEBUG_FLR_PARAM_TYPE = 12355i32;
pub const DEBUG_FLR_GSFAILURE_MODULE_COOKIE: DEBUG_FLR_PARAM_TYPE = 12311i32;
pub const DEBUG_FLR_GSFAILURE_NOT_UP2DATE: DEBUG_FLR_PARAM_TYPE = 12326i32;
pub const DEBUG_FLR_GSFAILURE_OFF_BY_ONE_OVERRUN: DEBUG_FLR_PARAM_TYPE = 12324i32;
pub const DEBUG_FLR_GSFAILURE_OVERRUN_LOCAL: DEBUG_FLR_PARAM_TYPE = 12316i32;
pub const DEBUG_FLR_GSFAILURE_OVERRUN_LOCAL_NAME: DEBUG_FLR_PARAM_TYPE = 12317i32;
pub const DEBUG_FLR_GSFAILURE_POSITIVELY_CORRUPTED_EBPESP: DEBUG_FLR_PARAM_TYPE = 12319i32;
pub const DEBUG_FLR_GSFAILURE_POSITIVE_BUFFER_OVERFLOW: DEBUG_FLR_PARAM_TYPE = 12322i32;
pub const DEBUG_FLR_GSFAILURE_PROBABLY_NOT_USING_GS: DEBUG_FLR_PARAM_TYPE = 12321i32;
pub const DEBUG_FLR_GSFAILURE_RA_SMASHED: DEBUG_FLR_PARAM_TYPE = 12325i32;
pub const DEBUG_FLR_GSFAILURE_UP2DATE_UNKNOWN: DEBUG_FLR_PARAM_TYPE = 12327i32;
pub const DEBUG_FLR_HANDLE_VALUE: DEBUG_FLR_PARAM_TYPE = 24i32;
pub const DEBUG_FLR_HANG: DEBUG_FLR_PARAM_TYPE = 8209i32;
pub const DEBUG_FLR_HANG_DATA_NEEDED: DEBUG_FLR_PARAM_TYPE = 1048584i32;
pub const DEBUG_FLR_HANG_REPORT_THREAD_IS_IDLE: DEBUG_FLR_PARAM_TYPE = 1048594i32;
pub const DEBUG_FLR_HARDWARE_BUCKET_TAG: DEBUG_FLR_PARAM_TYPE = 65581i32;
pub const DEBUG_FLR_HARDWARE_ERROR: DEBUG_FLR_PARAM_TYPE = 8214i32;
pub const DEBUG_FLR_HIGH_NONPAGED_POOL_USAGE: DEBUG_FLR_PARAM_TYPE = 8255i32;
pub const DEBUG_FLR_HIGH_PAGED_POOL_USAGE: DEBUG_FLR_PARAM_TYPE = 8256i32;
pub const DEBUG_FLR_HIGH_PROCESS_COMMIT: DEBUG_FLR_PARAM_TYPE = 8253i32;
pub const DEBUG_FLR_HIGH_SERVICE_COMMIT: DEBUG_FLR_PARAM_TYPE = 8254i32;
pub const DEBUG_FLR_HIGH_SHARED_COMMIT_USAGE: DEBUG_FLR_PARAM_TYPE = 8257i32;
pub const DEBUG_FLR_HOLDINFO: DEBUG_FLR_PARAM_TYPE = 65595i32;
pub const DEBUG_FLR_HOLDINFO_ACTIVE_HOLD_COUNT: DEBUG_FLR_PARAM_TYPE = 65596i32;
pub const DEBUG_FLR_HOLDINFO_ALWAYS_HOLD: DEBUG_FLR_PARAM_TYPE = 65600i32;
pub const DEBUG_FLR_HOLDINFO_ALWAYS_IGNORE: DEBUG_FLR_PARAM_TYPE = 65599i32;
pub const DEBUG_FLR_HOLDINFO_HISTORIC_HOLD_COUNT: DEBUG_FLR_PARAM_TYPE = 65598i32;
pub const DEBUG_FLR_HOLDINFO_LAST_SEEN_HOLD_DATE: DEBUG_FLR_PARAM_TYPE = 65604i32;
pub const DEBUG_FLR_HOLDINFO_MANUAL_HOLD: DEBUG_FLR_PARAM_TYPE = 65602i32;
pub const DEBUG_FLR_HOLDINFO_MAX_HOLD_LIMIT: DEBUG_FLR_PARAM_TYPE = 65601i32;
pub const DEBUG_FLR_HOLDINFO_NOTIFICATION_ALIASES: DEBUG_FLR_PARAM_TYPE = 65603i32;
pub const DEBUG_FLR_HOLDINFO_RECOMMEND_HOLD: DEBUG_FLR_PARAM_TYPE = 65605i32;
pub const DEBUG_FLR_HOLDINFO_TENET_SOCRE: DEBUG_FLR_PARAM_TYPE = 65597i32;
pub const DEBUG_FLR_IGNORE_BUCKET_ID_OFFSET: DEBUG_FLR_PARAM_TYPE = 8238i32;
pub const DEBUG_FLR_IGNORE_LARGE_MODULE_CORRUPTION: DEBUG_FLR_PARAM_TYPE = 8237i32;
pub const DEBUG_FLR_IGNORE_MODULE_HARDWARE_ID: DEBUG_FLR_PARAM_TYPE = 8240i32;
pub const DEBUG_FLR_IMAGE_CLASS: DEBUG_FLR_PARAM_TYPE = 65579i32;
pub const DEBUG_FLR_IMAGE_NAME: DEBUG_FLR_PARAM_TYPE = 65537i32;
pub const DEBUG_FLR_IMAGE_TIMESTAMP: DEBUG_FLR_PARAM_TYPE = -2147483646i32;
pub const DEBUG_FLR_IMAGE_VERSION: DEBUG_FLR_PARAM_TYPE = -2147483642i32;
pub const DEBUG_FLR_INSTR_POINTER_CLIFAULT: DEBUG_FLR_PARAM_TYPE = 12306i32;
pub const DEBUG_FLR_INSTR_POINTER_IN_FREE_BLOCK: DEBUG_FLR_PARAM_TYPE = 12343i32;
pub const DEBUG_FLR_INSTR_POINTER_IN_MODULE_NOT_IN_LIST: DEBUG_FLR_PARAM_TYPE = 12346i32;
pub const DEBUG_FLR_INSTR_POINTER_IN_PAGED_CODE: DEBUG_FLR_PARAM_TYPE = 12370i32;
pub const DEBUG_FLR_INSTR_POINTER_IN_RESERVED_BLOCK: DEBUG_FLR_PARAM_TYPE = 12344i32;
pub const DEBUG_FLR_INSTR_POINTER_IN_UNLOADED_MODULE: DEBUG_FLR_PARAM_TYPE = 12340i32;
pub const DEBUG_FLR_INSTR_POINTER_IN_VM_MAPPED_MODULE: DEBUG_FLR_PARAM_TYPE = 12345i32;
pub const DEBUG_FLR_INSTR_POINTER_MISALIGNED: DEBUG_FLR_PARAM_TYPE = 12305i32;
pub const DEBUG_FLR_INSTR_POINTER_NOT_IN_STREAM: DEBUG_FLR_PARAM_TYPE = 12347i32;
pub const DEBUG_FLR_INSTR_POINTER_ON_HEAP: DEBUG_FLR_PARAM_TYPE = 12337i32;
pub const DEBUG_FLR_INSTR_POINTER_ON_STACK: DEBUG_FLR_PARAM_TYPE = 12336i32;
pub const DEBUG_FLR_INSTR_SESSION_POOL_TAG: DEBUG_FLR_PARAM_TYPE = 1030i32;
pub const DEBUG_FLR_INTEL_CPU_BIOS_UPGRADE_NEEDED: DEBUG_FLR_PARAM_TYPE = 8229i32;
pub const DEBUG_FLR_INTERNAL_BUCKET_CONTINUABLE: DEBUG_FLR_PARAM_TYPE = 16389i32;
pub const DEBUG_FLR_INTERNAL_BUCKET_HITCOUNT: DEBUG_FLR_PARAM_TYPE = 16387i32;
pub const DEBUG_FLR_INTERNAL_BUCKET_STATUS_TEXT: DEBUG_FLR_PARAM_TYPE = 16390i32;
pub const DEBUG_FLR_INTERNAL_BUCKET_URL: DEBUG_FLR_PARAM_TYPE = 16385i32;
pub const DEBUG_FLR_INTERNAL_RAID_BUG: DEBUG_FLR_PARAM_TYPE = 16384i32;
pub const DEBUG_FLR_INTERNAL_RAID_BUG_DATABASE_STRING: DEBUG_FLR_PARAM_TYPE = 16388i32;
pub const DEBUG_FLR_INTERNAL_RESPONSE: DEBUG_FLR_PARAM_TYPE = 65550i32;
pub const DEBUG_FLR_INTERNAL_SOLUTION_TEXT: DEBUG_FLR_PARAM_TYPE = 16386i32;
pub const DEBUG_FLR_INVALID: DEBUG_FLR_PARAM_TYPE = 0i32;
pub const DEBUG_FLR_INVALID_DPC_FOUND: DEBUG_FLR_PARAM_TYPE = 7i32;
pub const DEBUG_FLR_INVALID_HEAP_ADDRESS: DEBUG_FLR_PARAM_TYPE = 18i32;
pub const DEBUG_FLR_INVALID_KERNEL_CONTEXT: DEBUG_FLR_PARAM_TYPE = 8205i32;
pub const DEBUG_FLR_INVALID_OPCODE: DEBUG_FLR_PARAM_TYPE = 8218i32;
pub const DEBUG_FLR_INVALID_PFN: DEBUG_FLR_PARAM_TYPE = 4i32;
pub const DEBUG_FLR_INVALID_USEREVENT: DEBUG_FLR_PARAM_TYPE = 261i32;
pub const DEBUG_FLR_INVALID_USER_CONTEXT: DEBUG_FLR_PARAM_TYPE = 8231i32;
pub const DEBUG_FLR_IOCONTROL_CODE: DEBUG_FLR_PARAM_TYPE = 4099i32;
pub const DEBUG_FLR_IOSB_ADDRESS: DEBUG_FLR_PARAM_TYPE = 260i32;
pub const DEBUG_FLR_IO_ERROR_CODE: DEBUG_FLR_PARAM_TYPE = 775i32;
pub const DEBUG_FLR_IRP_ADDRESS: DEBUG_FLR_PARAM_TYPE = 256i32;
pub const DEBUG_FLR_IRP_CANCEL_ROUTINE: DEBUG_FLR_PARAM_TYPE = 259i32;
pub const DEBUG_FLR_IRP_MAJOR_FN: DEBUG_FLR_PARAM_TYPE = 257i32;
pub const DEBUG_FLR_IRP_MINOR_FN: DEBUG_FLR_PARAM_TYPE = 258i32;
pub const DEBUG_FLR_KERNEL: DEBUG_FAILURE_TYPE = 1i32;
pub const DEBUG_FLR_KERNEL_LOG_PROCESS_NAME: DEBUG_FLR_PARAM_TYPE = 65582i32;
pub const DEBUG_FLR_KERNEL_LOG_STATUS: DEBUG_FLR_PARAM_TYPE = 65583i32;
pub const DEBUG_FLR_KERNEL_VERIFIER_ENABLED: DEBUG_FLR_PARAM_TYPE = 8234i32;
pub const DEBUG_FLR_KEYVALUE_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1122304i32;
pub const DEBUG_FLR_KEY_VALUES_STRING: DEBUG_FLR_PARAM_TYPE = 1122560i32;
pub const DEBUG_FLR_KEY_VALUES_VARIANT: DEBUG_FLR_PARAM_TYPE = 1122816i32;
pub const DEBUG_FLR_KM_MODULE_LIST: DEBUG_FLR_PARAM_TYPE = 1048629i32;
pub const DEBUG_FLR_LARGE_TICK_INCREMENT: DEBUG_FLR_PARAM_TYPE = 12369i32;
pub const DEBUG_FLR_LAST_CONTROL_TRANSFER: DEBUG_FLR_PARAM_TYPE = 10i32;
pub const DEBUG_FLR_LCIE_ISO_AVAILABLE: DEBUG_FLR_PARAM_TYPE = 1048618i32;
pub const DEBUG_FLR_LEAKED_SESSION_POOL_TAG: DEBUG_FLR_PARAM_TYPE = 1029i32;
pub const DEBUG_FLR_LEGACY_PAGE_TABLE_ACCESS: DEBUG_FLR_PARAM_TYPE = 8252i32;
pub const DEBUG_FLR_LIVE_KERNEL_DUMP: DEBUG_FLR_PARAM_TYPE = 8243i32;
pub const DEBUG_FLR_LOADERLOCK_BLOCKED_API: DEBUG_FLR_PARAM_TYPE = 1048605i32;
pub const DEBUG_FLR_LOADERLOCK_IN_WAIT_CHAIN: DEBUG_FLR_PARAM_TYPE = 1048587i32;
pub const DEBUG_FLR_LOADERLOCK_OWNER_API: DEBUG_FLR_PARAM_TYPE = 1048604i32;
pub const DEBUG_FLR_LOP_STACKHASH: DEBUG_FLR_PARAM_TYPE = 12309i32;
pub const DEBUG_FLR_LOW_SYSTEM_COMMIT: DEBUG_FLR_PARAM_TYPE = 8251i32;
pub const DEBUG_FLR_MACHINE_INFO_SHA1_HASH: DEBUG_FLR_PARAM_TYPE = 1048608i32;
pub const DEBUG_FLR_MANAGED_ANALYSIS_PROVIDER: DEBUG_FLR_PARAM_TYPE = 1804i32;
pub const DEBUG_FLR_MANAGED_BITNESS_MISMATCH: DEBUG_FLR_PARAM_TYPE = 1797i32;
pub const DEBUG_FLR_MANAGED_CODE: DEBUG_FLR_PARAM_TYPE = 1792i32;
pub const DEBUG_FLR_MANAGED_ENGINE_MODULE: DEBUG_FLR_PARAM_TYPE = 1803i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_ADDRESS: DEBUG_FLR_PARAM_TYPE = 2048i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_CALLSTACK: DEBUG_FLR_PARAM_TYPE = 2052i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_CMD: DEBUG_FLR_PARAM_TYPE = 2288i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_CONTEXT_MESSAGE: DEBUG_FLR_PARAM_TYPE = 1799i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_HRESULT: DEBUG_FLR_PARAM_TYPE = 2049i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_INNER_ADDRESS: DEBUG_FLR_PARAM_TYPE = 2064i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_INNER_CALLSTACK: DEBUG_FLR_PARAM_TYPE = 2068i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_INNER_HRESULT: DEBUG_FLR_PARAM_TYPE = 2065i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_INNER_MESSAGE: DEBUG_FLR_PARAM_TYPE = 2067i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_INNER_TYPE: DEBUG_FLR_PARAM_TYPE = 2066i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_MESSAGE: DEBUG_FLR_PARAM_TYPE = 2051i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_MESSAGE_deprecated: DEBUG_FLR_PARAM_TYPE = 1795i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_NESTED_ADDRESS: DEBUG_FLR_PARAM_TYPE = 2080i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_NESTED_CALLSTACK: DEBUG_FLR_PARAM_TYPE = 2084i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_NESTED_HRESULT: DEBUG_FLR_PARAM_TYPE = 2081i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_NESTED_MESSAGE: DEBUG_FLR_PARAM_TYPE = 2083i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_NESTED_TYPE: DEBUG_FLR_PARAM_TYPE = 2082i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_OBJECT: DEBUG_FLR_PARAM_TYPE = 1794i32;
pub const DEBUG_FLR_MANAGED_EXCEPTION_TYPE: DEBUG_FLR_PARAM_TYPE = 2050i32;
pub const DEBUG_FLR_MANAGED_FRAME_CHAIN_CORRUPTION: DEBUG_FLR_PARAM_TYPE = 12358i32;
pub const DEBUG_FLR_MANAGED_HRESULT_STRING: DEBUG_FLR_PARAM_TYPE = 1802i32;
pub const DEBUG_FLR_MANAGED_KERNEL_DEBUGGER: DEBUG_FLR_PARAM_TYPE = 1801i32;
pub const DEBUG_FLR_MANAGED_OBJECT: DEBUG_FLR_PARAM_TYPE = 1793i32;
pub const DEBUG_FLR_MANAGED_OBJECT_NAME: DEBUG_FLR_PARAM_TYPE = 1798i32;
pub const DEBUG_FLR_MANAGED_STACK_COMMAND: DEBUG_FLR_PARAM_TYPE = 1800i32;
pub const DEBUG_FLR_MANAGED_STACK_STRING: DEBUG_FLR_PARAM_TYPE = 1796i32;
pub const DEBUG_FLR_MANAGED_THREAD_CMD_CALLSTACK: DEBUG_FLR_PARAM_TYPE = 2544i32;
pub const DEBUG_FLR_MANAGED_THREAD_CMD_STACKOBJECTS: DEBUG_FLR_PARAM_TYPE = 2545i32;
pub const DEBUG_FLR_MANAGED_THREAD_ID: DEBUG_FLR_PARAM_TYPE = 2304i32;
pub const DEBUG_FLR_MANUAL_BREAKIN: DEBUG_FLR_PARAM_TYPE = 8208i32;
pub const DEBUG_FLR_MARKER_BUCKET: DEBUG_FLR_PARAM_TYPE = 65560i32;
pub const DEBUG_FLR_MARKER_FILE: DEBUG_FLR_PARAM_TYPE = 65549i32;
pub const DEBUG_FLR_MARKER_MODULE_FILE: DEBUG_FLR_PARAM_TYPE = 65558i32;
pub const DEBUG_FLR_MASK_ALL: DEBUG_FLR_PARAM_TYPE = -1i32;
pub const DEBUG_FLR_MEMDIAG_LASTRUN_STATUS: DEBUG_FLR_PARAM_TYPE = 12341i32;
pub const DEBUG_FLR_MEMDIAG_LASTRUN_TIME: DEBUG_FLR_PARAM_TYPE = 12342i32;
pub const DEBUG_FLR_MEMORY_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1134592i32;
pub const DEBUG_FLR_MEMORY_CORRUPTION_SIGNATURE: DEBUG_FLR_PARAM_TYPE = 12348i32;
pub const DEBUG_FLR_MEMORY_CORRUPTOR: DEBUG_FLR_PARAM_TYPE = 12289i32;
pub const DEBUG_FLR_MILCORE_BREAK: DEBUG_FLR_PARAM_TYPE = 8232i32;
pub const DEBUG_FLR_MINUTES_SINCE_LAST_EVENT: DEBUG_FLR_PARAM_TYPE = 1879048225i32;
pub const DEBUG_FLR_MINUTES_SINCE_LAST_EVENT_OF_THIS_TYPE: DEBUG_FLR_PARAM_TYPE = 1879048226i32;
pub const DEBUG_FLR_MISSING_CLR_SYMBOL: DEBUG_FLR_PARAM_TYPE = 8249i32;
pub const DEBUG_FLR_MISSING_IMPORTANT_SYMBOL: DEBUG_FLR_PARAM_TYPE = 8248i32;
pub const DEBUG_FLR_MM_INTERNAL_CODE: DEBUG_FLR_PARAM_TYPE = 4100i32;
pub const DEBUG_FLR_MODLIST_SHA1_HASH: DEBUG_FLR_PARAM_TYPE = 1048601i32;
pub const DEBUG_FLR_MODLIST_TSCHKSUM_SHA1_HASH: DEBUG_FLR_PARAM_TYPE = 1048606i32;
pub const DEBUG_FLR_MODLIST_UNLOADED_SHA1_HASH: DEBUG_FLR_PARAM_TYPE = 1048607i32;
pub const DEBUG_FLR_MODULE_BUCKET_ID: DEBUG_FLR_PARAM_TYPE = 65545i32;
pub const DEBUG_FLR_MODULE_LIST: DEBUG_FLR_PARAM_TYPE = 1048624i32;
pub const DEBUG_FLR_MODULE_NAME: DEBUG_FLR_PARAM_TYPE = 65542i32;
pub const DEBUG_FLR_MODULE_PRODUCTNAME: DEBUG_FLR_PARAM_TYPE = 65576i32;
pub const DEBUG_FLR_MOD_SPECIFIC_DATA_ONLY: DEBUG_FLR_PARAM_TYPE = 8226i32;
pub const DEBUG_FLR_NO_ARCH_IN_BUCKET: DEBUG_FLR_PARAM_TYPE = 8239i32;
pub const DEBUG_FLR_NO_BUGCHECK_IN_BUCKET: DEBUG_FLR_PARAM_TYPE = 8216i32;
pub const DEBUG_FLR_NO_IMAGE_IN_BUCKET: DEBUG_FLR_PARAM_TYPE = 8215i32;
pub const DEBUG_FLR_NO_IMAGE_TIMESTAMP_IN_BUCKET: DEBUG_FLR_PARAM_TYPE = 8233i32;
pub const DEBUG_FLR_NTGLOBALFLAG: DEBUG_FLR_PARAM_TYPE = 1048599i32;
pub const DEBUG_FLR_ON_DPC_STACK: DEBUG_FLR_PARAM_TYPE = 8242i32;
pub const DEBUG_FLR_ORIGINAL_CAB_NAME: DEBUG_FLR_PARAM_TYPE = 65568i32;
pub const DEBUG_FLR_OSBUILD_deprecated: DEBUG_FLR_PARAM_TYPE = 1052928i32;
pub const DEBUG_FLR_OS_BRANCH: DEBUG_FLR_PARAM_TYPE = 1052680i32;
pub const DEBUG_FLR_OS_BUILD: DEBUG_FLR_PARAM_TYPE = 1052678i32;
pub const DEBUG_FLR_OS_BUILD_LAYERS_XML: DEBUG_FLR_PARAM_TYPE = 1052711i32;
pub const DEBUG_FLR_OS_BUILD_STRING: DEBUG_FLR_PARAM_TYPE = 1052708i32;
pub const DEBUG_FLR_OS_BUILD_TIMESTAMP_ISO: DEBUG_FLR_PARAM_TYPE = 1052697i32;
pub const DEBUG_FLR_OS_BUILD_TIMESTAMP_LAB: DEBUG_FLR_PARAM_TYPE = 1052681i32;
pub const DEBUG_FLR_OS_FLAVOR: DEBUG_FLR_PARAM_TYPE = 1052685i32;
pub const DEBUG_FLR_OS_LOCALE: DEBUG_FLR_PARAM_TYPE = 1052696i32;
pub const DEBUG_FLR_OS_LOCALE_LCID: DEBUG_FLR_PARAM_TYPE = 1052709i32;
pub const DEBUG_FLR_OS_MAJOR: DEBUG_FLR_PARAM_TYPE = 1052706i32;
pub const DEBUG_FLR_OS_MINOR: DEBUG_FLR_PARAM_TYPE = 1052707i32;
pub const DEBUG_FLR_OS_NAME: DEBUG_FLR_PARAM_TYPE = 1052692i32;
pub const DEBUG_FLR_OS_NAME_EDITION: DEBUG_FLR_PARAM_TYPE = 1052693i32;
pub const DEBUG_FLR_OS_PLATFORM_ARCH: DEBUG_FLR_PARAM_TYPE = 1052694i32;
pub const DEBUG_FLR_OS_PLATFORM_ID: DEBUG_FLR_PARAM_TYPE = 1052710i32;
pub const DEBUG_FLR_OS_PRODUCT_TYPE: DEBUG_FLR_PARAM_TYPE = 1052688i32;
pub const DEBUG_FLR_OS_REVISION: DEBUG_FLR_PARAM_TYPE = 1052691i32;
pub const DEBUG_FLR_OS_SERVICEPACK: DEBUG_FLR_PARAM_TYPE = 1052679i32;
pub const DEBUG_FLR_OS_SERVICEPACK_deprecated: DEBUG_FLR_PARAM_TYPE = 1052695i32;
pub const DEBUG_FLR_OS_SKU: DEBUG_FLR_PARAM_TYPE = 1052687i32;
pub const DEBUG_FLR_OS_SUITE_MASK: DEBUG_FLR_PARAM_TYPE = 1052689i32;
pub const DEBUG_FLR_OS_VERSION: DEBUG_FLR_PARAM_TYPE = 1052682i32;
pub const DEBUG_FLR_OS_VERSION_deprecated: DEBUG_FLR_PARAM_TYPE = 12291i32;
pub const DEBUG_FLR_OVERLAPPED_MODULE: DEBUG_FLR_PARAM_TYPE = 8227i32;
pub const DEBUG_FLR_OVERLAPPED_UNLOADED_MODULE: DEBUG_FLR_PARAM_TYPE = 8230i32;
pub const DEBUG_FLR_PAGE_HASH_ERRORS: DEBUG_FLR_PARAM_TYPE = 4114i32;
pub const DEBUG_FLR_PG_MISMATCH: DEBUG_FLR_PARAM_TYPE = 27i32;
pub const DEBUG_FLR_PHONE_APPID: DEBUG_FLR_PARAM_TYPE = 1879048215i32;
pub const DEBUG_FLR_PHONE_APPVERSION: DEBUG_FLR_PARAM_TYPE = 1879048217i32;
pub const DEBUG_FLR_PHONE_BOOTLOADERVERSION: DEBUG_FLR_PARAM_TYPE = 1879048209i32;
pub const DEBUG_FLR_PHONE_BUILDBRANCH: DEBUG_FLR_PARAM_TYPE = 1879048196i32;
pub const DEBUG_FLR_PHONE_BUILDER: DEBUG_FLR_PARAM_TYPE = 1879048197i32;
pub const DEBUG_FLR_PHONE_BUILDNUMBER: DEBUG_FLR_PARAM_TYPE = 1879048194i32;
pub const DEBUG_FLR_PHONE_BUILDTIMESTAMP: DEBUG_FLR_PARAM_TYPE = 1879048195i32;
pub const DEBUG_FLR_PHONE_FIRMWAREREVISION: DEBUG_FLR_PARAM_TYPE = 1879048202i32;
pub const DEBUG_FLR_PHONE_HARDWAREREVISION: DEBUG_FLR_PARAM_TYPE = 1879048206i32;
pub const DEBUG_FLR_PHONE_LCID: DEBUG_FLR_PARAM_TYPE = 1879048198i32;
pub const DEBUG_FLR_PHONE_MCCMNC: DEBUG_FLR_PARAM_TYPE = 1879048201i32;
pub const DEBUG_FLR_PHONE_OPERATOR: DEBUG_FLR_PARAM_TYPE = 1879048200i32;
pub const DEBUG_FLR_PHONE_QFE: DEBUG_FLR_PARAM_TYPE = 1879048199i32;
pub const DEBUG_FLR_PHONE_RADIOHARDWAREREVISION: DEBUG_FLR_PARAM_TYPE = 1879048207i32;
pub const DEBUG_FLR_PHONE_RADIOSOFTWAREREVISION: DEBUG_FLR_PARAM_TYPE = 1879048208i32;
pub const DEBUG_FLR_PHONE_RAM: DEBUG_FLR_PARAM_TYPE = 1879048203i32;
pub const DEBUG_FLR_PHONE_REPORTGUID: DEBUG_FLR_PARAM_TYPE = 1879048210i32;
pub const DEBUG_FLR_PHONE_REPORTTIMESTAMP: DEBUG_FLR_PARAM_TYPE = 1879048214i32;
pub const DEBUG_FLR_PHONE_ROMVERSION: DEBUG_FLR_PARAM_TYPE = 1879048204i32;
pub const DEBUG_FLR_PHONE_SKUID: DEBUG_FLR_PARAM_TYPE = 1879048216i32;
pub const DEBUG_FLR_PHONE_SOCVERSION: DEBUG_FLR_PARAM_TYPE = 1879048205i32;
pub const DEBUG_FLR_PHONE_SOURCE: DEBUG_FLR_PARAM_TYPE = 1879048211i32;
pub const DEBUG_FLR_PHONE_SOURCEEXTERNAL: DEBUG_FLR_PARAM_TYPE = 1879048212i32;
pub const DEBUG_FLR_PHONE_UIF_APPID: DEBUG_FLR_PARAM_TYPE = 1879048220i32;
pub const DEBUG_FLR_PHONE_UIF_APPNAME: DEBUG_FLR_PARAM_TYPE = 1879048219i32;
pub const DEBUG_FLR_PHONE_UIF_CATEGORY: DEBUG_FLR_PARAM_TYPE = 1879048221i32;
pub const DEBUG_FLR_PHONE_UIF_COMMENT: DEBUG_FLR_PARAM_TYPE = 1879048218i32;
pub const DEBUG_FLR_PHONE_UIF_ORIGIN: DEBUG_FLR_PARAM_TYPE = 1879048222i32;
pub const DEBUG_FLR_PHONE_USERALIAS: DEBUG_FLR_PARAM_TYPE = 1879048213i32;
pub const DEBUG_FLR_PHONE_VERSIONMAJOR: DEBUG_FLR_PARAM_TYPE = 1879048192i32;
pub const DEBUG_FLR_PHONE_VERSIONMINOR: DEBUG_FLR_PARAM_TYPE = 1879048193i32;
pub const DEBUG_FLR_PLATFORM_BUCKET_STRING: DEBUG_FLR_PARAM_TYPE = 65630i32;
pub const DEBUG_FLR_PNP: DEBUG_FLR_PARAM_TYPE = 32768i32;
pub const DEBUG_FLR_PNP_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 33024i32;
pub const DEBUG_FLR_PNP_IRP_ADDRESS: DEBUG_FLR_PARAM_TYPE = 32770i32;
pub const DEBUG_FLR_PNP_IRP_ADDRESS_DEPRECATED: DEBUG_FLR_PARAM_TYPE = 264i32;
pub const DEBUG_FLR_PNP_TRIAGE_DATA: DEBUG_FLR_PARAM_TYPE = 32769i32;
pub const DEBUG_FLR_PNP_TRIAGE_DATA_DEPRECATED: DEBUG_FLR_PARAM_TYPE = 23i32;
pub const DEBUG_FLR_POISONED_TB: DEBUG_FLR_PARAM_TYPE = 8200i32;
pub const DEBUG_FLR_POOL_ADDRESS: DEBUG_FLR_PARAM_TYPE = 1024i32;
pub const DEBUG_FLR_POOL_CORRUPTOR: DEBUG_FLR_PARAM_TYPE = 12288i32;
pub const DEBUG_FLR_POSSIBLE_INVALID_CONTROL_TRANSFER: DEBUG_FLR_PARAM_TYPE = 8199i32;
pub const DEBUG_FLR_POSSIBLE_STACK_OVERFLOW: DEBUG_FLR_PARAM_TYPE = 8245i32;
pub const DEBUG_FLR_POWERREQUEST_ADDRESS: DEBUG_FLR_PARAM_TYPE = 29i32;
pub const DEBUG_FLR_PO_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 24833i32;
pub const DEBUG_FLR_PREVIOUS_IRQL: DEBUG_FLR_PARAM_TYPE = 513i32;
pub const DEBUG_FLR_PREVIOUS_MODE: DEBUG_FLR_PARAM_TYPE = 265i32;
pub const DEBUG_FLR_PRIMARY_PROBLEM_CLASS: DEBUG_FLR_PARAM_TYPE = 1048579i32;
pub const DEBUG_FLR_PRIMARY_PROBLEM_CLASS_DATA: DEBUG_FLR_PARAM_TYPE = 1048580i32;
pub const DEBUG_FLR_PROBLEM_CLASSES: DEBUG_FLR_PARAM_TYPE = 1048578i32;
pub const DEBUG_FLR_PROBLEM_CODE_PATH_HASH: DEBUG_FLR_PARAM_TYPE = 1048585i32;
pub const DEBUG_FLR_PROCESSES_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1142784i32;
pub const DEBUG_FLR_PROCESSOR_ID: DEBUG_FLR_PARAM_TYPE = -1073741814i32;
pub const DEBUG_FLR_PROCESSOR_INFO: DEBUG_FLR_PARAM_TYPE = 12339i32;
pub const DEBUG_FLR_PROCESS_BAM_CURRENT_THROTTLED: DEBUG_FLR_PARAM_TYPE = -268435437i32;
pub const DEBUG_FLR_PROCESS_BAM_PREVIOUS_THROTTLED: DEBUG_FLR_PARAM_TYPE = -268435436i32;
pub const DEBUG_FLR_PROCESS_INFO: DEBUG_FLR_PARAM_TYPE = 2097189i32;
pub const DEBUG_FLR_PROCESS_NAME: DEBUG_FLR_PARAM_TYPE = 65547i32;
pub const DEBUG_FLR_PROCESS_OBJECT: DEBUG_FLR_PARAM_TYPE = 8i32;
pub const DEBUG_FLR_PROCESS_PRODUCTNAME: DEBUG_FLR_PARAM_TYPE = 65575i32;
pub const DEBUG_FLR_RAISED_IRQL_USER_FAULT: DEBUG_FLR_PARAM_TYPE = 8220i32;
pub const DEBUG_FLR_READ_ADDRESS: DEBUG_FLR_PARAM_TYPE = 14i32;
pub const DEBUG_FLR_RECURRING_STACK: DEBUG_FLR_PARAM_TYPE = 12296i32;
pub const DEBUG_FLR_REGISTRYTXT_SOURCE: DEBUG_FLR_PARAM_TYPE = 65584i32;
pub const DEBUG_FLR_REGISTRYTXT_STRESS_ID: DEBUG_FLR_PARAM_TYPE = 12307i32;
pub const DEBUG_FLR_REGISTRY_DATA: DEBUG_FLR_PARAM_TYPE = 3145728i32;
pub const DEBUG_FLR_REPORT_INFO_CREATION_TIME: DEBUG_FLR_PARAM_TYPE = 1879048229i32;
pub const DEBUG_FLR_REPORT_INFO_GUID: DEBUG_FLR_PARAM_TYPE = 1879048227i32;
pub const DEBUG_FLR_REPORT_INFO_SOURCE: DEBUG_FLR_PARAM_TYPE = 1879048228i32;
pub const DEBUG_FLR_REQUESTED_IRQL: DEBUG_FLR_PARAM_TYPE = 514i32;
pub const DEBUG_FLR_RESERVED: DEBUG_FLR_PARAM_TYPE = 1i32;
pub const DEBUG_FLR_RESOURCE_CALL_TYPE: DEBUG_FLR_PARAM_TYPE = 4352i32;
pub const DEBUG_FLR_RESOURCE_CALL_TYPE_STR: DEBUG_FLR_PARAM_TYPE = 4353i32;
pub const DEBUG_FLR_SCM: DEBUG_FLR_PARAM_TYPE = 20992i32;
pub const DEBUG_FLR_SCM_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 21232i32;
pub const DEBUG_FLR_SCM_BLACKBOX_ENTRY: DEBUG_FLR_PARAM_TYPE = 21233i32;
pub const DEBUG_FLR_SCM_BLACKBOX_ENTRY_CONTROLCODE: DEBUG_FLR_PARAM_TYPE = 21234i32;
pub const DEBUG_FLR_SCM_BLACKBOX_ENTRY_SERVICENAME: DEBUG_FLR_PARAM_TYPE = 21236i32;
pub const DEBUG_FLR_SCM_BLACKBOX_ENTRY_STARTTIME: DEBUG_FLR_PARAM_TYPE = 21235i32;
pub const DEBUG_FLR_SEARCH_HANG: DEBUG_FLR_PARAM_TYPE = 1048614i32;
pub const DEBUG_FLR_SECURITY_COOKIES: DEBUG_FLR_PARAM_TYPE = 4105i32;
pub const DEBUG_FLR_SERVICE: DEBUG_FLR_PARAM_TYPE = 20480i32;
pub const DEBUG_FLR_SERVICETABLE_MODIFIED: DEBUG_FLR_PARAM_TYPE = 12371i32;
pub const DEBUG_FLR_SERVICE_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1146880i32;
pub const DEBUG_FLR_SERVICE_DEPENDONGROUP: DEBUG_FLR_PARAM_TYPE = 20486i32;
pub const DEBUG_FLR_SERVICE_DEPENDONSERVICE: DEBUG_FLR_PARAM_TYPE = 20485i32;
pub const DEBUG_FLR_SERVICE_DESCRIPTION: DEBUG_FLR_PARAM_TYPE = 20484i32;
pub const DEBUG_FLR_SERVICE_DISPLAYNAME: DEBUG_FLR_PARAM_TYPE = 20483i32;
pub const DEBUG_FLR_SERVICE_GROUP: DEBUG_FLR_PARAM_TYPE = 20482i32;
pub const DEBUG_FLR_SERVICE_NAME: DEBUG_FLR_PARAM_TYPE = 20481i32;
pub const DEBUG_FLR_SHOW_ERRORLOG: DEBUG_FLR_PARAM_TYPE = 8207i32;
pub const DEBUG_FLR_SHOW_LCIE_ISO_DATA: DEBUG_FLR_PARAM_TYPE = 1048619i32;
pub const DEBUG_FLR_SIMULTANEOUS_TELSVC_INSTANCES: DEBUG_FLR_PARAM_TYPE = 1879048223i32;
pub const DEBUG_FLR_SIMULTANEOUS_TELWP_INSTANCES: DEBUG_FLR_PARAM_TYPE = 1879048224i32;
pub const DEBUG_FLR_SINGLE_BIT_ERROR: DEBUG_FLR_PARAM_TYPE = 8203i32;
pub const DEBUG_FLR_SINGLE_BIT_PFN_PAGE_ERROR: DEBUG_FLR_PARAM_TYPE = 8213i32;
pub const DEBUG_FLR_SKIP_CORRUPT_MODULE_DETECTION: DEBUG_FLR_PARAM_TYPE = 8235i32;
pub const DEBUG_FLR_SKIP_MODULE_SPECIFIC_BUCKET_INFO: DEBUG_FLR_PARAM_TYPE = 65588i32;
pub const DEBUG_FLR_SKIP_STACK_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 8217i32;
pub const DEBUG_FLR_SM_BUFFER_HASH: DEBUG_FLR_PARAM_TYPE = 1342177286i32;
pub const DEBUG_FLR_SM_COMPRESSION_FORMAT: DEBUG_FLR_PARAM_TYPE = 1342177280i32;
pub const DEBUG_FLR_SM_ONEBIT_SOLUTION_COUNT: DEBUG_FLR_PARAM_TYPE = 1342177287i32;
pub const DEBUG_FLR_SM_SOURCE_OFFSET: DEBUG_FLR_PARAM_TYPE = 1342177283i32;
pub const DEBUG_FLR_SM_SOURCE_PFN1: DEBUG_FLR_PARAM_TYPE = 1342177281i32;
pub const DEBUG_FLR_SM_SOURCE_PFN2: DEBUG_FLR_PARAM_TYPE = 1342177282i32;
pub const DEBUG_FLR_SM_SOURCE_SIZE: DEBUG_FLR_PARAM_TYPE = 1342177284i32;
pub const DEBUG_FLR_SM_TARGET_PFN: DEBUG_FLR_PARAM_TYPE = 1342177285i32;
pub const DEBUG_FLR_SOLUTION_ID: DEBUG_FLR_PARAM_TYPE = 12293i32;
pub const DEBUG_FLR_SOLUTION_TYPE: DEBUG_FLR_PARAM_TYPE = 12295i32;
pub const DEBUG_FLR_SPECIAL_POOL_CORRUPTION_TYPE: DEBUG_FLR_PARAM_TYPE = 1025i32;
pub const DEBUG_FLR_STACK: DEBUG_FLR_PARAM_TYPE = 2097152i32;
pub const DEBUG_FLR_STACKHASH_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1138688i32;
pub const DEBUG_FLR_STACKUSAGE_FUNCTION: DEBUG_FLR_PARAM_TYPE = 12363i32;
pub const DEBUG_FLR_STACKUSAGE_FUNCTION_SIZE: DEBUG_FLR_PARAM_TYPE = 12364i32;
pub const DEBUG_FLR_STACKUSAGE_IMAGE: DEBUG_FLR_PARAM_TYPE = 12361i32;
pub const DEBUG_FLR_STACKUSAGE_IMAGE_SIZE: DEBUG_FLR_PARAM_TYPE = 12362i32;
pub const DEBUG_FLR_STACKUSAGE_RECURSION_COUNT: DEBUG_FLR_PARAM_TYPE = 12365i32;
pub const DEBUG_FLR_STACK_COMMAND: DEBUG_FLR_PARAM_TYPE = 65540i32;
pub const DEBUG_FLR_STACK_FRAME: DEBUG_FLR_PARAM_TYPE = 2097155i32;
pub const DEBUG_FLR_STACK_FRAMES: DEBUG_FLR_PARAM_TYPE = 2097212i32;
pub const DEBUG_FLR_STACK_FRAME_FLAGS: DEBUG_FLR_PARAM_TYPE = 2097163i32;
pub const DEBUG_FLR_STACK_FRAME_FUNCTION: DEBUG_FLR_PARAM_TYPE = 2097162i32;
pub const DEBUG_FLR_STACK_FRAME_IMAGE: DEBUG_FLR_PARAM_TYPE = 2097161i32;
pub const DEBUG_FLR_STACK_FRAME_INSTRUCTION: DEBUG_FLR_PARAM_TYPE = 2097157i32;
pub const DEBUG_FLR_STACK_FRAME_MODULE: DEBUG_FLR_PARAM_TYPE = 2097160i32;
pub const DEBUG_FLR_STACK_FRAME_MODULE_BASE: DEBUG_FLR_PARAM_TYPE = 2097224i32;
pub const DEBUG_FLR_STACK_FRAME_NUMBER: DEBUG_FLR_PARAM_TYPE = 2097156i32;
pub const DEBUG_FLR_STACK_FRAME_SRC: DEBUG_FLR_PARAM_TYPE = 2097225i32;
pub const DEBUG_FLR_STACK_FRAME_SYMBOL: DEBUG_FLR_PARAM_TYPE = 2097158i32;
pub const DEBUG_FLR_STACK_FRAME_SYMBOL_OFFSET: DEBUG_FLR_PARAM_TYPE = 2097159i32;
pub const DEBUG_FLR_STACK_OVERFLOW: DEBUG_FLR_PARAM_TYPE = 12301i32;
pub const DEBUG_FLR_STACK_POINTER_ERROR: DEBUG_FLR_PARAM_TYPE = 12302i32;
pub const DEBUG_FLR_STACK_POINTER_MISALIGNED: DEBUG_FLR_PARAM_TYPE = 12304i32;
pub const DEBUG_FLR_STACK_POINTER_ONEBIT_ERROR: DEBUG_FLR_PARAM_TYPE = 12303i32;
pub const DEBUG_FLR_STACK_SHA1_HASH_M: DEBUG_FLR_PARAM_TYPE = 2097221i32;
pub const DEBUG_FLR_STACK_SHA1_HASH_MF: DEBUG_FLR_PARAM_TYPE = 2097219i32;
pub const DEBUG_FLR_STACK_SHA1_HASH_MFO: DEBUG_FLR_PARAM_TYPE = 2097220i32;
pub const DEBUG_FLR_STACK_TEXT: DEBUG_FLR_PARAM_TYPE = 65541i32;
pub const DEBUG_FLR_STATUS_CODE: DEBUG_FLR_PARAM_TYPE = 4102i32;
pub const DEBUG_FLR_STORAGE: DEBUG_FLR_PARAM_TYPE = 29696i32;
pub const DEBUG_FLR_STORAGE_BLACKBOX: DEBUG_FLR_PARAM_TYPE = 29936i32;
pub const DEBUG_FLR_STORAGE_ISSUEDESCSTRING: DEBUG_FLR_PARAM_TYPE = 29700i32;
pub const DEBUG_FLR_STORAGE_MFGID: DEBUG_FLR_PARAM_TYPE = 29699i32;
pub const DEBUG_FLR_STORAGE_MODEL: DEBUG_FLR_PARAM_TYPE = 29698i32;
pub const DEBUG_FLR_STORAGE_ORGID: DEBUG_FLR_PARAM_TYPE = 29697i32;
pub const DEBUG_FLR_STORAGE_PRIVATE_DATASIZE: DEBUG_FLR_PARAM_TYPE = 29706i32;
pub const DEBUG_FLR_STORAGE_PRIVATE_OFFSET: DEBUG_FLR_PARAM_TYPE = 29705i32;
pub const DEBUG_FLR_STORAGE_PRIVATE_TOTSIZE: DEBUG_FLR_PARAM_TYPE = 29704i32;
pub const DEBUG_FLR_STORAGE_PUBLIC_DATASIZE: DEBUG_FLR_PARAM_TYPE = 29703i32;
pub const DEBUG_FLR_STORAGE_PUBLIC_OFFSET: DEBUG_FLR_PARAM_TYPE = 29702i32;
pub const DEBUG_FLR_STORAGE_PUBLIC_TOTSIZE: DEBUG_FLR_PARAM_TYPE = 29701i32;
pub const DEBUG_FLR_STORAGE_REASON: DEBUG_FLR_PARAM_TYPE = 29708i32;
pub const DEBUG_FLR_STORAGE_TOTALSIZE: DEBUG_FLR_PARAM_TYPE = 29707i32;
pub const DEBUG_FLR_STORE_DEVELOPER_NAME: DEBUG_FLR_PARAM_TYPE = 1610612743i32;
pub const DEBUG_FLR_STORE_IS_MICROSOFT_PRODUCT: DEBUG_FLR_PARAM_TYPE = 1610612754i32;
pub const DEBUG_FLR_STORE_LEGACY_PARENT_PRODUCT_ID: DEBUG_FLR_PARAM_TYPE = 1610612747i32;
pub const DEBUG_FLR_STORE_LEGACY_WINDOWS_PHONE_PRODUCT_ID: DEBUG_FLR_PARAM_TYPE = 1610612749i32;
pub const DEBUG_FLR_STORE_LEGACY_WINDOWS_STORE_PRODUCT_ID: DEBUG_FLR_PARAM_TYPE = 1610612748i32;
pub const DEBUG_FLR_STORE_LEGACY_XBOX_360_PRODUCT_ID: DEBUG_FLR_PARAM_TYPE = 1610612751i32;
pub const DEBUG_FLR_STORE_LEGACY_XBOX_ONE_PRODUCT_ID: DEBUG_FLR_PARAM_TYPE = 1610612750i32;
pub const DEBUG_FLR_STORE_PACKAGE_FAMILY_NAME: DEBUG_FLR_PARAM_TYPE = 1610612744i32;
pub const DEBUG_FLR_STORE_PACKAGE_IDENTITY_NAME: DEBUG_FLR_PARAM_TYPE = 1610612745i32;
pub const DEBUG_FLR_STORE_PREFERRED_SKU_ID: DEBUG_FLR_PARAM_TYPE = 1610612753i32;
pub const DEBUG_FLR_STORE_PRIMARY_PARENT_PRODUCT_ID: DEBUG_FLR_PARAM_TYPE = 1610612746i32;
pub const DEBUG_FLR_STORE_PRODUCT_DESCRIPTION: DEBUG_FLR_PARAM_TYPE = 1610612738i32;
pub const DEBUG_FLR_STORE_PRODUCT_DISPLAY_NAME: DEBUG_FLR_PARAM_TYPE = 1610612737i32;
pub const DEBUG_FLR_STORE_PRODUCT_EXTENDED_NAME: DEBUG_FLR_PARAM_TYPE = 1610612739i32;
pub const DEBUG_FLR_STORE_PRODUCT_ID: DEBUG_FLR_PARAM_TYPE = 1610612736i32;
pub const DEBUG_FLR_STORE_PUBLISHER_CERTIFICATE_NAME: DEBUG_FLR_PARAM_TYPE = 1610612742i32;
pub const DEBUG_FLR_STORE_PUBLISHER_ID: DEBUG_FLR_PARAM_TYPE = 1610612740i32;
pub const DEBUG_FLR_STORE_PUBLISHER_NAME: DEBUG_FLR_PARAM_TYPE = 1610612741i32;
pub const DEBUG_FLR_STORE_URL_APP: DEBUG_FLR_PARAM_TYPE = 1610612755i32;
pub const DEBUG_FLR_STORE_URL_APPHEALTH: DEBUG_FLR_PARAM_TYPE = 1610612756i32;
pub const DEBUG_FLR_STORE_XBOX_TITLE_ID: DEBUG_FLR_PARAM_TYPE = 1610612752i32;
pub const DEBUG_FLR_STREAM_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1130496i32;
pub const DEBUG_FLR_SUSPECT_CODE_PATH_HASH: DEBUG_FLR_PARAM_TYPE = 1048586i32;
pub const DEBUG_FLR_SVCHOST: DEBUG_FLR_PARAM_TYPE = 20736i32;
pub const DEBUG_FLR_SVCHOST_GROUP: DEBUG_FLR_PARAM_TYPE = 20737i32;
pub const DEBUG_FLR_SVCHOST_IMAGEPATH: DEBUG_FLR_PARAM_TYPE = 20738i32;
pub const DEBUG_FLR_SVCHOST_SERVICEDLL: DEBUG_FLR_PARAM_TYPE = 20739i32;
pub const DEBUG_FLR_SWITCH_PROCESS_CONTEXT: DEBUG_FLR_PARAM_TYPE = 8223i32;
pub const DEBUG_FLR_SYMBOL_FROM_RAW_STACK_ADDRESS: DEBUG_FLR_PARAM_TYPE = -2147483643i32;
pub const DEBUG_FLR_SYMBOL_NAME: DEBUG_FLR_PARAM_TYPE = 65538i32;
pub const DEBUG_FLR_SYMBOL_ON_RAW_STACK: DEBUG_FLR_PARAM_TYPE = 4104i32;
pub const DEBUG_FLR_SYMBOL_ROUTINE_NAME: DEBUG_FLR_PARAM_TYPE = 65580i32;
pub const DEBUG_FLR_SYMBOL_STACK_INDEX: DEBUG_FLR_PARAM_TYPE = 4103i32;
pub const DEBUG_FLR_SYSINFO_BASEBOARD_MANUFACTURER: DEBUG_FLR_PARAM_TYPE = 17156i32;
pub const DEBUG_FLR_SYSINFO_BASEBOARD_PRODUCT: DEBUG_FLR_PARAM_TYPE = 17157i32;
pub const DEBUG_FLR_SYSINFO_BASEBOARD_VERSION: DEBUG_FLR_PARAM_TYPE = 17158i32;
pub const DEBUG_FLR_SYSINFO_BIOS_DATE: DEBUG_FLR_PARAM_TYPE = 17161i32;
pub const DEBUG_FLR_SYSINFO_BIOS_VENDOR: DEBUG_FLR_PARAM_TYPE = 17159i32;
pub const DEBUG_FLR_SYSINFO_BIOS_VERSION: DEBUG_FLR_PARAM_TYPE = 17160i32;
pub const DEBUG_FLR_SYSINFO_SYSTEM_MANUFACTURER: DEBUG_FLR_PARAM_TYPE = 17152i32;
pub const DEBUG_FLR_SYSINFO_SYSTEM_PRODUCT: DEBUG_FLR_PARAM_TYPE = 17153i32;
pub const DEBUG_FLR_SYSINFO_SYSTEM_SKU: DEBUG_FLR_PARAM_TYPE = 17154i32;
pub const DEBUG_FLR_SYSINFO_SYSTEM_VERSION: DEBUG_FLR_PARAM_TYPE = 17155i32;
pub const DEBUG_FLR_SYSTEM_LOCALE_deprecated: DEBUG_FLR_PARAM_TYPE = 12298i32;
pub const DEBUG_FLR_SYSXML_CHECKSUM: DEBUG_FLR_PARAM_TYPE = 16897i32;
pub const DEBUG_FLR_SYSXML_LOCALEID: DEBUG_FLR_PARAM_TYPE = 16896i32;
pub const DEBUG_FLR_TARGET_MODE: DEBUG_FLR_PARAM_TYPE = 4107i32;
pub const DEBUG_FLR_TARGET_TIME: DEBUG_FLR_PARAM_TYPE = 8250i32;
pub const DEBUG_FLR_TESTRESULTGUID: DEBUG_FLR_PARAM_TYPE = -268435455i32;
pub const DEBUG_FLR_TESTRESULTSERVER: DEBUG_FLR_PARAM_TYPE = -268435456i32;
pub const DEBUG_FLR_THREADPOOL_WAITER: DEBUG_FLR_PARAM_TYPE = 4106i32;
pub const DEBUG_FLR_THREAD_ATTRIBUTES: DEBUG_FLR_PARAM_TYPE = 1048577i32;
pub const DEBUG_FLR_TIMELINE_ANALYSIS: DEBUG_FLR_PARAM_TYPE = 1126400i32;
pub const DEBUG_FLR_TIMELINE_TIMES: DEBUG_FLR_PARAM_TYPE = 1126401i32;
pub const DEBUG_FLR_TRAP_FRAME: DEBUG_FLR_PARAM_TYPE = -1073741822i32;
pub const DEBUG_FLR_TRAP_FRAME_RECURSION: DEBUG_FLR_PARAM_TYPE = 12300i32;
pub const DEBUG_FLR_TRIAGER_OS_BUILD_NAME: DEBUG_FLR_PARAM_TYPE = 12328i32;
pub const DEBUG_FLR_TSS: DEBUG_FLR_PARAM_TYPE = -1073741821i32;
pub const DEBUG_FLR_TWO_BIT_ERROR: DEBUG_FLR_PARAM_TYPE = 8204i32;
pub const DEBUG_FLR_ULS_SCRIPT_EXCEPTION: DEBUG_FLR_PARAM_TYPE = 1048617i32;
pub const DEBUG_FLR_UNALIGNED_STACK_POINTER: DEBUG_FLR_PARAM_TYPE = 12290i32;
pub const DEBUG_FLR_UNKNOWN: DEBUG_FAILURE_TYPE = 0i32;
pub const DEBUG_FLR_UNKNOWN_MODULE: DEBUG_FLR_PARAM_TYPE = 8201i32;
pub const DEBUG_FLR_UNRESPONSIVE_UI_FOLLOWUP_NAME: DEBUG_FLR_PARAM_TYPE = 65573i32;
pub const DEBUG_FLR_UNRESPONSIVE_UI_PROBLEM_CLASS: DEBUG_FLR_PARAM_TYPE = 1048581i32;
pub const DEBUG_FLR_UNRESPONSIVE_UI_PROBLEM_CLASS_DATA: DEBUG_FLR_PARAM_TYPE = 1048582i32;
pub const DEBUG_FLR_UNRESPONSIVE_UI_STACK: DEBUG_FLR_PARAM_TYPE = 65574i32;
pub const DEBUG_FLR_UNRESPONSIVE_UI_SYMBOL_NAME: DEBUG_FLR_PARAM_TYPE = 65572i32;
pub const DEBUG_FLR_UNRESPONSIVE_UI_THREAD: DEBUG_FLR_PARAM_TYPE = -1073741819i32;
pub const DEBUG_FLR_UNUSED001: DEBUG_FLR_PARAM_TYPE = 8197i32;
pub const DEBUG_FLR_URLS: DEBUG_FLR_PARAM_TYPE = 1048610i32;
pub const DEBUG_FLR_URLS_DISCOVERED: DEBUG_FLR_PARAM_TYPE = 1048609i32;
pub const DEBUG_FLR_URL_ENTRY: DEBUG_FLR_PARAM_TYPE = 1048611i32;
pub const DEBUG_FLR_URL_LCIE_ENTRY: DEBUG_FLR_PARAM_TYPE = 1048620i32;
pub const DEBUG_FLR_URL_URLMON_ENTRY: DEBUG_FLR_PARAM_TYPE = 1048621i32;
pub const DEBUG_FLR_URL_XMLHTTPREQ_SYNC_ENTRY: DEBUG_FLR_PARAM_TYPE = 1048622i32;
pub const DEBUG_FLR_USBPORT_OCADATA: DEBUG_FLR_PARAM_TYPE = 20i32;
pub const DEBUG_FLR_USER: DEBUG_FAILURE_TYPE = 2i32;
pub const DEBUG_FLR_USERBREAK_PEB_PAGEDOUT: DEBUG_FLR_PARAM_TYPE = 8225i32;
pub const DEBUG_FLR_USERMODE_DATA: DEBUG_FLR_PARAM_TYPE = 1048576i32;
pub const DEBUG_FLR_USER_GLOBAL_ATTRIBUTES: DEBUG_FLR_PARAM_TYPE = 3153920i32;
pub const DEBUG_FLR_USER_LCID: DEBUG_FLR_PARAM_TYPE = 1052690i32;
pub const DEBUG_FLR_USER_LCID_STR: DEBUG_FLR_PARAM_TYPE = 1052698i32;
pub const DEBUG_FLR_USER_MODE_BUCKET: DEBUG_FLR_PARAM_TYPE = 65614i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_EVENTTYPE: DEBUG_FLR_PARAM_TYPE = 65616i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_INDEX: DEBUG_FLR_PARAM_TYPE = 65615i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P0: DEBUG_FLR_PARAM_TYPE = 65619i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P1: DEBUG_FLR_PARAM_TYPE = 65620i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P2: DEBUG_FLR_PARAM_TYPE = 65621i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P3: DEBUG_FLR_PARAM_TYPE = 65622i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P4: DEBUG_FLR_PARAM_TYPE = 65623i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P5: DEBUG_FLR_PARAM_TYPE = 65624i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P6: DEBUG_FLR_PARAM_TYPE = 65625i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_P7: DEBUG_FLR_PARAM_TYPE = 65626i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_REPORTCREATIONTIME: DEBUG_FLR_PARAM_TYPE = 65618i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_REPORTGUID: DEBUG_FLR_PARAM_TYPE = 65617i32;
pub const DEBUG_FLR_USER_MODE_BUCKET_STRING: DEBUG_FLR_PARAM_TYPE = 65627i32;
pub const DEBUG_FLR_USER_NAME: DEBUG_FLR_PARAM_TYPE = 65548i32;
pub const DEBUG_FLR_USER_PROBLEM_CLASSES: DEBUG_FLR_PARAM_TYPE = 3162112i32;
pub const DEBUG_FLR_USER_THREAD_ATTRIBUTES: DEBUG_FLR_PARAM_TYPE = 3158016i32;
pub const DEBUG_FLR_USE_DEFAULT_CONTEXT: DEBUG_FLR_PARAM_TYPE = 8221i32;
pub const DEBUG_FLR_VERIFIER_DRIVER_ENTRY: DEBUG_FLR_PARAM_TYPE = 263i32;
pub const DEBUG_FLR_VERIFIER_FOUND_DEADLOCK: DEBUG_FLR_PARAM_TYPE = 26i32;
pub const DEBUG_FLR_VERIFIER_STOP: DEBUG_FLR_PARAM_TYPE = 8224i32;
pub const DEBUG_FLR_VIDEO_TDR_CONTEXT: DEBUG_FLR_PARAM_TYPE = 262i32;
pub const DEBUG_FLR_VIRTUAL_MACHINE: DEBUG_FLR_PARAM_TYPE = 17162i32;
pub const DEBUG_FLR_WAIT_CHAIN_COMMAND: DEBUG_FLR_PARAM_TYPE = 1048598i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_00: DEBUG_FLR_PARAM_TYPE = 16648i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_01: DEBUG_FLR_PARAM_TYPE = 16649i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_02: DEBUG_FLR_PARAM_TYPE = 16650i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_03: DEBUG_FLR_PARAM_TYPE = 16651i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_04: DEBUG_FLR_PARAM_TYPE = 16652i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_05: DEBUG_FLR_PARAM_TYPE = 16653i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_06: DEBUG_FLR_PARAM_TYPE = 16654i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_07: DEBUG_FLR_PARAM_TYPE = 16655i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_08: DEBUG_FLR_PARAM_TYPE = 16656i32;
pub const DEBUG_FLR_WATSON_GENERIC_BUCKETING_09: DEBUG_FLR_PARAM_TYPE = 16657i32;
pub const DEBUG_FLR_WATSON_GENERIC_EVENT_NAME: DEBUG_FLR_PARAM_TYPE = 16647i32;
pub const DEBUG_FLR_WATSON_IBUCKET: DEBUG_FLR_PARAM_TYPE = 16644i32;
pub const DEBUG_FLR_WATSON_IBUCKETTABLE_S1_RESP: DEBUG_FLR_PARAM_TYPE = 1048613i32;
pub const DEBUG_FLR_WATSON_IBUCKET_S1_RESP: DEBUG_FLR_PARAM_TYPE = 1048612i32;
pub const DEBUG_FLR_WATSON_MODULE: DEBUG_FLR_PARAM_TYPE = 16640i32;
pub const DEBUG_FLR_WATSON_MODULE_OFFSET: DEBUG_FLR_PARAM_TYPE = 16642i32;
pub const DEBUG_FLR_WATSON_MODULE_TIMESTAMP: DEBUG_FLR_PARAM_TYPE = 16645i32;
pub const DEBUG_FLR_WATSON_MODULE_VERSION: DEBUG_FLR_PARAM_TYPE = 16641i32;
pub const DEBUG_FLR_WATSON_PROCESS_TIMESTAMP: DEBUG_FLR_PARAM_TYPE = 16646i32;
pub const DEBUG_FLR_WATSON_PROCESS_VERSION: DEBUG_FLR_PARAM_TYPE = 16643i32;
pub const DEBUG_FLR_WCT_XML_AVAILABLE: DEBUG_FLR_PARAM_TYPE = 1048591i32;
pub const DEBUG_FLR_WERCOLLECTION_DEFAULTCOLLECTION_FAILURE: DEBUG_FLR_PARAM_TYPE = -268435438i32;
pub const DEBUG_FLR_WERCOLLECTION_MINIDUMP_WRITE_FAILURE: DEBUG_FLR_PARAM_TYPE = -268435439i32;
pub const DEBUG_FLR_WERCOLLECTION_PROCESSHEAPDUMP_REQUEST_FAILURE: DEBUG_FLR_PARAM_TYPE = -268435440i32;
pub const DEBUG_FLR_WERCOLLECTION_PROCESSTERMINATED: DEBUG_FLR_PARAM_TYPE = -268435441i32;
pub const DEBUG_FLR_WER_DATA_COLLECTION_INFO: DEBUG_FLR_PARAM_TYPE = 1048615i32;
pub const DEBUG_FLR_WER_MACHINE_ID: DEBUG_FLR_PARAM_TYPE = 1048616i32;
pub const DEBUG_FLR_WHEA_ERROR_RECORD: DEBUG_FLR_PARAM_TYPE = 25i32;
pub const DEBUG_FLR_WINLOGON_BLACKBOX: DEBUG_FLR_PARAM_TYPE = -268435432i32;
pub const DEBUG_FLR_WMI_QUERY_DATA: DEBUG_FLR_PARAM_TYPE = 3149824i32;
pub const DEBUG_FLR_WORKER_ROUTINE: DEBUG_FLR_PARAM_TYPE = 5i32;
pub const DEBUG_FLR_WORK_ITEM: DEBUG_FLR_PARAM_TYPE = 6i32;
pub const DEBUG_FLR_WORK_QUEUE_ITEM: DEBUG_FLR_PARAM_TYPE = 21i32;
pub const DEBUG_FLR_WQL_EVENTLOG_INFO: DEBUG_FLR_PARAM_TYPE = 16899i32;
pub const DEBUG_FLR_WQL_EVENT_COUNT: DEBUG_FLR_PARAM_TYPE = 16898i32;
pub const DEBUG_FLR_WRITE_ADDRESS: DEBUG_FLR_PARAM_TYPE = 15i32;
pub const DEBUG_FLR_WRONG_SYMBOLS: DEBUG_FLR_PARAM_TYPE = 8195i32;
pub const DEBUG_FLR_WRONG_SYMBOLS_SIZE: DEBUG_FLR_PARAM_TYPE = 8247i32;
pub const DEBUG_FLR_WRONG_SYMBOLS_TIMESTAMP: DEBUG_FLR_PARAM_TYPE = 8246i32;
pub const DEBUG_FLR_XBOX_LIVE_ENVIRONMENT: DEBUG_FLR_PARAM_TYPE = 12368i32;
pub const DEBUG_FLR_XBOX_SYSTEM_CRASHTIME: DEBUG_FLR_PARAM_TYPE = 12367i32;
pub const DEBUG_FLR_XBOX_SYSTEM_UPTIME: DEBUG_FLR_PARAM_TYPE = 12366i32;
pub const DEBUG_FLR_XCS_PATH: DEBUG_FLR_PARAM_TYPE = 1048603i32;
pub const DEBUG_FLR_XDV_HELP_LINK: DEBUG_FLR_PARAM_TYPE = -1073741811i32;
pub const DEBUG_FLR_XDV_RULE_INFO: DEBUG_FLR_PARAM_TYPE = -1073741810i32;
pub const DEBUG_FLR_XDV_STATE_VARIABLE: DEBUG_FLR_PARAM_TYPE = -1073741812i32;
pub const DEBUG_FLR_XDV_VIOLATED_CONDITION: DEBUG_FLR_PARAM_TYPE = -1073741813i32;
pub const DEBUG_FLR_XHCI_FIRMWARE_VERSION: DEBUG_FLR_PARAM_TYPE = 65590i32;
pub const DEBUG_FLR_XML_APPLICATION_NAME: DEBUG_FLR_PARAM_TYPE = 2097231i32;
pub const DEBUG_FLR_XML_ATTRIBUTE: DEBUG_FLR_PARAM_TYPE = 2097194i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_D1VALUE: DEBUG_FLR_PARAM_TYPE = 2097197i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_D2VALUE: DEBUG_FLR_PARAM_TYPE = 2097198i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_DOVALUE: DEBUG_FLR_PARAM_TYPE = 2097199i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_FRAME_NUMBER: DEBUG_FLR_PARAM_TYPE = 2097201i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_LIST: DEBUG_FLR_PARAM_TYPE = 2097193i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_NAME: DEBUG_FLR_PARAM_TYPE = 2097195i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_THREAD_INDEX: DEBUG_FLR_PARAM_TYPE = 2097202i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_VALUE: DEBUG_FLR_PARAM_TYPE = 2097196i32;
pub const DEBUG_FLR_XML_ATTRIBUTE_VALUE_TYPE: DEBUG_FLR_PARAM_TYPE = 2097200i32;
pub const DEBUG_FLR_XML_ENCODED_OFFSETS: DEBUG_FLR_PARAM_TYPE = 2097213i32;
pub const DEBUG_FLR_XML_EVENTTYPE: DEBUG_FLR_PARAM_TYPE = 2097235i32;
pub const DEBUG_FLR_XML_GLOBALATTRIBUTE_LIST: DEBUG_FLR_PARAM_TYPE = 2097192i32;
pub const DEBUG_FLR_XML_MODERN_ASYNC_REQUEST_OUTSTANDING: DEBUG_FLR_PARAM_TYPE = 2097234i32;
pub const DEBUG_FLR_XML_MODULE_INFO: DEBUG_FLR_PARAM_TYPE = 2097169i32;
pub const DEBUG_FLR_XML_MODULE_INFO_BASE: DEBUG_FLR_PARAM_TYPE = 2097186i32;
pub const DEBUG_FLR_XML_MODULE_INFO_CHECKSUM: DEBUG_FLR_PARAM_TYPE = 2097174i32;
pub const DEBUG_FLR_XML_MODULE_INFO_COMPANY_NAME: DEBUG_FLR_PARAM_TYPE = 2097182i32;
pub const DEBUG_FLR_XML_MODULE_INFO_DRIVER_GROUP: DEBUG_FLR_PARAM_TYPE = 2097251i32;
pub const DEBUG_FLR_XML_MODULE_INFO_FILE_DESCRIPTION: DEBUG_FLR_PARAM_TYPE = 2097183i32;
pub const DEBUG_FLR_XML_MODULE_INFO_FILE_FLAGS: DEBUG_FLR_PARAM_TYPE = 2097223i32;
pub const DEBUG_FLR_XML_MODULE_INFO_FIXED_FILE_VER: DEBUG_FLR_PARAM_TYPE = 2097178i32;
pub const DEBUG_FLR_XML_MODULE_INFO_FIXED_PROD_VER: DEBUG_FLR_PARAM_TYPE = 2097179i32;
pub const DEBUG_FLR_XML_MODULE_INFO_IMAGE_NAME: DEBUG_FLR_PARAM_TYPE = 2097172i32;
pub const DEBUG_FLR_XML_MODULE_INFO_IMAGE_PATH: DEBUG_FLR_PARAM_TYPE = 2097173i32;
pub const DEBUG_FLR_XML_MODULE_INFO_INDEX: DEBUG_FLR_PARAM_TYPE = 2097170i32;
pub const DEBUG_FLR_XML_MODULE_INFO_INTERNAL_NAME: DEBUG_FLR_PARAM_TYPE = 2097184i32;
pub const DEBUG_FLR_XML_MODULE_INFO_NAME: DEBUG_FLR_PARAM_TYPE = 2097171i32;
pub const DEBUG_FLR_XML_MODULE_INFO_ON_STACK: DEBUG_FLR_PARAM_TYPE = 2097177i32;
pub const DEBUG_FLR_XML_MODULE_INFO_ORIG_FILE_NAME: DEBUG_FLR_PARAM_TYPE = 2097185i32;
pub const DEBUG_FLR_XML_MODULE_INFO_PRODUCT_NAME: DEBUG_FLR_PARAM_TYPE = 2097188i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SIZE: DEBUG_FLR_PARAM_TYPE = 2097187i32;
pub const DEBUG_FLR_XML_MODULE_INFO_STRING_FILE_VER: DEBUG_FLR_PARAM_TYPE = 2097180i32;
pub const DEBUG_FLR_XML_MODULE_INFO_STRING_PROD_VER: DEBUG_FLR_PARAM_TYPE = 2097181i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMBOL_TYPE: DEBUG_FLR_PARAM_TYPE = 2097222i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_IMAGE_DETAIL: DEBUG_FLR_PARAM_TYPE = 2097245i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_IMAGE_ERROR: DEBUG_FLR_PARAM_TYPE = 2097244i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_IMAGE_SEC: DEBUG_FLR_PARAM_TYPE = 2097246i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_IMAGE_STATUS: DEBUG_FLR_PARAM_TYPE = 2097243i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_PDB_DETAIL: DEBUG_FLR_PARAM_TYPE = 2097249i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_PDB_ERROR: DEBUG_FLR_PARAM_TYPE = 2097248i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_PDB_SEC: DEBUG_FLR_PARAM_TYPE = 2097250i32;
pub const DEBUG_FLR_XML_MODULE_INFO_SYMSRV_PDB_STATUS: DEBUG_FLR_PARAM_TYPE = 2097247i32;
pub const DEBUG_FLR_XML_MODULE_INFO_TIMESTAMP: DEBUG_FLR_PARAM_TYPE = 2097175i32;
pub const DEBUG_FLR_XML_MODULE_INFO_UNLOADED: DEBUG_FLR_PARAM_TYPE = 2097176i32;
pub const DEBUG_FLR_XML_MODULE_LIST: DEBUG_FLR_PARAM_TYPE = 2097154i32;
pub const DEBUG_FLR_XML_PACKAGE_MONIKER: DEBUG_FLR_PARAM_TYPE = 2097232i32;
pub const DEBUG_FLR_XML_PACKAGE_NAME: DEBUG_FLR_PARAM_TYPE = 2097236i32;
pub const DEBUG_FLR_XML_PACKAGE_RELATIVE_APPLICATION_ID: DEBUG_FLR_PARAM_TYPE = 2097233i32;
pub const DEBUG_FLR_XML_PACKAGE_VERSION: DEBUG_FLR_PARAM_TYPE = 2097237i32;
pub const DEBUG_FLR_XML_PROBLEMCLASS: DEBUG_FLR_PARAM_TYPE = 2097204i32;
pub const DEBUG_FLR_XML_PROBLEMCLASS_FRAME_NUMBER: DEBUG_FLR_PARAM_TYPE = 2097208i32;
pub const DEBUG_FLR_XML_PROBLEMCLASS_LIST: DEBUG_FLR_PARAM_TYPE = 2097203i32;
pub const DEBUG_FLR_XML_PROBLEMCLASS_NAME: DEBUG_FLR_PARAM_TYPE = 2097205i32;
pub const DEBUG_FLR_XML_PROBLEMCLASS_THREAD_INDEX: DEBUG_FLR_PARAM_TYPE = 2097209i32;
pub const DEBUG_FLR_XML_PROBLEMCLASS_VALUE: DEBUG_FLR_PARAM_TYPE = 2097206i32;
pub const DEBUG_FLR_XML_PROBLEMCLASS_VALUE_TYPE: DEBUG_FLR_PARAM_TYPE = 2097207i32;
pub const DEBUG_FLR_XML_STACK_FRAME_TRIAGE_STATUS: DEBUG_FLR_PARAM_TYPE = 2097210i32;
pub const DEBUG_FLR_XML_SYSTEMINFO: DEBUG_FLR_PARAM_TYPE = 2097226i32;
pub const DEBUG_FLR_XML_SYSTEMINFO_SYSTEMMANUFACTURER: DEBUG_FLR_PARAM_TYPE = 2097227i32;
pub const DEBUG_FLR_XML_SYSTEMINFO_SYSTEMMARKER: DEBUG_FLR_PARAM_TYPE = 2097229i32;
pub const DEBUG_FLR_XML_SYSTEMINFO_SYSTEMMODEL: DEBUG_FLR_PARAM_TYPE = 2097228i32;
pub const DEBUG_FLR_XPROC_DUMP_AVAILABLE: DEBUG_FLR_PARAM_TYPE = 1048592i32;
pub const DEBUG_FLR_XPROC_HANG: DEBUG_FLR_PARAM_TYPE = 1048588i32;
pub const DEBUG_FLR_ZEROED_STACK: DEBUG_FLR_PARAM_TYPE = 8194i32;
pub const DEBUG_FORMAT_CAB_SECONDARY_ALL_IMAGES: u32 = 268435456u32;
pub const DEBUG_FORMAT_CAB_SECONDARY_FILES: u32 = 1073741824u32;
pub const DEBUG_FORMAT_DEFAULT: u32 = 0u32;
pub const DEBUG_FORMAT_NO_OVERWRITE: u32 = 2147483648u32;
pub const DEBUG_FORMAT_USER_SMALL_ADD_AVX_XSTATE_CONTEXT: u32 = 131072u32;
pub const DEBUG_FORMAT_USER_SMALL_CODE_SEGMENTS: u32 = 4096u32;
pub const DEBUG_FORMAT_USER_SMALL_DATA_SEGMENTS: u32 = 16u32;
pub const DEBUG_FORMAT_USER_SMALL_FILTER_MEMORY: u32 = 32u32;
pub const DEBUG_FORMAT_USER_SMALL_FILTER_PATHS: u32 = 64u32;
pub const DEBUG_FORMAT_USER_SMALL_FILTER_TRIAGE: u32 = 65536u32;
pub const DEBUG_FORMAT_USER_SMALL_FULL_AUXILIARY_STATE: u32 = 16384u32;
pub const DEBUG_FORMAT_USER_SMALL_FULL_MEMORY: u32 = 1u32;
pub const DEBUG_FORMAT_USER_SMALL_FULL_MEMORY_INFO: u32 = 1024u32;
pub const DEBUG_FORMAT_USER_SMALL_HANDLE_DATA: u32 = 2u32;
pub const DEBUG_FORMAT_USER_SMALL_IGNORE_INACCESSIBLE_MEM: u32 = 134217728u32;
pub const DEBUG_FORMAT_USER_SMALL_INDIRECT_MEMORY: u32 = 8u32;
pub const DEBUG_FORMAT_USER_SMALL_IPT_TRACE: u32 = 262144u32;
pub const DEBUG_FORMAT_USER_SMALL_MODULE_HEADERS: u32 = 32768u32;
pub const DEBUG_FORMAT_USER_SMALL_NO_AUXILIARY_STATE: u32 = 8192u32;
pub const DEBUG_FORMAT_USER_SMALL_NO_OPTIONAL_DATA: u32 = 512u32;
pub const DEBUG_FORMAT_USER_SMALL_PRIVATE_READ_WRITE_MEMORY: u32 = 256u32;
pub const DEBUG_FORMAT_USER_SMALL_PROCESS_THREAD_DATA: u32 = 128u32;
pub const DEBUG_FORMAT_USER_SMALL_SCAN_PARTIAL_PAGES: u32 = 268435456u32;
pub const DEBUG_FORMAT_USER_SMALL_THREAD_INFO: u32 = 2048u32;
pub const DEBUG_FORMAT_USER_SMALL_UNLOADED_MODULES: u32 = 4u32;
pub const DEBUG_FORMAT_WRITE_CAB: u32 = 536870912u32;
pub const DEBUG_FRAME_DEFAULT: u32 = 0u32;
pub const DEBUG_FRAME_IGNORE_INLINE: u32 = 1u32;
pub const DEBUG_GETFNENT_DEFAULT: u32 = 0u32;
pub const DEBUG_GETFNENT_RAW_ENTRY_ONLY: u32 = 1u32;
pub const DEBUG_GETMOD_DEFAULT: u32 = 0u32;
pub const DEBUG_GETMOD_NO_LOADED_MODULES: u32 = 1u32;
pub const DEBUG_GETMOD_NO_UNLOADED_MODULES: u32 = 2u32;
pub const DEBUG_GET_PROC_DEFAULT: u32 = 0u32;
pub const DEBUG_GET_PROC_FULL_MATCH: u32 = 1u32;
pub const DEBUG_GET_PROC_ONLY_MATCH: u32 = 2u32;
pub const DEBUG_GET_PROC_SERVICE_NAME: u32 = 4u32;
pub const DEBUG_GET_TEXT_COMPLETIONS_IS_DOT_COMMAND: u32 = 1u32;
pub const DEBUG_GET_TEXT_COMPLETIONS_IS_EXTENSION_COMMAND: u32 = 2u32;
pub const DEBUG_GET_TEXT_COMPLETIONS_IS_SYMBOL: u32 = 4u32;
pub const DEBUG_GET_TEXT_COMPLETIONS_NO_DOT_COMMANDS: u32 = 1u32;
pub const DEBUG_GET_TEXT_COMPLETIONS_NO_EXTENSION_COMMANDS: u32 = 2u32;
pub const DEBUG_GET_TEXT_COMPLETIONS_NO_SYMBOLS: u32 = 4u32;
pub const DEBUG_GSEL_ALLOW_HIGHER: u32 = 4u32;
pub const DEBUG_GSEL_ALLOW_LOWER: u32 = 2u32;
pub const DEBUG_GSEL_DEFAULT: u32 = 0u32;
pub const DEBUG_GSEL_INLINE_CALLSITE: u32 = 16u32;
pub const DEBUG_GSEL_NEAREST_ONLY: u32 = 8u32;
pub const DEBUG_GSEL_NO_SYMBOL_LOADS: u32 = 1u32;
pub const DEBUG_HANDLE_DATA_TYPE_ALL_HANDLE_OPERATIONS: u32 = 10u32;
pub const DEBUG_HANDLE_DATA_TYPE_BASIC: u32 = 0u32;
pub const DEBUG_HANDLE_DATA_TYPE_HANDLE_COUNT: u32 = 3u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_EVENT_1: u32 = 13u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_MUTANT_1: u32 = 7u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_MUTANT_2: u32 = 8u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_PROCESS_1: u32 = 11u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_PROCESS_2: u32 = 12u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_SECTION_1: u32 = 14u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_SEMAPHORE_1: u32 = 15u32;
pub const DEBUG_HANDLE_DATA_TYPE_MINI_THREAD_1: u32 = 6u32;
pub const DEBUG_HANDLE_DATA_TYPE_OBJECT_NAME: u32 = 2u32;
pub const DEBUG_HANDLE_DATA_TYPE_OBJECT_NAME_WIDE: u32 = 5u32;
pub const DEBUG_HANDLE_DATA_TYPE_PER_HANDLE_OPERATIONS: u32 = 9u32;
pub const DEBUG_HANDLE_DATA_TYPE_TYPE_NAME: u32 = 1u32;
pub const DEBUG_HANDLE_DATA_TYPE_TYPE_NAME_WIDE: u32 = 4u32;
pub const DEBUG_INTERRUPT_ACTIVE: u32 = 0u32;
pub const DEBUG_INTERRUPT_EXIT: u32 = 2u32;
pub const DEBUG_INTERRUPT_PASSIVE: u32 = 1u32;
pub const DEBUG_IOUTPUT_ADDR_TRANSLATE: u32 = 134217728u32;
pub const DEBUG_IOUTPUT_BREAKPOINT: u32 = 536870912u32;
pub const DEBUG_IOUTPUT_EVENT: u32 = 268435456u32;
pub const DEBUG_IOUTPUT_KD_PROTOCOL: u32 = 2147483648u32;
pub const DEBUG_IOUTPUT_REMOTING: u32 = 1073741824u32;
pub const DEBUG_KERNEL_ACTIVE_DUMP: u32 = 1030u32;
pub const DEBUG_KERNEL_CONNECTION: u32 = 0u32;
pub const DEBUG_KERNEL_DUMP: u32 = 1025u32;
pub const DEBUG_KERNEL_EXDI_DRIVER: u32 = 2u32;
pub const DEBUG_KERNEL_FULL_DUMP: u32 = 1026u32;
pub const DEBUG_KERNEL_IDNA: u32 = 3u32;
pub const DEBUG_KERNEL_INSTALL_DRIVER: u32 = 4u32;
pub const DEBUG_KERNEL_LOCAL: u32 = 1u32;
pub const DEBUG_KERNEL_REPT: u32 = 5u32;
pub const DEBUG_KERNEL_SMALL_DUMP: u32 = 1024u32;
pub const DEBUG_KERNEL_TRACE_LOG: u32 = 1028u32;
pub const DEBUG_KNOWN_STRUCT_GET_NAMES: u32 = 1u32;
pub const DEBUG_KNOWN_STRUCT_GET_SINGLE_LINE_OUTPUT: u32 = 2u32;
pub const DEBUG_KNOWN_STRUCT_SUPPRESS_TYPE_NAME: u32 = 3u32;
pub const DEBUG_LEVEL_ASSEMBLY: u32 = 1u32;
pub const DEBUG_LEVEL_SOURCE: u32 = 0u32;
pub const DEBUG_LIVE_USER_NON_INVASIVE: u32 = 33u32;
pub const DEBUG_LOG_APPEND: u32 = 1u32;
pub const DEBUG_LOG_DEFAULT: u32 = 0u32;
pub const DEBUG_LOG_DML: u32 = 4u32;
pub const DEBUG_LOG_UNICODE: u32 = 2u32;
pub const DEBUG_MANAGED_ALLOWED: u32 = 1u32;
pub const DEBUG_MANAGED_DISABLED: u32 = 0u32;
pub const DEBUG_MANAGED_DLL_LOADED: u32 = 2u32;
pub const DEBUG_MANRESET_DEFAULT: u32 = 0u32;
pub const DEBUG_MANRESET_LOAD_DLL: u32 = 1u32;
pub const DEBUG_MANSTR_LOADED_SUPPORT_DLL: u32 = 1u32;
pub const DEBUG_MANSTR_LOAD_STATUS: u32 = 2u32;
pub const DEBUG_MANSTR_NONE: u32 = 0u32;
pub const DEBUG_MODNAME_IMAGE: u32 = 0u32;
pub const DEBUG_MODNAME_LOADED_IMAGE: u32 = 2u32;
pub const DEBUG_MODNAME_MAPPED_IMAGE: u32 = 4u32;
pub const DEBUG_MODNAME_MODULE: u32 = 1u32;
pub const DEBUG_MODNAME_SYMBOL_FILE: u32 = 3u32;
pub const DEBUG_MODULE_EXE_MODULE: u32 = 4u32;
pub const DEBUG_MODULE_EXPLICIT: u32 = 8u32;
pub const DEBUG_MODULE_LOADED: u32 = 0u32;
pub const DEBUG_MODULE_SECONDARY: u32 = 16u32;
pub const DEBUG_MODULE_SYM_BAD_CHECKSUM: u32 = 65536u32;
pub const DEBUG_MODULE_SYNTHETIC: u32 = 32u32;
pub const DEBUG_MODULE_UNLOADED: u32 = 1u32;
pub const DEBUG_MODULE_USER_MODE: u32 = 2u32;
pub const DEBUG_NOTIFY_SESSION_ACCESSIBLE: u32 = 2u32;
pub const DEBUG_NOTIFY_SESSION_ACTIVE: u32 = 0u32;
pub const DEBUG_NOTIFY_SESSION_INACCESSIBLE: u32 = 3u32;
pub const DEBUG_NOTIFY_SESSION_INACTIVE: u32 = 1u32;
pub const DEBUG_OFFSINFO_VIRTUAL_SOURCE: u32 = 1u32;
pub const DEBUG_OUTCBF_COMBINED_EXPLICIT_FLUSH: u32 = 1u32;
pub const DEBUG_OUTCBF_DML_HAS_SPECIAL_CHARACTERS: u32 = 4u32;
pub const DEBUG_OUTCBF_DML_HAS_TAGS: u32 = 2u32;
pub const DEBUG_OUTCBI_ANY_FORMAT: u32 = 6u32;
pub const DEBUG_OUTCBI_DML: u32 = 4u32;
pub const DEBUG_OUTCBI_EXPLICIT_FLUSH: u32 = 1u32;
pub const DEBUG_OUTCBI_TEXT: u32 = 2u32;
pub const DEBUG_OUTCB_DML: u32 = 1u32;
pub const DEBUG_OUTCB_EXPLICIT_FLUSH: u32 = 2u32;
pub const DEBUG_OUTCB_TEXT: u32 = 0u32;
pub const DEBUG_OUTCTL_ALL_CLIENTS: u32 = 1u32;
pub const DEBUG_OUTCTL_ALL_OTHER_CLIENTS: u32 = 2u32;
pub const DEBUG_OUTCTL_AMBIENT: u32 = 4294967295u32;
pub const DEBUG_OUTCTL_AMBIENT_DML: u32 = 4294967294u32;
pub const DEBUG_OUTCTL_AMBIENT_TEXT: u32 = 4294967295u32;
pub const DEBUG_OUTCTL_DML: u32 = 32u32;
pub const DEBUG_OUTCTL_IGNORE: u32 = 3u32;
pub const DEBUG_OUTCTL_LOG_ONLY: u32 = 4u32;
pub const DEBUG_OUTCTL_NOT_LOGGED: u32 = 8u32;
pub const DEBUG_OUTCTL_OVERRIDE_MASK: u32 = 16u32;
pub const DEBUG_OUTCTL_SEND_MASK: u32 = 7u32;
pub const DEBUG_OUTCTL_THIS_CLIENT: u32 = 0u32;
pub const DEBUG_OUTPUT_DEBUGGEE: u32 = 128u32;
pub const DEBUG_OUTPUT_DEBUGGEE_PROMPT: u32 = 256u32;
pub const DEBUG_OUTPUT_ERROR: u32 = 2u32;
pub const DEBUG_OUTPUT_EXTENSION_WARNING: u32 = 64u32;
pub const DEBUG_OUTPUT_IDENTITY_DEFAULT: u32 = 0u32;
pub const DEBUG_OUTPUT_NAME_END: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("**NAME**");
pub const DEBUG_OUTPUT_NAME_END_T: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**NAME**");
pub const DEBUG_OUTPUT_NAME_END_WIDE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**NAME**");
pub const DEBUG_OUTPUT_NORMAL: u32 = 1u32;
pub const DEBUG_OUTPUT_OFFSET_END: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("**OFF**");
pub const DEBUG_OUTPUT_OFFSET_END_T: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**OFF**");
pub const DEBUG_OUTPUT_OFFSET_END_WIDE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**OFF**");
pub const DEBUG_OUTPUT_PROMPT: u32 = 16u32;
pub const DEBUG_OUTPUT_PROMPT_REGISTERS: u32 = 32u32;
pub const DEBUG_OUTPUT_STATUS: u32 = 1024u32;
pub const DEBUG_OUTPUT_SYMBOLS: u32 = 512u32;
pub const DEBUG_OUTPUT_SYMBOLS_DEFAULT: u32 = 0u32;
pub const DEBUG_OUTPUT_SYMBOLS_NO_NAMES: u32 = 1u32;
pub const DEBUG_OUTPUT_SYMBOLS_NO_OFFSETS: u32 = 2u32;
pub const DEBUG_OUTPUT_SYMBOLS_NO_TYPES: u32 = 16u32;
pub const DEBUG_OUTPUT_SYMBOLS_NO_VALUES: u32 = 4u32;
pub const DEBUG_OUTPUT_TYPE_END: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("**TYPE**");
pub const DEBUG_OUTPUT_TYPE_END_T: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**TYPE**");
pub const DEBUG_OUTPUT_TYPE_END_WIDE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**TYPE**");
pub const DEBUG_OUTPUT_VALUE_END: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("**VALUE**");
pub const DEBUG_OUTPUT_VALUE_END_T: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**VALUE**");
pub const DEBUG_OUTPUT_VALUE_END_WIDE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("**VALUE**");
pub const DEBUG_OUTPUT_VERBOSE: u32 = 8u32;
pub const DEBUG_OUTPUT_WARNING: u32 = 4u32;
pub const DEBUG_OUTPUT_XML: u32 = 2048u32;
pub const DEBUG_OUTSYM_ALLOW_DISPLACEMENT: u32 = 4u32;
pub const DEBUG_OUTSYM_DEFAULT: u32 = 0u32;
pub const DEBUG_OUTSYM_FORCE_OFFSET: u32 = 1u32;
pub const DEBUG_OUTSYM_SOURCE_LINE: u32 = 2u32;
pub const DEBUG_OUTTYPE_ADDRESS_AT_END: u32 = 131072u32;
pub const DEBUG_OUTTYPE_ADDRESS_OF_FIELD: u32 = 65536u32;
pub const DEBUG_OUTTYPE_BLOCK_RECURSE: u32 = 2097152u32;
pub const DEBUG_OUTTYPE_COMPACT_OUTPUT: u32 = 8u32;
pub const DEBUG_OUTTYPE_DEFAULT: u32 = 0u32;
pub const DEBUG_OUTTYPE_NO_INDENT: u32 = 1u32;
pub const DEBUG_OUTTYPE_NO_OFFSET: u32 = 2u32;
pub const DEBUG_OUTTYPE_VERBOSE: u32 = 4u32;
pub const DEBUG_OUT_TEXT_REPL_DEFAULT: u32 = 0u32;
pub const DEBUG_PHYSICAL_CACHED: u32 = 1u32;
pub const DEBUG_PHYSICAL_DEFAULT: u32 = 0u32;
pub const DEBUG_PHYSICAL_UNCACHED: u32 = 2u32;
pub const DEBUG_PHYSICAL_WRITE_COMBINED: u32 = 3u32;
pub const DEBUG_PROCESS_DETACH_ON_EXIT: u32 = 1u32;
pub const DEBUG_PROCESS_ONLY_THIS_PROCESS: u32 = 2u32;
pub const DEBUG_PROC_DESC_DEFAULT: u32 = 0u32;
pub const DEBUG_PROC_DESC_NO_COMMAND_LINE: u32 = 8u32;
pub const DEBUG_PROC_DESC_NO_MTS_PACKAGES: u32 = 4u32;
pub const DEBUG_PROC_DESC_NO_PATHS: u32 = 1u32;
pub const DEBUG_PROC_DESC_NO_SERVICES: u32 = 2u32;
pub const DEBUG_PROC_DESC_NO_SESSION_ID: u32 = 16u32;
pub const DEBUG_PROC_DESC_NO_USER_NAME: u32 = 32u32;
pub const DEBUG_PROC_DESC_WITH_ARCHITECTURE: u32 = 128u32;
pub const DEBUG_PROC_DESC_WITH_PACKAGEFAMILY: u32 = 64u32;
pub const DEBUG_REGISTERS_ALL: u32 = 7u32;
pub const DEBUG_REGISTERS_DEFAULT: u32 = 0u32;
pub const DEBUG_REGISTERS_FLOAT: u32 = 4u32;
pub const DEBUG_REGISTERS_INT32: u32 = 1u32;
pub const DEBUG_REGISTERS_INT64: u32 = 2u32;
pub const DEBUG_REGISTER_SUB_REGISTER: u32 = 1u32;
pub const DEBUG_REGSRC_DEBUGGEE: u32 = 0u32;
pub const DEBUG_REGSRC_EXPLICIT: u32 = 1u32;
pub const DEBUG_REGSRC_FRAME: u32 = 2u32;
pub const DEBUG_REQUEST_ADD_CACHED_SYMBOL_INFO: u32 = 16u32;
pub const DEBUG_REQUEST_CLOSE_TOKEN: u32 = 30u32;
pub const DEBUG_REQUEST_CURRENT_OUTPUT_CALLBACKS_ARE_DML_AWARE: u32 = 19u32;
pub const DEBUG_REQUEST_DUPLICATE_TOKEN: u32 = 28u32;
pub const DEBUG_REQUEST_EXT_TYPED_DATA_ANSI: u32 = 12u32;
pub const DEBUG_REQUEST_GET_ADDITIONAL_CREATE_OPTIONS: u32 = 4u32;
pub const DEBUG_REQUEST_GET_CACHED_SYMBOL_INFO: u32 = 15u32;
pub const DEBUG_REQUEST_GET_CAPTURED_EVENT_CODE_OFFSET: u32 = 10u32;
pub const DEBUG_REQUEST_GET_DUMP_HEADER: u32 = 21u32;
pub const DEBUG_REQUEST_GET_EXTENSION_SEARCH_PATH_WIDE: u32 = 13u32;
pub const DEBUG_REQUEST_GET_IMAGE_ARCHITECTURE: u32 = 39u32;
pub const DEBUG_REQUEST_GET_INSTRUMENTATION_VERSION: u32 = 37u32;
pub const DEBUG_REQUEST_GET_MODULE_ARCHITECTURE: u32 = 38u32;
pub const DEBUG_REQUEST_GET_OFFSET_UNWIND_INFORMATION: u32 = 20u32;
pub const DEBUG_REQUEST_GET_TEXT_COMPLETIONS_ANSI: u32 = 18u32;
pub const DEBUG_REQUEST_GET_TEXT_COMPLETIONS_WIDE: u32 = 14u32;
pub const DEBUG_REQUEST_GET_WIN32_MAJOR_MINOR_VERSIONS: u32 = 6u32;
pub const DEBUG_REQUEST_INLINE_QUERY: u32 = 35u32;
pub const DEBUG_REQUEST_MIDORI: u32 = 23u32;
pub const DEBUG_REQUEST_MISC_INFORMATION: u32 = 25u32;
pub const DEBUG_REQUEST_OPEN_PROCESS_TOKEN: u32 = 26u32;
pub const DEBUG_REQUEST_OPEN_THREAD_TOKEN: u32 = 27u32;
pub const DEBUG_REQUEST_PROCESS_DESCRIPTORS: u32 = 24u32;
pub const DEBUG_REQUEST_QUERY_INFO_TOKEN: u32 = 29u32;
pub const DEBUG_REQUEST_READ_CAPTURED_EVENT_CODE_STREAM: u32 = 11u32;
pub const DEBUG_REQUEST_READ_USER_MINIDUMP_STREAM: u32 = 7u32;
pub const DEBUG_REQUEST_REMOVE_CACHED_SYMBOL_INFO: u32 = 17u32;
pub const DEBUG_REQUEST_RESUME_THREAD: u32 = 34u32;
pub const DEBUG_REQUEST_SET_ADDITIONAL_CREATE_OPTIONS: u32 = 5u32;
pub const DEBUG_REQUEST_SET_DUMP_HEADER: u32 = 22u32;
pub const DEBUG_REQUEST_SET_LOCAL_IMPLICIT_COMMAND_LINE: u32 = 9u32;
pub const DEBUG_REQUEST_SOURCE_PATH_HAS_SOURCE_SERVER: u32 = 0u32;
pub const DEBUG_REQUEST_TARGET_CAN_DETACH: u32 = 8u32;
pub const DEBUG_REQUEST_TARGET_EXCEPTION_CONTEXT: u32 = 1u32;
pub const DEBUG_REQUEST_TARGET_EXCEPTION_RECORD: u32 = 3u32;
pub const DEBUG_REQUEST_TARGET_EXCEPTION_THREAD: u32 = 2u32;
pub const DEBUG_REQUEST_TL_INSTRUMENTATION_AWARE: u32 = 36u32;
pub const DEBUG_REQUEST_WOW_MODULE: u32 = 32u32;
pub const DEBUG_REQUEST_WOW_PROCESS: u32 = 31u32;
pub const DEBUG_SCOPE_GROUP_ALL: u32 = 3u32;
pub const DEBUG_SCOPE_GROUP_ARGUMENTS: u32 = 1u32;
pub const DEBUG_SCOPE_GROUP_BY_DATAMODEL: u32 = 4u32;
pub const DEBUG_SCOPE_GROUP_LOCALS: u32 = 2u32;
pub const DEBUG_SERVERS_ALL: u32 = 3u32;
pub const DEBUG_SERVERS_DEBUGGER: u32 = 1u32;
pub const DEBUG_SERVERS_PROCESS: u32 = 2u32;
pub const DEBUG_SESSION_ACTIVE: u32 = 0u32;
pub const DEBUG_SESSION_END: u32 = 4u32;
pub const DEBUG_SESSION_END_SESSION_ACTIVE_DETACH: u32 = 2u32;
pub const DEBUG_SESSION_END_SESSION_ACTIVE_TERMINATE: u32 = 1u32;
pub const DEBUG_SESSION_END_SESSION_PASSIVE: u32 = 3u32;
pub const DEBUG_SESSION_FAILURE: u32 = 7u32;
pub const DEBUG_SESSION_HIBERNATE: u32 = 6u32;
pub const DEBUG_SESSION_REBOOT: u32 = 5u32;
pub const DEBUG_SOURCE_IS_STATEMENT: u32 = 1u32;
pub const DEBUG_SRCFILE_SYMBOL_CHECKSUMINFO: u32 = 2u32;
pub const DEBUG_SRCFILE_SYMBOL_TOKEN: u32 = 0u32;
pub const DEBUG_SRCFILE_SYMBOL_TOKEN_SOURCE_COMMAND_WIDE: u32 = 1u32;
pub const DEBUG_STACK_ARGUMENTS: u32 = 1u32;
pub const DEBUG_STACK_COLUMN_NAMES: u32 = 16u32;
pub const DEBUG_STACK_DML: u32 = 2048u32;
pub const DEBUG_STACK_FRAME_ADDRESSES: u32 = 8u32;
pub const DEBUG_STACK_FRAME_ADDRESSES_RA_ONLY: u32 = 256u32;
pub const DEBUG_STACK_FRAME_ARCH: u32 = 16384u32;
pub const DEBUG_STACK_FRAME_MEMORY_USAGE: u32 = 512u32;
pub const DEBUG_STACK_FRAME_NUMBERS: u32 = 64u32;
pub const DEBUG_STACK_FRAME_OFFSETS: u32 = 4096u32;
pub const DEBUG_STACK_FUNCTION_INFO: u32 = 2u32;
pub const DEBUG_STACK_NONVOLATILE_REGISTERS: u32 = 32u32;
pub const DEBUG_STACK_PARAMETERS: u32 = 128u32;
pub const DEBUG_STACK_PARAMETERS_NEWLINE: u32 = 1024u32;
pub const DEBUG_STACK_PROVIDER: u32 = 8192u32;
pub const DEBUG_STACK_SOURCE_LINE: u32 = 4u32;
pub const DEBUG_STATUS_BREAK: u32 = 6u32;
pub const DEBUG_STATUS_GO: u32 = 1u32;
pub const DEBUG_STATUS_GO_HANDLED: u32 = 2u32;
pub const DEBUG_STATUS_GO_NOT_HANDLED: u32 = 3u32;
pub const DEBUG_STATUS_IGNORE_EVENT: u32 = 9u32;
pub const DEBUG_STATUS_INSIDE_WAIT: u64 = 4294967296u64;
pub const DEBUG_STATUS_MASK: u32 = 31u32;
pub const DEBUG_STATUS_NO_CHANGE: u32 = 0u32;
pub const DEBUG_STATUS_NO_DEBUGGEE: u32 = 7u32;
pub const DEBUG_STATUS_OUT_OF_SYNC: u32 = 15u32;
pub const DEBUG_STATUS_RESTART_REQUESTED: u32 = 10u32;
pub const DEBUG_STATUS_REVERSE_GO: u32 = 11u32;
pub const DEBUG_STATUS_REVERSE_STEP_BRANCH: u32 = 12u32;
pub const DEBUG_STATUS_REVERSE_STEP_INTO: u32 = 14u32;
pub const DEBUG_STATUS_REVERSE_STEP_OVER: u32 = 13u32;
pub const DEBUG_STATUS_STEP_BRANCH: u32 = 8u32;
pub const DEBUG_STATUS_STEP_INTO: u32 = 5u32;
pub const DEBUG_STATUS_STEP_OVER: u32 = 4u32;
pub const DEBUG_STATUS_TIMEOUT: u32 = 17u32;
pub const DEBUG_STATUS_WAIT_INPUT: u32 = 16u32;
pub const DEBUG_STATUS_WAIT_TIMEOUT: u64 = 8589934592u64;
pub const DEBUG_SYMBOL_EXPANDED: u32 = 16u32;
pub const DEBUG_SYMBOL_EXPANSION_LEVEL_MASK: u32 = 15u32;
pub const DEBUG_SYMBOL_IS_ARGUMENT: u32 = 256u32;
pub const DEBUG_SYMBOL_IS_ARRAY: u32 = 64u32;
pub const DEBUG_SYMBOL_IS_FLOAT: u32 = 128u32;
pub const DEBUG_SYMBOL_IS_LOCAL: u32 = 512u32;
pub const DEBUG_SYMBOL_READ_ONLY: u32 = 32u32;
pub const DEBUG_SYMENT_IS_CODE: u32 = 1u32;
pub const DEBUG_SYMENT_IS_DATA: u32 = 2u32;
pub const DEBUG_SYMENT_IS_LOCAL: u32 = 8u32;
pub const DEBUG_SYMENT_IS_MANAGED: u32 = 16u32;
pub const DEBUG_SYMENT_IS_PARAMETER: u32 = 4u32;
pub const DEBUG_SYMENT_IS_SYNTHETIC: u32 = 32u32;
pub const DEBUG_SYMINFO_BREAKPOINT_SOURCE_LINE: u32 = 0u32;
pub const DEBUG_SYMINFO_GET_MODULE_SYMBOL_NAMES_AND_OFFSETS: u32 = 3u32;
pub const DEBUG_SYMINFO_GET_SYMBOL_NAME_BY_OFFSET_AND_TAG_WIDE: u32 = 2u32;
pub const DEBUG_SYMINFO_IMAGEHLP_MODULEW64: u32 = 1u32;
pub const DEBUG_SYMTYPE_CODEVIEW: u32 = 2u32;
pub const DEBUG_SYMTYPE_COFF: u32 = 1u32;
pub const DEBUG_SYMTYPE_DEFERRED: u32 = 5u32;
pub const DEBUG_SYMTYPE_DIA: u32 = 7u32;
pub const DEBUG_SYMTYPE_EXPORT: u32 = 4u32;
pub const DEBUG_SYMTYPE_NONE: u32 = 0u32;
pub const DEBUG_SYMTYPE_PDB: u32 = 3u32;
pub const DEBUG_SYMTYPE_SYM: u32 = 6u32;
pub const DEBUG_SYSOBJINFO_CURRENT_PROCESS_COOKIE: u32 = 2u32;
pub const DEBUG_SYSOBJINFO_THREAD_BASIC_INFORMATION: u32 = 0u32;
pub const DEBUG_SYSOBJINFO_THREAD_NAME_WIDE: u32 = 1u32;
pub const DEBUG_SYSVERSTR_BUILD: u32 = 1u32;
pub const DEBUG_SYSVERSTR_SERVICE_PACK: u32 = 0u32;
pub const DEBUG_TBINFO_AFFINITY: u32 = 32u32;
pub const DEBUG_TBINFO_ALL: u32 = 63u32;
pub const DEBUG_TBINFO_EXIT_STATUS: u32 = 1u32;
pub const DEBUG_TBINFO_PRIORITY: u32 = 4u32;
pub const DEBUG_TBINFO_PRIORITY_CLASS: u32 = 2u32;
pub const DEBUG_TBINFO_START_OFFSET: u32 = 16u32;
pub const DEBUG_TBINFO_TIMES: u32 = 8u32;
pub const DEBUG_TYPED_DATA_IS_IN_MEMORY: u32 = 1u32;
pub const DEBUG_TYPED_DATA_PHYSICAL_CACHED: u32 = 4u32;
pub const DEBUG_TYPED_DATA_PHYSICAL_DEFAULT: u32 = 2u32;
pub const DEBUG_TYPED_DATA_PHYSICAL_MEMORY: u32 = 14u32;
pub const DEBUG_TYPED_DATA_PHYSICAL_UNCACHED: u32 = 6u32;
pub const DEBUG_TYPED_DATA_PHYSICAL_WRITE_COMBINED: u32 = 8u32;
pub const DEBUG_TYPEOPTS_FORCERADIX_OUTPUT: u32 = 4u32;
pub const DEBUG_TYPEOPTS_LONGSTATUS_DISPLAY: u32 = 2u32;
pub const DEBUG_TYPEOPTS_MATCH_MAXSIZE: u32 = 8u32;
pub const DEBUG_TYPEOPTS_UNICODE_DISPLAY: u32 = 1u32;
pub const DEBUG_USER_WINDOWS_DUMP: u32 = 1025u32;
pub const DEBUG_USER_WINDOWS_DUMP_WINDOWS_CE: u32 = 1029u32;
pub const DEBUG_USER_WINDOWS_IDNA: u32 = 2u32;
pub const DEBUG_USER_WINDOWS_PROCESS: u32 = 0u32;
pub const DEBUG_USER_WINDOWS_PROCESS_SERVER: u32 = 1u32;
pub const DEBUG_USER_WINDOWS_REPT: u32 = 3u32;
pub const DEBUG_USER_WINDOWS_SMALL_DUMP: u32 = 1024u32;
pub const DEBUG_VALUE_FLOAT128: u32 = 9u32;
pub const DEBUG_VALUE_FLOAT32: u32 = 5u32;
pub const DEBUG_VALUE_FLOAT64: u32 = 6u32;
pub const DEBUG_VALUE_FLOAT80: u32 = 7u32;
pub const DEBUG_VALUE_FLOAT82: u32 = 8u32;
pub const DEBUG_VALUE_INT16: u32 = 2u32;
pub const DEBUG_VALUE_INT32: u32 = 3u32;
pub const DEBUG_VALUE_INT64: u32 = 4u32;
pub const DEBUG_VALUE_INT8: u32 = 1u32;
pub const DEBUG_VALUE_INVALID: u32 = 0u32;
pub const DEBUG_VALUE_TYPES: u32 = 12u32;
pub const DEBUG_VALUE_VECTOR128: u32 = 11u32;
pub const DEBUG_VALUE_VECTOR64: u32 = 10u32;
pub const DEBUG_VSEARCH_DEFAULT: u32 = 0u32;
pub const DEBUG_VSEARCH_WRITABLE_ONLY: u32 = 1u32;
pub const DEBUG_VSOURCE_DEBUGGEE: u32 = 1u32;
pub const DEBUG_VSOURCE_DUMP_WITHOUT_MEMINFO: u32 = 3u32;
pub const DEBUG_VSOURCE_INVALID: u32 = 0u32;
pub const DEBUG_VSOURCE_MAPPED_IMAGE: u32 = 2u32;
pub const DEBUG_WAIT_DEFAULT: u32 = 0u32;
pub const DISK_READ_0_BYTES: TANALYZE_RETURN = 3i32;
pub const DISK_WRITE: TANALYZE_RETURN = 4i32;
pub const DUMP_HANDLE_FLAG_CID_TABLE: u32 = 32u32;
pub const DUMP_HANDLE_FLAG_KERNEL_TABLE: u32 = 16u32;
pub const DUMP_HANDLE_FLAG_PRINT_FREE_ENTRY: u32 = 4u32;
pub const DUMP_HANDLE_FLAG_PRINT_OBJECT: u32 = 2u32;
pub const DbgPoolRegionMax: DEBUG_POOL_REGION = 6i32;
pub const DbgPoolRegionNonPaged: DEBUG_POOL_REGION = 3i32;
pub const DbgPoolRegionNonPagedExpansion: DEBUG_POOL_REGION = 4i32;
pub const DbgPoolRegionPaged: DEBUG_POOL_REGION = 2i32;
pub const DbgPoolRegionSessionPaged: DEBUG_POOL_REGION = 5i32;
pub const DbgPoolRegionSpecial: DEBUG_POOL_REGION = 1i32;
pub const DbgPoolRegionUnknown: DEBUG_POOL_REGION = 0i32;
pub const ERROR_DBG_CANCELLED: u32 = 3221226695u32;
pub const ERROR_DBG_TIMEOUT: u32 = 3221226932u32;
pub const EXIT_ON_CONTROLC: u32 = 8u32;
pub const EXIT_STATUS: TANALYZE_RETURN = 2i32;
pub const EXTDLL_DATA_QUERY_BUILD_BINDIR: u32 = 1u32;
pub const EXTDLL_DATA_QUERY_BUILD_BINDIR_SYMSRV: u32 = 11u32;
pub const EXTDLL_DATA_QUERY_BUILD_SYMDIR: u32 = 2u32;
pub const EXTDLL_DATA_QUERY_BUILD_SYMDIR_SYMSRV: u32 = 12u32;
pub const EXTDLL_DATA_QUERY_BUILD_WOW64BINDIR: u32 = 4u32;
pub const EXTDLL_DATA_QUERY_BUILD_WOW64BINDIR_SYMSRV: u32 = 14u32;
pub const EXTDLL_DATA_QUERY_BUILD_WOW64SYMDIR: u32 = 3u32;
pub const EXTDLL_DATA_QUERY_BUILD_WOW64SYMDIR_SYMSRV: u32 = 13u32;
pub const EXT_ANALYZER_FLAG_ID: u32 = 2u32;
pub const EXT_ANALYZER_FLAG_MOD: u32 = 1u32;
pub const EXT_API_VERSION_NUMBER: u32 = 5u32;
pub const EXT_API_VERSION_NUMBER32: u32 = 5u32;
pub const EXT_API_VERSION_NUMBER64: u32 = 6u32;
pub const EXT_FIND_FILE_ALLOW_GIVEN_PATH: u32 = 1u32;
pub const EXT_TDF_PHYSICAL_CACHED: u32 = 4u32;
pub const EXT_TDF_PHYSICAL_DEFAULT: u32 = 2u32;
pub const EXT_TDF_PHYSICAL_MEMORY: u32 = 14u32;
pub const EXT_TDF_PHYSICAL_UNCACHED: u32 = 6u32;
pub const EXT_TDF_PHYSICAL_WRITE_COMBINED: u32 = 8u32;
pub const EXT_TDOP_COPY: EXT_TDOP = 0i32;
pub const EXT_TDOP_COUNT: EXT_TDOP = 19i32;
pub const EXT_TDOP_EVALUATE: EXT_TDOP = 5i32;
pub const EXT_TDOP_GET_ARRAY_ELEMENT: EXT_TDOP = 12i32;
pub const EXT_TDOP_GET_DEREFERENCE: EXT_TDOP = 13i32;
pub const EXT_TDOP_GET_FIELD: EXT_TDOP = 4i32;
pub const EXT_TDOP_GET_FIELD_OFFSET: EXT_TDOP = 11i32;
pub const EXT_TDOP_GET_POINTER_TO: EXT_TDOP = 16i32;
pub const EXT_TDOP_GET_TYPE_NAME: EXT_TDOP = 6i32;
pub const EXT_TDOP_GET_TYPE_SIZE: EXT_TDOP = 14i32;
pub const EXT_TDOP_HAS_FIELD: EXT_TDOP = 10i32;
pub const EXT_TDOP_OUTPUT_FULL_VALUE: EXT_TDOP = 9i32;
pub const EXT_TDOP_OUTPUT_SIMPLE_VALUE: EXT_TDOP = 8i32;
pub const EXT_TDOP_OUTPUT_TYPE_DEFINITION: EXT_TDOP = 15i32;
pub const EXT_TDOP_OUTPUT_TYPE_NAME: EXT_TDOP = 7i32;
pub const EXT_TDOP_RELEASE: EXT_TDOP = 1i32;
pub const EXT_TDOP_SET_FROM_EXPR: EXT_TDOP = 2i32;
pub const EXT_TDOP_SET_FROM_TYPE_ID_AND_U64: EXT_TDOP = 17i32;
pub const EXT_TDOP_SET_FROM_U64_EXPR: EXT_TDOP = 3i32;
pub const EXT_TDOP_SET_PTR_FROM_TYPE_ID_AND_U64: EXT_TDOP = 18i32;
pub const ErrorClassError: ErrorClass = 1i32;
pub const ErrorClassWarning: ErrorClass = 0i32;
pub const FAILURE_ANALYSIS_ASSUME_HANG: u32 = 4u32;
pub const FAILURE_ANALYSIS_AUTOBUG_PROCESSING: u32 = 64u32;
pub const FAILURE_ANALYSIS_AUTOSET_SYMPATH: u32 = 16384u32;
pub const FAILURE_ANALYSIS_CALLSTACK_XML: u32 = 256u32;
pub const FAILURE_ANALYSIS_CALLSTACK_XML_FULL_SOURCE_INFO: u32 = 16777216u32;
pub const FAILURE_ANALYSIS_CREATE_INSTANCE: u32 = 1048576u32;
pub const FAILURE_ANALYSIS_EXCEPTION_AS_HANG: u32 = 32u32;
pub const FAILURE_ANALYSIS_HEAP_CORRUPTION_BLAME_FUNCTION: u32 = 33554432u32;
pub const FAILURE_ANALYSIS_IGNORE_BREAKIN: u32 = 8u32;
pub const FAILURE_ANALYSIS_LIVE_DEBUG_HOLD_CHECK: u32 = 2097152u32;
pub const FAILURE_ANALYSIS_MODULE_INFO_XML: u32 = 4096u32;
pub const FAILURE_ANALYSIS_MULTI_TARGET: u32 = 131072u32;
pub const FAILURE_ANALYSIS_NO_DB_LOOKUP: u32 = 1u32;
pub const FAILURE_ANALYSIS_NO_IMAGE_CORRUPTION: u32 = 8192u32;
pub const FAILURE_ANALYSIS_PERMIT_HEAP_ACCESS_VIOLATIONS: u32 = 67108864u32;
pub const FAILURE_ANALYSIS_REGISTRY_DATA: u32 = 512u32;
pub const FAILURE_ANALYSIS_SET_FAILURE_CONTEXT: u32 = 16u32;
pub const FAILURE_ANALYSIS_SHOW_SOURCE: u32 = 262144u32;
pub const FAILURE_ANALYSIS_SHOW_WCT_STACKS: u32 = 524288u32;
pub const FAILURE_ANALYSIS_USER_ATTRIBUTES: u32 = 2048u32;
pub const FAILURE_ANALYSIS_USER_ATTRIBUTES_ALL: u32 = 32768u32;
pub const FAILURE_ANALYSIS_USER_ATTRIBUTES_FRAMES: u32 = 65536u32;
pub const FAILURE_ANALYSIS_VERBOSE: u32 = 2u32;
pub const FAILURE_ANALYSIS_WMI_QUERY_DATA: u32 = 1024u32;
pub const FAILURE_ANALYSIS_XML_FILE_OUTPUT: u32 = 4194304u32;
pub const FAILURE_ANALYSIS_XML_OUTPUT: u32 = 128u32;
pub const FAILURE_ANALYSIS_XSD_VERIFY: u32 = 8388608u32;
pub const FAILURE_ANALYSIS_XSLT_FILE_INPUT: u32 = 268435456u32;
pub const FAILURE_ANALYSIS_XSLT_FILE_OUTPUT: u32 = 536870912u32;
pub const FA_PLUGIN_INITIALIZATION: FA_EXTENSION_PLUGIN_PHASE = 1i32;
pub const FA_PLUGIN_POST_BUCKETING: FA_EXTENSION_PLUGIN_PHASE = 8i32;
pub const FA_PLUGIN_PRE_BUCKETING: FA_EXTENSION_PLUGIN_PHASE = 4i32;
pub const FA_PLUGIN_STACK_ANALYSIS: FA_EXTENSION_PLUGIN_PHASE = 2i32;
pub const FIELDS_DID_NOT_MATCH: u32 = 4u32;
pub const FormatBSTRString: PreferredFormat = 8i32;
pub const FormatEnumNameOnly: PreferredFormat = 12i32;
pub const FormatEscapedStringWithQuote: PreferredFormat = 13i32;
pub const FormatHString: PreferredFormat = 10i32;
pub const FormatNone: PreferredFormat = 0i32;
pub const FormatQuotedHString: PreferredFormat = 9i32;
pub const FormatQuotedString: PreferredFormat = 2i32;
pub const FormatQuotedUTF32String: PreferredFormat = 15i32;
pub const FormatQuotedUTF8String: PreferredFormat = 6i32;
pub const FormatQuotedUnicodeString: PreferredFormat = 4i32;
pub const FormatRaw: PreferredFormat = 11i32;
pub const FormatSingleCharacter: PreferredFormat = 1i32;
pub const FormatString: PreferredFormat = 3i32;
pub const FormatUTF32String: PreferredFormat = 14i32;
pub const FormatUTF8String: PreferredFormat = 7i32;
pub const FormatUnicodeString: PreferredFormat = 5i32;
pub const IG_DISASSEMBLE_BUFFER: u32 = 44u32;
pub const IG_DUMP_SYMBOL_INFO: u32 = 22u32;
pub const IG_FIND_FILE: u32 = 40u32;
pub const IG_GET_ANY_MODULE_IN_RANGE: u32 = 45u32;
pub const IG_GET_BUS_DATA: u32 = 20u32;
pub const IG_GET_CACHE_SIZE: u32 = 32u32;
pub const IG_GET_CLR_DATA_INTERFACE: u32 = 38u32;
pub const IG_GET_CONTEXT_EX: u32 = 48u32;
pub const IG_GET_CURRENT_PROCESS: u32 = 26u32;
pub const IG_GET_CURRENT_PROCESS_HANDLE: u32 = 28u32;
pub const IG_GET_CURRENT_THREAD: u32 = 25u32;
pub const IG_GET_DEBUGGER_DATA: u32 = 14u32;
pub const IG_GET_EXCEPTION_RECORD: u32 = 18u32;
pub const IG_GET_EXPRESSION_EX: u32 = 30u32;
pub const IG_GET_INPUT_LINE: u32 = 29u32;
pub const IG_GET_KERNEL_VERSION: u32 = 15u32;
pub const IG_GET_PEB_ADDRESS: u32 = 129u32;
pub const IG_GET_SET_SYMPATH: u32 = 17u32;
pub const IG_GET_TEB_ADDRESS: u32 = 128u32;
pub const IG_GET_THREAD_OS_INFO: u32 = 37u32;
pub const IG_GET_TYPE_SIZE: u32 = 27u32;
pub const IG_IS_PTR64: u32 = 19u32;
pub const IG_KD_CONTEXT: u32 = 1u32;
pub const IG_KSTACK_HELP: u32 = 10u32;
pub const IG_LOWMEM_CHECK: u32 = 23u32;
pub const IG_MATCH_PATTERN_A: u32 = 39u32;
pub const IG_OBSOLETE_PLACEHOLDER_36: u32 = 36u32;
pub const IG_PHYSICAL_TO_VIRTUAL: u32 = 47u32;
pub const IG_POINTER_SEARCH_PHYSICAL: u32 = 35u32;
pub const IG_QUERY_TARGET_INTERFACE: u32 = 42u32;
pub const IG_READ_CONTROL_SPACE: u32 = 2u32;
pub const IG_READ_IO_SPACE: u32 = 4u32;
pub const IG_READ_IO_SPACE_EX: u32 = 8u32;
pub const IG_READ_MSR: u32 = 12u32;
pub const IG_READ_PHYSICAL: u32 = 6u32;
pub const IG_READ_PHYSICAL_WITH_FLAGS: u32 = 33u32;
pub const IG_RELOAD_SYMBOLS: u32 = 16u32;
pub const IG_SEARCH_MEMORY: u32 = 24u32;
pub const IG_SET_BUS_DATA: u32 = 21u32;
pub const IG_SET_THREAD: u32 = 11u32;
pub const IG_TRANSLATE_VIRTUAL_TO_PHYSICAL: u32 = 31u32;
pub const IG_TYPED_DATA: u32 = 43u32;
pub const IG_TYPED_DATA_OBSOLETE: u32 = 41u32;
pub const IG_VIRTUAL_TO_PHYSICAL: u32 = 46u32;
pub const IG_WRITE_CONTROL_SPACE: u32 = 3u32;
pub const IG_WRITE_IO_SPACE: u32 = 5u32;
pub const IG_WRITE_IO_SPACE_EX: u32 = 9u32;
pub const IG_WRITE_MSR: u32 = 13u32;
pub const IG_WRITE_PHYSICAL: u32 = 7u32;
pub const IG_WRITE_PHYSICAL_WITH_FLAGS: u32 = 34u32;
pub const INCORRECT_VERSION_INFO: u32 = 7u32;
pub const INSUFFICIENT_SPACE_TO_COPY: u32 = 10u32;
pub const Identical: SignatureComparison = 4i32;
pub const IntrinsicBool: IntrinsicKind = 1i32;
pub const IntrinsicChar: IntrinsicKind = 2i32;
pub const IntrinsicChar16: IntrinsicKind = 10i32;
pub const IntrinsicChar32: IntrinsicKind = 11i32;
pub const IntrinsicFloat: IntrinsicKind = 8i32;
pub const IntrinsicHRESULT: IntrinsicKind = 9i32;
pub const IntrinsicInt: IntrinsicKind = 4i32;
pub const IntrinsicLong: IntrinsicKind = 6i32;
pub const IntrinsicUInt: IntrinsicKind = 5i32;
pub const IntrinsicULong: IntrinsicKind = 7i32;
pub const IntrinsicVoid: IntrinsicKind = 0i32;
pub const IntrinsicWChar: IntrinsicKind = 3i32;
pub const KDEXTS_LOCK_CALLBACKROUTINE_DEFINED: u32 = 2u32;
pub const KD_SECONDARY_VERSION_AMD64_CONTEXT: u32 = 2u32;
pub const KD_SECONDARY_VERSION_AMD64_OBSOLETE_CONTEXT_1: u32 = 0u32;
pub const KD_SECONDARY_VERSION_AMD64_OBSOLETE_CONTEXT_2: u32 = 1u32;
pub const KD_SECONDARY_VERSION_DEFAULT: u32 = 0u32;
pub const LanguageAssembly: LanguageKind = 3i32;
pub const LanguageC: LanguageKind = 1i32;
pub const LanguageCPP: LanguageKind = 2i32;
pub const LanguageUnknown: LanguageKind = 0i32;
pub const LessSpecific: SignatureComparison = 2i32;
pub const LocationConstant: LocationKind = 2i32;
pub const LocationMember: LocationKind = 0i32;
pub const LocationNone: LocationKind = 3i32;
pub const LocationStatic: LocationKind = 1i32;
pub const MAX_STACK_IN_BYTES: u32 = 4096u32;
pub const MEMORY_READ_ERROR: u32 = 1u32;
pub const MODULE_ORDERS_LOADTIME: u32 = 268435456u32;
pub const MODULE_ORDERS_MASK: u32 = 4026531840u32;
pub const MODULE_ORDERS_MODULENAME: u32 = 536870912u32;
pub const MoreSpecific: SignatureComparison = 3i32;
pub const NO_TYPE: TANALYZE_RETURN = 0i32;
pub const NT_STATUS_CODE: TANALYZE_RETURN = 5i32;
pub const NULL_FIELD_NAME: u32 = 6u32;
pub const NULL_SYM_DUMP_PARAM: u32 = 5u32;
pub const ObjectContext: ModelObjectKind = 1i32;
pub const ObjectError: ModelObjectKind = 6i32;
pub const ObjectIntrinsic: ModelObjectKind = 7i32;
pub const ObjectKeyReference: ModelObjectKind = 9i32;
pub const ObjectMethod: ModelObjectKind = 8i32;
pub const ObjectNoValue: ModelObjectKind = 5i32;
pub const ObjectPropertyAccessor: ModelObjectKind = 0i32;
pub const ObjectSynthetic: ModelObjectKind = 4i32;
pub const ObjectTargetObject: ModelObjectKind = 2i32;
pub const ObjectTargetObjectReference: ModelObjectKind = 3i32;
pub const PHYS_FLAG_CACHED: u32 = 1u32;
pub const PHYS_FLAG_DEFAULT: u32 = 0u32;
pub const PHYS_FLAG_UNCACHED: u32 = 2u32;
pub const PHYS_FLAG_WRITE_COMBINED: u32 = 3u32;
pub const PROCESS_END: TANALYZE_RETURN = 1i32;
pub const PTR_SEARCH_NO_SYMBOL_CHECK: u32 = 2147483648u32;
pub const PTR_SEARCH_PHYS_ALL_HITS: u32 = 1u32;
pub const PTR_SEARCH_PHYS_PTE: u32 = 2u32;
pub const PTR_SEARCH_PHYS_RANGE_CHECK_ONLY: u32 = 4u32;
pub const PTR_SEARCH_PHYS_SIZE_SHIFT: u32 = 3u32;
pub const PointerCXHat: PointerKind = 3i32;
pub const PointerManagedReference: PointerKind = 4i32;
pub const PointerRValueReference: PointerKind = 2i32;
pub const PointerReference: PointerKind = 1i32;
pub const PointerStandard: PointerKind = 0i32;
pub const RawSearchNoBases: RawSearchFlags = 1i32;
pub const RawSearchNone: RawSearchFlags = 0i32;
pub const STACK_FRAME_TYPE_IGNORE: u32 = 255u32;
pub const STACK_FRAME_TYPE_INIT: u32 = 0u32;
pub const STACK_FRAME_TYPE_INLINE: u32 = 2u32;
pub const STACK_FRAME_TYPE_RA: u32 = 128u32;
pub const STACK_FRAME_TYPE_STACK: u32 = 1u32;
pub const SYMBOL_TYPE_INDEX_NOT_FOUND: u32 = 2u32;
pub const SYMBOL_TYPE_INFO_NOT_FOUND: u32 = 3u32;
pub const ScriptDebugAsyncBreak: ScriptDebugEvent = 3i32;
pub const ScriptDebugBreak: ScriptDebugState = 3i32;
pub const ScriptDebugBreakpoint: ScriptDebugEvent = 0i32;
pub const ScriptDebugEventFilterAbort: ScriptDebugEventFilter = 3i32;
pub const ScriptDebugEventFilterEntry: ScriptDebugEventFilter = 0i32;
pub const ScriptDebugEventFilterException: ScriptDebugEventFilter = 1i32;
pub const ScriptDebugEventFilterUnhandledException: ScriptDebugEventFilter = 2i32;
pub const ScriptDebugException: ScriptDebugEvent = 2i32;
pub const ScriptDebugExecuting: ScriptDebugState = 2i32;
pub const ScriptDebugNoDebugger: ScriptDebugState = 0i32;
pub const ScriptDebugNotExecuting: ScriptDebugState = 1i32;
pub const ScriptDebugStep: ScriptDebugEvent = 1i32;
pub const ScriptExecutionNormal: ScriptExecutionKind = 0i32;
pub const ScriptExecutionStepIn: ScriptExecutionKind = 1i32;
pub const ScriptExecutionStepOut: ScriptExecutionKind = 2i32;
pub const ScriptExecutionStepOver: ScriptExecutionKind = 3i32;
pub const ScriptRename: ScriptChangeKind = 0i32;
pub const Symbol: SymbolKind = 0i32;
pub const SymbolBaseClass: SymbolKind = 6i32;
pub const SymbolConstant: SymbolKind = 4i32;
pub const SymbolData: SymbolKind = 5i32;
pub const SymbolField: SymbolKind = 3i32;
pub const SymbolFunction: SymbolKind = 8i32;
pub const SymbolModule: SymbolKind = 1i32;
pub const SymbolPublic: SymbolKind = 7i32;
pub const SymbolSearchCaseInsensitive: SymbolSearchOptions = 2i32;
pub const SymbolSearchCompletion: SymbolSearchOptions = 1i32;
pub const SymbolSearchNone: SymbolSearchOptions = 0i32;
pub const SymbolType: SymbolKind = 2i32;
pub const TRIAGE_FOLLOWUP_DEFAULT: u32 = 2u32;
pub const TRIAGE_FOLLOWUP_FAIL: u32 = 0u32;
pub const TRIAGE_FOLLOWUP_IGNORE: u32 = 1u32;
pub const TRIAGE_FOLLOWUP_SUCCESS: u32 = 3u32;
pub const TypeArray: TypeKind = 3i32;
pub const TypeEnum: TypeKind = 6i32;
pub const TypeExtendedArray: TypeKind = 8i32;
pub const TypeFunction: TypeKind = 4i32;
pub const TypeIntrinsic: TypeKind = 7i32;
pub const TypeMemberPointer: TypeKind = 2i32;
pub const TypePointer: TypeKind = 1i32;
pub const TypeTypedef: TypeKind = 5i32;
pub const TypeUDT: TypeKind = 0i32;
pub const UNAVAILABLE_ERROR: u32 = 12u32;
pub const Unrelated: SignatureComparison = 0i32;
pub const VarArgsCStyle: VarArgsKind = 1i32;
pub const VarArgsNone: VarArgsKind = 0i32;
pub const WDBGEXTS_ADDRESS_DEFAULT: u32 = 0u32;
pub const WDBGEXTS_ADDRESS_RESERVED0: u32 = 2147483648u32;
pub const WDBGEXTS_ADDRESS_SEG16: u32 = 1u32;
pub const WDBGEXTS_ADDRESS_SEG32: u32 = 2u32;
pub const WIN_95: OS_TYPE = 0i32;
pub const WIN_98: OS_TYPE = 1i32;
pub const WIN_ME: OS_TYPE = 2i32;
pub const WIN_NT4: OS_TYPE = 3i32;
pub const WIN_NT5: OS_TYPE = 4i32;
pub const WIN_NT5_1: OS_TYPE = 5i32;
pub const WIN_NT5_2: OS_TYPE = 6i32;
pub const WIN_NT6_0: OS_TYPE = 7i32;
pub const WIN_NT6_1: OS_TYPE = 8i32;
pub const WIN_UNDEFINED: OS_TYPE = 255i32;
pub const _EXTSAPI_VER_: u32 = 10u32;
pub type CallingConventionKind = i32;
pub type DBGKD_MAJOR_TYPES = i32;
pub type DEBUG_FAILURE_TYPE = i32;
pub type DEBUG_FLR_PARAM_TYPE = i32;
pub type DEBUG_POOL_REGION = i32;
pub type EXT_TDOP = i32;
pub type ErrorClass = i32;
pub type FA_ENTRY_TYPE = i32;
pub type FA_EXTENSION_PLUGIN_PHASE = i32;
pub type IntrinsicKind = i32;
pub type LanguageKind = i32;
pub type LocationKind = i32;
pub type ModelObjectKind = i32;
pub type OS_TYPE = i32;
pub type PointerKind = i32;
pub type PreferredFormat = i32;
pub type RawSearchFlags = i32;
pub type ScriptChangeKind = i32;
pub type ScriptDebugEvent = i32;
pub type ScriptDebugEventFilter = i32;
pub type ScriptDebugState = i32;
pub type ScriptExecutionKind = i32;
pub type SignatureComparison = i32;
pub type SymbolKind = i32;
pub type SymbolSearchOptions = i32;
pub type TANALYZE_RETURN = i32;
pub type TypeKind = i32;
pub type VarArgsKind = i32;
#[repr(C)]
pub struct ArrayDimension {
    pub LowerBound: i64,
    pub Length: u64,
    pub Stride: u64,
}
impl ::core::marker::Copy for ArrayDimension {}
impl ::core::clone::Clone for ArrayDimension {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BUSDATA {
    pub BusDataType: u32,
    pub BusNumber: u32,
    pub SlotNumber: u32,
    pub Buffer: *mut ::core::ffi::c_void,
    pub Offset: u32,
    pub Length: u32,
}
impl ::core::marker::Copy for BUSDATA {}
impl ::core::clone::Clone for BUSDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CKCL_DATA {
    pub NextLogEvent: *mut ::core::ffi::c_void,
    pub TAnalyzeString: ::windows_sys::core::PSTR,
    pub TAnalyzeReturnType: TANALYZE_RETURN,
}
impl ::core::marker::Copy for CKCL_DATA {}
impl ::core::clone::Clone for CKCL_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CKCL_LISTHEAD {
    pub LogEventListHead: *mut CKCL_DATA,
    pub Heap: super::super::super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CKCL_LISTHEAD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CKCL_LISTHEAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPU_INFO {
    pub Type: u32,
    pub NumCPUs: u32,
    pub CurrentProc: u32,
    pub ProcInfo: [DEBUG_PROCESSOR_IDENTIFICATION_ALL; 2048],
    pub Mhz: u32,
}
impl ::core::marker::Copy for CPU_INFO {}
impl ::core::clone::Clone for CPU_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPU_INFO_v1 {
    pub Type: u32,
    pub NumCPUs: u32,
    pub CurrentProc: u32,
    pub ProcInfo: [DEBUG_PROCESSOR_IDENTIFICATION_ALL; 32],
    pub Mhz: u32,
}
impl ::core::marker::Copy for CPU_INFO_v1 {}
impl ::core::clone::Clone for CPU_INFO_v1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPU_INFO_v2 {
    pub Type: u32,
    pub NumCPUs: u32,
    pub CurrentProc: u32,
    pub ProcInfo: [DEBUG_PROCESSOR_IDENTIFICATION_ALL; 1280],
    pub Mhz: u32,
}
impl ::core::marker::Copy for CPU_INFO_v2 {}
impl ::core::clone::Clone for CPU_INFO_v2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct DBGKD_DEBUG_DATA_HEADER32 {
    pub List: super::super::super::Kernel::LIST_ENTRY32,
    pub OwnerTag: u32,
    pub Size: u32,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for DBGKD_DEBUG_DATA_HEADER32 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for DBGKD_DEBUG_DATA_HEADER32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct DBGKD_DEBUG_DATA_HEADER64 {
    pub List: super::super::super::Kernel::LIST_ENTRY64,
    pub OwnerTag: u32,
    pub Size: u32,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for DBGKD_DEBUG_DATA_HEADER64 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for DBGKD_DEBUG_DATA_HEADER64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DBGKD_GET_VERSION32 {
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub ProtocolVersion: u16,
    pub Flags: u16,
    pub KernBase: u32,
    pub PsLoadedModuleList: u32,
    pub MachineType: u16,
    pub ThCallbackStack: u16,
    pub NextCallback: u16,
    pub FramePointer: u16,
    pub KiCallUserMode: u32,
    pub KeUserCallbackDispatcher: u32,
    pub BreakpointWithStatus: u32,
    pub DebuggerDataList: u32,
}
impl ::core::marker::Copy for DBGKD_GET_VERSION32 {}
impl ::core::clone::Clone for DBGKD_GET_VERSION32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DBGKD_GET_VERSION64 {
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub ProtocolVersion: u8,
    pub KdSecondaryVersion: u8,
    pub Flags: u16,
    pub MachineType: u16,
    pub MaxPacketType: u8,
    pub MaxStateChange: u8,
    pub MaxManipulate: u8,
    pub Simulation: u8,
    pub Unused: [u16; 1],
    pub KernBase: u64,
    pub PsLoadedModuleList: u64,
    pub DebuggerDataList: u64,
}
impl ::core::marker::Copy for DBGKD_GET_VERSION64 {}
impl ::core::clone::Clone for DBGKD_GET_VERSION64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DBG_THREAD_ATTRIBUTES {
    pub ThreadIndex: u32,
    pub ProcessID: u64,
    pub ThreadID: u64,
    pub AttributeBits: u64,
    pub BoolBits: u32,
    pub BlockedOnPID: u64,
    pub BlockedOnTID: u64,
    pub CritSecAddress: u64,
    pub Timeout_msec: u32,
    pub StringData: [u8; 100],
    pub SymName: [u8; 100],
}
impl ::core::marker::Copy for DBG_THREAD_ATTRIBUTES {}
impl ::core::clone::Clone for DBG_THREAD_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_ANALYSIS_PROCESSOR_INFO {
    pub SizeOfStruct: u32,
    pub Model: u32,
    pub Family: u32,
    pub Stepping: u32,
    pub Architecture: u32,
    pub Revision: u32,
    pub CurrentClockSpeed: u32,
    pub CurrentVoltage: u32,
    pub MaxClockSpeed: u32,
    pub ProcessorType: u32,
    pub DeviceID: [u8; 32],
    pub Manufacturer: [u8; 64],
    pub Name: [u8; 64],
    pub Version: [u8; 64],
    pub Description: [u8; 64],
}
impl ::core::marker::Copy for DEBUG_ANALYSIS_PROCESSOR_INFO {}
impl ::core::clone::Clone for DEBUG_ANALYSIS_PROCESSOR_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_BREAKPOINT_PARAMETERS {
    pub Offset: u64,
    pub Id: u32,
    pub BreakType: u32,
    pub ProcType: u32,
    pub Flags: u32,
    pub DataSize: u32,
    pub DataAccessType: u32,
    pub PassCount: u32,
    pub CurrentPassCount: u32,
    pub MatchThread: u32,
    pub CommandSize: u32,
    pub OffsetExpressionSize: u32,
}
impl ::core::marker::Copy for DEBUG_BREAKPOINT_PARAMETERS {}
impl ::core::clone::Clone for DEBUG_BREAKPOINT_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_CACHED_SYMBOL_INFO {
    pub ModBase: u64,
    pub Arg1: u64,
    pub Arg2: u64,
    pub Id: u32,
    pub Arg3: u32,
}
impl ::core::marker::Copy for DEBUG_CACHED_SYMBOL_INFO {}
impl ::core::clone::Clone for DEBUG_CACHED_SYMBOL_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_CLIENT_CONTEXT {
    pub cbSize: u32,
    pub eClient: u32,
}
impl ::core::marker::Copy for DEBUG_CLIENT_CONTEXT {}
impl ::core::clone::Clone for DEBUG_CLIENT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_CPU_MICROCODE_VERSION {
    pub SizeOfStruct: u32,
    pub CachedSignature: i64,
    pub InitialSignature: i64,
    pub ProcessorModel: u32,
    pub ProcessorFamily: u32,
    pub ProcessorStepping: u32,
    pub ProcessorArchRev: u32,
}
impl ::core::marker::Copy for DEBUG_CPU_MICROCODE_VERSION {}
impl ::core::clone::Clone for DEBUG_CPU_MICROCODE_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_CPU_SPEED_INFO {
    pub SizeOfStruct: u32,
    pub CurrentSpeed: u32,
    pub RatedSpeed: u32,
    pub NameString: [u16; 256],
}
impl ::core::marker::Copy for DEBUG_CPU_SPEED_INFO {}
impl ::core::clone::Clone for DEBUG_CPU_SPEED_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_CREATE_PROCESS_OPTIONS {
    pub CreateFlags: u32,
    pub EngCreateFlags: u32,
    pub VerifierFlags: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for DEBUG_CREATE_PROCESS_OPTIONS {}
impl ::core::clone::Clone for DEBUG_CREATE_PROCESS_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_DECODE_ERROR {
    pub SizeOfStruct: u32,
    pub Code: u32,
    pub TreatAsStatus: super::super::super::super::Foundation::BOOL,
    pub Source: [u8; 64],
    pub Message: [u8; 260],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_DECODE_ERROR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_DECODE_ERROR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_DEVICE_OBJECT_INFO {
    pub SizeOfStruct: u32,
    pub DevObjAddress: u64,
    pub ReferenceCount: u32,
    pub QBusy: super::super::super::super::Foundation::BOOL,
    pub DriverObject: u64,
    pub CurrentIrp: u64,
    pub DevExtension: u64,
    pub DevObjExtension: u64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_DEVICE_OBJECT_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_DEVICE_OBJECT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_DRIVER_OBJECT_INFO {
    pub SizeOfStruct: u32,
    pub DriverSize: u32,
    pub DriverObjAddress: u64,
    pub DriverStart: u64,
    pub DriverExtension: u64,
    pub DeviceObject: u64,
    pub DriverName: DEBUG_DRIVER_OBJECT_INFO_0,
}
impl ::core::marker::Copy for DEBUG_DRIVER_OBJECT_INFO {}
impl ::core::clone::Clone for DEBUG_DRIVER_OBJECT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_DRIVER_OBJECT_INFO_0 {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: u64,
}
impl ::core::marker::Copy for DEBUG_DRIVER_OBJECT_INFO_0 {}
impl ::core::clone::Clone for DEBUG_DRIVER_OBJECT_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_EVENT_CONTEXT {
    pub Size: u32,
    pub ProcessEngineId: u32,
    pub ThreadEngineId: u32,
    pub FrameEngineId: u32,
}
impl ::core::marker::Copy for DEBUG_EVENT_CONTEXT {}
impl ::core::clone::Clone for DEBUG_EVENT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_EXCEPTION_FILTER_PARAMETERS {
    pub ExecutionOption: u32,
    pub ContinueOption: u32,
    pub TextSize: u32,
    pub CommandSize: u32,
    pub SecondCommandSize: u32,
    pub ExceptionCode: u32,
}
impl ::core::marker::Copy for DEBUG_EXCEPTION_FILTER_PARAMETERS {}
impl ::core::clone::Clone for DEBUG_EXCEPTION_FILTER_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_GET_TEXT_COMPLETIONS_IN {
    pub Flags: u32,
    pub MatchCountLimit: u32,
    pub Reserved: [u64; 3],
}
impl ::core::marker::Copy for DEBUG_GET_TEXT_COMPLETIONS_IN {}
impl ::core::clone::Clone for DEBUG_GET_TEXT_COMPLETIONS_IN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_GET_TEXT_COMPLETIONS_OUT {
    pub Flags: u32,
    pub ReplaceIndex: u32,
    pub MatchCount: u32,
    pub Reserved1: u32,
    pub Reserved2: [u64; 2],
}
impl ::core::marker::Copy for DEBUG_GET_TEXT_COMPLETIONS_OUT {}
impl ::core::clone::Clone for DEBUG_GET_TEXT_COMPLETIONS_OUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_HANDLE_DATA_BASIC {
    pub TypeNameSize: u32,
    pub ObjectNameSize: u32,
    pub Attributes: u32,
    pub GrantedAccess: u32,
    pub HandleCount: u32,
    pub PointerCount: u32,
}
impl ::core::marker::Copy for DEBUG_HANDLE_DATA_BASIC {}
impl ::core::clone::Clone for DEBUG_HANDLE_DATA_BASIC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_IRP_INFO {
    pub SizeOfStruct: u32,
    pub IrpAddress: u64,
    pub IoStatus: u32,
    pub StackCount: u32,
    pub CurrentLocation: u32,
    pub MdlAddress: u64,
    pub Thread: u64,
    pub CancelRoutine: u64,
    pub CurrentStack: DEBUG_IRP_STACK_INFO,
    pub Stack: [DEBUG_IRP_STACK_INFO; 10],
}
impl ::core::marker::Copy for DEBUG_IRP_INFO {}
impl ::core::clone::Clone for DEBUG_IRP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_IRP_STACK_INFO {
    pub Major: u8,
    pub Minor: u8,
    pub DeviceObject: u64,
    pub FileObject: u64,
    pub CompletionRoutine: u64,
    pub StackAddress: u64,
}
impl ::core::marker::Copy for DEBUG_IRP_STACK_INFO {}
impl ::core::clone::Clone for DEBUG_IRP_STACK_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_LAST_EVENT_INFO_BREAKPOINT {
    pub Id: u32,
}
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_BREAKPOINT {}
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_BREAKPOINT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_LAST_EVENT_INFO_EXCEPTION {
    pub ExceptionRecord: super::EXCEPTION_RECORD64,
    pub FirstChance: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_EXCEPTION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_EXCEPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_LAST_EVENT_INFO_EXIT_PROCESS {
    pub ExitCode: u32,
}
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_EXIT_PROCESS {}
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_EXIT_PROCESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_LAST_EVENT_INFO_EXIT_THREAD {
    pub ExitCode: u32,
}
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_EXIT_THREAD {}
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_EXIT_THREAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_LAST_EVENT_INFO_LOAD_MODULE {
    pub Base: u64,
}
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_LOAD_MODULE {}
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_LOAD_MODULE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_LAST_EVENT_INFO_SERVICE_EXCEPTION {
    pub Kind: u32,
    pub DataSize: u32,
    pub Address: u64,
}
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_SERVICE_EXCEPTION {}
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_SERVICE_EXCEPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_LAST_EVENT_INFO_SYSTEM_ERROR {
    pub Error: u32,
    pub Level: u32,
}
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_SYSTEM_ERROR {}
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_SYSTEM_ERROR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_LAST_EVENT_INFO_UNLOAD_MODULE {
    pub Base: u64,
}
impl ::core::marker::Copy for DEBUG_LAST_EVENT_INFO_UNLOAD_MODULE {}
impl ::core::clone::Clone for DEBUG_LAST_EVENT_INFO_UNLOAD_MODULE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_MODULE_AND_ID {
    pub ModuleBase: u64,
    pub Id: u64,
}
impl ::core::marker::Copy for DEBUG_MODULE_AND_ID {}
impl ::core::clone::Clone for DEBUG_MODULE_AND_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_MODULE_PARAMETERS {
    pub Base: u64,
    pub Size: u32,
    pub TimeDateStamp: u32,
    pub Checksum: u32,
    pub Flags: u32,
    pub SymbolType: u32,
    pub ImageNameSize: u32,
    pub ModuleNameSize: u32,
    pub LoadedImageNameSize: u32,
    pub SymbolFileNameSize: u32,
    pub MappedImageNameSize: u32,
    pub Reserved: [u64; 2],
}
impl ::core::marker::Copy for DEBUG_MODULE_PARAMETERS {}
impl ::core::clone::Clone for DEBUG_MODULE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_OFFSET_REGION {
    pub Base: u64,
    pub Size: u64,
}
impl ::core::marker::Copy for DEBUG_OFFSET_REGION {}
impl ::core::clone::Clone for DEBUG_OFFSET_REGION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_PNP_TRIAGE_INFO {
    pub SizeOfStruct: u32,
    pub Lock_Address: u64,
    pub Lock_ActiveCount: i32,
    pub Lock_ContentionCount: u32,
    pub Lock_NumberOfExclusiveWaiters: u32,
    pub Lock_NumberOfSharedWaiters: u32,
    pub Lock_Flag: u16,
    pub TriagedThread: u64,
    pub ThreadCount: i32,
    pub TriagedThread_WaitTime: u64,
}
impl ::core::marker::Copy for DEBUG_PNP_TRIAGE_INFO {}
impl ::core::clone::Clone for DEBUG_PNP_TRIAGE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_POOLTAG_DESCRIPTION {
    pub SizeOfStruct: u32,
    pub PoolTag: u32,
    pub Description: [u8; 260],
    pub Binary: [u8; 32],
    pub Owner: [u8; 32],
}
impl ::core::marker::Copy for DEBUG_POOLTAG_DESCRIPTION {}
impl ::core::clone::Clone for DEBUG_POOLTAG_DESCRIPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_POOL_DATA {
    pub SizeofStruct: u32,
    pub PoolBlock: u64,
    pub Pool: u64,
    pub PreviousSize: u32,
    pub Size: u32,
    pub PoolTag: u32,
    pub ProcessBilled: u64,
    pub Anonymous: DEBUG_POOL_DATA_0,
    pub Reserved2: [u64; 4],
    pub PoolTagDescription: [u8; 64],
}
impl ::core::marker::Copy for DEBUG_POOL_DATA {}
impl ::core::clone::Clone for DEBUG_POOL_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union DEBUG_POOL_DATA_0 {
    pub Anonymous: DEBUG_POOL_DATA_0_0,
    pub AsUlong: u32,
}
impl ::core::marker::Copy for DEBUG_POOL_DATA_0 {}
impl ::core::clone::Clone for DEBUG_POOL_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_POOL_DATA_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for DEBUG_POOL_DATA_0_0 {}
impl ::core::clone::Clone for DEBUG_POOL_DATA_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union DEBUG_PROCESSOR_IDENTIFICATION_ALL {
    pub Alpha: DEBUG_PROCESSOR_IDENTIFICATION_ALPHA,
    pub Amd64: DEBUG_PROCESSOR_IDENTIFICATION_AMD64,
    pub Ia64: DEBUG_PROCESSOR_IDENTIFICATION_IA64,
    pub X86: DEBUG_PROCESSOR_IDENTIFICATION_X86,
    pub Arm: DEBUG_PROCESSOR_IDENTIFICATION_ARM,
    pub Arm64: DEBUG_PROCESSOR_IDENTIFICATION_ARM64,
}
impl ::core::marker::Copy for DEBUG_PROCESSOR_IDENTIFICATION_ALL {}
impl ::core::clone::Clone for DEBUG_PROCESSOR_IDENTIFICATION_ALL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_PROCESSOR_IDENTIFICATION_ALPHA {
    pub Type: u32,
    pub Revision: u32,
}
impl ::core::marker::Copy for DEBUG_PROCESSOR_IDENTIFICATION_ALPHA {}
impl ::core::clone::Clone for DEBUG_PROCESSOR_IDENTIFICATION_ALPHA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_PROCESSOR_IDENTIFICATION_AMD64 {
    pub Family: u32,
    pub Model: u32,
    pub Stepping: u32,
    pub VendorString: [u8; 16],
}
impl ::core::marker::Copy for DEBUG_PROCESSOR_IDENTIFICATION_AMD64 {}
impl ::core::clone::Clone for DEBUG_PROCESSOR_IDENTIFICATION_AMD64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_PROCESSOR_IDENTIFICATION_ARM {
    pub Model: u32,
    pub Revision: u32,
    pub VendorString: [u8; 16],
}
impl ::core::marker::Copy for DEBUG_PROCESSOR_IDENTIFICATION_ARM {}
impl ::core::clone::Clone for DEBUG_PROCESSOR_IDENTIFICATION_ARM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_PROCESSOR_IDENTIFICATION_ARM64 {
    pub Model: u32,
    pub Revision: u32,
    pub VendorString: [u8; 16],
}
impl ::core::marker::Copy for DEBUG_PROCESSOR_IDENTIFICATION_ARM64 {}
impl ::core::clone::Clone for DEBUG_PROCESSOR_IDENTIFICATION_ARM64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_PROCESSOR_IDENTIFICATION_IA64 {
    pub Model: u32,
    pub Revision: u32,
    pub Family: u32,
    pub ArchRev: u32,
    pub VendorString: [u8; 16],
}
impl ::core::marker::Copy for DEBUG_PROCESSOR_IDENTIFICATION_IA64 {}
impl ::core::clone::Clone for DEBUG_PROCESSOR_IDENTIFICATION_IA64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_PROCESSOR_IDENTIFICATION_X86 {
    pub Family: u32,
    pub Model: u32,
    pub Stepping: u32,
    pub VendorString: [u8; 16],
}
impl ::core::marker::Copy for DEBUG_PROCESSOR_IDENTIFICATION_X86 {}
impl ::core::clone::Clone for DEBUG_PROCESSOR_IDENTIFICATION_X86 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_READ_USER_MINIDUMP_STREAM {
    pub StreamType: u32,
    pub Flags: u32,
    pub Offset: u64,
    pub Buffer: *mut ::core::ffi::c_void,
    pub BufferSize: u32,
    pub BufferUsed: u32,
}
impl ::core::marker::Copy for DEBUG_READ_USER_MINIDUMP_STREAM {}
impl ::core::clone::Clone for DEBUG_READ_USER_MINIDUMP_STREAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_REGISTER_DESCRIPTION {
    pub Type: u32,
    pub Flags: u32,
    pub SubregMaster: u32,
    pub SubregLength: u32,
    pub SubregMask: u64,
    pub SubregShift: u32,
    pub Reserved0: u32,
}
impl ::core::marker::Copy for DEBUG_REGISTER_DESCRIPTION {}
impl ::core::clone::Clone for DEBUG_REGISTER_DESCRIPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_SMBIOS_INFO {
    pub SizeOfStruct: u32,
    pub SmbiosMajorVersion: u8,
    pub SmbiosMinorVersion: u8,
    pub DMIVersion: u8,
    pub TableSize: u32,
    pub BiosMajorRelease: u8,
    pub BiosMinorRelease: u8,
    pub FirmwareMajorRelease: u8,
    pub FirmwareMinorRelease: u8,
    pub BaseBoardManufacturer: [u8; 64],
    pub BaseBoardProduct: [u8; 64],
    pub BaseBoardVersion: [u8; 64],
    pub BiosReleaseDate: [u8; 64],
    pub BiosVendor: [u8; 64],
    pub BiosVersion: [u8; 64],
    pub SystemFamily: [u8; 64],
    pub SystemManufacturer: [u8; 64],
    pub SystemProductName: [u8; 64],
    pub SystemSKU: [u8; 64],
    pub SystemVersion: [u8; 64],
}
impl ::core::marker::Copy for DEBUG_SMBIOS_INFO {}
impl ::core::clone::Clone for DEBUG_SMBIOS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_SPECIFIC_FILTER_PARAMETERS {
    pub ExecutionOption: u32,
    pub ContinueOption: u32,
    pub TextSize: u32,
    pub CommandSize: u32,
    pub ArgumentSize: u32,
}
impl ::core::marker::Copy for DEBUG_SPECIFIC_FILTER_PARAMETERS {}
impl ::core::clone::Clone for DEBUG_SPECIFIC_FILTER_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_STACK_FRAME {
    pub InstructionOffset: u64,
    pub ReturnOffset: u64,
    pub FrameOffset: u64,
    pub StackOffset: u64,
    pub FuncTableEntry: u64,
    pub Params: [u64; 4],
    pub Reserved: [u64; 6],
    pub Virtual: super::super::super::super::Foundation::BOOL,
    pub FrameNumber: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_STACK_FRAME {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_STACK_FRAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_STACK_FRAME_EX {
    pub InstructionOffset: u64,
    pub ReturnOffset: u64,
    pub FrameOffset: u64,
    pub StackOffset: u64,
    pub FuncTableEntry: u64,
    pub Params: [u64; 4],
    pub Reserved: [u64; 6],
    pub Virtual: super::super::super::super::Foundation::BOOL,
    pub FrameNumber: u32,
    pub InlineFrameContext: u32,
    pub Reserved1: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_STACK_FRAME_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_STACK_FRAME_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_SYMBOL_ENTRY {
    pub ModuleBase: u64,
    pub Offset: u64,
    pub Id: u64,
    pub Arg64: u64,
    pub Size: u32,
    pub Flags: u32,
    pub TypeId: u32,
    pub NameSize: u32,
    pub Token: u32,
    pub Tag: u32,
    pub Arg32: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for DEBUG_SYMBOL_ENTRY {}
impl ::core::clone::Clone for DEBUG_SYMBOL_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_SYMBOL_PARAMETERS {
    pub Module: u64,
    pub TypeId: u32,
    pub ParentSymbol: u32,
    pub SubElements: u32,
    pub Flags: u32,
    pub Reserved: u64,
}
impl ::core::marker::Copy for DEBUG_SYMBOL_PARAMETERS {}
impl ::core::clone::Clone for DEBUG_SYMBOL_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_SYMBOL_SOURCE_ENTRY {
    pub ModuleBase: u64,
    pub Offset: u64,
    pub FileNameId: u64,
    pub EngineInternal: u64,
    pub Size: u32,
    pub Flags: u32,
    pub FileNameSize: u32,
    pub StartLine: u32,
    pub EndLine: u32,
    pub StartColumn: u32,
    pub EndColumn: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for DEBUG_SYMBOL_SOURCE_ENTRY {}
impl ::core::clone::Clone for DEBUG_SYMBOL_SOURCE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_THREAD_BASIC_INFORMATION {
    pub Valid: u32,
    pub ExitStatus: u32,
    pub PriorityClass: u32,
    pub Priority: u32,
    pub CreateTime: u64,
    pub ExitTime: u64,
    pub KernelTime: u64,
    pub UserTime: u64,
    pub StartOffset: u64,
    pub Affinity: u64,
}
impl ::core::marker::Copy for DEBUG_THREAD_BASIC_INFORMATION {}
impl ::core::clone::Clone for DEBUG_THREAD_BASIC_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_TRIAGE_FOLLOWUP_INFO {
    pub SizeOfStruct: u32,
    pub OwnerNameSize: u32,
    pub OwnerName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for DEBUG_TRIAGE_FOLLOWUP_INFO {}
impl ::core::clone::Clone for DEBUG_TRIAGE_FOLLOWUP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_TRIAGE_FOLLOWUP_INFO_2 {
    pub SizeOfStruct: u32,
    pub OwnerNameSize: u32,
    pub OwnerName: ::windows_sys::core::PSTR,
    pub FeaturePathSize: u32,
    pub FeaturePath: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for DEBUG_TRIAGE_FOLLOWUP_INFO_2 {}
impl ::core::clone::Clone for DEBUG_TRIAGE_FOLLOWUP_INFO_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DEBUG_TYPED_DATA {
    pub ModBase: u64,
    pub Offset: u64,
    pub EngineHandle: u64,
    pub Data: u64,
    pub Size: u32,
    pub Flags: u32,
    pub TypeId: u32,
    pub BaseTypeId: u32,
    pub Tag: u32,
    pub Register: u32,
    pub Internal: [u64; 9],
}
impl ::core::marker::Copy for DEBUG_TYPED_DATA {}
impl ::core::clone::Clone for DEBUG_TYPED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_VALUE {
    pub Anonymous: DEBUG_VALUE_0,
    pub TailOfRawBytes: u32,
    pub Type: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_VALUE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union DEBUG_VALUE_0 {
    pub I8: u8,
    pub I16: u16,
    pub I32: u32,
    pub Anonymous: DEBUG_VALUE_0_0,
    pub F32: f32,
    pub F64: f64,
    pub F80Bytes: [u8; 10],
    pub F82Bytes: [u8; 11],
    pub F128Bytes: [u8; 16],
    pub VI8: [u8; 16],
    pub VI16: [u16; 8],
    pub VI32: [u32; 4],
    pub VI64: [u64; 2],
    pub VF32: [f32; 4],
    pub VF64: [f64; 2],
    pub I64Parts32: DEBUG_VALUE_0_2,
    pub F128Parts64: DEBUG_VALUE_0_1,
    pub RawBytes: [u8; 24],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_VALUE_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_VALUE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_VALUE_0_0 {
    pub I64: u64,
    pub Nat: super::super::super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_VALUE_0_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_VALUE_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_VALUE_0_1 {
    pub LowPart: u64,
    pub HighPart: i64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_VALUE_0_1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_VALUE_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DEBUG_VALUE_0_2 {
    pub LowPart: u32,
    pub HighPart: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DEBUG_VALUE_0_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DEBUG_VALUE_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXTSTACKTRACE {
    pub FramePointer: u32,
    pub ProgramCounter: u32,
    pub ReturnAddress: u32,
    pub Args: [u32; 4],
}
impl ::core::marker::Copy for EXTSTACKTRACE {}
impl ::core::clone::Clone for EXTSTACKTRACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXTSTACKTRACE32 {
    pub FramePointer: u32,
    pub ProgramCounter: u32,
    pub ReturnAddress: u32,
    pub Args: [u32; 4],
}
impl ::core::marker::Copy for EXTSTACKTRACE32 {}
impl ::core::clone::Clone for EXTSTACKTRACE32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXTSTACKTRACE64 {
    pub FramePointer: u64,
    pub ProgramCounter: u64,
    pub ReturnAddress: u64,
    pub Args: [u64; 4],
}
impl ::core::marker::Copy for EXTSTACKTRACE64 {}
impl ::core::clone::Clone for EXTSTACKTRACE64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXT_API_VERSION {
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub Revision: u16,
    pub Reserved: u16,
}
impl ::core::marker::Copy for EXT_API_VERSION {}
impl ::core::clone::Clone for EXT_API_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXT_CAB_XML_DATA {
    pub SizeOfStruct: u32,
    pub XmlObjectTag: ::windows_sys::core::PCWSTR,
    pub NumSubTags: u32,
    pub SubTags: [EXT_CAB_XML_DATA_0; 1],
}
impl ::core::marker::Copy for EXT_CAB_XML_DATA {}
impl ::core::clone::Clone for EXT_CAB_XML_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXT_CAB_XML_DATA_0 {
    pub SubTag: ::windows_sys::core::PCWSTR,
    pub MatchPattern: ::windows_sys::core::PCWSTR,
    pub ReturnText: ::windows_sys::core::PWSTR,
    pub ReturnTextSize: u32,
    pub _bitfield: u32,
    pub Reserved2: u32,
}
impl ::core::marker::Copy for EXT_CAB_XML_DATA_0 {}
impl ::core::clone::Clone for EXT_CAB_XML_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EXT_FIND_FILE {
    pub FileName: ::windows_sys::core::PCWSTR,
    pub IndexedSize: u64,
    pub ImageTimeDateStamp: u32,
    pub ImageCheckSum: u32,
    pub ExtraInfo: *mut ::core::ffi::c_void,
    pub ExtraInfoSize: u32,
    pub Flags: u32,
    pub FileMapping: *mut ::core::ffi::c_void,
    pub FileMappingSize: u64,
    pub FileHandle: super::super::super::super::Foundation::HANDLE,
    pub FoundFileName: ::windows_sys::core::PWSTR,
    pub FoundFileNameChars: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EXT_FIND_FILE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EXT_FIND_FILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXT_MATCH_PATTERN_A {
    pub Str: ::windows_sys::core::PCSTR,
    pub Pattern: ::windows_sys::core::PCSTR,
    pub CaseSensitive: u32,
}
impl ::core::marker::Copy for EXT_MATCH_PATTERN_A {}
impl ::core::clone::Clone for EXT_MATCH_PATTERN_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXT_TYPED_DATA {
    pub Operation: EXT_TDOP,
    pub Flags: u32,
    pub InData: DEBUG_TYPED_DATA,
    pub OutData: DEBUG_TYPED_DATA,
    pub InStrIndex: u32,
    pub In32: u32,
    pub Out32: u32,
    pub In64: u64,
    pub Out64: u64,
    pub StrBufferIndex: u32,
    pub StrBufferChars: u32,
    pub StrCharsNeeded: u32,
    pub DataBufferIndex: u32,
    pub DataBufferBytes: u32,
    pub DataBytesNeeded: u32,
    pub Status: ::windows_sys::core::HRESULT,
    pub Reserved: [u64; 8],
}
impl ::core::marker::Copy for EXT_TYPED_DATA {}
impl ::core::clone::Clone for EXT_TYPED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FA_ENTRY {
    pub Tag: DEBUG_FLR_PARAM_TYPE,
    pub FullSize: u16,
    pub DataSize: u16,
}
impl ::core::marker::Copy for FA_ENTRY {}
impl ::core::clone::Clone for FA_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FIELD_INFO {
    pub fName: *mut u8,
    pub printName: *mut u8,
    pub size: u32,
    pub fOptions: u32,
    pub address: u64,
    pub Anonymous: FIELD_INFO_0,
    pub TypeId: u32,
    pub FieldOffset: u32,
    pub BufferSize: u32,
    pub BitField: FIELD_INFO_1,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for FIELD_INFO {}
impl ::core::clone::Clone for FIELD_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union FIELD_INFO_0 {
    pub fieldCallBack: *mut ::core::ffi::c_void,
    pub pBuffer: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for FIELD_INFO_0 {}
impl ::core::clone::Clone for FIELD_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FIELD_INFO_1 {
    pub Position: u16,
    pub Size: u16,
}
impl ::core::marker::Copy for FIELD_INFO_1 {}
impl ::core::clone::Clone for FIELD_INFO_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_CONTEXT_EX {
    pub Status: u32,
    pub ContextSize: u32,
    pub pContext: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for GET_CONTEXT_EX {}
impl ::core::clone::Clone for GET_CONTEXT_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_CURRENT_PROCESS_ADDRESS {
    pub Processor: u32,
    pub CurrentThread: u64,
    pub Address: u64,
}
impl ::core::marker::Copy for GET_CURRENT_PROCESS_ADDRESS {}
impl ::core::clone::Clone for GET_CURRENT_PROCESS_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_CURRENT_THREAD_ADDRESS {
    pub Processor: u32,
    pub Address: u64,
}
impl ::core::marker::Copy for GET_CURRENT_THREAD_ADDRESS {}
impl ::core::clone::Clone for GET_CURRENT_THREAD_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_EXPRESSION_EX {
    pub Expression: ::windows_sys::core::PCSTR,
    pub Remainder: ::windows_sys::core::PCSTR,
    pub Value: u64,
}
impl ::core::marker::Copy for GET_EXPRESSION_EX {}
impl ::core::clone::Clone for GET_EXPRESSION_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_INPUT_LINE {
    pub Prompt: ::windows_sys::core::PCSTR,
    pub Buffer: ::windows_sys::core::PSTR,
    pub BufferSize: u32,
    pub InputSize: u32,
}
impl ::core::marker::Copy for GET_INPUT_LINE {}
impl ::core::clone::Clone for GET_INPUT_LINE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_PEB_ADDRESS {
    pub CurrentThread: u64,
    pub Address: u64,
}
impl ::core::marker::Copy for GET_PEB_ADDRESS {}
impl ::core::clone::Clone for GET_PEB_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_SET_SYMPATH {
    pub Args: ::windows_sys::core::PCSTR,
    pub Result: ::windows_sys::core::PSTR,
    pub Length: i32,
}
impl ::core::marker::Copy for GET_SET_SYMPATH {}
impl ::core::clone::Clone for GET_SET_SYMPATH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_TEB_ADDRESS {
    pub Address: u64,
}
impl ::core::marker::Copy for GET_TEB_ADDRESS {}
impl ::core::clone::Clone for GET_TEB_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union INLINE_FRAME_CONTEXT {
    pub ContextValue: u32,
    pub Anonymous: INLINE_FRAME_CONTEXT_0,
}
impl ::core::marker::Copy for INLINE_FRAME_CONTEXT {}
impl ::core::clone::Clone for INLINE_FRAME_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INLINE_FRAME_CONTEXT_0 {
    pub FrameId: u8,
    pub FrameType: u8,
    pub FrameSignature: u16,
}
impl ::core::marker::Copy for INLINE_FRAME_CONTEXT_0 {}
impl ::core::clone::Clone for INLINE_FRAME_CONTEXT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IOSPACE {
    pub Address: u32,
    pub Length: u32,
    pub Data: u32,
}
impl ::core::marker::Copy for IOSPACE {}
impl ::core::clone::Clone for IOSPACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IOSPACE32 {
    pub Address: u32,
    pub Length: u32,
    pub Data: u32,
}
impl ::core::marker::Copy for IOSPACE32 {}
impl ::core::clone::Clone for IOSPACE32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IOSPACE64 {
    pub Address: u64,
    pub Length: u32,
    pub Data: u32,
}
impl ::core::marker::Copy for IOSPACE64 {}
impl ::core::clone::Clone for IOSPACE64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IOSPACE_EX {
    pub Address: u32,
    pub Length: u32,
    pub Data: u32,
    pub InterfaceType: u32,
    pub BusNumber: u32,
    pub AddressSpace: u32,
}
impl ::core::marker::Copy for IOSPACE_EX {}
impl ::core::clone::Clone for IOSPACE_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IOSPACE_EX32 {
    pub Address: u32,
    pub Length: u32,
    pub Data: u32,
    pub InterfaceType: u32,
    pub BusNumber: u32,
    pub AddressSpace: u32,
}
impl ::core::marker::Copy for IOSPACE_EX32 {}
impl ::core::clone::Clone for IOSPACE_EX32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IOSPACE_EX64 {
    pub Address: u64,
    pub Length: u32,
    pub Data: u32,
    pub InterfaceType: u32,
    pub BusNumber: u32,
    pub AddressSpace: u32,
}
impl ::core::marker::Copy for IOSPACE_EX64 {}
impl ::core::clone::Clone for IOSPACE_EX64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct KDDEBUGGER_DATA32 {
    pub Header: DBGKD_DEBUG_DATA_HEADER32,
    pub KernBase: u32,
    pub BreakpointWithStatus: u32,
    pub SavedContext: u32,
    pub ThCallbackStack: u16,
    pub NextCallback: u16,
    pub FramePointer: u16,
    pub _bitfield: u16,
    pub KiCallUserMode: u32,
    pub KeUserCallbackDispatcher: u32,
    pub PsLoadedModuleList: u32,
    pub PsActiveProcessHead: u32,
    pub PspCidTable: u32,
    pub ExpSystemResourcesList: u32,
    pub ExpPagedPoolDescriptor: u32,
    pub ExpNumberOfPagedPools: u32,
    pub KeTimeIncrement: u32,
    pub KeBugCheckCallbackListHead: u32,
    pub KiBugcheckData: u32,
    pub IopErrorLogListHead: u32,
    pub ObpRootDirectoryObject: u32,
    pub ObpTypeObjectType: u32,
    pub MmSystemCacheStart: u32,
    pub MmSystemCacheEnd: u32,
    pub MmSystemCacheWs: u32,
    pub MmPfnDatabase: u32,
    pub MmSystemPtesStart: u32,
    pub MmSystemPtesEnd: u32,
    pub MmSubsectionBase: u32,
    pub MmNumberOfPagingFiles: u32,
    pub MmLowestPhysicalPage: u32,
    pub MmHighestPhysicalPage: u32,
    pub MmNumberOfPhysicalPages: u32,
    pub MmMaximumNonPagedPoolInBytes: u32,
    pub MmNonPagedSystemStart: u32,
    pub MmNonPagedPoolStart: u32,
    pub MmNonPagedPoolEnd: u32,
    pub MmPagedPoolStart: u32,
    pub MmPagedPoolEnd: u32,
    pub MmPagedPoolInformation: u32,
    pub MmPageSize: u32,
    pub MmSizeOfPagedPoolInBytes: u32,
    pub MmTotalCommitLimit: u32,
    pub MmTotalCommittedPages: u32,
    pub MmSharedCommit: u32,
    pub MmDriverCommit: u32,
    pub MmProcessCommit: u32,
    pub MmPagedPoolCommit: u32,
    pub MmExtendedCommit: u32,
    pub MmZeroedPageListHead: u32,
    pub MmFreePageListHead: u32,
    pub MmStandbyPageListHead: u32,
    pub MmModifiedPageListHead: u32,
    pub MmModifiedNoWritePageListHead: u32,
    pub MmAvailablePages: u32,
    pub MmResidentAvailablePages: u32,
    pub PoolTrackTable: u32,
    pub NonPagedPoolDescriptor: u32,
    pub MmHighestUserAddress: u32,
    pub MmSystemRangeStart: u32,
    pub MmUserProbeAddress: u32,
    pub KdPrintCircularBuffer: u32,
    pub KdPrintCircularBufferEnd: u32,
    pub KdPrintWritePointer: u32,
    pub KdPrintRolloverCount: u32,
    pub MmLoadedUserImageList: u32,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for KDDEBUGGER_DATA32 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for KDDEBUGGER_DATA32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct KDDEBUGGER_DATA64 {
    pub Header: DBGKD_DEBUG_DATA_HEADER64,
    pub KernBase: u64,
    pub BreakpointWithStatus: u64,
    pub SavedContext: u64,
    pub ThCallbackStack: u16,
    pub NextCallback: u16,
    pub FramePointer: u16,
    pub _bitfield: u16,
    pub KiCallUserMode: u64,
    pub KeUserCallbackDispatcher: u64,
    pub PsLoadedModuleList: u64,
    pub PsActiveProcessHead: u64,
    pub PspCidTable: u64,
    pub ExpSystemResourcesList: u64,
    pub ExpPagedPoolDescriptor: u64,
    pub ExpNumberOfPagedPools: u64,
    pub KeTimeIncrement: u64,
    pub KeBugCheckCallbackListHead: u64,
    pub KiBugcheckData: u64,
    pub IopErrorLogListHead: u64,
    pub ObpRootDirectoryObject: u64,
    pub ObpTypeObjectType: u64,
    pub MmSystemCacheStart: u64,
    pub MmSystemCacheEnd: u64,
    pub MmSystemCacheWs: u64,
    pub MmPfnDatabase: u64,
    pub MmSystemPtesStart: u64,
    pub MmSystemPtesEnd: u64,
    pub MmSubsectionBase: u64,
    pub MmNumberOfPagingFiles: u64,
    pub MmLowestPhysicalPage: u64,
    pub MmHighestPhysicalPage: u64,
    pub MmNumberOfPhysicalPages: u64,
    pub MmMaximumNonPagedPoolInBytes: u64,
    pub MmNonPagedSystemStart: u64,
    pub MmNonPagedPoolStart: u64,
    pub MmNonPagedPoolEnd: u64,
    pub MmPagedPoolStart: u64,
    pub MmPagedPoolEnd: u64,
    pub MmPagedPoolInformation: u64,
    pub MmPageSize: u64,
    pub MmSizeOfPagedPoolInBytes: u64,
    pub MmTotalCommitLimit: u64,
    pub MmTotalCommittedPages: u64,
    pub MmSharedCommit: u64,
    pub MmDriverCommit: u64,
    pub MmProcessCommit: u64,
    pub MmPagedPoolCommit: u64,
    pub MmExtendedCommit: u64,
    pub MmZeroedPageListHead: u64,
    pub MmFreePageListHead: u64,
    pub MmStandbyPageListHead: u64,
    pub MmModifiedPageListHead: u64,
    pub MmModifiedNoWritePageListHead: u64,
    pub MmAvailablePages: u64,
    pub MmResidentAvailablePages: u64,
    pub PoolTrackTable: u64,
    pub NonPagedPoolDescriptor: u64,
    pub MmHighestUserAddress: u64,
    pub MmSystemRangeStart: u64,
    pub MmUserProbeAddress: u64,
    pub KdPrintCircularBuffer: u64,
    pub KdPrintCircularBufferEnd: u64,
    pub KdPrintWritePointer: u64,
    pub KdPrintRolloverCount: u64,
    pub MmLoadedUserImageList: u64,
    pub NtBuildLab: u64,
    pub KiNormalSystemCall: u64,
    pub KiProcessorBlock: u64,
    pub MmUnloadedDrivers: u64,
    pub MmLastUnloadedDriver: u64,
    pub MmTriageActionTaken: u64,
    pub MmSpecialPoolTag: u64,
    pub KernelVerifier: u64,
    pub MmVerifierData: u64,
    pub MmAllocatedNonPagedPool: u64,
    pub MmPeakCommitment: u64,
    pub MmTotalCommitLimitMaximum: u64,
    pub CmNtCSDVersion: u64,
    pub MmPhysicalMemoryBlock: u64,
    pub MmSessionBase: u64,
    pub MmSessionSize: u64,
    pub MmSystemParentTablePage: u64,
    pub MmVirtualTranslationBase: u64,
    pub OffsetKThreadNextProcessor: u16,
    pub OffsetKThreadTeb: u16,
    pub OffsetKThreadKernelStack: u16,
    pub OffsetKThreadInitialStack: u16,
    pub OffsetKThreadApcProcess: u16,
    pub OffsetKThreadState: u16,
    pub OffsetKThreadBStore: u16,
    pub OffsetKThreadBStoreLimit: u16,
    pub SizeEProcess: u16,
    pub OffsetEprocessPeb: u16,
    pub OffsetEprocessParentCID: u16,
    pub OffsetEprocessDirectoryTableBase: u16,
    pub SizePrcb: u16,
    pub OffsetPrcbDpcRoutine: u16,
    pub OffsetPrcbCurrentThread: u16,
    pub OffsetPrcbMhz: u16,
    pub OffsetPrcbCpuType: u16,
    pub OffsetPrcbVendorString: u16,
    pub OffsetPrcbProcStateContext: u16,
    pub OffsetPrcbNumber: u16,
    pub SizeEThread: u16,
    pub L1tfHighPhysicalBitIndex: u8,
    pub L1tfSwizzleBitIndex: u8,
    pub Padding0: u32,
    pub KdPrintCircularBufferPtr: u64,
    pub KdPrintBufferSize: u64,
    pub KeLoaderBlock: u64,
    pub SizePcr: u16,
    pub OffsetPcrSelfPcr: u16,
    pub OffsetPcrCurrentPrcb: u16,
    pub OffsetPcrContainedPrcb: u16,
    pub OffsetPcrInitialBStore: u16,
    pub OffsetPcrBStoreLimit: u16,
    pub OffsetPcrInitialStack: u16,
    pub OffsetPcrStackLimit: u16,
    pub OffsetPrcbPcrPage: u16,
    pub OffsetPrcbProcStateSpecialReg: u16,
    pub GdtR0Code: u16,
    pub GdtR0Data: u16,
    pub GdtR0Pcr: u16,
    pub GdtR3Code: u16,
    pub GdtR3Data: u16,
    pub GdtR3Teb: u16,
    pub GdtLdt: u16,
    pub GdtTss: u16,
    pub Gdt64R3CmCode: u16,
    pub Gdt64R3CmTeb: u16,
    pub IopNumTriageDumpDataBlocks: u64,
    pub IopTriageDumpDataBlocks: u64,
    pub VfCrashDataBlock: u64,
    pub MmBadPagesDetected: u64,
    pub MmZeroedPageSingleBitErrorsDetected: u64,
    pub EtwpDebuggerData: u64,
    pub OffsetPrcbContext: u16,
    pub OffsetPrcbMaxBreakpoints: u16,
    pub OffsetPrcbMaxWatchpoints: u16,
    pub OffsetKThreadStackLimit: u32,
    pub OffsetKThreadStackBase: u32,
    pub OffsetKThreadQueueListEntry: u32,
    pub OffsetEThreadIrpList: u32,
    pub OffsetPrcbIdleThread: u16,
    pub OffsetPrcbNormalDpcState: u16,
    pub OffsetPrcbDpcStack: u16,
    pub OffsetPrcbIsrStack: u16,
    pub SizeKDPC_STACK_FRAME: u16,
    pub OffsetKPriQueueThreadListHead: u16,
    pub OffsetKThreadWaitReason: u16,
    pub Padding1: u16,
    pub PteBase: u64,
    pub RetpolineStubFunctionTable: u64,
    pub RetpolineStubFunctionTableSize: u32,
    pub RetpolineStubOffset: u32,
    pub RetpolineStubSize: u32,
    pub OffsetEProcessMmHotPatchContext: u16,
    pub OffsetKThreadShadowStackLimit: u32,
    pub OffsetKThreadShadowStackBase: u32,
    pub ShadowStackEnabled: u64,
    pub PointerAuthMask: u64,
    pub OffsetPrcbExceptionStack: u16,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for KDDEBUGGER_DATA64 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for KDDEBUGGER_DATA64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct KDEXTS_LOCK_INFO {
    pub SizeOfStruct: u32,
    pub Address: u64,
    pub OwningThread: u64,
    pub ExclusiveOwned: super::super::super::super::Foundation::BOOL,
    pub NumOwners: u32,
    pub ContentionCount: u32,
    pub NumExclusiveWaiters: u32,
    pub NumSharedWaiters: u32,
    pub pOwnerThreads: *mut u64,
    pub pWaiterThreads: *mut u64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for KDEXTS_LOCK_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for KDEXTS_LOCK_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct KDEXTS_PTE_INFO {
    pub SizeOfStruct: u32,
    pub VirtualAddress: u64,
    pub PpeAddress: u64,
    pub PdeAddress: u64,
    pub PteAddress: u64,
    pub Pfn: u64,
    pub Levels: u64,
    pub _bitfield1: u32,
    pub _bitfield2: u32,
}
impl ::core::marker::Copy for KDEXTS_PTE_INFO {}
impl ::core::clone::Clone for KDEXTS_PTE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct KDEXT_FILELOCK_OWNER {
    pub Sizeofstruct: u32,
    pub FileObject: u64,
    pub OwnerThread: u64,
    pub WaitIrp: u64,
    pub DeviceObject: u64,
    pub BlockingDirver: [u8; 32],
}
impl ::core::marker::Copy for KDEXT_FILELOCK_OWNER {}
impl ::core::clone::Clone for KDEXT_FILELOCK_OWNER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct KDEXT_HANDLE_INFORMATION {
    pub HandleTableEntry: u64,
    pub Handle: u64,
    pub Object: u64,
    pub ObjectBody: u64,
    pub GrantedAccess: u64,
    pub HandleAttributes: u32,
    pub PagedOut: super::super::super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for KDEXT_HANDLE_INFORMATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for KDEXT_HANDLE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct KDEXT_PROCESS_FIND_PARAMS {
    pub SizeofStruct: u32,
    pub Pid: u32,
    pub Session: u32,
    pub ImageName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for KDEXT_PROCESS_FIND_PARAMS {}
impl ::core::clone::Clone for KDEXT_PROCESS_FIND_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct KDEXT_THREAD_FIND_PARAMS {
    pub SizeofStruct: u32,
    pub StackPointer: u64,
    pub Cid: u32,
    pub Thread: u64,
}
impl ::core::marker::Copy for KDEXT_THREAD_FIND_PARAMS {}
impl ::core::clone::Clone for KDEXT_THREAD_FIND_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct Location {
    pub HostDefined: u64,
    pub Offset: u64,
}
impl ::core::marker::Copy for Location {}
impl ::core::clone::Clone for Location {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OS_INFO {
    pub MajorVer: u32,
    pub MinorVer: u32,
    pub Build: u32,
    pub BuildQfe: u32,
    pub ProductType: u32,
    pub Suite: u32,
    pub Revision: u32,
    pub s: OS_INFO_0,
    pub SrvPackNumber: u32,
    pub ServicePackBuild: u32,
    pub Architecture: u32,
    pub Lcid: u32,
    pub Name: [u8; 64],
    pub FullName: [u8; 256],
    pub Language: [u8; 30],
    pub BuildVersion: [u8; 64],
    pub ServicePackString: [u8; 64],
}
impl ::core::marker::Copy for OS_INFO {}
impl ::core::clone::Clone for OS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OS_INFO_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for OS_INFO_0 {}
impl ::core::clone::Clone for OS_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OS_INFO_v1 {
    pub Type: OS_TYPE,
    pub Anonymous: OS_INFO_v1_0,
    pub ProductType: u32,
    pub Suite: u32,
    pub s: OS_INFO_v1_1,
    pub SrvPackNumber: u32,
    pub Language: [u8; 30],
    pub OsString: [u8; 64],
    pub ServicePackString: [u8; 64],
}
impl ::core::marker::Copy for OS_INFO_v1 {}
impl ::core::clone::Clone for OS_INFO_v1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union OS_INFO_v1_0 {
    pub Version: OS_INFO_v1_0_0,
    pub Ver64: u64,
}
impl ::core::marker::Copy for OS_INFO_v1_0 {}
impl ::core::clone::Clone for OS_INFO_v1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OS_INFO_v1_0_0 {
    pub Major: u32,
    pub Minor: u32,
}
impl ::core::marker::Copy for OS_INFO_v1_0_0 {}
impl ::core::clone::Clone for OS_INFO_v1_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OS_INFO_v1_1 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for OS_INFO_v1_1 {}
impl ::core::clone::Clone for OS_INFO_v1_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL {
    pub Address: u64,
    pub BufLen: u32,
    pub Buf: [u8; 1],
}
impl ::core::marker::Copy for PHYSICAL {}
impl ::core::clone::Clone for PHYSICAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_TO_VIRTUAL {
    pub Status: u32,
    pub Size: u32,
    pub PdeAddress: u64,
}
impl ::core::marker::Copy for PHYSICAL_TO_VIRTUAL {}
impl ::core::clone::Clone for PHYSICAL_TO_VIRTUAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_WITH_FLAGS {
    pub Address: u64,
    pub BufLen: u32,
    pub Flags: u32,
    pub Buf: [u8; 1],
}
impl ::core::marker::Copy for PHYSICAL_WITH_FLAGS {}
impl ::core::clone::Clone for PHYSICAL_WITH_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POINTER_SEARCH_PHYSICAL {
    pub Offset: u64,
    pub Length: u64,
    pub PointerMin: u64,
    pub PointerMax: u64,
    pub Flags: u32,
    pub MatchOffsets: *mut u64,
    pub MatchOffsetsSize: u32,
    pub MatchOffsetsCount: u32,
}
impl ::core::marker::Copy for POINTER_SEARCH_PHYSICAL {}
impl ::core::clone::Clone for POINTER_SEARCH_PHYSICAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESSORINFO {
    pub Processor: u16,
    pub NumberProcessors: u16,
}
impl ::core::marker::Copy for PROCESSORINFO {}
impl ::core::clone::Clone for PROCESSORINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESS_COMMIT_USAGE {
    pub ImageFileName: [u8; 16],
    pub ClientId: u64,
    pub ProcessAddress: u64,
    pub CommitCharge: u64,
    pub SharedCommitCharge: u64,
    pub ReleasedCommitDebt: u64,
    pub Reserved: u64,
}
impl ::core::marker::Copy for PROCESS_COMMIT_USAGE {}
impl ::core::clone::Clone for PROCESS_COMMIT_USAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROCESS_NAME_ENTRY {
    pub ProcessId: u32,
    pub NameOffset: u32,
    pub NameSize: u32,
    pub NextEntry: u32,
}
impl ::core::marker::Copy for PROCESS_NAME_ENTRY {}
impl ::core::clone::Clone for PROCESS_NAME_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct READCONTROLSPACE {
    pub Processor: u16,
    pub Address: u32,
    pub BufLen: u32,
    pub Buf: [u8; 1],
}
impl ::core::marker::Copy for READCONTROLSPACE {}
impl ::core::clone::Clone for READCONTROLSPACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct READCONTROLSPACE32 {
    pub Processor: u16,
    pub Address: u32,
    pub BufLen: u32,
    pub Buf: [u8; 1],
}
impl ::core::marker::Copy for READCONTROLSPACE32 {}
impl ::core::clone::Clone for READCONTROLSPACE32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct READCONTROLSPACE64 {
    pub Processor: u16,
    pub Address: u64,
    pub BufLen: u32,
    pub Buf: [u8; 1],
}
impl ::core::marker::Copy for READCONTROLSPACE64 {}
impl ::core::clone::Clone for READCONTROLSPACE64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct READ_WRITE_MSR {
    pub Msr: u32,
    pub Value: i64,
}
impl ::core::marker::Copy for READ_WRITE_MSR {}
impl ::core::clone::Clone for READ_WRITE_MSR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEARCHMEMORY {
    pub SearchAddress: u64,
    pub SearchLength: u64,
    pub FoundAddress: u64,
    pub PatternLength: u32,
    pub Pattern: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for SEARCHMEMORY {}
impl ::core::clone::Clone for SEARCHMEMORY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STACK_SRC_INFO {
    pub ImagePath: ::windows_sys::core::PCWSTR,
    pub ModuleName: ::windows_sys::core::PCWSTR,
    pub Function: ::windows_sys::core::PCWSTR,
    pub Displacement: u32,
    pub Row: u32,
    pub Column: u32,
}
impl ::core::marker::Copy for STACK_SRC_INFO {}
impl ::core::clone::Clone for STACK_SRC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct STACK_SYM_FRAME_INFO {
    pub StackFrameEx: DEBUG_STACK_FRAME_EX,
    pub SrcInfo: STACK_SRC_INFO,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for STACK_SYM_FRAME_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for STACK_SYM_FRAME_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYMBOL_INFO_EX {
    pub SizeOfStruct: u32,
    pub TypeOfInfo: u32,
    pub Offset: u64,
    pub Line: u32,
    pub Displacement: u32,
    pub Reserved: [u32; 4],
}
impl ::core::marker::Copy for SYMBOL_INFO_EX {}
impl ::core::clone::Clone for SYMBOL_INFO_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYM_DUMP_PARAM {
    pub size: u32,
    pub sName: *mut u8,
    pub Options: u32,
    pub addr: u64,
    pub listLink: *mut FIELD_INFO,
    pub Anonymous: SYM_DUMP_PARAM_0,
    pub CallbackRoutine: PSYM_DUMP_FIELD_CALLBACK,
    pub nFields: u32,
    pub Fields: *mut FIELD_INFO,
    pub ModBase: u64,
    pub TypeId: u32,
    pub TypeSize: u32,
    pub BufferSize: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for SYM_DUMP_PARAM {}
impl ::core::clone::Clone for SYM_DUMP_PARAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union SYM_DUMP_PARAM_0 {
    pub Context: *mut ::core::ffi::c_void,
    pub pBuffer: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for SYM_DUMP_PARAM_0 {}
impl ::core::clone::Clone for SYM_DUMP_PARAM_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ScriptDebugEventInformation {
    pub DebugEvent: ScriptDebugEvent,
    pub EventPosition: ScriptDebugPosition,
    pub EventSpanEnd: ScriptDebugPosition,
    pub u: ScriptDebugEventInformation_0,
}
impl ::core::marker::Copy for ScriptDebugEventInformation {}
impl ::core::clone::Clone for ScriptDebugEventInformation {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union ScriptDebugEventInformation_0 {
    pub ExceptionInformation: ScriptDebugEventInformation_0_1,
    pub BreakpointInformation: ScriptDebugEventInformation_0_0,
}
impl ::core::marker::Copy for ScriptDebugEventInformation_0 {}
impl ::core::clone::Clone for ScriptDebugEventInformation_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ScriptDebugEventInformation_0_0 {
    pub BreakpointId: u64,
}
impl ::core::marker::Copy for ScriptDebugEventInformation_0_0 {}
impl ::core::clone::Clone for ScriptDebugEventInformation_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ScriptDebugEventInformation_0_1 {
    pub IsUncaught: u8,
}
impl ::core::marker::Copy for ScriptDebugEventInformation_0_1 {}
impl ::core::clone::Clone for ScriptDebugEventInformation_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ScriptDebugPosition {
    pub Line: u32,
    pub Column: u32,
}
impl ::core::marker::Copy for ScriptDebugPosition {}
impl ::core::clone::Clone for ScriptDebugPosition {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TARGET_DEBUG_INFO {
    pub SizeOfStruct: u32,
    pub EntryDate: u64,
    pub DebugeeClass: u32,
    pub SysUpTime: u64,
    pub AppUpTime: u64,
    pub CrashTime: u64,
    pub OsInfo: OS_INFO,
    pub Cpu: CPU_INFO,
    pub DumpFile: [u8; 260],
}
impl ::core::marker::Copy for TARGET_DEBUG_INFO {}
impl ::core::clone::Clone for TARGET_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TARGET_DEBUG_INFO_v1 {
    pub SizeOfStruct: u32,
    pub Id: u64,
    pub Source: u64,
    pub EntryDate: u64,
    pub SysUpTime: u64,
    pub AppUpTime: u64,
    pub CrashTime: u64,
    pub Mode: u64,
    pub OsInfo: OS_INFO_v1,
    pub Cpu: CPU_INFO_v1,
    pub DumpFile: [u8; 260],
    pub FailureData: *mut ::core::ffi::c_void,
    pub StackTr: [u8; 4096],
}
impl ::core::marker::Copy for TARGET_DEBUG_INFO_v1 {}
impl ::core::clone::Clone for TARGET_DEBUG_INFO_v1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TARGET_DEBUG_INFO_v2 {
    pub SizeOfStruct: u32,
    pub EntryDate: u64,
    pub DebugeeClass: u32,
    pub SysUpTime: u64,
    pub AppUpTime: u64,
    pub CrashTime: u64,
    pub OsInfo: OS_INFO,
    pub Cpu: CPU_INFO_v2,
    pub DumpFile: [u8; 260],
}
impl ::core::marker::Copy for TARGET_DEBUG_INFO_v2 {}
impl ::core::clone::Clone for TARGET_DEBUG_INFO_v2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRANSLATE_VIRTUAL_TO_PHYSICAL {
    pub Virtual: u64,
    pub Physical: u64,
}
impl ::core::marker::Copy for TRANSLATE_VIRTUAL_TO_PHYSICAL {}
impl ::core::clone::Clone for TRANSLATE_VIRTUAL_TO_PHYSICAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct VIRTUAL_TO_PHYSICAL {
    pub Status: u32,
    pub Size: u32,
    pub PdeAddress: u64,
    pub Virtual: u64,
    pub Physical: u64,
}
impl ::core::marker::Copy for VIRTUAL_TO_PHYSICAL {}
impl ::core::clone::Clone for VIRTUAL_TO_PHYSICAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDBGEXTS_CLR_DATA_INTERFACE {
    pub Iid: *const ::windows_sys::core::GUID,
    pub Iface: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for WDBGEXTS_CLR_DATA_INTERFACE {}
impl ::core::clone::Clone for WDBGEXTS_CLR_DATA_INTERFACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDBGEXTS_DISASSEMBLE_BUFFER {
    pub InOffset: u64,
    pub OutOffset: u64,
    pub AddrFlags: u32,
    pub FormatFlags: u32,
    pub DataBufferBytes: u32,
    pub DisasmBufferChars: u32,
    pub DataBuffer: *mut ::core::ffi::c_void,
    pub DisasmBuffer: ::windows_sys::core::PWSTR,
    pub Reserved0: [u64; 3],
}
impl ::core::marker::Copy for WDBGEXTS_DISASSEMBLE_BUFFER {}
impl ::core::clone::Clone for WDBGEXTS_DISASSEMBLE_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDBGEXTS_MODULE_IN_RANGE {
    pub Start: u64,
    pub End: u64,
    pub FoundModBase: u64,
    pub FoundModSize: u32,
}
impl ::core::marker::Copy for WDBGEXTS_MODULE_IN_RANGE {}
impl ::core::clone::Clone for WDBGEXTS_MODULE_IN_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDBGEXTS_QUERY_INTERFACE {
    pub Iid: *const ::windows_sys::core::GUID,
    pub Iface: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for WDBGEXTS_QUERY_INTERFACE {}
impl ::core::clone::Clone for WDBGEXTS_QUERY_INTERFACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDBGEXTS_THREAD_OS_INFO {
    pub ThreadId: u32,
    pub ExitStatus: u32,
    pub PriorityClass: u32,
    pub Priority: u32,
    pub CreateTime: u64,
    pub ExitTime: u64,
    pub KernelTime: u64,
    pub UserTime: u64,
    pub StartOffset: u64,
    pub Affinity: u64,
}
impl ::core::marker::Copy for WDBGEXTS_THREAD_OS_INFO {}
impl ::core::clone::Clone for WDBGEXTS_THREAD_OS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct WINDBG_EXTENSION_APIS {
    pub nSize: u32,
    pub lpOutputRoutine: PWINDBG_OUTPUT_ROUTINE,
    pub lpGetExpressionRoutine: PWINDBG_GET_EXPRESSION,
    pub lpGetSymbolRoutine: PWINDBG_GET_SYMBOL,
    pub lpDisasmRoutine: PWINDBG_DISASM,
    pub lpCheckControlCRoutine: PWINDBG_CHECK_CONTROL_C,
    pub lpReadProcessMemoryRoutine: PWINDBG_READ_PROCESS_MEMORY_ROUTINE,
    pub lpWriteProcessMemoryRoutine: PWINDBG_WRITE_PROCESS_MEMORY_ROUTINE,
    pub lpGetThreadContextRoutine: PWINDBG_GET_THREAD_CONTEXT_ROUTINE,
    pub lpSetThreadContextRoutine: PWINDBG_SET_THREAD_CONTEXT_ROUTINE,
    pub lpIoctlRoutine: PWINDBG_IOCTL_ROUTINE,
    pub lpStackTraceRoutine: PWINDBG_STACKTRACE_ROUTINE,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for WINDBG_EXTENSION_APIS {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for WINDBG_EXTENSION_APIS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct WINDBG_EXTENSION_APIS32 {
    pub nSize: u32,
    pub lpOutputRoutine: PWINDBG_OUTPUT_ROUTINE,
    pub lpGetExpressionRoutine: PWINDBG_GET_EXPRESSION32,
    pub lpGetSymbolRoutine: PWINDBG_GET_SYMBOL32,
    pub lpDisasmRoutine: PWINDBG_DISASM32,
    pub lpCheckControlCRoutine: PWINDBG_CHECK_CONTROL_C,
    pub lpReadProcessMemoryRoutine: PWINDBG_READ_PROCESS_MEMORY_ROUTINE32,
    pub lpWriteProcessMemoryRoutine: PWINDBG_WRITE_PROCESS_MEMORY_ROUTINE32,
    pub lpGetThreadContextRoutine: PWINDBG_GET_THREAD_CONTEXT_ROUTINE,
    pub lpSetThreadContextRoutine: PWINDBG_SET_THREAD_CONTEXT_ROUTINE,
    pub lpIoctlRoutine: PWINDBG_IOCTL_ROUTINE,
    pub lpStackTraceRoutine: PWINDBG_STACKTRACE_ROUTINE32,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for WINDBG_EXTENSION_APIS32 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for WINDBG_EXTENSION_APIS32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct WINDBG_EXTENSION_APIS64 {
    pub nSize: u32,
    pub lpOutputRoutine: PWINDBG_OUTPUT_ROUTINE,
    pub lpGetExpressionRoutine: PWINDBG_GET_EXPRESSION64,
    pub lpGetSymbolRoutine: PWINDBG_GET_SYMBOL64,
    pub lpDisasmRoutine: PWINDBG_DISASM64,
    pub lpCheckControlCRoutine: PWINDBG_CHECK_CONTROL_C,
    pub lpReadProcessMemoryRoutine: PWINDBG_READ_PROCESS_MEMORY_ROUTINE64,
    pub lpWriteProcessMemoryRoutine: PWINDBG_WRITE_PROCESS_MEMORY_ROUTINE64,
    pub lpGetThreadContextRoutine: PWINDBG_GET_THREAD_CONTEXT_ROUTINE,
    pub lpSetThreadContextRoutine: PWINDBG_SET_THREAD_CONTEXT_ROUTINE,
    pub lpIoctlRoutine: PWINDBG_IOCTL_ROUTINE,
    pub lpStackTraceRoutine: PWINDBG_STACKTRACE_ROUTINE64,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for WINDBG_EXTENSION_APIS64 {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for WINDBG_EXTENSION_APIS64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WINDBG_OLDKD_EXTENSION_APIS {
    pub nSize: u32,
    pub lpOutputRoutine: PWINDBG_OUTPUT_ROUTINE,
    pub lpGetExpressionRoutine: PWINDBG_GET_EXPRESSION32,
    pub lpGetSymbolRoutine: PWINDBG_GET_SYMBOL32,
    pub lpDisasmRoutine: PWINDBG_DISASM32,
    pub lpCheckControlCRoutine: PWINDBG_CHECK_CONTROL_C,
    pub lpReadVirtualMemRoutine: PWINDBG_READ_PROCESS_MEMORY_ROUTINE32,
    pub lpWriteVirtualMemRoutine: PWINDBG_WRITE_PROCESS_MEMORY_ROUTINE32,
    pub lpReadPhysicalMemRoutine: PWINDBG_OLDKD_READ_PHYSICAL_MEMORY,
    pub lpWritePhysicalMemRoutine: PWINDBG_OLDKD_WRITE_PHYSICAL_MEMORY,
}
impl ::core::marker::Copy for WINDBG_OLDKD_EXTENSION_APIS {}
impl ::core::clone::Clone for WINDBG_OLDKD_EXTENSION_APIS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WINDBG_OLD_EXTENSION_APIS {
    pub nSize: u32,
    pub lpOutputRoutine: PWINDBG_OUTPUT_ROUTINE,
    pub lpGetExpressionRoutine: PWINDBG_GET_EXPRESSION,
    pub lpGetSymbolRoutine: PWINDBG_GET_SYMBOL,
    pub lpDisasmRoutine: PWINDBG_DISASM,
    pub lpCheckControlCRoutine: PWINDBG_CHECK_CONTROL_C,
}
impl ::core::marker::Copy for WINDBG_OLD_EXTENSION_APIS {}
impl ::core::clone::Clone for WINDBG_OLD_EXTENSION_APIS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XML_DRIVER_NODE_INFO {
    pub FileName: [u8; 64],
    pub FileSize: u64,
    pub CreationDate: u64,
    pub Version: [u8; 64],
    pub Manufacturer: [u8; 260],
    pub ProductName: [u8; 260],
    pub Group: [u8; 260],
    pub Altitude: [u8; 260],
}
impl ::core::marker::Copy for XML_DRIVER_NODE_INFO {}
impl ::core::clone::Clone for XML_DRIVER_NODE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
pub type ENTRY_CALLBACK = ::core::option::Option<unsafe extern "system" fn(entryaddress: u64, context: *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type EXTDLL_ITERATERTLBALANCEDNODES = ::core::option::Option<unsafe extern "system" fn(rootnode: u64, entryoffset: u32, callback: ENTRY_CALLBACK, callbackcontext: *mut ::core::ffi::c_void) -> ()>;
pub type EXTDLL_QUERYDATABYTAG = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, dwdatatag: u32, pqueryinfo: *const ::core::ffi::c_void, pdata: *mut u8, cbdata: u32) -> ::windows_sys::core::HRESULT>;
pub type EXTDLL_QUERYDATABYTAGEX = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, dwdatatag: u32, pqueryinfo: *const ::core::ffi::c_void, pdata: *mut u8, cbdata: u32, pdataex: *mut u8, cbdataex: u32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type EXTS_JOB_PROCESS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(job: u64, process: u64, context: *mut ::core::ffi::c_void) -> super::super::super::super::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type EXTS_TABLE_ENTRY_CALLBACK = ::core::option::Option<unsafe extern "system" fn(entry: u64, context: *mut ::core::ffi::c_void) -> super::super::super::super::Foundation::BOOLEAN>;
pub type EXT_ANALYSIS_PLUGIN = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, callphase: FA_EXTENSION_PLUGIN_PHASE, panalysis: IDebugFailureAnalysis2) -> ::windows_sys::core::HRESULT>;
pub type EXT_ANALYZER = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, bucketsuffix: ::windows_sys::core::PSTR, cbbucketsuffix: u32, debugtext: ::windows_sys::core::PSTR, cbdebugtext: u32, flags: *const u32, panalysis: IDebugFailureAnalysis) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type EXT_DECODE_ERROR = ::core::option::Option<unsafe extern "system" fn(pdecodeerror: *mut DEBUG_DECODE_ERROR) -> ()>;
pub type EXT_GET_DEBUG_FAILURE_ANALYSIS = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, flags: u32, classid: ::windows_sys::core::GUID, ppanalysis: *mut IDebugFailureAnalysis2) -> ::windows_sys::core::HRESULT>;
pub type EXT_GET_ENVIRONMENT_VARIABLE = ::core::option::Option<unsafe extern "system" fn(peb: u64, variable: ::windows_sys::core::PCSTR, buffer: ::windows_sys::core::PCSTR, buffersize: u32) -> ::windows_sys::core::HRESULT>;
pub type EXT_GET_FAILURE_ANALYSIS = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, flags: u32, ppanalysis: *mut IDebugFailureAnalysis) -> ::windows_sys::core::HRESULT>;
pub type EXT_GET_FA_ENTRIES_DATA = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, count: *mut u32, entries: *mut *mut FA_ENTRY) -> ::windows_sys::core::HRESULT>;
pub type EXT_GET_HANDLE_TRACE = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, tracetype: u32, startindex: u32, handlevalue: *mut u64, stackfunctions: *mut u64, stacktracesize: u32) -> ::windows_sys::core::HRESULT>;
pub type EXT_RELOAD_TRIAGER = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4) -> ::windows_sys::core::HRESULT>;
pub type EXT_TARGET_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, ptargetinfo: *mut TARGET_DEBUG_INFO) -> ::windows_sys::core::HRESULT>;
pub type EXT_TRIAGE_FOLLOWUP = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, symbolname: ::windows_sys::core::PCSTR, ownerinfo: *mut DEBUG_TRIAGE_FOLLOWUP_INFO) -> u32>;
pub type EXT_XML_DATA = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient4, pxmpdata: *mut EXT_CAB_XML_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type KDEXTS_LOCK_CALLBACKROUTINE = ::core::option::Option<unsafe extern "system" fn(plock: *mut KDEXTS_LOCK_INFO, context: *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type KDEXT_DUMP_HANDLE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(handleinfo: *const KDEXT_HANDLE_INFORMATION, flags: u32, context: *mut ::core::ffi::c_void) -> super::super::super::super::Foundation::BOOLEAN>;
pub type PDEBUG_EXTENSION_CALL = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, args: ::windows_sys::core::PCSTR) -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_EXTENSION_CANUNLOAD = ::core::option::Option<unsafe extern "system" fn() -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_EXTENSION_INITIALIZE = ::core::option::Option<unsafe extern "system" fn(version: *mut u32, flags: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_EXTENSION_KNOWN_STRUCT = ::core::option::Option<unsafe extern "system" fn(flags: u32, offset: u64, typename: ::windows_sys::core::PCSTR, buffer: ::windows_sys::core::PSTR, bufferchars: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_EXTENSION_KNOWN_STRUCT_EX = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, flags: u32, offset: u64, typename: ::windows_sys::core::PCSTR, buffer: ::windows_sys::core::PSTR, bufferchars: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_EXTENSION_NOTIFY = ::core::option::Option<unsafe extern "system" fn(notify: u32, argument: u64) -> ()>;
pub type PDEBUG_EXTENSION_PROVIDE_VALUE = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, flags: u32, name: ::windows_sys::core::PCWSTR, value: *mut u64, typemodbase: *mut u64, typeid: *mut u32, typeflags: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_EXTENSION_QUERY_VALUE_NAMES = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, flags: u32, buffer: ::windows_sys::core::PWSTR, bufferchars: u32, bufferneeded: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_EXTENSION_UNINITIALIZE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
pub type PDEBUG_EXTENSION_UNLOAD = ::core::option::Option<unsafe extern "system" fn() -> ()>;
pub type PDEBUG_STACK_PROVIDER_BEGINTHREADSTACKRECONSTRUCTION = ::core::option::Option<unsafe extern "system" fn(streamtype: u32, minidumpstreambuffer: *const ::core::ffi::c_void, buffersize: u32) -> ::windows_sys::core::HRESULT>;
pub type PDEBUG_STACK_PROVIDER_ENDTHREADSTACKRECONSTRUCTION = ::core::option::Option<unsafe extern "system" fn() -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PDEBUG_STACK_PROVIDER_FREESTACKSYMFRAMES = ::core::option::Option<unsafe extern "system" fn(stacksymframes: *const STACK_SYM_FRAME_INFO) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PDEBUG_STACK_PROVIDER_RECONSTRUCTSTACK = ::core::option::Option<unsafe extern "system" fn(systemthreadid: u32, nativeframes: *const DEBUG_STACK_FRAME_EX, countnativeframes: u32, stacksymframes: *mut *mut STACK_SYM_FRAME_INFO, stacksymframesfilled: *mut u32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMERATE_HANDLES = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, process: u64, handletodump: u64, flags: u32, callback: KDEXT_DUMP_HANDLE_CALLBACK, context: *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMERATE_HASH_TABLE = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, hashtable: u64, callback: EXTS_TABLE_ENTRY_CALLBACK, context: *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMERATE_JOB_PROCESSES = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, job: u64, callback: EXTS_JOB_PROCESS_CALLBACK, context: *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMERATE_SYSTEM_LOCKS = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, flags: u32, callback: KDEXTS_LOCK_CALLBACKROUTINE, context: *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
pub type PFIND_FILELOCK_OWNERINFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, pfilelockowner: *mut KDEXT_FILELOCK_OWNER) -> ::windows_sys::core::HRESULT>;
pub type PFIND_MATCHING_PROCESS = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, processinfo: *const KDEXT_PROCESS_FIND_PARAMS, process: *mut u64) -> ::windows_sys::core::HRESULT>;
pub type PFIND_MATCHING_THREAD = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, threadinfo: *mut KDEXT_THREAD_FIND_PARAMS) -> ::windows_sys::core::HRESULT>;
pub type PGET_CPU_MICROCODE_VERSION = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, pcpumicrocodeversion: *mut DEBUG_CPU_MICROCODE_VERSION) -> ::windows_sys::core::HRESULT>;
pub type PGET_CPU_PSPEED_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, pcpuspeedinfo: *mut DEBUG_CPU_SPEED_INFO) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PGET_DEVICE_OBJECT_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, deviceobject: u64, pdevobjinfo: *mut DEBUG_DEVICE_OBJECT_INFO) -> ::windows_sys::core::HRESULT>;
pub type PGET_DRIVER_OBJECT_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, driverobject: u64, pdrvobjinfo: *mut DEBUG_DRIVER_OBJECT_INFO) -> ::windows_sys::core::HRESULT>;
pub type PGET_FULL_IMAGE_NAME = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, process: u64, fullimagename: *mut ::windows_sys::core::PSTR) -> ::windows_sys::core::HRESULT>;
pub type PGET_IRP_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, irp: u64, irpinfo: *mut DEBUG_IRP_INFO) -> ::windows_sys::core::HRESULT>;
pub type PGET_PNP_TRIAGE_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, ppnptriageinfo: *mut DEBUG_PNP_TRIAGE_INFO) -> ::windows_sys::core::HRESULT>;
pub type PGET_POOL_DATA = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, pool: u64, pooldata: *mut DEBUG_POOL_DATA) -> ::windows_sys::core::HRESULT>;
pub type PGET_POOL_REGION = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, pool: u64, poolregion: *mut DEBUG_POOL_REGION) -> ::windows_sys::core::HRESULT>;
pub type PGET_POOL_TAG_DESCRIPTION = ::core::option::Option<unsafe extern "system" fn(pooltag: u32, pdescription: *mut DEBUG_POOLTAG_DESCRIPTION) -> ::windows_sys::core::HRESULT>;
pub type PGET_PROCESS_COMMIT = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, totalcommitcharge: *mut u64, numberofprocesses: *mut u32, commitdata: *mut *mut PROCESS_COMMIT_USAGE) -> ::windows_sys::core::HRESULT>;
pub type PGET_SMBIOS_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, psmbiosinfo: *mut DEBUG_SMBIOS_INFO) -> ::windows_sys::core::HRESULT>;
pub type PKDEXTS_GET_PTE_INFO = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, r#virtual: u64, pteinfo: *mut KDEXTS_PTE_INFO) -> ::windows_sys::core::HRESULT>;
pub type PSYM_DUMP_FIELD_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pfield: *mut FIELD_INFO, usercontext: *mut ::core::ffi::c_void) -> u32>;
pub type PWINDBG_CHECK_CONTROL_C = ::core::option::Option<unsafe extern "system" fn() -> u32>;
pub type PWINDBG_CHECK_VERSION = ::core::option::Option<unsafe extern "system" fn() -> u32>;
pub type PWINDBG_DISASM = ::core::option::Option<unsafe extern "system" fn(lpoffset: *mut usize, lpbuffer: ::windows_sys::core::PCSTR, fshoweffectiveaddress: u32) -> u32>;
pub type PWINDBG_DISASM32 = ::core::option::Option<unsafe extern "system" fn(lpoffset: *mut u32, lpbuffer: ::windows_sys::core::PCSTR, fshoweffectiveaddress: u32) -> u32>;
pub type PWINDBG_DISASM64 = ::core::option::Option<unsafe extern "system" fn(lpoffset: *mut u64, lpbuffer: ::windows_sys::core::PCSTR, fshoweffectiveaddress: u32) -> u32>;
pub type PWINDBG_EXTENSION_API_VERSION = ::core::option::Option<unsafe extern "system" fn() -> *mut EXT_API_VERSION>;
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub type PWINDBG_EXTENSION_DLL_INIT = ::core::option::Option<unsafe extern "system" fn(lpextensionapis: *mut WINDBG_EXTENSION_APIS, majorversion: u16, minorversion: u16) -> ()>;
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub type PWINDBG_EXTENSION_DLL_INIT32 = ::core::option::Option<unsafe extern "system" fn(lpextensionapis: *mut WINDBG_EXTENSION_APIS32, majorversion: u16, minorversion: u16) -> ()>;
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub type PWINDBG_EXTENSION_DLL_INIT64 = ::core::option::Option<unsafe extern "system" fn(lpextensionapis: *mut WINDBG_EXTENSION_APIS64, majorversion: u16, minorversion: u16) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PWINDBG_EXTENSION_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hcurrentprocess: super::super::super::super::Foundation::HANDLE, hcurrentthread: super::super::super::super::Foundation::HANDLE, dwcurrentpc: u32, dwprocessor: u32, lpargumentstring: ::windows_sys::core::PCSTR) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PWINDBG_EXTENSION_ROUTINE32 = ::core::option::Option<unsafe extern "system" fn(hcurrentprocess: super::super::super::super::Foundation::HANDLE, hcurrentthread: super::super::super::super::Foundation::HANDLE, dwcurrentpc: u32, dwprocessor: u32, lpargumentstring: ::windows_sys::core::PCSTR) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PWINDBG_EXTENSION_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(hcurrentprocess: super::super::super::super::Foundation::HANDLE, hcurrentthread: super::super::super::super::Foundation::HANDLE, dwcurrentpc: u64, dwprocessor: u32, lpargumentstring: ::windows_sys::core::PCSTR) -> ()>;
pub type PWINDBG_GET_EXPRESSION = ::core::option::Option<unsafe extern "system" fn(lpexpression: ::windows_sys::core::PCSTR) -> usize>;
pub type PWINDBG_GET_EXPRESSION32 = ::core::option::Option<unsafe extern "system" fn(lpexpression: ::windows_sys::core::PCSTR) -> u32>;
pub type PWINDBG_GET_EXPRESSION64 = ::core::option::Option<unsafe extern "system" fn(lpexpression: ::windows_sys::core::PCSTR) -> u64>;
pub type PWINDBG_GET_SYMBOL = ::core::option::Option<unsafe extern "system" fn(offset: *mut ::core::ffi::c_void, pchbuffer: ::windows_sys::core::PCSTR, pdisplacement: *mut usize) -> ()>;
pub type PWINDBG_GET_SYMBOL32 = ::core::option::Option<unsafe extern "system" fn(offset: u32, pchbuffer: ::windows_sys::core::PCSTR, pdisplacement: *mut u32) -> ()>;
pub type PWINDBG_GET_SYMBOL64 = ::core::option::Option<unsafe extern "system" fn(offset: u64, pchbuffer: ::windows_sys::core::PCSTR, pdisplacement: *mut u64) -> ()>;
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub type PWINDBG_GET_THREAD_CONTEXT_ROUTINE = ::core::option::Option<unsafe extern "system" fn(processor: u32, lpcontext: *mut super::CONTEXT, cbsizeofcontext: u32) -> u32>;
pub type PWINDBG_IOCTL_ROUTINE = ::core::option::Option<unsafe extern "system" fn(ioctltype: u16, lpvdata: *mut ::core::ffi::c_void, cbsize: u32) -> u32>;
pub type PWINDBG_OLDKD_EXTENSION_ROUTINE = ::core::option::Option<unsafe extern "system" fn(dwcurrentpc: u32, lpextensionapis: *mut WINDBG_OLDKD_EXTENSION_APIS, lpargumentstring: ::windows_sys::core::PCSTR) -> ()>;
pub type PWINDBG_OLDKD_READ_PHYSICAL_MEMORY = ::core::option::Option<unsafe extern "system" fn(address: u64, buffer: *mut ::core::ffi::c_void, count: u32, bytesread: *mut u32) -> u32>;
pub type PWINDBG_OLDKD_WRITE_PHYSICAL_MEMORY = ::core::option::Option<unsafe extern "system" fn(address: u64, buffer: *mut ::core::ffi::c_void, length: u32, byteswritten: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub type PWINDBG_OLD_EXTENSION_ROUTINE = ::core::option::Option<unsafe extern "system" fn(dwcurrentpc: u32, lpextensionapis: *mut WINDBG_EXTENSION_APIS, lpargumentstring: ::windows_sys::core::PCSTR) -> ()>;
pub type PWINDBG_OUTPUT_ROUTINE = ::core::option::Option<unsafe extern "system" fn(lpformat: ::windows_sys::core::PCSTR) -> ()>;
pub type PWINDBG_READ_PROCESS_MEMORY_ROUTINE = ::core::option::Option<unsafe extern "system" fn(offset: usize, lpbuffer: *mut ::core::ffi::c_void, cb: u32, lpcbbytesread: *mut u32) -> u32>;
pub type PWINDBG_READ_PROCESS_MEMORY_ROUTINE32 = ::core::option::Option<unsafe extern "system" fn(offset: u32, lpbuffer: *mut ::core::ffi::c_void, cb: u32, lpcbbytesread: *mut u32) -> u32>;
pub type PWINDBG_READ_PROCESS_MEMORY_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(offset: u64, lpbuffer: *mut ::core::ffi::c_void, cb: u32, lpcbbytesread: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub type PWINDBG_SET_THREAD_CONTEXT_ROUTINE = ::core::option::Option<unsafe extern "system" fn(processor: u32, lpcontext: *mut super::CONTEXT, cbsizeofcontext: u32) -> u32>;
pub type PWINDBG_STACKTRACE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(framepointer: u32, stackpointer: u32, programcounter: u32, stackframes: *mut EXTSTACKTRACE, frames: u32) -> u32>;
pub type PWINDBG_STACKTRACE_ROUTINE32 = ::core::option::Option<unsafe extern "system" fn(framepointer: u32, stackpointer: u32, programcounter: u32, stackframes: *mut EXTSTACKTRACE32, frames: u32) -> u32>;
pub type PWINDBG_STACKTRACE_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(framepointer: u64, stackpointer: u64, programcounter: u64, stackframes: *mut EXTSTACKTRACE64, frames: u32) -> u32>;
pub type PWINDBG_WRITE_PROCESS_MEMORY_ROUTINE = ::core::option::Option<unsafe extern "system" fn(offset: usize, lpbuffer: *const ::core::ffi::c_void, cb: u32, lpcbbyteswritten: *mut u32) -> u32>;
pub type PWINDBG_WRITE_PROCESS_MEMORY_ROUTINE32 = ::core::option::Option<unsafe extern "system" fn(offset: u32, lpbuffer: *const ::core::ffi::c_void, cb: u32, lpcbbyteswritten: *mut u32) -> u32>;
pub type PWINDBG_WRITE_PROCESS_MEMORY_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(offset: u64, lpbuffer: *const ::core::ffi::c_void, cb: u32, lpcbbyteswritten: *mut u32) -> u32>;
pub type fnDebugFailureAnalysisCreateInstance = ::core::option::Option<unsafe extern "system" fn(client: IDebugClient, args: ::windows_sys::core::PCWSTR, flags: u32, rclsid: *const ::windows_sys::core::GUID, riid: *const ::windows_sys::core::GUID, ppv: *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
