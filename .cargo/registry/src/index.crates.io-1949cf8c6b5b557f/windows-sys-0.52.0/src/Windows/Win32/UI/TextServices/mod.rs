#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msctfmonitor.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DoMsCtfMonitor(dwflags : u32, heventforservicestop : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
::windows_targets::link!("msctfmonitor.dll" "system" fn InitLocalMsCtfMonitor(dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("msctfmonitor.dll" "system" fn UninitLocalMsCtfMonitor() -> ::windows_sys::core::HRESULT);
pub type IAccClientDocMgr = *mut ::core::ffi::c_void;
pub type IAccDictionary = *mut ::core::ffi::c_void;
pub type IAccServerDocMgr = *mut ::core::ffi::c_void;
pub type IAccStore = *mut ::core::ffi::c_void;
pub type IAnchor = *mut ::core::ffi::c_void;
pub type IClonableWrapper = *mut ::core::ffi::c_void;
pub type ICoCreateLocally = *mut ::core::ffi::c_void;
pub type ICoCreatedLocally = *mut ::core::ffi::c_void;
pub type IDocWrap = *mut ::core::ffi::c_void;
pub type IEnumITfCompositionView = *mut ::core::ffi::c_void;
pub type IEnumSpeechCommands = *mut ::core::ffi::c_void;
pub type IEnumTfCandidates = *mut ::core::ffi::c_void;
pub type IEnumTfContextViews = *mut ::core::ffi::c_void;
pub type IEnumTfContexts = *mut ::core::ffi::c_void;
pub type IEnumTfDisplayAttributeInfo = *mut ::core::ffi::c_void;
pub type IEnumTfDocumentMgrs = *mut ::core::ffi::c_void;
pub type IEnumTfFunctionProviders = *mut ::core::ffi::c_void;
pub type IEnumTfInputProcessorProfiles = *mut ::core::ffi::c_void;
pub type IEnumTfLangBarItems = *mut ::core::ffi::c_void;
pub type IEnumTfLanguageProfiles = *mut ::core::ffi::c_void;
pub type IEnumTfLatticeElements = *mut ::core::ffi::c_void;
pub type IEnumTfProperties = *mut ::core::ffi::c_void;
pub type IEnumTfPropertyValue = *mut ::core::ffi::c_void;
pub type IEnumTfRanges = *mut ::core::ffi::c_void;
pub type IEnumTfUIElements = *mut ::core::ffi::c_void;
pub type IInternalDocWrap = *mut ::core::ffi::c_void;
pub type ISpeechCommandProvider = *mut ::core::ffi::c_void;
pub type ITextStoreACP = *mut ::core::ffi::c_void;
pub type ITextStoreACP2 = *mut ::core::ffi::c_void;
pub type ITextStoreACPEx = *mut ::core::ffi::c_void;
pub type ITextStoreACPServices = *mut ::core::ffi::c_void;
pub type ITextStoreACPSink = *mut ::core::ffi::c_void;
pub type ITextStoreACPSinkEx = *mut ::core::ffi::c_void;
pub type ITextStoreAnchor = *mut ::core::ffi::c_void;
pub type ITextStoreAnchorEx = *mut ::core::ffi::c_void;
pub type ITextStoreAnchorSink = *mut ::core::ffi::c_void;
pub type ITextStoreSinkAnchorEx = *mut ::core::ffi::c_void;
pub type ITfActiveLanguageProfileNotifySink = *mut ::core::ffi::c_void;
pub type ITfCandidateList = *mut ::core::ffi::c_void;
pub type ITfCandidateListUIElement = *mut ::core::ffi::c_void;
pub type ITfCandidateListUIElementBehavior = *mut ::core::ffi::c_void;
pub type ITfCandidateString = *mut ::core::ffi::c_void;
pub type ITfCategoryMgr = *mut ::core::ffi::c_void;
pub type ITfCleanupContextDurationSink = *mut ::core::ffi::c_void;
pub type ITfCleanupContextSink = *mut ::core::ffi::c_void;
pub type ITfClientId = *mut ::core::ffi::c_void;
pub type ITfCompartment = *mut ::core::ffi::c_void;
pub type ITfCompartmentEventSink = *mut ::core::ffi::c_void;
pub type ITfCompartmentMgr = *mut ::core::ffi::c_void;
pub type ITfComposition = *mut ::core::ffi::c_void;
pub type ITfCompositionSink = *mut ::core::ffi::c_void;
pub type ITfCompositionView = *mut ::core::ffi::c_void;
pub type ITfConfigureSystemKeystrokeFeed = *mut ::core::ffi::c_void;
pub type ITfContext = *mut ::core::ffi::c_void;
pub type ITfContextComposition = *mut ::core::ffi::c_void;
pub type ITfContextKeyEventSink = *mut ::core::ffi::c_void;
pub type ITfContextOwner = *mut ::core::ffi::c_void;
pub type ITfContextOwnerCompositionServices = *mut ::core::ffi::c_void;
pub type ITfContextOwnerCompositionSink = *mut ::core::ffi::c_void;
pub type ITfContextOwnerServices = *mut ::core::ffi::c_void;
pub type ITfContextView = *mut ::core::ffi::c_void;
pub type ITfCreatePropertyStore = *mut ::core::ffi::c_void;
pub type ITfDisplayAttributeInfo = *mut ::core::ffi::c_void;
pub type ITfDisplayAttributeMgr = *mut ::core::ffi::c_void;
pub type ITfDisplayAttributeNotifySink = *mut ::core::ffi::c_void;
pub type ITfDisplayAttributeProvider = *mut ::core::ffi::c_void;
pub type ITfDocumentMgr = *mut ::core::ffi::c_void;
pub type ITfEditRecord = *mut ::core::ffi::c_void;
pub type ITfEditSession = *mut ::core::ffi::c_void;
pub type ITfEditTransactionSink = *mut ::core::ffi::c_void;
pub type ITfFnAdviseText = *mut ::core::ffi::c_void;
pub type ITfFnBalloon = *mut ::core::ffi::c_void;
pub type ITfFnConfigure = *mut ::core::ffi::c_void;
pub type ITfFnConfigureRegisterEudc = *mut ::core::ffi::c_void;
pub type ITfFnConfigureRegisterWord = *mut ::core::ffi::c_void;
pub type ITfFnCustomSpeechCommand = *mut ::core::ffi::c_void;
pub type ITfFnGetLinguisticAlternates = *mut ::core::ffi::c_void;
pub type ITfFnGetPreferredTouchKeyboardLayout = *mut ::core::ffi::c_void;
pub type ITfFnGetSAPIObject = *mut ::core::ffi::c_void;
pub type ITfFnLMInternal = *mut ::core::ffi::c_void;
pub type ITfFnLMProcessor = *mut ::core::ffi::c_void;
pub type ITfFnLangProfileUtil = *mut ::core::ffi::c_void;
pub type ITfFnPlayBack = *mut ::core::ffi::c_void;
pub type ITfFnPropertyUIStatus = *mut ::core::ffi::c_void;
pub type ITfFnReconversion = *mut ::core::ffi::c_void;
pub type ITfFnSearchCandidateProvider = *mut ::core::ffi::c_void;
pub type ITfFnShowHelp = *mut ::core::ffi::c_void;
pub type ITfFunction = *mut ::core::ffi::c_void;
pub type ITfFunctionProvider = *mut ::core::ffi::c_void;
pub type ITfInputProcessorProfileActivationSink = *mut ::core::ffi::c_void;
pub type ITfInputProcessorProfileMgr = *mut ::core::ffi::c_void;
pub type ITfInputProcessorProfileSubstituteLayout = *mut ::core::ffi::c_void;
pub type ITfInputProcessorProfiles = *mut ::core::ffi::c_void;
pub type ITfInputProcessorProfilesEx = *mut ::core::ffi::c_void;
pub type ITfInputScope = *mut ::core::ffi::c_void;
pub type ITfInputScope2 = *mut ::core::ffi::c_void;
pub type ITfInsertAtSelection = *mut ::core::ffi::c_void;
pub type ITfIntegratableCandidateListUIElement = *mut ::core::ffi::c_void;
pub type ITfKeyEventSink = *mut ::core::ffi::c_void;
pub type ITfKeyTraceEventSink = *mut ::core::ffi::c_void;
pub type ITfKeystrokeMgr = *mut ::core::ffi::c_void;
pub type ITfLMLattice = *mut ::core::ffi::c_void;
pub type ITfLangBarEventSink = *mut ::core::ffi::c_void;
pub type ITfLangBarItem = *mut ::core::ffi::c_void;
pub type ITfLangBarItemBalloon = *mut ::core::ffi::c_void;
pub type ITfLangBarItemBitmap = *mut ::core::ffi::c_void;
pub type ITfLangBarItemBitmapButton = *mut ::core::ffi::c_void;
pub type ITfLangBarItemButton = *mut ::core::ffi::c_void;
pub type ITfLangBarItemMgr = *mut ::core::ffi::c_void;
pub type ITfLangBarItemSink = *mut ::core::ffi::c_void;
pub type ITfLangBarMgr = *mut ::core::ffi::c_void;
pub type ITfLanguageProfileNotifySink = *mut ::core::ffi::c_void;
pub type ITfMSAAControl = *mut ::core::ffi::c_void;
pub type ITfMenu = *mut ::core::ffi::c_void;
pub type ITfMessagePump = *mut ::core::ffi::c_void;
pub type ITfMouseSink = *mut ::core::ffi::c_void;
pub type ITfMouseTracker = *mut ::core::ffi::c_void;
pub type ITfMouseTrackerACP = *mut ::core::ffi::c_void;
pub type ITfPersistentPropertyLoaderACP = *mut ::core::ffi::c_void;
pub type ITfPreservedKeyNotifySink = *mut ::core::ffi::c_void;
pub type ITfProperty = *mut ::core::ffi::c_void;
pub type ITfPropertyStore = *mut ::core::ffi::c_void;
pub type ITfQueryEmbedded = *mut ::core::ffi::c_void;
pub type ITfRange = *mut ::core::ffi::c_void;
pub type ITfRangeACP = *mut ::core::ffi::c_void;
pub type ITfRangeBackup = *mut ::core::ffi::c_void;
pub type ITfReadOnlyProperty = *mut ::core::ffi::c_void;
pub type ITfReadingInformationUIElement = *mut ::core::ffi::c_void;
pub type ITfReverseConversion = *mut ::core::ffi::c_void;
pub type ITfReverseConversionList = *mut ::core::ffi::c_void;
pub type ITfReverseConversionMgr = *mut ::core::ffi::c_void;
pub type ITfSource = *mut ::core::ffi::c_void;
pub type ITfSourceSingle = *mut ::core::ffi::c_void;
pub type ITfSpeechUIServer = *mut ::core::ffi::c_void;
pub type ITfStatusSink = *mut ::core::ffi::c_void;
pub type ITfSystemDeviceTypeLangBarItem = *mut ::core::ffi::c_void;
pub type ITfSystemLangBarItem = *mut ::core::ffi::c_void;
pub type ITfSystemLangBarItemSink = *mut ::core::ffi::c_void;
pub type ITfSystemLangBarItemText = *mut ::core::ffi::c_void;
pub type ITfTextEditSink = *mut ::core::ffi::c_void;
pub type ITfTextInputProcessor = *mut ::core::ffi::c_void;
pub type ITfTextInputProcessorEx = *mut ::core::ffi::c_void;
pub type ITfTextLayoutSink = *mut ::core::ffi::c_void;
pub type ITfThreadFocusSink = *mut ::core::ffi::c_void;
pub type ITfThreadMgr = *mut ::core::ffi::c_void;
pub type ITfThreadMgr2 = *mut ::core::ffi::c_void;
pub type ITfThreadMgrEventSink = *mut ::core::ffi::c_void;
pub type ITfThreadMgrEx = *mut ::core::ffi::c_void;
pub type ITfToolTipUIElement = *mut ::core::ffi::c_void;
pub type ITfTransitoryExtensionSink = *mut ::core::ffi::c_void;
pub type ITfTransitoryExtensionUIElement = *mut ::core::ffi::c_void;
pub type ITfUIElement = *mut ::core::ffi::c_void;
pub type ITfUIElementMgr = *mut ::core::ffi::c_void;
pub type ITfUIElementSink = *mut ::core::ffi::c_void;
pub type IUIManagerEventSink = *mut ::core::ffi::c_void;
pub type IVersionInfo = *mut ::core::ffi::c_void;
pub const AccClientDocMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfc48cc30_4f3e_4fa1_803b_ad0e196a83b1);
pub const AccDictionary: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6572ee16_5fe5_4331_bb6d_76a49c56e423);
pub const AccServerDocMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6089a37e_eb8a_482d_bd6f_f9f46904d16d);
pub const AccStore: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5440837f_4bff_4ae5_a1b1_7722ecc6332a);
pub const CAND_CANCELED: TfCandidateResult = 2i32;
pub const CAND_FINALIZED: TfCandidateResult = 0i32;
pub const CAND_SELECTED: TfCandidateResult = 1i32;
pub const CLSID_TF_CategoryMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa4b544a1_438d_4b41_9325_869523e2d6c7);
pub const CLSID_TF_ClassicLangBar: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3318360c_1afc_4d09_a86b_9f9cb6dceb9c);
pub const CLSID_TF_DisplayAttributeMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3ce74de4_53d3_4d74_8b83_431b3828ba53);
pub const CLSID_TF_InputProcessorProfiles: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x33c53a50_f456_4884_b049_85fd643ecfed);
pub const CLSID_TF_LangBarItemMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb9931692_a2b3_4fab_bf33_9ec6f9fb96ac);
pub const CLSID_TF_LangBarMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xebb08c45_6c4a_4fdc_ae53_4eb8c4c7db8e);
pub const CLSID_TF_ThreadMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x529a9e6b_6587_4f23_ab9e_9c7d683e3c50);
pub const CLSID_TF_TransitoryExtensionUIEntry: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae6be008_07fb_400d_8beb_337a64f7051f);
pub const CLSID_TsfServices: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x39aedc00_6b60_46db_8d31_3642be0e4373);
pub const DCM_FLAGS_CTFMON: u32 = 2u32;
pub const DCM_FLAGS_LOCALTHREADTSF: u32 = 4u32;
pub const DCM_FLAGS_TASKENG: u32 = 1u32;
pub const DocWrap: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbf426f7e_7a5e_44d6_830c_a390ea9462a3);
pub const GETIF_DICTGRAM: TfSapiObject = 4i32;
pub const GETIF_RECOCONTEXT: TfSapiObject = 1i32;
pub const GETIF_RECOGNIZER: TfSapiObject = 2i32;
pub const GETIF_RECOGNIZERNOINIT: TfSapiObject = 5i32;
pub const GETIF_RESMGR: TfSapiObject = 0i32;
pub const GETIF_VOICE: TfSapiObject = 3i32;
pub const GUID_APP_FUNCTIONPROVIDER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4caef01e_12af_4b0e_9db1_a6ec5b881208);
pub const GUID_COMPARTMENT_CONVERSIONMODEBIAS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5497f516_ee91_436e_b946_aa2c05f1ac5b);
pub const GUID_COMPARTMENT_EMPTYCONTEXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7487dbf_804e_41c5_894d_ad96fd4eea13);
pub const GUID_COMPARTMENT_ENABLED_PROFILES_UPDATED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x92c1fd48_a9ae_4a7c_be08_4329e4723817);
pub const GUID_COMPARTMENT_HANDWRITING_OPENCLOSE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf9ae2c6b_1866_4361_af72_7aa30948890e);
pub const GUID_COMPARTMENT_KEYBOARD_DISABLED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x71a5b253_1951_466b_9fbc_9c8808fa84f2);
pub const GUID_COMPARTMENT_KEYBOARD_INPUTMODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb6592511_bcee_4122_a7c4_09f4b3fa4396);
pub const GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xccf05dd8_4a87_11d7_a6e2_00065b84435c);
pub const GUID_COMPARTMENT_KEYBOARD_INPUTMODE_SENTENCE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xccf05dd9_4a87_11d7_a6e2_00065b84435c);
pub const GUID_COMPARTMENT_KEYBOARD_OPENCLOSE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x58273aad_01bb_4164_95c6_755ba0b5162d);
pub const GUID_COMPARTMENT_SAPI_AUDIO: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x51af2086_cc6b_457d_b5aa_8b19dc290ab4);
pub const GUID_COMPARTMENT_SPEECH_CFGMENU: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfb6c5c2d_4e83_4bb6_91a2_e019bff6762d);
pub const GUID_COMPARTMENT_SPEECH_DISABLED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x56c5c607_0703_4e59_8e52_cbc84e8bbe35);
pub const GUID_COMPARTMENT_SPEECH_GLOBALSTATE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2a54fe8e_0d08_460c_a75d_87035ff436c5);
pub const GUID_COMPARTMENT_SPEECH_OPENCLOSE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x544d6a63_e2e8_4752_bbd1_000960bca083);
pub const GUID_COMPARTMENT_SPEECH_UI_STATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd92016f0_9367_4fe7_9abf_bc59dacbe0e3);
pub const GUID_COMPARTMENT_TIPUISTATUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x148ca3ec_0366_401c_8d75_ed978d85fbc9);
pub const GUID_COMPARTMENT_TRANSITORYEXTENSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8be347f5_c7a0_11d7_b408_00065b84435c);
pub const GUID_COMPARTMENT_TRANSITORYEXTENSION_DOCUMENTMANAGER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8be347f7_c7a0_11d7_b408_00065b84435c);
pub const GUID_COMPARTMENT_TRANSITORYEXTENSION_PARENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8be347f8_c7a0_11d7_b408_00065b84435c);
pub const GUID_INTEGRATIONSTYLE_SEARCHBOX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe6d1bd11_82f7_4903_ae21_1a6397cde2eb);
pub const GUID_LBI_INPUTMODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2c77a81e_41cc_4178_a3a7_5f8a987568e6);
pub const GUID_LBI_SAPILAYR_CFGMENUBUTTON: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd02f24a1_942d_422e_8d99_b4f2addee999);
pub const GUID_MODEBIAS_CHINESE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7add26de_4328_489b_83ae_6493750cad5c);
pub const GUID_MODEBIAS_CONVERSATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0f4ec104_1790_443b_95f1_e10f939d6546);
pub const GUID_MODEBIAS_DATETIME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2bdb372_7f61_4039_92ef_1c35599f0222);
pub const GUID_MODEBIAS_FILENAME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7f707fe_44c6_4fca_8e76_86ab50c7931b);
pub const GUID_MODEBIAS_FULLWIDTHALPHANUMERIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x81489fb8_b36a_473d_8146_e4a2258b24ae);
pub const GUID_MODEBIAS_FULLWIDTHHANGUL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc01ae6c9_45b5_4fd0_9cb1_9f4cebc39fea);
pub const GUID_MODEBIAS_HALFWIDTHKATAKANA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x005f6b63_78d4_41cc_8859_485ca821a795);
pub const GUID_MODEBIAS_HANGUL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x76ef0541_23b3_4d77_a074_691801ccea17);
pub const GUID_MODEBIAS_HIRAGANA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd73d316e_9b91_46f1_a280_31597f52c694);
pub const GUID_MODEBIAS_KATAKANA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2e0eeddd_3a1a_499e_8543_3c7ee7949811);
pub const GUID_MODEBIAS_NAME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfddc10f0_d239_49bf_b8fc_5410caaa427e);
pub const GUID_MODEBIAS_NONE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000000_0000_0000_0000_000000000000);
pub const GUID_MODEBIAS_NUMERIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4021766c_e872_48fd_9cee_4ec5c75e16c3);
pub const GUID_MODEBIAS_READING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe31643a3_6466_4cbf_8d8b_0bd4d8545461);
pub const GUID_MODEBIAS_URLHISTORY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8b0e54d9_63f2_4c68_84d4_79aee7a59f09);
pub const GUID_PROP_ATTRIBUTE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x34b45670_7526_11d2_a147_00105a2799b5);
pub const GUID_PROP_COMPOSING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe12ac060_af15_11d2_afc5_00105a2799b5);
pub const GUID_PROP_INPUTSCOPE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1713dd5a_68e7_4a5b_9af6_592a595c778d);
pub const GUID_PROP_LANGID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3280ce20_8032_11d2_b603_00105a2799b5);
pub const GUID_PROP_MODEBIAS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x372e0716_974f_40ac_a088_08cdc92ebfbc);
pub const GUID_PROP_READING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5463f7c0_8e31_11d2_bf46_00105a2799b5);
pub const GUID_PROP_TEXTOWNER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf1e2d520_0969_11d3_8df0_00105a2799b5);
pub const GUID_PROP_TKB_ALTERNATES: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x70b2a803_968d_462e_b93b_2164c91517f7);
pub const GUID_SYSTEM_FUNCTIONPROVIDER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9a698bb0_0f21_11d3_8df1_00105a2799b5);
pub const GUID_TFCAT_CATEGORY_OF_TIP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x534c48c1_0607_4098_a521_4fc899c73e90);
pub const GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb95f181b_ea4c_4af1_8056_7c321abbb091);
pub const GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x046b8c80_1647_40f7_9b21_b93b81aabc1b);
pub const GUID_TFCAT_PROPSTYLE_STATIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x565fb8d8_6bd4_4ca1_b223_0f2ccb8f4f96);
pub const GUID_TFCAT_PROP_AUDIODATA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9b7be3a9_e8ab_4d47_a8fe_254fa423436d);
pub const GUID_TFCAT_PROP_INKDATA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7c6a82ae_b0d7_4f14_a745_14f28b009d61);
pub const GUID_TFCAT_TIPCAP_COMLESS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x364215d9_75bc_11d7_a6ef_00065b84435c);
pub const GUID_TFCAT_TIPCAP_DUALMODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3af314a2_d79f_4b1b_9992_15086d339b05);
pub const GUID_TFCAT_TIPCAP_IMMERSIVEONLY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3a4259ac_640d_4ad4_89f7_1eb67e7c4ee8);
pub const GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x13a016df_560b_46cd_947a_4c3af1e0e35d);
pub const GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xccf05dd7_4a87_11d7_a6e2_00065b84435c);
pub const GUID_TFCAT_TIPCAP_LOCALSERVER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x74769ee9_4a66_4f9d_90d6_bf8b7c3eb461);
pub const GUID_TFCAT_TIPCAP_SECUREMODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x49d2f9ce_1f5e_11d7_a6d3_00065b84435c);
pub const GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x25504fb4_7bab_4bc1_9c69_cf81890f0ef5);
pub const GUID_TFCAT_TIPCAP_TSF3: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x07dcb4af_98de_4548_bef7_25bd45979a1f);
pub const GUID_TFCAT_TIPCAP_UIELEMENTENABLED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x49d2f9cf_1f5e_11d7_a6d3_00065b84435c);
pub const GUID_TFCAT_TIPCAP_WOW16: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x364215da_75bc_11d7_a6ef_00065b84435c);
pub const GUID_TFCAT_TIP_HANDWRITING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x246ecb87_c2f2_4abe_905b_c8b38add2c43);
pub const GUID_TFCAT_TIP_KEYBOARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x34745c63_b2f0_4784_8b67_5e12c8701a31);
pub const GUID_TFCAT_TIP_SPEECH: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb5a73cd1_8355_426b_a161_259808f26b14);
pub const GUID_TFCAT_TRANSITORYEXTENSIONUI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6302de22_a5cf_4b02_bfe8_4d72b2bed3c6);
pub const GUID_TS_SERVICE_ACCESSIBLE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf9786200_a5bf_4a0f_8c24_fb16f5d1aabb);
pub const GUID_TS_SERVICE_ACTIVEX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xea937a50_c9a6_4b7d_894a_49d99b784834);
pub const GUID_TS_SERVICE_DATAOBJECT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6086fbb5_e225_46ce_a770_c1bbd3e05d7b);
pub const GXFPF_NEAREST: u32 = 2u32;
pub const GXFPF_ROUND_NEAREST: u32 = 1u32;
pub const ILMCM_CHECKLAYOUTANDTIPENABLED: u32 = 1u32;
pub const ILMCM_LANGUAGEBAROFF: u32 = 2u32;
pub const IS_ADDRESS_CITY: InputScope = 17i32;
pub const IS_ADDRESS_COUNTRYNAME: InputScope = 18i32;
pub const IS_ADDRESS_COUNTRYSHORTNAME: InputScope = 19i32;
pub const IS_ADDRESS_FULLPOSTALADDRESS: InputScope = 13i32;
pub const IS_ADDRESS_POSTALCODE: InputScope = 14i32;
pub const IS_ADDRESS_STATEORPROVINCE: InputScope = 16i32;
pub const IS_ADDRESS_STREET: InputScope = 15i32;
pub const IS_ALPHANUMERIC_FULLWIDTH: InputScope = 41i32;
pub const IS_ALPHANUMERIC_HALFWIDTH: InputScope = 40i32;
pub const IS_ALPHANUMERIC_PIN: InputScope = 65i32;
pub const IS_ALPHANUMERIC_PIN_SET: InputScope = 66i32;
pub const IS_BOPOMOFO: InputScope = 43i32;
pub const IS_CHAT: InputScope = 58i32;
pub const IS_CHAT_WITHOUT_EMOJI: InputScope = 68i32;
pub const IS_CHINESE_FULLWIDTH: InputScope = 54i32;
pub const IS_CHINESE_HALFWIDTH: InputScope = 53i32;
pub const IS_CURRENCY_AMOUNT: InputScope = 21i32;
pub const IS_CURRENCY_AMOUNTANDSYMBOL: InputScope = 20i32;
pub const IS_CURRENCY_CHINESE: InputScope = 42i32;
pub const IS_DATE_DAY: InputScope = 24i32;
pub const IS_DATE_DAYNAME: InputScope = 27i32;
pub const IS_DATE_FULLDATE: InputScope = 22i32;
pub const IS_DATE_MONTH: InputScope = 23i32;
pub const IS_DATE_MONTHNAME: InputScope = 26i32;
pub const IS_DATE_YEAR: InputScope = 25i32;
pub const IS_DEFAULT: InputScope = 0i32;
pub const IS_DIGITS: InputScope = 28i32;
pub const IS_EMAILNAME_OR_ADDRESS: InputScope = 60i32;
pub const IS_EMAIL_SMTPEMAILADDRESS: InputScope = 5i32;
pub const IS_EMAIL_USERNAME: InputScope = 4i32;
pub const IS_ENUMSTRING: InputScope = -5i32;
pub const IS_FILE_FILENAME: InputScope = 3i32;
pub const IS_FILE_FULLFILEPATH: InputScope = 2i32;
pub const IS_FORMULA: InputScope = 51i32;
pub const IS_FORMULA_NUMBER: InputScope = 67i32;
pub const IS_HANGUL_FULLWIDTH: InputScope = 49i32;
pub const IS_HANGUL_HALFWIDTH: InputScope = 48i32;
pub const IS_HANJA: InputScope = 47i32;
pub const IS_HIRAGANA: InputScope = 44i32;
pub const IS_KATAKANA_FULLWIDTH: InputScope = 46i32;
pub const IS_KATAKANA_HALFWIDTH: InputScope = 45i32;
pub const IS_LOGINNAME: InputScope = 6i32;
pub const IS_MAPS: InputScope = 62i32;
pub const IS_NAME_OR_PHONENUMBER: InputScope = 59i32;
pub const IS_NATIVE_SCRIPT: InputScope = 55i32;
pub const IS_NUMBER: InputScope = 29i32;
pub const IS_NUMBER_FULLWIDTH: InputScope = 39i32;
pub const IS_NUMERIC_PASSWORD: InputScope = 63i32;
pub const IS_NUMERIC_PIN: InputScope = 64i32;
pub const IS_ONECHAR: InputScope = 30i32;
pub const IS_PASSWORD: InputScope = 31i32;
pub const IS_PERSONALNAME_FULLNAME: InputScope = 7i32;
pub const IS_PERSONALNAME_GIVENNAME: InputScope = 9i32;
pub const IS_PERSONALNAME_MIDDLENAME: InputScope = 10i32;
pub const IS_PERSONALNAME_PREFIX: InputScope = 8i32;
pub const IS_PERSONALNAME_SUFFIX: InputScope = 12i32;
pub const IS_PERSONALNAME_SURNAME: InputScope = 11i32;
pub const IS_PHRASELIST: InputScope = -1i32;
pub const IS_PRIVATE: InputScope = 61i32;
pub const IS_REGULAREXPRESSION: InputScope = -2i32;
pub const IS_SEARCH: InputScope = 50i32;
pub const IS_SEARCH_INCREMENTAL: InputScope = 52i32;
pub const IS_SRGS: InputScope = -3i32;
pub const IS_TELEPHONE_AREACODE: InputScope = 34i32;
pub const IS_TELEPHONE_COUNTRYCODE: InputScope = 33i32;
pub const IS_TELEPHONE_FULLTELEPHONENUMBER: InputScope = 32i32;
pub const IS_TELEPHONE_LOCALNUMBER: InputScope = 35i32;
pub const IS_TEXT: InputScope = 57i32;
pub const IS_TIME_FULLTIME: InputScope = 36i32;
pub const IS_TIME_HOUR: InputScope = 37i32;
pub const IS_TIME_MINORSEC: InputScope = 38i32;
pub const IS_URL: InputScope = 1i32;
pub const IS_XML: InputScope = -4i32;
pub const IS_YOMI: InputScope = 56i32;
pub const LIBID_MSAATEXTLib: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x150e2d7a_dac1_4582_947d_2a8fd78b82cd);
pub const MSAAControl: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x08cd963f_7a3e_4f5c_9bd8_d692bb043c5b);
pub const STYLE_ACTIVE_SELECTION: TfIntegratableCandidateListSelectionStyle = 0i32;
pub const STYLE_IMPLIED_SELECTION: TfIntegratableCandidateListSelectionStyle = 1i32;
pub const TF_AE_END: TfActiveSelEnd = 2i32;
pub const TF_AE_NONE: TfActiveSelEnd = 0i32;
pub const TF_AE_START: TfActiveSelEnd = 1i32;
pub const TF_ANCHOR_END: TfAnchor = 1i32;
pub const TF_ANCHOR_START: TfAnchor = 0i32;
pub const TF_ATTR_CONVERTED: TF_DA_ATTR_INFO = 2i32;
pub const TF_ATTR_FIXEDCONVERTED: TF_DA_ATTR_INFO = 5i32;
pub const TF_ATTR_INPUT: TF_DA_ATTR_INFO = 0i32;
pub const TF_ATTR_INPUT_ERROR: TF_DA_ATTR_INFO = 4i32;
pub const TF_ATTR_OTHER: TF_DA_ATTR_INFO = -1i32;
pub const TF_ATTR_TARGET_CONVERTED: TF_DA_ATTR_INFO = 1i32;
pub const TF_ATTR_TARGET_NOTCONVERTED: TF_DA_ATTR_INFO = 3i32;
pub const TF_CHAR_EMBEDDED: u32 = 65532u32;
pub const TF_CLUIE_COUNT: u32 = 2u32;
pub const TF_CLUIE_CURRENTPAGE: u32 = 32u32;
pub const TF_CLUIE_DOCUMENTMGR: u32 = 1u32;
pub const TF_CLUIE_PAGEINDEX: u32 = 16u32;
pub const TF_CLUIE_SELECTION: u32 = 4u32;
pub const TF_CLUIE_STRING: u32 = 8u32;
pub const TF_COMMANDING_ENABLED: u32 = 4u32;
pub const TF_COMMANDING_ON: u32 = 8u32;
pub const TF_CONVERSIONMODE_ALPHANUMERIC: u32 = 0u32;
pub const TF_CONVERSIONMODE_CHARCODE: u32 = 32u32;
pub const TF_CONVERSIONMODE_EUDC: u32 = 512u32;
pub const TF_CONVERSIONMODE_FIXED: u32 = 2048u32;
pub const TF_CONVERSIONMODE_FULLSHAPE: u32 = 8u32;
pub const TF_CONVERSIONMODE_KATAKANA: u32 = 2u32;
pub const TF_CONVERSIONMODE_NATIVE: u32 = 1u32;
pub const TF_CONVERSIONMODE_NOCONVERSION: u32 = 256u32;
pub const TF_CONVERSIONMODE_ROMAN: u32 = 16u32;
pub const TF_CONVERSIONMODE_SOFTKEYBOARD: u32 = 128u32;
pub const TF_CONVERSIONMODE_SYMBOL: u32 = 1024u32;
pub const TF_CT_COLORREF: TF_DA_COLORTYPE = 2i32;
pub const TF_CT_NONE: TF_DA_COLORTYPE = 0i32;
pub const TF_CT_SYSCOLOR: TF_DA_COLORTYPE = 1i32;
pub const TF_DEFAULT_SELECTION: u32 = 4294967295u32;
pub const TF_DICTATION_ENABLED: u32 = 2u32;
pub const TF_DICTATION_ON: u32 = 1u32;
pub const TF_DISABLE_BALLOON: u32 = 2u32;
pub const TF_DISABLE_COMMANDING: u32 = 4u32;
pub const TF_DISABLE_DICTATION: u32 = 2u32;
pub const TF_DISABLE_SPEECH: u32 = 1u32;
pub const TF_DTLBI_NONE: LANG_BAR_ITEM_ICON_MODE_FLAGS = 0u32;
pub const TF_DTLBI_USEPROFILEICON: LANG_BAR_ITEM_ICON_MODE_FLAGS = 1u32;
pub const TF_ENABLE_PROCESS_ATOM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_CTF_ENABLE_PROCESS_ATOM_");
pub const TF_ES_ASYNC: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 8u32;
pub const TF_ES_ASYNCDONTCARE: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 0u32;
pub const TF_ES_READ: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 2u32;
pub const TF_ES_READWRITE: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 6u32;
pub const TF_ES_SYNC: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 1u32;
pub const TF_E_ALREADY_EXISTS: ::windows_sys::core::HRESULT = -2147220218i32;
pub const TF_E_COMPOSITION_REJECTED: ::windows_sys::core::HRESULT = -2147220216i32;
pub const TF_E_DISCONNECTED: ::windows_sys::core::HRESULT = -2147220220i32;
pub const TF_E_EMPTYCONTEXT: ::windows_sys::core::HRESULT = -2147220215i32;
pub const TF_E_FORMAT: ::windows_sys::core::HRESULT = -2147220982i32;
pub const TF_E_INVALIDPOINT: ::windows_sys::core::HRESULT = -2147220985i32;
pub const TF_E_INVALIDPOS: ::windows_sys::core::HRESULT = -2147220992i32;
pub const TF_E_INVALIDVIEW: ::windows_sys::core::HRESULT = -2147220219i32;
pub const TF_E_LOCKED: ::windows_sys::core::HRESULT = -2147220224i32;
pub const TF_E_NOCONVERSION: ::windows_sys::core::HRESULT = -2147219968i32;
pub const TF_E_NOINTERFACE: ::windows_sys::core::HRESULT = -2147220988i32;
pub const TF_E_NOLAYOUT: ::windows_sys::core::HRESULT = -2147220986i32;
pub const TF_E_NOLOCK: ::windows_sys::core::HRESULT = -2147220991i32;
pub const TF_E_NOOBJECT: ::windows_sys::core::HRESULT = -2147220990i32;
pub const TF_E_NOPROVIDER: ::windows_sys::core::HRESULT = -2147220221i32;
pub const TF_E_NOSELECTION: ::windows_sys::core::HRESULT = -2147220987i32;
pub const TF_E_NOSERVICE: ::windows_sys::core::HRESULT = -2147220989i32;
pub const TF_E_NOTOWNEDRANGE: ::windows_sys::core::HRESULT = -2147220222i32;
pub const TF_E_RANGE_NOT_COVERED: ::windows_sys::core::HRESULT = -2147220217i32;
pub const TF_E_READONLY: ::windows_sys::core::HRESULT = -2147220983i32;
pub const TF_E_STACKFULL: ::windows_sys::core::HRESULT = -2147220223i32;
pub const TF_E_SYNCHRONOUS: ::windows_sys::core::HRESULT = -2147220984i32;
pub const TF_FLOATINGLANGBAR_WNDTITLE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TF_FloatingLangBar_WndTitle");
pub const TF_FLOATINGLANGBAR_WNDTITLEA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("TF_FloatingLangBar_WndTitle");
pub const TF_FLOATINGLANGBAR_WNDTITLEW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TF_FloatingLangBar_WndTitle");
pub const TF_GRAVITY_BACKWARD: TfGravity = 0i32;
pub const TF_GRAVITY_FORWARD: TfGravity = 1i32;
pub const TF_GTP_INCL_TEXT: GET_TEXT_AND_PROPERTY_UPDATES_FLAGS = 1u32;
pub const TF_GTP_NONE: GET_TEXT_AND_PROPERTY_UPDATES_FLAGS = 0u32;
pub const TF_HF_OBJECT: u32 = 1u32;
pub const TF_IAS_NOQUERY: INSERT_TEXT_AT_SELECTION_FLAGS = 1u32;
pub const TF_IAS_NO_DEFAULT_COMPOSITION: INSERT_TEXT_AT_SELECTION_FLAGS = 2147483648u32;
pub const TF_IAS_QUERYONLY: INSERT_TEXT_AT_SELECTION_FLAGS = 2u32;
pub const TF_IE_CORRECTION: u32 = 1u32;
pub const TF_INVALID_COOKIE: u32 = 4294967295u32;
pub const TF_INVALID_EDIT_COOKIE: u32 = 0u32;
pub const TF_IPPMF_DISABLEPROFILE: u32 = 2u32;
pub const TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE: u32 = 4u32;
pub const TF_IPPMF_ENABLEPROFILE: u32 = 1u32;
pub const TF_IPPMF_FORPROCESS: u32 = 268435456u32;
pub const TF_IPPMF_FORSESSION: u32 = 536870912u32;
pub const TF_IPPMF_FORSYSTEMALL: u32 = 1073741824u32;
pub const TF_IPP_CAPS_COMLESSSUPPORT: u32 = 8u32;
pub const TF_IPP_CAPS_DISABLEONTRANSITORY: u32 = 1u32;
pub const TF_IPP_CAPS_IMMERSIVESUPPORT: u32 = 65536u32;
pub const TF_IPP_CAPS_SECUREMODESUPPORT: u32 = 2u32;
pub const TF_IPP_CAPS_SYSTRAYSUPPORT: u32 = 131072u32;
pub const TF_IPP_CAPS_UIELEMENTENABLED: u32 = 4u32;
pub const TF_IPP_CAPS_WOW16SUPPORT: u32 = 16u32;
pub const TF_IPP_FLAG_ACTIVE: u32 = 1u32;
pub const TF_IPP_FLAG_ENABLED: u32 = 2u32;
pub const TF_IPP_FLAG_SUBSTITUTEDBYINPUTPROCESSOR: u32 = 4u32;
pub const TF_IPSINK_FLAG_ACTIVE: u32 = 1u32;
pub const TF_LBI_BALLOON: u32 = 16u32;
pub const TF_LBI_BITMAP: u32 = 8u32;
pub const TF_LBI_BMPF_VERTICAL: u32 = 1u32;
pub const TF_LBI_CLK_LEFT: TfLBIClick = 2i32;
pub const TF_LBI_CLK_RIGHT: TfLBIClick = 1i32;
pub const TF_LBI_CUSTOMUI: u32 = 32u32;
pub const TF_LBI_DESC_MAXLEN: u32 = 32u32;
pub const TF_LBI_ICON: u32 = 1u32;
pub const TF_LBI_STATUS: u32 = 65536u32;
pub const TF_LBI_STATUS_BTN_TOGGLED: u32 = 65536u32;
pub const TF_LBI_STATUS_DISABLED: u32 = 2u32;
pub const TF_LBI_STATUS_HIDDEN: u32 = 1u32;
pub const TF_LBI_STYLE_BTN_BUTTON: u32 = 65536u32;
pub const TF_LBI_STYLE_BTN_MENU: u32 = 131072u32;
pub const TF_LBI_STYLE_BTN_TOGGLE: u32 = 262144u32;
pub const TF_LBI_STYLE_HIDDENBYDEFAULT: u32 = 16u32;
pub const TF_LBI_STYLE_HIDDENSTATUSCONTROL: u32 = 1u32;
pub const TF_LBI_STYLE_HIDEONNOOTHERITEMS: u32 = 4u32;
pub const TF_LBI_STYLE_SHOWNINTRAY: u32 = 2u32;
pub const TF_LBI_STYLE_SHOWNINTRAYONLY: u32 = 8u32;
pub const TF_LBI_STYLE_TEXTCOLORICON: u32 = 32u32;
pub const TF_LBI_TEXT: u32 = 2u32;
pub const TF_LBI_TOOLTIP: u32 = 4u32;
pub const TF_LBMENUF_CHECKED: u32 = 1u32;
pub const TF_LBMENUF_GRAYED: u32 = 16u32;
pub const TF_LBMENUF_RADIOCHECKED: u32 = 8u32;
pub const TF_LBMENUF_SEPARATOR: u32 = 4u32;
pub const TF_LBMENUF_SUBMENU: u32 = 2u32;
pub const TF_LB_BALLOON_MISS: TfLBBalloonStyle = 2i32;
pub const TF_LB_BALLOON_RECO: TfLBBalloonStyle = 0i32;
pub const TF_LB_BALLOON_SHOW: TfLBBalloonStyle = 1i32;
pub const TF_LC_CHANGE: TfLayoutCode = 1i32;
pub const TF_LC_CREATE: TfLayoutCode = 0i32;
pub const TF_LC_DESTROY: TfLayoutCode = 2i32;
pub const TF_LS_DASH: TF_DA_LINESTYLE = 3i32;
pub const TF_LS_DOT: TF_DA_LINESTYLE = 2i32;
pub const TF_LS_NONE: TF_DA_LINESTYLE = 0i32;
pub const TF_LS_SOLID: TF_DA_LINESTYLE = 1i32;
pub const TF_LS_SQUIGGLE: TF_DA_LINESTYLE = 4i32;
pub const TF_MENUREADY: u32 = 1u32;
pub const TF_MOD_ALT: u32 = 1u32;
pub const TF_MOD_CONTROL: u32 = 2u32;
pub const TF_MOD_IGNORE_ALL_MODIFIER: u32 = 1024u32;
pub const TF_MOD_LALT: u32 = 64u32;
pub const TF_MOD_LCONTROL: u32 = 128u32;
pub const TF_MOD_LSHIFT: u32 = 256u32;
pub const TF_MOD_ON_KEYUP: u32 = 512u32;
pub const TF_MOD_RALT: u32 = 8u32;
pub const TF_MOD_RCONTROL: u32 = 16u32;
pub const TF_MOD_RSHIFT: u32 = 32u32;
pub const TF_MOD_SHIFT: u32 = 4u32;
pub const TF_POPF_ALL: u32 = 1u32;
pub const TF_PROCESS_ATOM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_CTF_PROCESS_ATOM_");
pub const TF_PROFILETYPE_INPUTPROCESSOR: u32 = 1u32;
pub const TF_PROFILETYPE_KEYBOARDLAYOUT: u32 = 2u32;
pub const TF_PROFILE_ARRAY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd38eff65_aa46_4fd5_91a7_67845fb02f5b);
pub const TF_PROFILE_CANTONESE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0aec109c_7e96_11d4_b2ef_0080c882687e);
pub const TF_PROFILE_CHANGJIE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4bdf9f03_c7d3_11d4_b2ab_0080c882687e);
pub const TF_PROFILE_DAYI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x037b2c25_480c_4d7f_b027_d6ca6b69788a);
pub const TF_PROFILE_NEWCHANGJIE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf3ba907a_6c7e_11d4_97fa_0080c882687e);
pub const TF_PROFILE_NEWPHONETIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb2f9c502_1742_11d4_9790_0080c882687e);
pub const TF_PROFILE_NEWQUICK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0b883ba0_c1c7_11d4_87f9_0080c882687e);
pub const TF_PROFILE_PHONETIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x761309de_317a_11d4_9b5d_0080c882687e);
pub const TF_PROFILE_PINYIN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf3ba9077_6c7e_11d4_97fa_0080c882687e);
pub const TF_PROFILE_QUICK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6024b45f_5c54_11d4_b921_0080c882687e);
pub const TF_PROFILE_SIMPLEFAST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfa550b04_5ad7_411f_a5ac_ca038ec515d7);
pub const TF_PROFILE_TIGRINYA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3cab88b7_cc3e_46a6_9765_b772ad7761ff);
pub const TF_PROFILE_WUBI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x82590c13_f4dd_44f4_ba1d_8667246fdf8e);
pub const TF_PROFILE_YI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x409c8376_007b_4357_ae8e_26316ee3fb0d);
pub const TF_PROPUI_STATUS_SAVETOFILE: u32 = 1u32;
pub const TF_RCM_COMLESS: u32 = 1u32;
pub const TF_RCM_HINT_COLLISION: u32 = 8u32;
pub const TF_RCM_HINT_READING_LENGTH: u32 = 4u32;
pub const TF_RCM_VKEY: u32 = 2u32;
pub const TF_RIP_FLAG_FREEUNUSEDLIBRARIES: u32 = 1u32;
pub const TF_RIUIE_CONTEXT: u32 = 1u32;
pub const TF_RIUIE_ERRORINDEX: u32 = 8u32;
pub const TF_RIUIE_MAXREADINGSTRINGLENGTH: u32 = 4u32;
pub const TF_RIUIE_STRING: u32 = 2u32;
pub const TF_RIUIE_VERTICALORDER: u32 = 16u32;
pub const TF_RP_HIDDENINSETTINGUI: u32 = 2u32;
pub const TF_RP_LOCALPROCESS: u32 = 4u32;
pub const TF_RP_LOCALTHREAD: u32 = 8u32;
pub const TF_RP_SUBITEMINSETTINGUI: u32 = 16u32;
pub const TF_SD_BACKWARD: TfShiftDir = 0i32;
pub const TF_SD_FORWARD: TfShiftDir = 1i32;
pub const TF_SD_LOADING: u32 = 2u32;
pub const TF_SD_READONLY: u32 = 1u32;
pub const TF_SENTENCEMODE_AUTOMATIC: u32 = 4u32;
pub const TF_SENTENCEMODE_CONVERSATION: u32 = 16u32;
pub const TF_SENTENCEMODE_NONE: u32 = 0u32;
pub const TF_SENTENCEMODE_PHRASEPREDICT: u32 = 8u32;
pub const TF_SENTENCEMODE_PLAURALCLAUSE: u32 = 1u32;
pub const TF_SENTENCEMODE_SINGLECONVERT: u32 = 2u32;
pub const TF_SFT_DESKBAND: u32 = 2048u32;
pub const TF_SFT_DOCK: u32 = 2u32;
pub const TF_SFT_EXTRAICONSONMINIMIZED: u32 = 512u32;
pub const TF_SFT_HIDDEN: u32 = 8u32;
pub const TF_SFT_HIGHTRANSPARENCY: u32 = 64u32;
pub const TF_SFT_LABELS: u32 = 128u32;
pub const TF_SFT_LOWTRANSPARENCY: u32 = 32u32;
pub const TF_SFT_MINIMIZED: u32 = 4u32;
pub const TF_SFT_NOEXTRAICONSONMINIMIZED: u32 = 1024u32;
pub const TF_SFT_NOLABELS: u32 = 256u32;
pub const TF_SFT_NOTRANSPARENCY: u32 = 16u32;
pub const TF_SFT_SHOWNORMAL: u32 = 1u32;
pub const TF_SHOW_BALLOON: u32 = 1u32;
pub const TF_SPEECHUI_SHOWN: u32 = 16u32;
pub const TF_SS_DISJOINTSEL: u32 = 1u32;
pub const TF_SS_REGIONS: u32 = 2u32;
pub const TF_SS_TKBAUTOCORRECTENABLE: u32 = 16u32;
pub const TF_SS_TKBPREDICTIONENABLE: u32 = 32u32;
pub const TF_SS_TRANSITORY: u32 = 4u32;
pub const TF_ST_CORRECTION: u32 = 1u32;
pub const TF_S_ASYNC: ::windows_sys::core::HRESULT = 262912i32;
pub const TF_TF_IGNOREEND: u32 = 2u32;
pub const TF_TF_MOVESTART: u32 = 1u32;
pub const TF_TMAE_COMLESS: u32 = 8u32;
pub const TF_TMAE_CONSOLE: u32 = 64u32;
pub const TF_TMAE_NOACTIVATEKEYBOARDLAYOUT: u32 = 32u32;
pub const TF_TMAE_NOACTIVATETIP: u32 = 1u32;
pub const TF_TMAE_SECUREMODE: u32 = 2u32;
pub const TF_TMAE_UIELEMENTENABLEDONLY: u32 = 4u32;
pub const TF_TMAE_WOW16: u32 = 16u32;
pub const TF_TMF_ACTIVATED: u32 = 2147483648u32;
pub const TF_TMF_COMLESS: u32 = 8u32;
pub const TF_TMF_CONSOLE: u32 = 64u32;
pub const TF_TMF_IMMERSIVEMODE: u32 = 1073741824u32;
pub const TF_TMF_NOACTIVATETIP: u32 = 1u32;
pub const TF_TMF_SECUREMODE: u32 = 2u32;
pub const TF_TMF_UIELEMENTENABLEDONLY: u32 = 4u32;
pub const TF_TMF_WOW16: u32 = 16u32;
pub const TF_TRANSITORYEXTENSION_ATSELECTION: u32 = 2u32;
pub const TF_TRANSITORYEXTENSION_FLOATING: u32 = 1u32;
pub const TF_TRANSITORYEXTENSION_NONE: u32 = 0u32;
pub const TF_TU_CORRECTION: u32 = 1u32;
pub const TF_URP_ALLPROFILES: u32 = 2u32;
pub const TF_URP_LOCALPROCESS: u32 = 4u32;
pub const TF_URP_LOCALTHREAD: u32 = 8u32;
pub const TF_US_HIDETIPUI: u32 = 1u32;
pub const TKBLT_CLASSIC: TKBLayoutType = 1i32;
pub const TKBLT_OPTIMIZED: TKBLayoutType = 2i32;
pub const TKBLT_UNDEFINED: TKBLayoutType = 0i32;
pub const TKBL_CLASSIC_TRADITIONAL_CHINESE_CHANGJIE: u32 = 61506u32;
pub const TKBL_CLASSIC_TRADITIONAL_CHINESE_DAYI: u32 = 61507u32;
pub const TKBL_CLASSIC_TRADITIONAL_CHINESE_PHONETIC: u32 = 1028u32;
pub const TKBL_OPT_JAPANESE_ABC: u32 = 1041u32;
pub const TKBL_OPT_KOREAN_HANGUL_2_BULSIK: u32 = 1042u32;
pub const TKBL_OPT_SIMPLIFIED_CHINESE_PINYIN: u32 = 2052u32;
pub const TKBL_OPT_TRADITIONAL_CHINESE_PHONETIC: u32 = 1028u32;
pub const TKBL_UNDEFINED: u32 = 0u32;
pub const TKB_ALTERNATES_AUTOCORRECTION_APPLIED: u32 = 4u32;
pub const TKB_ALTERNATES_FOR_AUTOCORRECTION: u32 = 2u32;
pub const TKB_ALTERNATES_FOR_PREDICTION: u32 = 3u32;
pub const TKB_ALTERNATES_STANDARD: u32 = 1u32;
pub const TSATTRID_App: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa80f77df_4237_40e5_849c_b5fa51c13ac7);
pub const TSATTRID_App_IncorrectGrammar: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbd54e398_ad03_4b74_b6b3_5edb19996388);
pub const TSATTRID_App_IncorrectSpelling: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf42de43c_ef12_430d_944c_9a08970a25d2);
pub const TSATTRID_Font: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x573ea825_749b_4f8a_9cfd_21c3605ca828);
pub const TSATTRID_Font_FaceName: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb536aeb6_053b_4eb8_b65a_50da1e81e72e);
pub const TSATTRID_Font_SizePts: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc8493302_a5e9_456d_af04_8005e4130f03);
pub const TSATTRID_Font_Style: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x68b2a77f_6b0e_4f28_8177_571c2f3a42b1);
pub const TSATTRID_Font_Style_Animation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdcf73d22_e029_47b7_bb36_f263a3d004cc);
pub const TSATTRID_Font_Style_Animation_BlinkingBackground: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x86e5b104_0104_4b10_b585_00f2527522b5);
pub const TSATTRID_Font_Style_Animation_LasVegasLights: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf40423d5_0f87_4f8f_bada_e6d60c25e152);
pub const TSATTRID_Font_Style_Animation_MarchingBlackAnts: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7644e067_f186_4902_bfc6_ec815aa20e9d);
pub const TSATTRID_Font_Style_Animation_MarchingRedAnts: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x78368dad_50fb_4c6f_840b_d486bb6cf781);
pub const TSATTRID_Font_Style_Animation_Shimmer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2ce31b58_5293_4c36_8809_bf8bb51a27b3);
pub const TSATTRID_Font_Style_Animation_SparkleText: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x533aad20_962c_4e9f_8c09_b42ea4749711);
pub const TSATTRID_Font_Style_Animation_WipeDown: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5872e874_367b_4803_b160_c90ff62569d0);
pub const TSATTRID_Font_Style_Animation_WipeRight: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb855cbe3_3d2c_4600_b1e9_e1c9ce02f842);
pub const TSATTRID_Font_Style_BackgroundColor: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb50eaa4e_3091_4468_81db_d79ea190c7c7);
pub const TSATTRID_Font_Style_Blink: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbfb2c036_7acf_4532_b720_b416dd7765a8);
pub const TSATTRID_Font_Style_Bold: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x48813a43_8a20_4940_8e58_97823f7b268a);
pub const TSATTRID_Font_Style_Capitalize: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7d85a3ba_b4fd_43b3_befc_6b985c843141);
pub const TSATTRID_Font_Style_Color: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x857a7a37_b8af_4e9a_81b4_acf700c8411b);
pub const TSATTRID_Font_Style_Emboss: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbd8ed742_349e_4e37_82fb_437979cb53a7);
pub const TSATTRID_Font_Style_Engrave: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9c3371de_8332_4897_be5d_89233223179a);
pub const TSATTRID_Font_Style_Height: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7e937477_12e6_458b_926a_1fa44ee8f391);
pub const TSATTRID_Font_Style_Hidden: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb1e28770_881c_475f_863f_887a647b1090);
pub const TSATTRID_Font_Style_Italic: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8740682a_a765_48e1_acfc_d22222b2f810);
pub const TSATTRID_Font_Style_Kerning: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcc26e1b4_2f9a_47c8_8bff_bf1eb7cce0dd);
pub const TSATTRID_Font_Style_Lowercase: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x76d8ccb5_ca7b_4498_8ee9_d5c4f6f74c60);
pub const TSATTRID_Font_Style_Outlined: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x10e6db31_db0d_4ac6_a7f5_9c9cff6f2ab4);
pub const TSATTRID_Font_Style_Overline: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe3989f4a_992b_4301_8ce1_a5b7c6d1f3c8);
pub const TSATTRID_Font_Style_Overline_Double: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdc46063a_e115_46e3_bcd8_ca6772aa95b4);
pub const TSATTRID_Font_Style_Overline_Single: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8440d94c_51ce_47b2_8d4c_15751e5f721b);
pub const TSATTRID_Font_Style_Position: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x15cd26ab_f2fb_4062_b5a6_9a49e1a5cc0b);
pub const TSATTRID_Font_Style_Protected: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1c557cb2_14cf_4554_a574_ecb2f7e7efd4);
pub const TSATTRID_Font_Style_Shadow: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5f686d2f_c6cd_4c56_8a1a_994a4b9766be);
pub const TSATTRID_Font_Style_SmallCaps: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfacb6bc6_9100_4cc6_b969_11eea45a86b4);
pub const TSATTRID_Font_Style_Spacing: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x98c1200d_8f06_409a_8e49_6a554bf7c153);
pub const TSATTRID_Font_Style_Strikethrough: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0c562193_2d08_4668_9601_ced41309d7af);
pub const TSATTRID_Font_Style_Strikethrough_Double: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x62489b31_a3e7_4f94_ac43_ebaf8fcc7a9f);
pub const TSATTRID_Font_Style_Strikethrough_Single: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x75d736b6_3c8f_4b97_ab78_1877cb990d31);
pub const TSATTRID_Font_Style_Subscript: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5774fb84_389b_43bc_a74b_1568347cf0f4);
pub const TSATTRID_Font_Style_Superscript: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2ea4993c_563c_49aa_9372_0bef09a9255b);
pub const TSATTRID_Font_Style_Underline: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc3c9c9f3_7902_444b_9a7b_48e70f4b50f7);
pub const TSATTRID_Font_Style_Underline_Double: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x74d24aa6_1db3_4c69_a176_31120e7586d5);
pub const TSATTRID_Font_Style_Underline_Single: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1b6720e5_0f73_4951_a6b3_6f19e43c9461);
pub const TSATTRID_Font_Style_Uppercase: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x33a300e8_e340_4937_b697_8f234045cd9a);
pub const TSATTRID_Font_Style_Weight: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x12f3189c_8bb0_461b_b1fa_eaf907047fe0);
pub const TSATTRID_List: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x436d673b_26f1_4aee_9e65_8f83a4ed4884);
pub const TSATTRID_List_LevelIndel: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7f7cc899_311f_487b_ad5d_e2a459e12d42);
pub const TSATTRID_List_Type: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae3e665e_4bce_49e3_a0fe_2db47d3a17ae);
pub const TSATTRID_List_Type_Arabic: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1338c5d6_98a3_4fa3_9bd1_7a60eef8e9e0);
pub const TSATTRID_List_Type_Bullet: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbccd77c5_4c4d_4ce2_b102_559f3b2bfcea);
pub const TSATTRID_List_Type_LowerLetter: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x96372285_f3cf_491e_a925_3832347fd237);
pub const TSATTRID_List_Type_LowerRoman: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x90466262_3980_4b8e_9368_918bd1218a41);
pub const TSATTRID_List_Type_UpperLetter: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7987b7cd_ce52_428b_9b95_a357f6f10c45);
pub const TSATTRID_List_Type_UpperRoman: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0f6ab552_4a80_467f_b2f1_127e2aa3ba9e);
pub const TSATTRID_OTHERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb3c32af9_57d0_46a9_bca8_dac238a13057);
pub const TSATTRID_Text: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7edb8e68_81f9_449d_a15a_87a8388faac0);
pub const TSATTRID_Text_Alignment: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x139941e6_1767_456d_938e_35ba568b5cd4);
pub const TSATTRID_Text_Alignment_Center: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa4a95c16_53bf_4d55_8b87_4bdd8d4275fc);
pub const TSATTRID_Text_Alignment_Justify: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xed350740_a0f7_42d3_8ea8_f81b6488faf0);
pub const TSATTRID_Text_Alignment_Left: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x16ae95d3_6361_43a2_8495_d00f397f1693);
pub const TSATTRID_Text_Alignment_Right: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb36f0f98_1b9e_4360_8616_03fb08a78456);
pub const TSATTRID_Text_EmbeddedObject: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7edb8e68_81f9_449d_a15a_87a8388faac0);
pub const TSATTRID_Text_Hyphenation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdadf4525_618e_49eb_b1a8_3b68bd7648e3);
pub const TSATTRID_Text_Language: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd8c04ef1_5753_4c25_8887_85443fe5f819);
pub const TSATTRID_Text_Link: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x47cd9051_3722_4cd8_b7c8_4e17ca1759f5);
pub const TSATTRID_Text_Orientation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bab707f_8785_4c39_8b52_96f878303ffb);
pub const TSATTRID_Text_Para: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5edc5822_99dc_4dd6_aec3_b62baa5b2e7c);
pub const TSATTRID_Text_Para_FirstLineIndent: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x07c97a13_7472_4dd8_90a9_91e3d7e4f29c);
pub const TSATTRID_Text_Para_LeftIndent: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfb2848e9_7471_41c9_b6b3_8a1450e01897);
pub const TSATTRID_Text_Para_LineSpacing: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x699b380d_7f8c_46d6_a73b_dfe3d1538df3);
pub const TSATTRID_Text_Para_LineSpacing_AtLeast: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xadfedf31_2d44_4434_a5ff_7f4c4990a905);
pub const TSATTRID_Text_Para_LineSpacing_Double: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x82fb1805_a6c4_4231_ac12_6260af2aba28);
pub const TSATTRID_Text_Para_LineSpacing_Exactly: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3d45ad40_23de_48d7_a6b3_765420c620cc);
pub const TSATTRID_Text_Para_LineSpacing_Multiple: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x910f1e3c_d6d0_4f65_8a3c_42b4b31868c5);
pub const TSATTRID_Text_Para_LineSpacing_OnePtFive: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0428a021_0397_4b57_9a17_0795994cd3c5);
pub const TSATTRID_Text_Para_LineSpacing_Single: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xed350740_a0f7_42d3_8ea8_f81b6488faf0);
pub const TSATTRID_Text_Para_RightIndent: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2c7f26f9_a5e2_48da_b98a_520cb16513bf);
pub const TSATTRID_Text_Para_SpaceAfter: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7b0a3f55_22dc_425f_a411_93da1d8f9baa);
pub const TSATTRID_Text_Para_SpaceBefore: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8df98589_194a_4601_b251_9865a3e906dd);
pub const TSATTRID_Text_ReadOnly: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x85836617_de32_4afd_a50f_a2db110e6e4d);
pub const TSATTRID_Text_RightToLeft: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca666e71_1b08_453d_bfdd_28e08c8aaf7a);
pub const TSATTRID_Text_VerticalWriting: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bba8195_046f_4ea9_b311_97fd66c4274b);
pub const TS_AE_END: TsActiveSelEnd = 2i32;
pub const TS_AE_NONE: TsActiveSelEnd = 0i32;
pub const TS_AE_START: TsActiveSelEnd = 1i32;
pub const TS_AS_ATTR_CHANGE: u32 = 8u32;
pub const TS_AS_LAYOUT_CHANGE: u32 = 4u32;
pub const TS_AS_SEL_CHANGE: u32 = 2u32;
pub const TS_AS_STATUS_CHANGE: u32 = 16u32;
pub const TS_AS_TEXT_CHANGE: u32 = 1u32;
pub const TS_ATTR_FIND_BACKWARDS: u32 = 1u32;
pub const TS_ATTR_FIND_HIDDEN: u32 = 32u32;
pub const TS_ATTR_FIND_UPDATESTART: u32 = 4u32;
pub const TS_ATTR_FIND_WANT_END: u32 = 16u32;
pub const TS_ATTR_FIND_WANT_OFFSET: u32 = 2u32;
pub const TS_ATTR_FIND_WANT_VALUE: u32 = 8u32;
pub const TS_CHAR_EMBEDDED: u32 = 65532u32;
pub const TS_CHAR_REGION: u32 = 0u32;
pub const TS_CHAR_REPLACEMENT: u32 = 65533u32;
pub const TS_CH_FOLLOWING_DEL: ANCHOR_CHANGE_HISTORY_FLAGS = 2u32;
pub const TS_CH_PRECEDING_DEL: ANCHOR_CHANGE_HISTORY_FLAGS = 1u32;
pub const TS_DEFAULT_SELECTION: u32 = 4294967295u32;
pub const TS_E_FORMAT: ::windows_sys::core::HRESULT = -2147220982i32;
pub const TS_E_INVALIDPOINT: ::windows_sys::core::HRESULT = -2147220985i32;
pub const TS_E_INVALIDPOS: ::windows_sys::core::HRESULT = -2147220992i32;
pub const TS_E_NOINTERFACE: ::windows_sys::core::HRESULT = -2147220988i32;
pub const TS_E_NOLAYOUT: ::windows_sys::core::HRESULT = -2147220986i32;
pub const TS_E_NOLOCK: ::windows_sys::core::HRESULT = -2147220991i32;
pub const TS_E_NOOBJECT: ::windows_sys::core::HRESULT = -2147220990i32;
pub const TS_E_NOSELECTION: ::windows_sys::core::HRESULT = -2147220987i32;
pub const TS_E_NOSERVICE: ::windows_sys::core::HRESULT = -2147220989i32;
pub const TS_E_READONLY: ::windows_sys::core::HRESULT = -2147220983i32;
pub const TS_E_SYNCHRONOUS: ::windows_sys::core::HRESULT = -2147220984i32;
pub const TS_GEA_HIDDEN: u32 = 1u32;
pub const TS_GR_BACKWARD: TsGravity = 0i32;
pub const TS_GR_FORWARD: TsGravity = 1i32;
pub const TS_GTA_HIDDEN: u32 = 1u32;
pub const TS_IAS_NOQUERY: u32 = 1u32;
pub const TS_IAS_QUERYONLY: u32 = 2u32;
pub const TS_IE_COMPOSITION: u32 = 2u32;
pub const TS_IE_CORRECTION: u32 = 1u32;
pub const TS_LC_CHANGE: TsLayoutCode = 1i32;
pub const TS_LC_CREATE: TsLayoutCode = 0i32;
pub const TS_LC_DESTROY: TsLayoutCode = 2i32;
pub const TS_LF_READ: TEXT_STORE_LOCK_FLAGS = 2u32;
pub const TS_LF_READWRITE: TEXT_STORE_LOCK_FLAGS = 6u32;
pub const TS_LF_SYNC: u32 = 1u32;
pub const TS_RT_HIDDEN: TsRunType = 1i32;
pub const TS_RT_OPAQUE: TsRunType = 2i32;
pub const TS_RT_PLAIN: TsRunType = 0i32;
pub const TS_SD_BACKWARD: TsShiftDir = 0i32;
pub const TS_SD_EMBEDDEDHANDWRITINGVIEW_ENABLED: u32 = 128u32;
pub const TS_SD_EMBEDDEDHANDWRITINGVIEW_VISIBLE: u32 = 256u32;
pub const TS_SD_FORWARD: TsShiftDir = 1i32;
pub const TS_SD_INPUTPANEMANUALDISPLAYENABLE: u32 = 64u32;
pub const TS_SD_LOADING: u32 = 2u32;
pub const TS_SD_READONLY: u32 = 1u32;
pub const TS_SD_RESERVED: u32 = 4u32;
pub const TS_SD_TKBAUTOCORRECTENABLE: u32 = 8u32;
pub const TS_SD_TKBPREDICTIONENABLE: u32 = 16u32;
pub const TS_SD_UIINTEGRATIONENABLE: u32 = 32u32;
pub const TS_SHIFT_COUNT_HIDDEN: u32 = 1u32;
pub const TS_SHIFT_COUNT_ONLY: u32 = 8u32;
pub const TS_SHIFT_HALT_HIDDEN: u32 = 2u32;
pub const TS_SHIFT_HALT_VISIBLE: u32 = 4u32;
pub const TS_SS_DISJOINTSEL: u32 = 1u32;
pub const TS_SS_NOHIDDENTEXT: u32 = 8u32;
pub const TS_SS_REGIONS: u32 = 2u32;
pub const TS_SS_TKBAUTOCORRECTENABLE: u32 = 16u32;
pub const TS_SS_TKBPREDICTIONENABLE: u32 = 32u32;
pub const TS_SS_TRANSITORY: u32 = 4u32;
pub const TS_SS_UWPCONTROL: u32 = 64u32;
pub const TS_STRF_END: u32 = 2u32;
pub const TS_STRF_MID: u32 = 1u32;
pub const TS_STRF_START: u32 = 0u32;
pub const TS_ST_CORRECTION: TEXT_STORE_TEXT_CHANGE_FLAGS = 1u32;
pub const TS_ST_NONE: TEXT_STORE_TEXT_CHANGE_FLAGS = 0u32;
pub const TS_S_ASYNC: ::windows_sys::core::HRESULT = 262912i32;
pub const TS_TC_CORRECTION: TEXT_STORE_CHANGE_FLAGS = 1u32;
pub const TS_TC_NONE: TEXT_STORE_CHANGE_FLAGS = 0u32;
pub const TS_VCOOKIE_NUL: u32 = 4294967295u32;
pub type ANCHOR_CHANGE_HISTORY_FLAGS = u32;
pub type GET_TEXT_AND_PROPERTY_UPDATES_FLAGS = u32;
pub type INSERT_TEXT_AT_SELECTION_FLAGS = u32;
pub type InputScope = i32;
pub type LANG_BAR_ITEM_ICON_MODE_FLAGS = u32;
pub type TEXT_STORE_CHANGE_FLAGS = u32;
pub type TEXT_STORE_LOCK_FLAGS = u32;
pub type TEXT_STORE_TEXT_CHANGE_FLAGS = u32;
pub type TF_CONTEXT_EDIT_CONTEXT_FLAGS = u32;
pub type TF_DA_ATTR_INFO = i32;
pub type TF_DA_COLORTYPE = i32;
pub type TF_DA_LINESTYLE = i32;
pub type TKBLayoutType = i32;
pub type TfActiveSelEnd = i32;
pub type TfAnchor = i32;
pub type TfCandidateResult = i32;
pub type TfGravity = i32;
pub type TfIntegratableCandidateListSelectionStyle = i32;
pub type TfLBBalloonStyle = i32;
pub type TfLBIClick = i32;
pub type TfLayoutCode = i32;
pub type TfSapiObject = i32;
pub type TfShiftDir = i32;
pub type TsActiveSelEnd = i32;
pub type TsGravity = i32;
pub type TsLayoutCode = i32;
pub type TsRunType = i32;
pub type TsShiftDir = i32;
pub type HKL = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TF_DA_COLOR {
    pub r#type: TF_DA_COLORTYPE,
    pub Anonymous: TF_DA_COLOR_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TF_DA_COLOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TF_DA_COLOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union TF_DA_COLOR_0 {
    pub nIndex: i32,
    pub cr: super::super::Foundation::COLORREF,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TF_DA_COLOR_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TF_DA_COLOR_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TF_DISPLAYATTRIBUTE {
    pub crText: TF_DA_COLOR,
    pub crBk: TF_DA_COLOR,
    pub lsStyle: TF_DA_LINESTYLE,
    pub fBoldLine: super::super::Foundation::BOOL,
    pub crLine: TF_DA_COLOR,
    pub bAttr: TF_DA_ATTR_INFO,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TF_DISPLAYATTRIBUTE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TF_DISPLAYATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TF_HALTCOND {
    pub pHaltRange: ITfRange,
    pub aHaltPos: TfAnchor,
    pub dwFlags: u32,
}
impl ::core::marker::Copy for TF_HALTCOND {}
impl ::core::clone::Clone for TF_HALTCOND {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TF_INPUTPROCESSORPROFILE {
    pub dwProfileType: u32,
    pub langid: u16,
    pub clsid: ::windows_sys::core::GUID,
    pub guidProfile: ::windows_sys::core::GUID,
    pub catid: ::windows_sys::core::GUID,
    pub hklSubstitute: HKL,
    pub dwCaps: u32,
    pub hkl: HKL,
    pub dwFlags: u32,
}
impl ::core::marker::Copy for TF_INPUTPROCESSORPROFILE {}
impl ::core::clone::Clone for TF_INPUTPROCESSORPROFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TF_LANGBARITEMINFO {
    pub clsidService: ::windows_sys::core::GUID,
    pub guidItem: ::windows_sys::core::GUID,
    pub dwStyle: u32,
    pub ulSort: u32,
    pub szDescription: [u16; 32],
}
impl ::core::marker::Copy for TF_LANGBARITEMINFO {}
impl ::core::clone::Clone for TF_LANGBARITEMINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TF_LANGUAGEPROFILE {
    pub clsid: ::windows_sys::core::GUID,
    pub langid: u16,
    pub catid: ::windows_sys::core::GUID,
    pub fActive: super::super::Foundation::BOOL,
    pub guidProfile: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TF_LANGUAGEPROFILE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TF_LANGUAGEPROFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TF_LBBALLOONINFO {
    pub style: TfLBBalloonStyle,
    pub bstrText: ::windows_sys::core::BSTR,
}
impl ::core::marker::Copy for TF_LBBALLOONINFO {}
impl ::core::clone::Clone for TF_LBBALLOONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TF_LMLATTELEMENT {
    pub dwFrameStart: u32,
    pub dwFrameLen: u32,
    pub dwFlags: u32,
    pub Anonymous: TF_LMLATTELEMENT_0,
    pub bstrText: ::windows_sys::core::BSTR,
}
impl ::core::marker::Copy for TF_LMLATTELEMENT {}
impl ::core::clone::Clone for TF_LMLATTELEMENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union TF_LMLATTELEMENT_0 {
    pub iCost: i32,
}
impl ::core::marker::Copy for TF_LMLATTELEMENT_0 {}
impl ::core::clone::Clone for TF_LMLATTELEMENT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TF_PERSISTENT_PROPERTY_HEADER_ACP {
    pub guidType: ::windows_sys::core::GUID,
    pub ichStart: i32,
    pub cch: i32,
    pub cb: u32,
    pub dwPrivate: u32,
    pub clsidTIP: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for TF_PERSISTENT_PROPERTY_HEADER_ACP {}
impl ::core::clone::Clone for TF_PERSISTENT_PROPERTY_HEADER_ACP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TF_PRESERVEDKEY {
    pub uVKey: u32,
    pub uModifiers: u32,
}
impl ::core::marker::Copy for TF_PRESERVEDKEY {}
impl ::core::clone::Clone for TF_PRESERVEDKEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct TF_PROPERTYVAL {
    pub guidId: ::windows_sys::core::GUID,
    pub varValue: super::super::System::Variant::VARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for TF_PROPERTYVAL {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for TF_PROPERTYVAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TF_SELECTION {
    pub range: ITfRange,
    pub style: TF_SELECTIONSTYLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TF_SELECTION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TF_SELECTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TF_SELECTIONSTYLE {
    pub ase: TfActiveSelEnd,
    pub fInterimChar: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TF_SELECTIONSTYLE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TF_SELECTIONSTYLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct TS_ATTRVAL {
    pub idAttr: ::windows_sys::core::GUID,
    pub dwOverlapId: u32,
    pub varValue: super::super::System::Variant::VARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for TS_ATTRVAL {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for TS_ATTRVAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TS_RUNINFO {
    pub uCount: u32,
    pub r#type: TsRunType,
}
impl ::core::marker::Copy for TS_RUNINFO {}
impl ::core::clone::Clone for TS_RUNINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TS_SELECTIONSTYLE {
    pub ase: TsActiveSelEnd,
    pub fInterimChar: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TS_SELECTIONSTYLE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TS_SELECTIONSTYLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TS_SELECTION_ACP {
    pub acpStart: i32,
    pub acpEnd: i32,
    pub style: TS_SELECTIONSTYLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TS_SELECTION_ACP {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TS_SELECTION_ACP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TS_SELECTION_ANCHOR {
    pub paStart: IAnchor,
    pub paEnd: IAnchor,
    pub style: TS_SELECTIONSTYLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TS_SELECTION_ANCHOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TS_SELECTION_ANCHOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TS_STATUS {
    pub dwDynamicFlags: u32,
    pub dwStaticFlags: u32,
}
impl ::core::marker::Copy for TS_STATUS {}
impl ::core::clone::Clone for TS_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TS_TEXTCHANGE {
    pub acpStart: i32,
    pub acpOldEnd: i32,
    pub acpNewEnd: i32,
}
impl ::core::marker::Copy for TS_TEXTCHANGE {}
impl ::core::clone::Clone for TS_TEXTCHANGE {
    fn clone(&self) -> Self {
        *self
    }
}
