::windows_targets::link!("wmvcore.dll" "system" fn WMCreateBackupRestorer(pcallback : ::windows_sys::core::IUnknown, ppbackup : *mut IWMLicenseBackup) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateEditor(ppeditor : *mut IWMMetadataEditor) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateIndexer(ppindexer : *mut IWMIndexer) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateProfileManager(ppprofilemanager : *mut IWMProfileManager) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateReader(punkcert : ::windows_sys::core::IUnknown, dwrights : u32, ppreader : *mut IWMReader) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateSyncReader(punkcert : ::windows_sys::core::IUnknown, dwrights : u32, ppsyncreader : *mut IWMSyncReader) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateWriter(punkcert : ::windows_sys::core::IUnknown, ppwriter : *mut IWMWriter) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateWriterFileSink(ppsink : *mut IWMWriterFileSink) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateWriterNetworkSink(ppsink : *mut IWMWriterNetworkSink) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wmvcore.dll" "system" fn WMCreateWriterPushSink(ppsink : *mut IWMWriterPushSink) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wmvcore.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WMIsContentProtected(pwszfilename : ::windows_sys::core::PCWSTR, pfisprotected : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
pub type INSNetSourceCreator = *mut ::core::ffi::c_void;
pub type INSSBuffer = *mut ::core::ffi::c_void;
pub type INSSBuffer2 = *mut ::core::ffi::c_void;
pub type INSSBuffer3 = *mut ::core::ffi::c_void;
pub type INSSBuffer4 = *mut ::core::ffi::c_void;
pub type IWMAddressAccess = *mut ::core::ffi::c_void;
pub type IWMAddressAccess2 = *mut ::core::ffi::c_void;
pub type IWMAuthorizer = *mut ::core::ffi::c_void;
pub type IWMBackupRestoreProps = *mut ::core::ffi::c_void;
pub type IWMBandwidthSharing = *mut ::core::ffi::c_void;
pub type IWMClientConnections = *mut ::core::ffi::c_void;
pub type IWMClientConnections2 = *mut ::core::ffi::c_void;
pub type IWMCodecInfo = *mut ::core::ffi::c_void;
pub type IWMCodecInfo2 = *mut ::core::ffi::c_void;
pub type IWMCodecInfo3 = *mut ::core::ffi::c_void;
pub type IWMCredentialCallback = *mut ::core::ffi::c_void;
pub type IWMDRMEditor = *mut ::core::ffi::c_void;
pub type IWMDRMMessageParser = *mut ::core::ffi::c_void;
pub type IWMDRMReader = *mut ::core::ffi::c_void;
pub type IWMDRMReader2 = *mut ::core::ffi::c_void;
pub type IWMDRMReader3 = *mut ::core::ffi::c_void;
pub type IWMDRMTranscryptionManager = *mut ::core::ffi::c_void;
pub type IWMDRMTranscryptor = *mut ::core::ffi::c_void;
pub type IWMDRMTranscryptor2 = *mut ::core::ffi::c_void;
pub type IWMDRMWriter = *mut ::core::ffi::c_void;
pub type IWMDRMWriter2 = *mut ::core::ffi::c_void;
pub type IWMDRMWriter3 = *mut ::core::ffi::c_void;
pub type IWMDeviceRegistration = *mut ::core::ffi::c_void;
pub type IWMGetSecureChannel = *mut ::core::ffi::c_void;
pub type IWMHeaderInfo = *mut ::core::ffi::c_void;
pub type IWMHeaderInfo2 = *mut ::core::ffi::c_void;
pub type IWMHeaderInfo3 = *mut ::core::ffi::c_void;
pub type IWMIStreamProps = *mut ::core::ffi::c_void;
pub type IWMImageInfo = *mut ::core::ffi::c_void;
pub type IWMIndexer = *mut ::core::ffi::c_void;
pub type IWMIndexer2 = *mut ::core::ffi::c_void;
pub type IWMInputMediaProps = *mut ::core::ffi::c_void;
pub type IWMLanguageList = *mut ::core::ffi::c_void;
pub type IWMLicenseBackup = *mut ::core::ffi::c_void;
pub type IWMLicenseRestore = *mut ::core::ffi::c_void;
pub type IWMLicenseRevocationAgent = *mut ::core::ffi::c_void;
pub type IWMMediaProps = *mut ::core::ffi::c_void;
pub type IWMMetadataEditor = *mut ::core::ffi::c_void;
pub type IWMMetadataEditor2 = *mut ::core::ffi::c_void;
pub type IWMMutualExclusion = *mut ::core::ffi::c_void;
pub type IWMMutualExclusion2 = *mut ::core::ffi::c_void;
pub type IWMOutputMediaProps = *mut ::core::ffi::c_void;
pub type IWMPacketSize = *mut ::core::ffi::c_void;
pub type IWMPacketSize2 = *mut ::core::ffi::c_void;
pub type IWMPlayerHook = *mut ::core::ffi::c_void;
pub type IWMPlayerTimestampHook = *mut ::core::ffi::c_void;
pub type IWMProfile = *mut ::core::ffi::c_void;
pub type IWMProfile2 = *mut ::core::ffi::c_void;
pub type IWMProfile3 = *mut ::core::ffi::c_void;
pub type IWMProfileManager = *mut ::core::ffi::c_void;
pub type IWMProfileManager2 = *mut ::core::ffi::c_void;
pub type IWMProfileManagerLanguage = *mut ::core::ffi::c_void;
pub type IWMPropertyVault = *mut ::core::ffi::c_void;
pub type IWMProximityDetection = *mut ::core::ffi::c_void;
pub type IWMReader = *mut ::core::ffi::c_void;
pub type IWMReaderAccelerator = *mut ::core::ffi::c_void;
pub type IWMReaderAdvanced = *mut ::core::ffi::c_void;
pub type IWMReaderAdvanced2 = *mut ::core::ffi::c_void;
pub type IWMReaderAdvanced3 = *mut ::core::ffi::c_void;
pub type IWMReaderAdvanced4 = *mut ::core::ffi::c_void;
pub type IWMReaderAdvanced5 = *mut ::core::ffi::c_void;
pub type IWMReaderAdvanced6 = *mut ::core::ffi::c_void;
pub type IWMReaderAllocatorEx = *mut ::core::ffi::c_void;
pub type IWMReaderCallback = *mut ::core::ffi::c_void;
pub type IWMReaderCallbackAdvanced = *mut ::core::ffi::c_void;
pub type IWMReaderNetworkConfig = *mut ::core::ffi::c_void;
pub type IWMReaderNetworkConfig2 = *mut ::core::ffi::c_void;
pub type IWMReaderPlaylistBurn = *mut ::core::ffi::c_void;
pub type IWMReaderStreamClock = *mut ::core::ffi::c_void;
pub type IWMReaderTimecode = *mut ::core::ffi::c_void;
pub type IWMReaderTypeNegotiation = *mut ::core::ffi::c_void;
pub type IWMRegisterCallback = *mut ::core::ffi::c_void;
pub type IWMRegisteredDevice = *mut ::core::ffi::c_void;
pub type IWMSBufferAllocator = *mut ::core::ffi::c_void;
pub type IWMSInternalAdminNetSource = *mut ::core::ffi::c_void;
pub type IWMSInternalAdminNetSource2 = *mut ::core::ffi::c_void;
pub type IWMSInternalAdminNetSource3 = *mut ::core::ffi::c_void;
pub type IWMSecureChannel = *mut ::core::ffi::c_void;
pub type IWMStatusCallback = *mut ::core::ffi::c_void;
pub type IWMStreamConfig = *mut ::core::ffi::c_void;
pub type IWMStreamConfig2 = *mut ::core::ffi::c_void;
pub type IWMStreamConfig3 = *mut ::core::ffi::c_void;
pub type IWMStreamList = *mut ::core::ffi::c_void;
pub type IWMStreamPrioritization = *mut ::core::ffi::c_void;
pub type IWMSyncReader = *mut ::core::ffi::c_void;
pub type IWMSyncReader2 = *mut ::core::ffi::c_void;
pub type IWMVideoMediaProps = *mut ::core::ffi::c_void;
pub type IWMWatermarkInfo = *mut ::core::ffi::c_void;
pub type IWMWriter = *mut ::core::ffi::c_void;
pub type IWMWriterAdvanced = *mut ::core::ffi::c_void;
pub type IWMWriterAdvanced2 = *mut ::core::ffi::c_void;
pub type IWMWriterAdvanced3 = *mut ::core::ffi::c_void;
pub type IWMWriterFileSink = *mut ::core::ffi::c_void;
pub type IWMWriterFileSink2 = *mut ::core::ffi::c_void;
pub type IWMWriterFileSink3 = *mut ::core::ffi::c_void;
pub type IWMWriterNetworkSink = *mut ::core::ffi::c_void;
pub type IWMWriterPostView = *mut ::core::ffi::c_void;
pub type IWMWriterPostViewCallback = *mut ::core::ffi::c_void;
pub type IWMWriterPreprocess = *mut ::core::ffi::c_void;
pub type IWMWriterPushSink = *mut ::core::ffi::c_void;
pub type IWMWriterSink = *mut ::core::ffi::c_void;
pub const AM_CONFIGASFWRITER_PARAM_AUTOINDEX: _AM_ASFWRITERCONFIG_PARAM = 1i32;
pub const AM_CONFIGASFWRITER_PARAM_DONTCOMPRESS: _AM_ASFWRITERCONFIG_PARAM = 3i32;
pub const AM_CONFIGASFWRITER_PARAM_MULTIPASS: _AM_ASFWRITERCONFIG_PARAM = 2i32;
pub const CLSID_ClientNetManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcd12a3ce_9c42_11d2_beed_0060082f2054);
pub const CLSID_WMBandwidthSharing_Exclusive: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaf6060aa_5197_11d2_b6af_00c04fd908e9);
pub const CLSID_WMBandwidthSharing_Partial: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaf6060ab_5197_11d2_b6af_00c04fd908e9);
pub const CLSID_WMMUTEX_Bitrate: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd6e22a01_35da_11d1_9034_00a0c90349be);
pub const CLSID_WMMUTEX_Language: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd6e22a00_35da_11d1_9034_00a0c90349be);
pub const CLSID_WMMUTEX_Presentation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd6e22a02_35da_11d1_9034_00a0c90349be);
pub const CLSID_WMMUTEX_Unknown: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd6e22a03_35da_11d1_9034_00a0c90349be);
pub const DRM_OPL_TYPES: u32 = 1u32;
pub const NETSOURCE_URLCREDPOLICY_SETTING_ANONYMOUSONLY: NETSOURCE_URLCREDPOLICY_SETTINGS = 2i32;
pub const NETSOURCE_URLCREDPOLICY_SETTING_MUSTPROMPTUSER: NETSOURCE_URLCREDPOLICY_SETTINGS = 1i32;
pub const NETSOURCE_URLCREDPOLICY_SETTING_SILENTLOGONOK: NETSOURCE_URLCREDPOLICY_SETTINGS = 0i32;
pub const WEBSTREAM_SAMPLE_TYPE_FILE: WEBSTREAM_SAMPLE_TYPE = 1i32;
pub const WEBSTREAM_SAMPLE_TYPE_RENDER: WEBSTREAM_SAMPLE_TYPE = 2i32;
pub const WMDRM_IMPORT_INIT_STRUCT_DEFINED: u32 = 1u32;
pub const WMFORMAT_MPEG2Video: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe06d80e3_db46_11cf_b4d1_00805f6cbbea);
pub const WMFORMAT_Script: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c8510f2_debe_4ca7_bba5_f07a104f8dff);
pub const WMFORMAT_VideoInfo: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x05589f80_c356_11ce_bf01_00aa0055595a);
pub const WMFORMAT_WaveFormatEx: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x05589f81_c356_11ce_bf01_00aa0055595a);
pub const WMFORMAT_WebStream: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xda1e6b13_8359_4050_b398_388e965bf00c);
pub const WMMEDIASUBTYPE_ACELPnet: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000130_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_Base: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000000_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_DRM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000009_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_I420: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x30323449_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_IYUV: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x56555949_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_M4S2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3253344d_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_MP3: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000055_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_MP43: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3334504d_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_MP4S: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5334504d_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_MPEG2_VIDEO: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe06d8026_db46_11cf_b4d1_00805f6cbbea);
pub const WMMEDIASUBTYPE_MSS1: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3153534d_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_MSS2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3253534d_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_P422: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x32323450_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_PCM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_RGB1: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe436eb78_524f_11ce_9f53_0020af0ba770);
pub const WMMEDIASUBTYPE_RGB24: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe436eb7d_524f_11ce_9f53_0020af0ba770);
pub const WMMEDIASUBTYPE_RGB32: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe436eb7e_524f_11ce_9f53_0020af0ba770);
pub const WMMEDIASUBTYPE_RGB4: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe436eb79_524f_11ce_9f53_0020af0ba770);
pub const WMMEDIASUBTYPE_RGB555: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe436eb7c_524f_11ce_9f53_0020af0ba770);
pub const WMMEDIASUBTYPE_RGB565: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe436eb7b_524f_11ce_9f53_0020af0ba770);
pub const WMMEDIASUBTYPE_RGB8: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe436eb7a_524f_11ce_9f53_0020af0ba770);
pub const WMMEDIASUBTYPE_UYVY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x59565955_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_VIDEOIMAGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1d4a45f2_e5f6_4b44_8388_f0ae5c0e0c37);
pub const WMMEDIASUBTYPE_WMAudioV2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000161_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMAudioV7: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000161_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMAudioV8: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000161_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMAudioV9: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000162_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMAudio_Lossless: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000163_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMSP1: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0000000a_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMSP2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0000000b_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMV1: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x31564d57_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMV2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x32564d57_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMV3: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x33564d57_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMVA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x41564d57_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WMVP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50564d57_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WVC1: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x31435657_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WVP2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x32505657_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_WebStream: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x776257d4_c627_41cb_8f81_7ac7ff1c40cc);
pub const WMMEDIASUBTYPE_YUY2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x32595559_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_YV12: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x32315659_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_YVU9: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x39555659_0000_0010_8000_00aa00389b71);
pub const WMMEDIASUBTYPE_YVYU: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x55595659_0000_0010_8000_00aa00389b71);
pub const WMMEDIATYPE_Audio: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x73647561_0000_0010_8000_00aa00389b71);
pub const WMMEDIATYPE_FileTransfer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd9e47579_930e_4427_adfc_ad80f290e470);
pub const WMMEDIATYPE_Image: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x34a50fd8_8aa5_4386_81fe_a0efe0488e31);
pub const WMMEDIATYPE_Script: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x73636d64_0000_0010_8000_00aa00389b71);
pub const WMMEDIATYPE_Text: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9bba1ea7_5ab2_4829_ba57_0940209bcf3e);
pub const WMMEDIATYPE_Video: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x73646976_0000_0010_8000_00aa00389b71);
pub const WMSCRIPTTYPE_TwoStrings: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x82f38a70_c29f_11d1_97ad_00a0c95ea850);
pub const WMT_ACQUIRE_LICENSE: WMT_STATUS = 23i32;
pub const WMT_BACKUPRESTORE_BEGIN: WMT_STATUS = 21i32;
pub const WMT_BACKUPRESTORE_CONNECTING: WMT_STATUS = 28i32;
pub const WMT_BACKUPRESTORE_DISCONNECTING: WMT_STATUS = 29i32;
pub const WMT_BACKUPRESTORE_END: WMT_STATUS = 27i32;
pub const WMT_BUFFERING_START: WMT_STATUS = 2i32;
pub const WMT_BUFFERING_STOP: WMT_STATUS = 3i32;
pub const WMT_CLEANPOINT_ONLY: WMT_STREAM_SELECTION = 1i32;
pub const WMT_CLIENT_CONNECT: WMT_STATUS = 32i32;
pub const WMT_CLIENT_CONNECT_EX: WMT_STATUS = 37i32;
pub const WMT_CLIENT_DISCONNECT: WMT_STATUS = 33i32;
pub const WMT_CLIENT_DISCONNECT_EX: WMT_STATUS = 38i32;
pub const WMT_CLIENT_PROPERTIES: WMT_STATUS = 42i32;
pub const WMT_CLOSED: WMT_STATUS = 13i32;
pub const WMT_CODECINFO_AUDIO: WMT_CODEC_INFO_TYPE = 0i32;
pub const WMT_CODECINFO_UNKNOWN: WMT_CODEC_INFO_TYPE = -1i32;
pub const WMT_CODECINFO_VIDEO: WMT_CODEC_INFO_TYPE = 1i32;
pub const WMT_CONNECTING: WMT_STATUS = 8i32;
pub const WMT_CONTENT_ENABLER: WMT_STATUS = 51i32;
pub const WMT_CREDENTIAL_CLEAR_TEXT: WMT_CREDENTIAL_FLAGS = 4i32;
pub const WMT_CREDENTIAL_DONT_CACHE: WMT_CREDENTIAL_FLAGS = 2i32;
pub const WMT_CREDENTIAL_ENCRYPT: WMT_CREDENTIAL_FLAGS = 16i32;
pub const WMT_CREDENTIAL_PROXY: WMT_CREDENTIAL_FLAGS = 8i32;
pub const WMT_CREDENTIAL_SAVE: WMT_CREDENTIAL_FLAGS = 1i32;
pub const WMT_DMOCATEGORY_AUDIO_WATERMARK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x65221c5a_fa75_4b39_b50c_06c336b6a3ef);
pub const WMT_DMOCATEGORY_VIDEO_WATERMARK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x187cc922_8efc_4404_9daf_63f4830df1bc);
pub const WMT_DRMLA_TAMPERED: WMT_DRMLA_TRUST = 2i32;
pub const WMT_DRMLA_TRUSTED: WMT_DRMLA_TRUST = 1i32;
pub const WMT_DRMLA_UNTRUSTED: WMT_DRMLA_TRUST = 0i32;
pub const WMT_END_OF_FILE: WMT_STATUS = 4i32;
pub const WMT_END_OF_SEGMENT: WMT_STATUS = 5i32;
pub const WMT_END_OF_STREAMING: WMT_STATUS = 6i32;
pub const WMT_EOF: WMT_STATUS = 4i32;
pub const WMT_ERROR: WMT_STATUS = 0i32;
pub const WMT_ERROR_WITHURL: WMT_STATUS = 30i32;
pub const WMT_FM_FILESINK_DATA_UNITS: WMT_FILESINK_MODE = 2i32;
pub const WMT_FM_FILESINK_UNBUFFERED: WMT_FILESINK_MODE = 4i32;
pub const WMT_FM_SINGLE_BUFFERS: WMT_FILESINK_MODE = 1i32;
pub const WMT_IMAGETYPE_BITMAP: WMT_ATTR_IMAGETYPE = 1i32;
pub const WMT_IMAGETYPE_GIF: WMT_ATTR_IMAGETYPE = 3i32;
pub const WMT_IMAGETYPE_JPEG: WMT_ATTR_IMAGETYPE = 2i32;
pub const WMT_INDEX_PROGRESS: WMT_STATUS = 16i32;
pub const WMT_INDIVIDUALIZE: WMT_STATUS = 24i32;
pub const WMT_INIT_PLAYLIST_BURN: WMT_STATUS = 44i32;
pub const WMT_IT_BITMAP: WMT_IMAGE_TYPE = 1i32;
pub const WMT_IT_FRAME_NUMBERS: WMT_INDEXER_TYPE = 1i32;
pub const WMT_IT_GIF: WMT_IMAGE_TYPE = 3i32;
pub const WMT_IT_JPEG: WMT_IMAGE_TYPE = 2i32;
pub const WMT_IT_NEAREST_CLEAN_POINT: WMT_INDEX_TYPE = 3i32;
pub const WMT_IT_NEAREST_DATA_UNIT: WMT_INDEX_TYPE = 1i32;
pub const WMT_IT_NEAREST_OBJECT: WMT_INDEX_TYPE = 2i32;
pub const WMT_IT_NONE: WMT_IMAGE_TYPE = 0i32;
pub const WMT_IT_PRESENTATION_TIME: WMT_INDEXER_TYPE = 0i32;
pub const WMT_IT_TIMECODE: WMT_INDEXER_TYPE = 2i32;
pub const WMT_LICENSEURL_SIGNATURE_STATE: WMT_STATUS = 43i32;
pub const WMT_LOCATING: WMT_STATUS = 7i32;
pub const WMT_MISSING_CODEC: WMT_STATUS = 10i32;
pub const WMT_MS_CLASS_MIXED: WMT_MUSICSPEECH_CLASS_MODE = 2i32;
pub const WMT_MS_CLASS_MUSIC: WMT_MUSICSPEECH_CLASS_MODE = 0i32;
pub const WMT_MS_CLASS_SPEECH: WMT_MUSICSPEECH_CLASS_MODE = 1i32;
pub const WMT_NATIVE_OUTPUT_PROPS_CHANGED: WMT_STATUS = 34i32;
pub const WMT_NEEDS_INDIVIDUALIZATION: WMT_STATUS = 25i32;
pub const WMT_NEW_METADATA: WMT_STATUS = 20i32;
pub const WMT_NEW_SOURCEFLAGS: WMT_STATUS = 19i32;
pub const WMT_NO_RIGHTS: WMT_STATUS = 9i32;
pub const WMT_NO_RIGHTS_EX: WMT_STATUS = 26i32;
pub const WMT_OFF: WMT_STREAM_SELECTION = 0i32;
pub const WMT_OFFSET_FORMAT_100NS: WMT_OFFSET_FORMAT = 0i32;
pub const WMT_OFFSET_FORMAT_100NS_APPROXIMATE: WMT_OFFSET_FORMAT = 4i32;
pub const WMT_OFFSET_FORMAT_FRAME_NUMBERS: WMT_OFFSET_FORMAT = 1i32;
pub const WMT_OFFSET_FORMAT_PLAYLIST_OFFSET: WMT_OFFSET_FORMAT = 2i32;
pub const WMT_OFFSET_FORMAT_TIMECODE: WMT_OFFSET_FORMAT = 3i32;
pub const WMT_ON: WMT_STREAM_SELECTION = 2i32;
pub const WMT_OPENED: WMT_STATUS = 1i32;
pub const WMT_PLAY_MODE_AUTOSELECT: WMT_PLAY_MODE = 0i32;
pub const WMT_PLAY_MODE_DOWNLOAD: WMT_PLAY_MODE = 2i32;
pub const WMT_PLAY_MODE_LOCAL: WMT_PLAY_MODE = 1i32;
pub const WMT_PLAY_MODE_STREAMING: WMT_PLAY_MODE = 3i32;
pub const WMT_PREROLL_COMPLETE: WMT_STATUS = 41i32;
pub const WMT_PREROLL_READY: WMT_STATUS = 40i32;
pub const WMT_PROTOCOL_HTTP: WMT_NET_PROTOCOL = 0i32;
pub const WMT_PROXIMITY_COMPLETED: WMT_STATUS = 50i32;
pub const WMT_PROXIMITY_RESULT: WMT_STATUS = 49i32;
pub const WMT_PROXY_SETTING_AUTO: WMT_PROXY_SETTINGS = 2i32;
pub const WMT_PROXY_SETTING_BROWSER: WMT_PROXY_SETTINGS = 3i32;
pub const WMT_PROXY_SETTING_MANUAL: WMT_PROXY_SETTINGS = 1i32;
pub const WMT_PROXY_SETTING_MAX: WMT_PROXY_SETTINGS = 4i32;
pub const WMT_PROXY_SETTING_NONE: WMT_PROXY_SETTINGS = 0i32;
pub const WMT_RECONNECT_END: WMT_STATUS = 36i32;
pub const WMT_RECONNECT_START: WMT_STATUS = 35i32;
pub const WMT_RESTRICTED_LICENSE: WMT_STATUS = 31i32;
pub const WMT_RIGHT_COLLABORATIVE_PLAY: WMT_RIGHTS = 256i32;
pub const WMT_RIGHT_COPY: WMT_RIGHTS = 128i32;
pub const WMT_RIGHT_COPY_TO_CD: WMT_RIGHTS = 8i32;
pub const WMT_RIGHT_COPY_TO_NON_SDMI_DEVICE: WMT_RIGHTS = 2i32;
pub const WMT_RIGHT_COPY_TO_SDMI_DEVICE: WMT_RIGHTS = 16i32;
pub const WMT_RIGHT_ONE_TIME: WMT_RIGHTS = 32i32;
pub const WMT_RIGHT_PLAYBACK: WMT_RIGHTS = 1i32;
pub const WMT_RIGHT_SAVE_STREAM_PROTECTED: WMT_RIGHTS = 64i32;
pub const WMT_RIGHT_SDMI_NOMORECOPIES: WMT_RIGHTS = 131072i32;
pub const WMT_RIGHT_SDMI_TRIGGER: WMT_RIGHTS = 65536i32;
pub const WMT_SAVEAS_START: WMT_STATUS = 17i32;
pub const WMT_SAVEAS_STOP: WMT_STATUS = 18i32;
pub const WMT_SET_FEC_SPAN: WMT_STATUS = 39i32;
pub const WMT_SOURCE_SWITCH: WMT_STATUS = 22i32;
pub const WMT_STARTED: WMT_STATUS = 11i32;
pub const WMT_STOPPED: WMT_STATUS = 12i32;
pub const WMT_STRIDING: WMT_STATUS = 14i32;
pub const WMT_Storage_Format_MP3: WMT_STORAGE_FORMAT = 0i32;
pub const WMT_Storage_Format_V1: WMT_STORAGE_FORMAT = 1i32;
pub const WMT_TIMECODE_FRAMERATE_24: WMT_TIMECODE_FRAMERATE = 3i32;
pub const WMT_TIMECODE_FRAMERATE_25: WMT_TIMECODE_FRAMERATE = 2i32;
pub const WMT_TIMECODE_FRAMERATE_30: WMT_TIMECODE_FRAMERATE = 0i32;
pub const WMT_TIMECODE_FRAMERATE_30DROP: WMT_TIMECODE_FRAMERATE = 1i32;
pub const WMT_TIMER: WMT_STATUS = 15i32;
pub const WMT_TRANSCRYPTOR_CLOSED: WMT_STATUS = 48i32;
pub const WMT_TRANSCRYPTOR_INIT: WMT_STATUS = 45i32;
pub const WMT_TRANSCRYPTOR_READ: WMT_STATUS = 47i32;
pub const WMT_TRANSCRYPTOR_SEEKED: WMT_STATUS = 46i32;
pub const WMT_TYPE_BINARY: WMT_ATTR_DATATYPE = 2i32;
pub const WMT_TYPE_BOOL: WMT_ATTR_DATATYPE = 3i32;
pub const WMT_TYPE_DWORD: WMT_ATTR_DATATYPE = 0i32;
pub const WMT_TYPE_GUID: WMT_ATTR_DATATYPE = 6i32;
pub const WMT_TYPE_QWORD: WMT_ATTR_DATATYPE = 4i32;
pub const WMT_TYPE_STRING: WMT_ATTR_DATATYPE = 1i32;
pub const WMT_TYPE_WORD: WMT_ATTR_DATATYPE = 5i32;
pub const WMT_Transport_Type_Reliable: WMT_TRANSPORT_TYPE = 1i32;
pub const WMT_Transport_Type_Unreliable: WMT_TRANSPORT_TYPE = 0i32;
pub const WMT_VER_4_0: WMT_VERSION = 262144i32;
pub const WMT_VER_7_0: WMT_VERSION = 458752i32;
pub const WMT_VER_8_0: WMT_VERSION = 524288i32;
pub const WMT_VER_9_0: WMT_VERSION = 589824i32;
pub const WMT_VIDEOIMAGE_INTEGER_DENOMINATOR: i32 = 65536i32;
pub const WMT_VIDEOIMAGE_MAGIC_NUMBER: u32 = 491406834u32;
pub const WMT_VIDEOIMAGE_MAGIC_NUMBER_2: u32 = 491406835u32;
pub const WMT_VIDEOIMAGE_SAMPLE_ADV_BLENDING: u32 = 8u32;
pub const WMT_VIDEOIMAGE_SAMPLE_BLENDING: u32 = 4u32;
pub const WMT_VIDEOIMAGE_SAMPLE_INPUT_FRAME: u32 = 1u32;
pub const WMT_VIDEOIMAGE_SAMPLE_MOTION: u32 = 1u32;
pub const WMT_VIDEOIMAGE_SAMPLE_OUTPUT_FRAME: u32 = 2u32;
pub const WMT_VIDEOIMAGE_SAMPLE_ROTATION: u32 = 2u32;
pub const WMT_VIDEOIMAGE_SAMPLE_USES_CURRENT_INPUT_FRAME: u32 = 4u32;
pub const WMT_VIDEOIMAGE_SAMPLE_USES_PREVIOUS_INPUT_FRAME: u32 = 8u32;
pub const WMT_VIDEOIMAGE_TRANSITION_BOW_TIE: u32 = 11u32;
pub const WMT_VIDEOIMAGE_TRANSITION_CIRCLE: u32 = 12u32;
pub const WMT_VIDEOIMAGE_TRANSITION_CROSS_FADE: u32 = 13u32;
pub const WMT_VIDEOIMAGE_TRANSITION_DIAGONAL: u32 = 14u32;
pub const WMT_VIDEOIMAGE_TRANSITION_DIAMOND: u32 = 15u32;
pub const WMT_VIDEOIMAGE_TRANSITION_FADE_TO_COLOR: u32 = 16u32;
pub const WMT_VIDEOIMAGE_TRANSITION_FILLED_V: u32 = 17u32;
pub const WMT_VIDEOIMAGE_TRANSITION_FLIP: u32 = 18u32;
pub const WMT_VIDEOIMAGE_TRANSITION_INSET: u32 = 19u32;
pub const WMT_VIDEOIMAGE_TRANSITION_IRIS: u32 = 20u32;
pub const WMT_VIDEOIMAGE_TRANSITION_PAGE_ROLL: u32 = 21u32;
pub const WMT_VIDEOIMAGE_TRANSITION_RECTANGLE: u32 = 23u32;
pub const WMT_VIDEOIMAGE_TRANSITION_REVEAL: u32 = 24u32;
pub const WMT_VIDEOIMAGE_TRANSITION_SLIDE: u32 = 27u32;
pub const WMT_VIDEOIMAGE_TRANSITION_SPLIT: u32 = 29u32;
pub const WMT_VIDEOIMAGE_TRANSITION_STAR: u32 = 30u32;
pub const WMT_VIDEOIMAGE_TRANSITION_WHEEL: u32 = 31u32;
pub const WMT_WMETYPE_AUDIO: WMT_WATERMARK_ENTRY_TYPE = 1i32;
pub const WMT_WMETYPE_VIDEO: WMT_WATERMARK_ENTRY_TYPE = 2i32;
pub const WM_AETYPE_EXCLUDE: WM_AETYPE = 101i32;
pub const WM_AETYPE_INCLUDE: WM_AETYPE = 105i32;
pub const WM_CL_INTERLACED420: u32 = 0u32;
pub const WM_CL_PROGRESSIVE420: u32 = 1u32;
pub const WM_CT_BOTTOM_FIELD_FIRST: u32 = 32u32;
pub const WM_CT_INTERLACED: u32 = 128u32;
pub const WM_CT_REPEAT_FIRST_FIELD: u32 = 16u32;
pub const WM_CT_TOP_FIELD_FIRST: u32 = 64u32;
pub const WM_DM_DEINTERLACE_HALFSIZE: WM_DM_INTERLACED_TYPE = 2i32;
pub const WM_DM_DEINTERLACE_HALFSIZEDOUBLERATE: WM_DM_INTERLACED_TYPE = 3i32;
pub const WM_DM_DEINTERLACE_INVERSETELECINE: WM_DM_INTERLACED_TYPE = 4i32;
pub const WM_DM_DEINTERLACE_NORMAL: WM_DM_INTERLACED_TYPE = 1i32;
pub const WM_DM_DEINTERLACE_VERTICALHALFSIZEDOUBLERATE: WM_DM_INTERLACED_TYPE = 5i32;
pub const WM_DM_IT_DISABLE_COHERENT_MODE: WM_DM_IT_FIRST_FRAME_COHERENCY = 0i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_AA_BOTTOM: WM_DM_IT_FIRST_FRAME_COHERENCY = 6i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_AA_TOP: WM_DM_IT_FIRST_FRAME_COHERENCY = 1i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_BB_BOTTOM: WM_DM_IT_FIRST_FRAME_COHERENCY = 7i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_BB_TOP: WM_DM_IT_FIRST_FRAME_COHERENCY = 2i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_BC_BOTTOM: WM_DM_IT_FIRST_FRAME_COHERENCY = 8i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_BC_TOP: WM_DM_IT_FIRST_FRAME_COHERENCY = 3i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_CD_BOTTOM: WM_DM_IT_FIRST_FRAME_COHERENCY = 9i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_CD_TOP: WM_DM_IT_FIRST_FRAME_COHERENCY = 4i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_DD_BOTTOM: WM_DM_IT_FIRST_FRAME_COHERENCY = 10i32;
pub const WM_DM_IT_FIRST_FRAME_IN_CLIP_IS_DD_TOP: WM_DM_IT_FIRST_FRAME_COHERENCY = 5i32;
pub const WM_DM_NOTINTERLACED: WM_DM_INTERLACED_TYPE = 0i32;
pub const WM_MAX_STREAMS: u32 = 63u32;
pub const WM_MAX_VIDEO_STREAMS: u32 = 63u32;
pub const WM_PLAYBACK_DRC_HIGH: WM_PLAYBACK_DRC_LEVEL = 0i32;
pub const WM_PLAYBACK_DRC_LOW: WM_PLAYBACK_DRC_LEVEL = 2i32;
pub const WM_PLAYBACK_DRC_MEDIUM: WM_PLAYBACK_DRC_LEVEL = 1i32;
pub const WM_SFEX_DATALOSS: WM_SFEX_TYPE = 4i32;
pub const WM_SFEX_NOTASYNCPOINT: WM_SFEX_TYPE = 2i32;
pub const WM_SF_CLEANPOINT: WM_SF_TYPE = 1i32;
pub const WM_SF_DATALOSS: WM_SF_TYPE = 4i32;
pub const WM_SF_DISCONTINUITY: WM_SF_TYPE = 2i32;
pub const WM_SampleExtensionGUID_ChromaLocation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4c5acca0_9276_4b2c_9e4c_a0edefdd217e);
pub const WM_SampleExtensionGUID_ColorSpaceInfo: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf79ada56_30eb_4f2b_9f7a_f24b139a1157);
pub const WM_SampleExtensionGUID_ContentType: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd590dc20_07bc_436c_9cf7_f3bbfbf1a4dc);
pub const WM_SampleExtensionGUID_FileName: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe165ec0e_19ed_45d7_b4a7_25cbd1e28e9b);
pub const WM_SampleExtensionGUID_OutputCleanPoint: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf72a3c6f_6eb4_4ebc_b192_09ad9759e828);
pub const WM_SampleExtensionGUID_PixelAspectRatio: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1b1ee554_f9ea_4bc8_821a_376b74e4c4b8);
pub const WM_SampleExtensionGUID_SampleDuration: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc6bd9450_867f_4907_83a3_c77921b733ad);
pub const WM_SampleExtensionGUID_SampleProtectionSalt: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5403deee_b9ee_438f_aa83_3804997e569d);
pub const WM_SampleExtensionGUID_Timecode: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x399595ec_8667_4e2d_8fdb_98814ce76c1e);
pub const WM_SampleExtensionGUID_UserDataInfo: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x732bb4fa_78be_4549_99bd_02db1a55b7a8);
pub const WM_SampleExtension_ChromaLocation_Size: u32 = 1u32;
pub const WM_SampleExtension_ColorSpaceInfo_Size: u32 = 3u32;
pub const WM_SampleExtension_ContentType_Size: u32 = 1u32;
pub const WM_SampleExtension_PixelAspectRatio_Size: u32 = 2u32;
pub const WM_SampleExtension_SampleDuration_Size: u32 = 2u32;
pub const WM_SampleExtension_Timecode_Size: u32 = 14u32;
pub const g_dwWMContentAttributes: u32 = 5u32;
pub const g_dwWMNSCAttributes: u32 = 5u32;
pub const g_dwWMSpecialAttributes: u32 = 20u32;
pub const g_wszASFLeakyBucketPairs: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ASFLeakyBucketPairs");
pub const g_wszAllowInterlacedOutput: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AllowInterlacedOutput");
pub const g_wszAverageLevel: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AverageLevel");
pub const g_wszBufferAverage: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Buffer Average");
pub const g_wszComplexity: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_COMPLEXITYEX");
pub const g_wszComplexityLive: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_COMPLEXITYEXLIVE");
pub const g_wszComplexityMax: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_COMPLEXITYEXMAX");
pub const g_wszComplexityOffline: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_COMPLEXITYEXOFFLINE");
pub const g_wszDecoderComplexityRequested: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_DECODERCOMPLEXITYPROFILE");
pub const g_wszDedicatedDeliveryThread: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DedicatedDeliveryThread");
pub const g_wszDeinterlaceMode: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DeinterlaceMode");
pub const g_wszDeliverOnReceive: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DeliverOnReceive");
pub const g_wszDeviceConformanceTemplate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DeviceConformanceTemplate");
pub const g_wszDynamicRangeControl: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DynamicRangeControl");
pub const g_wszEDL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_EDL");
pub const g_wszEarlyDataDelivery: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EarlyDataDelivery");
pub const g_wszEnableDiscreteOutput: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableDiscreteOutput");
pub const g_wszEnableFrameInterpolation: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableFrameInterpolation");
pub const g_wszEnableWMAProSPDIFOutput: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableWMAProSPDIFOutput");
pub const g_wszFailSeekOnError: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FailSeekOnError");
pub const g_wszFixedFrameRate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FixedFrameRate");
pub const g_wszFold6To2Channels3: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Fold6To2Channels3");
pub const g_wszFoldToChannelsTemplate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Fold%luTo%luChannels%lu");
pub const g_wszInitialPatternForInverseTelecine: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InitialPatternForInverseTelecine");
pub const g_wszInterlacedCoding: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InterlacedCoding");
pub const g_wszIsVBRSupported: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_ISVBRSUPPORTED");
pub const g_wszJPEGCompressionQuality: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("JPEGCompressionQuality");
pub const g_wszJustInTimeDecode: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("JustInTimeDecode");
pub const g_wszMixedClassMode: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MixedClassMode");
pub const g_wszMusicClassMode: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MusicClassMode");
pub const g_wszMusicSpeechClassMode: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MusicSpeechClassMode");
pub const g_wszNeedsPreviousSample: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NeedsPreviousSample");
pub const g_wszNumPasses: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_PASSESUSED");
pub const g_wszOriginalSourceFormatTag: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_SOURCEFORMATTAG");
pub const g_wszOriginalWaveFormat: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_ORIGINALWAVEFORMAT");
pub const g_wszPeakValue: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeakValue");
pub const g_wszPermitSeeksBeyondEndOfStream: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PermitSeeksBeyondEndOfStream");
pub const g_wszReloadIndexOnSeek: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ReloadIndexOnSeek");
pub const g_wszScrambledAudio: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ScrambledAudio");
pub const g_wszSingleOutputBuffer: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SingleOutputBuffer");
pub const g_wszSoftwareScaling: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftwareScaling");
pub const g_wszSourceBufferTime: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SourceBufferTime");
pub const g_wszSourceMaxBytesAtOnce: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SourceMaxBytesAtOnce");
pub const g_wszSpeakerConfig: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SpeakerConfig");
pub const g_wszSpeechCaps: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SpeechFormatCap");
pub const g_wszSpeechClassMode: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SpeechClassMode");
pub const g_wszStreamLanguage: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StreamLanguage");
pub const g_wszStreamNumIndexObjects: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StreamNumIndexObjects");
pub const g_wszUsePacketAtSeekPoint: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UsePacketAtSeekPoint");
pub const g_wszVBRBitrateMax: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_RMAX");
pub const g_wszVBRBufferWindowMax: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_BMAX");
pub const g_wszVBREnabled: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_VBRENABLED");
pub const g_wszVBRPeak: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VBR Peak");
pub const g_wszVBRQuality: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("_VBRQUALITY");
pub const g_wszVideoSampleDurations: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VideoSampleDurations");
pub const g_wszWMADID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ADID");
pub const g_wszWMASFPacketCount: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ASFPacketCount");
pub const g_wszWMASFSecurityObjectsSize: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ASFSecurityObjectsSize");
pub const g_wszWMAlbumArtist: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AlbumArtist");
pub const g_wszWMAlbumArtistSort: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AlbumArtistSort");
pub const g_wszWMAlbumCoverURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AlbumCoverURL");
pub const g_wszWMAlbumTitle: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AlbumTitle");
pub const g_wszWMAlbumTitleSort: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AlbumTitleSort");
pub const g_wszWMAspectRatioX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AspectRatioX");
pub const g_wszWMAspectRatioY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AspectRatioY");
pub const g_wszWMAudioFileURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AudioFileURL");
pub const g_wszWMAudioSourceURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AudioSourceURL");
pub const g_wszWMAuthor: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Author");
pub const g_wszWMAuthorSort: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AuthorSort");
pub const g_wszWMAuthorURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/AuthorURL");
pub const g_wszWMBannerImageData: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BannerImageData");
pub const g_wszWMBannerImageType: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BannerImageType");
pub const g_wszWMBannerImageURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BannerImageURL");
pub const g_wszWMBeatsPerMinute: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/BeatsPerMinute");
pub const g_wszWMBitrate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Bitrate");
pub const g_wszWMBroadcast: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Broadcast");
pub const g_wszWMCategory: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Category");
pub const g_wszWMCodec: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Codec");
pub const g_wszWMComposer: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Composer");
pub const g_wszWMComposerSort: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ComposerSort");
pub const g_wszWMConductor: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Conductor");
pub const g_wszWMContainerFormat: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ContainerFormat");
pub const g_wszWMContentDistributor: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ContentDistributor");
pub const g_wszWMContentGroupDescription: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ContentGroupDescription");
pub const g_wszWMCopyright: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Copyright");
pub const g_wszWMCopyrightURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CopyrightURL");
pub const g_wszWMCurrentBitrate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CurrentBitrate");
pub const g_wszWMDRM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/DRM");
pub const g_wszWMDRM_ContentID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_ContentID");
pub const g_wszWMDRM_Flags: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_Flags");
pub const g_wszWMDRM_HeaderSignPrivKey: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_HeaderSignPrivKey");
pub const g_wszWMDRM_IndividualizedVersion: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_IndividualizedVersion");
pub const g_wszWMDRM_KeyID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_KeyID");
pub const g_wszWMDRM_KeySeed: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_KeySeed");
pub const g_wszWMDRM_LASignatureCert: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_LASignatureCert");
pub const g_wszWMDRM_LASignatureLicSrvCert: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_LASignatureLicSrvCert");
pub const g_wszWMDRM_LASignaturePrivKey: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_LASignaturePrivKey");
pub const g_wszWMDRM_LASignatureRootCert: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_LASignatureRootCert");
pub const g_wszWMDRM_Level: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_Level");
pub const g_wszWMDRM_LicenseAcqURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_LicenseAcqURL");
pub const g_wszWMDRM_SourceID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_SourceID");
pub const g_wszWMDRM_V1LicenseAcqURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DRM_V1LicenseAcqURL");
pub const g_wszWMDVDID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/DVDID");
pub const g_wszWMDescription: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const g_wszWMDirector: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Director");
pub const g_wszWMDuration: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Duration");
pub const g_wszWMEncodedBy: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/EncodedBy");
pub const g_wszWMEncodingSettings: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/EncodingSettings");
pub const g_wszWMEncodingTime: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/EncodingTime");
pub const g_wszWMEpisodeNumber: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/EpisodeNumber");
pub const g_wszWMFileSize: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FileSize");
pub const g_wszWMGenre: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Genre");
pub const g_wszWMGenreID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/GenreID");
pub const g_wszWMHasArbitraryDataStream: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HasArbitraryDataStream");
pub const g_wszWMHasAttachedImages: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HasAttachedImages");
pub const g_wszWMHasAudio: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HasAudio");
pub const g_wszWMHasFileTransferStream: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HasFileTransferStream");
pub const g_wszWMHasImage: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HasImage");
pub const g_wszWMHasScript: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HasScript");
pub const g_wszWMHasVideo: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HasVideo");
pub const g_wszWMISAN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ISAN");
pub const g_wszWMISRC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ISRC");
pub const g_wszWMInitialKey: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/InitialKey");
pub const g_wszWMIsCompilation: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/IsCompilation");
pub const g_wszWMIsVBR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsVBR");
pub const g_wszWMLanguage: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Language");
pub const g_wszWMLyrics: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Lyrics");
pub const g_wszWMLyrics_Synchronised: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Lyrics_Synchronised");
pub const g_wszWMMCDI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MCDI");
pub const g_wszWMMediaClassPrimaryID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaClassPrimaryID");
pub const g_wszWMMediaClassSecondaryID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaClassSecondaryID");
pub const g_wszWMMediaCredits: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaCredits");
pub const g_wszWMMediaIsDelay: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsDelay");
pub const g_wszWMMediaIsFinale: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsFinale");
pub const g_wszWMMediaIsLive: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsLive");
pub const g_wszWMMediaIsPremiere: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsPremiere");
pub const g_wszWMMediaIsRepeat: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsRepeat");
pub const g_wszWMMediaIsSAP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsSAP");
pub const g_wszWMMediaIsStereo: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsStereo");
pub const g_wszWMMediaIsSubtitled: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsSubtitled");
pub const g_wszWMMediaIsTape: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaIsTape");
pub const g_wszWMMediaNetworkAffiliation: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaNetworkAffiliation");
pub const g_wszWMMediaOriginalBroadcastDateTime: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaOriginalBroadcastDateTime");
pub const g_wszWMMediaOriginalChannel: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaOriginalChannel");
pub const g_wszWMMediaStationCallSign: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaStationCallSign");
pub const g_wszWMMediaStationName: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/MediaStationName");
pub const g_wszWMModifiedBy: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ModifiedBy");
pub const g_wszWMMood: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Mood");
pub const g_wszWMNSCAddress: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NSC_Address");
pub const g_wszWMNSCDescription: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NSC_Description");
pub const g_wszWMNSCEmail: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NSC_Email");
pub const g_wszWMNSCName: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NSC_Name");
pub const g_wszWMNSCPhone: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NSC_Phone");
pub const g_wszWMNumberOfFrames: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NumberOfFrames");
pub const g_wszWMOptimalBitrate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OptimalBitrate");
pub const g_wszWMOriginalAlbumTitle: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/OriginalAlbumTitle");
pub const g_wszWMOriginalArtist: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/OriginalArtist");
pub const g_wszWMOriginalFilename: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/OriginalFilename");
pub const g_wszWMOriginalLyricist: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/OriginalLyricist");
pub const g_wszWMOriginalReleaseTime: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/OriginalReleaseTime");
pub const g_wszWMOriginalReleaseYear: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/OriginalReleaseYear");
pub const g_wszWMParentalRating: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ParentalRating");
pub const g_wszWMParentalRatingReason: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ParentalRatingReason");
pub const g_wszWMPartOfSet: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/PartOfSet");
pub const g_wszWMPeakBitrate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/PeakBitrate");
pub const g_wszWMPeriod: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Period");
pub const g_wszWMPicture: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Picture");
pub const g_wszWMPlaylistDelay: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/PlaylistDelay");
pub const g_wszWMProducer: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Producer");
pub const g_wszWMPromotionURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/PromotionURL");
pub const g_wszWMProtected: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Is_Protected");
pub const g_wszWMProtectionType: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ProtectionType");
pub const g_wszWMProvider: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Provider");
pub const g_wszWMProviderCopyright: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ProviderCopyright");
pub const g_wszWMProviderRating: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ProviderRating");
pub const g_wszWMProviderStyle: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ProviderStyle");
pub const g_wszWMPublisher: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Publisher");
pub const g_wszWMRadioStationName: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/RadioStationName");
pub const g_wszWMRadioStationOwner: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/RadioStationOwner");
pub const g_wszWMRating: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Rating");
pub const g_wszWMSeasonNumber: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/SeasonNumber");
pub const g_wszWMSeekable: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Seekable");
pub const g_wszWMSharedUserRating: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/SharedUserRating");
pub const g_wszWMSignature_Name: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Signature_Name");
pub const g_wszWMSkipBackward: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Can_Skip_Backward");
pub const g_wszWMSkipForward: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Can_Skip_Forward");
pub const g_wszWMStreamTypeInfo: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/StreamTypeInfo");
pub const g_wszWMStridable: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Stridable");
pub const g_wszWMSubTitle: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/SubTitle");
pub const g_wszWMSubTitleDescription: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/SubTitleDescription");
pub const g_wszWMSubscriptionContentID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/SubscriptionContentID");
pub const g_wszWMText: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Text");
pub const g_wszWMTitle: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Title");
pub const g_wszWMTitleSort: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TitleSort");
pub const g_wszWMToolName: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ToolName");
pub const g_wszWMToolVersion: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/ToolVersion");
pub const g_wszWMTrack: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Track");
pub const g_wszWMTrackNumber: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/TrackNumber");
pub const g_wszWMTrusted: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Is_Trusted");
pub const g_wszWMUniqueFileIdentifier: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/UniqueFileIdentifier");
pub const g_wszWMUse_Advanced_DRM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Use_Advanced_DRM");
pub const g_wszWMUse_DRM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Use_DRM");
pub const g_wszWMUserWebURL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/UserWebURL");
pub const g_wszWMVideoClosedCaptioning: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/VideoClosedCaptioning");
pub const g_wszWMVideoFrameRate: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/VideoFrameRate");
pub const g_wszWMVideoHeight: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/VideoHeight");
pub const g_wszWMVideoWidth: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/VideoWidth");
pub const g_wszWMWMADRCAverageReference: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMADRCAverageReference");
pub const g_wszWMWMADRCAverageTarget: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMADRCAverageTarget");
pub const g_wszWMWMADRCPeakReference: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMADRCPeakReference");
pub const g_wszWMWMADRCPeakTarget: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMADRCPeakTarget");
pub const g_wszWMWMCPDistributor: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMCPDistributor");
pub const g_wszWMWMCPDistributorID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMCPDistributorID");
pub const g_wszWMWMCollectionGroupID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMCollectionGroupID");
pub const g_wszWMWMCollectionID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMCollectionID");
pub const g_wszWMWMContentID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMContentID");
pub const g_wszWMWMShadowFileSourceDRMType: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMShadowFileSourceDRMType");
pub const g_wszWMWMShadowFileSourceFileType: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/WMShadowFileSourceFileType");
pub const g_wszWMWriter: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Writer");
pub const g_wszWMYear: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WM/Year");
pub const g_wszWatermarkCLSID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WatermarkCLSID");
pub const g_wszWatermarkConfig: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WatermarkConfig");
pub type NETSOURCE_URLCREDPOLICY_SETTINGS = i32;
pub type WEBSTREAM_SAMPLE_TYPE = i32;
pub type WMT_ATTR_DATATYPE = i32;
pub type WMT_ATTR_IMAGETYPE = i32;
pub type WMT_CODEC_INFO_TYPE = i32;
pub type WMT_CREDENTIAL_FLAGS = i32;
pub type WMT_DRMLA_TRUST = i32;
pub type WMT_FILESINK_MODE = i32;
pub type WMT_IMAGE_TYPE = i32;
pub type WMT_INDEXER_TYPE = i32;
pub type WMT_INDEX_TYPE = i32;
pub type WMT_MUSICSPEECH_CLASS_MODE = i32;
pub type WMT_NET_PROTOCOL = i32;
pub type WMT_OFFSET_FORMAT = i32;
pub type WMT_PLAY_MODE = i32;
pub type WMT_PROXY_SETTINGS = i32;
pub type WMT_RIGHTS = i32;
pub type WMT_STATUS = i32;
pub type WMT_STORAGE_FORMAT = i32;
pub type WMT_STREAM_SELECTION = i32;
pub type WMT_TIMECODE_FRAMERATE = i32;
pub type WMT_TRANSPORT_TYPE = i32;
pub type WMT_VERSION = i32;
pub type WMT_WATERMARK_ENTRY_TYPE = i32;
pub type WM_AETYPE = i32;
pub type WM_DM_INTERLACED_TYPE = i32;
pub type WM_DM_IT_FIRST_FRAME_COHERENCY = i32;
pub type WM_PLAYBACK_DRC_LEVEL = i32;
pub type WM_SFEX_TYPE = i32;
pub type WM_SF_TYPE = i32;
pub type _AM_ASFWRITERCONFIG_PARAM = i32;
#[repr(C)]
pub struct AM_WMT_EVENT_DATA {
    pub hrStatus: ::windows_sys::core::HRESULT,
    pub pData: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for AM_WMT_EVENT_DATA {}
impl ::core::clone::Clone for AM_WMT_EVENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRM_COPY_OPL {
    pub wMinimumCopyLevel: u16,
    pub oplIdIncludes: DRM_OPL_OUTPUT_IDS,
    pub oplIdExcludes: DRM_OPL_OUTPUT_IDS,
}
impl ::core::marker::Copy for DRM_COPY_OPL {}
impl ::core::clone::Clone for DRM_COPY_OPL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRM_MINIMUM_OUTPUT_PROTECTION_LEVELS {
    pub wCompressedDigitalVideo: u16,
    pub wUncompressedDigitalVideo: u16,
    pub wAnalogVideo: u16,
    pub wCompressedDigitalAudio: u16,
    pub wUncompressedDigitalAudio: u16,
}
impl ::core::marker::Copy for DRM_MINIMUM_OUTPUT_PROTECTION_LEVELS {}
impl ::core::clone::Clone for DRM_MINIMUM_OUTPUT_PROTECTION_LEVELS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRM_OPL_OUTPUT_IDS {
    pub cIds: u16,
    pub rgIds: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for DRM_OPL_OUTPUT_IDS {}
impl ::core::clone::Clone for DRM_OPL_OUTPUT_IDS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRM_OUTPUT_PROTECTION {
    pub guidId: ::windows_sys::core::GUID,
    pub bConfigData: u8,
}
impl ::core::marker::Copy for DRM_OUTPUT_PROTECTION {}
impl ::core::clone::Clone for DRM_OUTPUT_PROTECTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRM_PLAY_OPL {
    pub minOPL: DRM_MINIMUM_OUTPUT_PROTECTION_LEVELS,
    pub oplIdReserved: DRM_OPL_OUTPUT_IDS,
    pub vopi: DRM_VIDEO_OUTPUT_PROTECTION_IDS,
}
impl ::core::marker::Copy for DRM_PLAY_OPL {}
impl ::core::clone::Clone for DRM_PLAY_OPL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRM_VAL16 {
    pub val: [u8; 16],
}
impl ::core::marker::Copy for DRM_VAL16 {}
impl ::core::clone::Clone for DRM_VAL16 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRM_VIDEO_OUTPUT_PROTECTION_IDS {
    pub cEntries: u16,
    pub rgVop: *mut DRM_OUTPUT_PROTECTION,
}
impl ::core::marker::Copy for DRM_VIDEO_OUTPUT_PROTECTION_IDS {}
impl ::core::clone::Clone for DRM_VIDEO_OUTPUT_PROTECTION_IDS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMDRM_IMPORT_INIT_STRUCT {
    pub dwVersion: u32,
    pub cbEncryptedSessionKeyMessage: u32,
    pub pbEncryptedSessionKeyMessage: *mut u8,
    pub cbEncryptedKeyMessage: u32,
    pub pbEncryptedKeyMessage: *mut u8,
}
impl ::core::marker::Copy for WMDRM_IMPORT_INIT_STRUCT {}
impl ::core::clone::Clone for WMDRM_IMPORT_INIT_STRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct WMMPEG2VIDEOINFO {
    pub hdr: WMVIDEOINFOHEADER2,
    pub dwStartTimeCode: u32,
    pub cbSequenceHeader: u32,
    pub dwProfile: u32,
    pub dwLevel: u32,
    pub dwFlags: u32,
    pub dwSequenceHeader: [u32; 1],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for WMMPEG2VIDEOINFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for WMMPEG2VIDEOINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMSCRIPTFORMAT {
    pub scriptType: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for WMSCRIPTFORMAT {}
impl ::core::clone::Clone for WMSCRIPTFORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_BUFFER_SEGMENT {
    pub pBuffer: INSSBuffer,
    pub cbOffset: u32,
    pub cbLength: u32,
}
impl ::core::marker::Copy for WMT_BUFFER_SEGMENT {}
impl ::core::clone::Clone for WMT_BUFFER_SEGMENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_COLORSPACEINFO_EXTENSION_DATA {
    pub ucColorPrimaries: u8,
    pub ucColorTransferChar: u8,
    pub ucColorMatrixCoef: u8,
}
impl ::core::marker::Copy for WMT_COLORSPACEINFO_EXTENSION_DATA {}
impl ::core::clone::Clone for WMT_COLORSPACEINFO_EXTENSION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_FILESINK_DATA_UNIT {
    pub packetHeaderBuffer: WMT_BUFFER_SEGMENT,
    pub cPayloads: u32,
    pub pPayloadHeaderBuffers: *mut WMT_BUFFER_SEGMENT,
    pub cPayloadDataFragments: u32,
    pub pPayloadDataFragments: *mut WMT_PAYLOAD_FRAGMENT,
}
impl ::core::marker::Copy for WMT_FILESINK_DATA_UNIT {}
impl ::core::clone::Clone for WMT_FILESINK_DATA_UNIT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_PAYLOAD_FRAGMENT {
    pub dwPayloadIndex: u32,
    pub segmentData: WMT_BUFFER_SEGMENT,
}
impl ::core::marker::Copy for WMT_PAYLOAD_FRAGMENT {}
impl ::core::clone::Clone for WMT_PAYLOAD_FRAGMENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
pub struct WMT_TIMECODE_EXTENSION_DATA {
    pub wRange: u16,
    pub dwTimecode: u32,
    pub dwUserbits: u32,
    pub dwAmFlags: u32,
}
impl ::core::marker::Copy for WMT_TIMECODE_EXTENSION_DATA {}
impl ::core::clone::Clone for WMT_TIMECODE_EXTENSION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_VIDEOIMAGE_SAMPLE {
    pub dwMagic: u32,
    pub cbStruct: u32,
    pub dwControlFlags: u32,
    pub dwInputFlagsCur: u32,
    pub lCurMotionXtoX: i32,
    pub lCurMotionYtoX: i32,
    pub lCurMotionXoffset: i32,
    pub lCurMotionXtoY: i32,
    pub lCurMotionYtoY: i32,
    pub lCurMotionYoffset: i32,
    pub lCurBlendCoef1: i32,
    pub lCurBlendCoef2: i32,
    pub dwInputFlagsPrev: u32,
    pub lPrevMotionXtoX: i32,
    pub lPrevMotionYtoX: i32,
    pub lPrevMotionXoffset: i32,
    pub lPrevMotionXtoY: i32,
    pub lPrevMotionYtoY: i32,
    pub lPrevMotionYoffset: i32,
    pub lPrevBlendCoef1: i32,
    pub lPrevBlendCoef2: i32,
}
impl ::core::marker::Copy for WMT_VIDEOIMAGE_SAMPLE {}
impl ::core::clone::Clone for WMT_VIDEOIMAGE_SAMPLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WMT_VIDEOIMAGE_SAMPLE2 {
    pub dwMagic: u32,
    pub dwStructSize: u32,
    pub dwControlFlags: u32,
    pub dwViewportWidth: u32,
    pub dwViewportHeight: u32,
    pub dwCurrImageWidth: u32,
    pub dwCurrImageHeight: u32,
    pub fCurrRegionX0: f32,
    pub fCurrRegionY0: f32,
    pub fCurrRegionWidth: f32,
    pub fCurrRegionHeight: f32,
    pub fCurrBlendCoef: f32,
    pub dwPrevImageWidth: u32,
    pub dwPrevImageHeight: u32,
    pub fPrevRegionX0: f32,
    pub fPrevRegionY0: f32,
    pub fPrevRegionWidth: f32,
    pub fPrevRegionHeight: f32,
    pub fPrevBlendCoef: f32,
    pub dwEffectType: u32,
    pub dwNumEffectParas: u32,
    pub fEffectPara0: f32,
    pub fEffectPara1: f32,
    pub fEffectPara2: f32,
    pub fEffectPara3: f32,
    pub fEffectPara4: f32,
    pub bKeepPrevImage: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WMT_VIDEOIMAGE_SAMPLE2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WMT_VIDEOIMAGE_SAMPLE2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_WATERMARK_ENTRY {
    pub wmetType: WMT_WATERMARK_ENTRY_TYPE,
    pub clsid: ::windows_sys::core::GUID,
    pub cbDisplayName: u32,
    pub pwszDisplayName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for WMT_WATERMARK_ENTRY {}
impl ::core::clone::Clone for WMT_WATERMARK_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_WEBSTREAM_FORMAT {
    pub cbSize: u16,
    pub cbSampleHeaderFixedData: u16,
    pub wVersion: u16,
    pub wReserved: u16,
}
impl ::core::marker::Copy for WMT_WEBSTREAM_FORMAT {}
impl ::core::clone::Clone for WMT_WEBSTREAM_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WMT_WEBSTREAM_SAMPLE_HEADER {
    pub cbLength: u16,
    pub wPart: u16,
    pub cTotalParts: u16,
    pub wSampleType: u16,
    pub wszURL: [u16; 1],
}
impl ::core::marker::Copy for WMT_WEBSTREAM_SAMPLE_HEADER {}
impl ::core::clone::Clone for WMT_WEBSTREAM_SAMPLE_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct WMVIDEOINFOHEADER {
    pub rcSource: super::super::Foundation::RECT,
    pub rcTarget: super::super::Foundation::RECT,
    pub dwBitRate: u32,
    pub dwBitErrorRate: u32,
    pub AvgTimePerFrame: i64,
    pub bmiHeader: super::super::Graphics::Gdi::BITMAPINFOHEADER,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for WMVIDEOINFOHEADER {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for WMVIDEOINFOHEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub struct WMVIDEOINFOHEADER2 {
    pub rcSource: super::super::Foundation::RECT,
    pub rcTarget: super::super::Foundation::RECT,
    pub dwBitRate: u32,
    pub dwBitErrorRate: u32,
    pub AvgTimePerFrame: i64,
    pub dwInterlaceFlags: u32,
    pub dwCopyProtectFlags: u32,
    pub dwPictAspectRatioX: u32,
    pub dwPictAspectRatioY: u32,
    pub dwReserved1: u32,
    pub dwReserved2: u32,
    pub bmiHeader: super::super::Graphics::Gdi::BITMAPINFOHEADER,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::marker::Copy for WMVIDEOINFOHEADER2 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
impl ::core::clone::Clone for WMVIDEOINFOHEADER2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WM_ADDRESS_ACCESSENTRY {
    pub dwIPAddress: u32,
    pub dwMask: u32,
}
impl ::core::marker::Copy for WM_ADDRESS_ACCESSENTRY {}
impl ::core::clone::Clone for WM_ADDRESS_ACCESSENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WM_CLIENT_PROPERTIES {
    pub dwIPAddress: u32,
    pub dwPort: u32,
}
impl ::core::marker::Copy for WM_CLIENT_PROPERTIES {}
impl ::core::clone::Clone for WM_CLIENT_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WM_CLIENT_PROPERTIES_EX {
    pub cbSize: u32,
    pub pwszIPAddress: ::windows_sys::core::PCWSTR,
    pub pwszPort: ::windows_sys::core::PCWSTR,
    pub pwszDNSName: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for WM_CLIENT_PROPERTIES_EX {}
impl ::core::clone::Clone for WM_CLIENT_PROPERTIES_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WM_LEAKY_BUCKET_PAIR {
    pub dwBitrate: u32,
    pub msBufferWindow: u32,
}
impl ::core::marker::Copy for WM_LEAKY_BUCKET_PAIR {}
impl ::core::clone::Clone for WM_LEAKY_BUCKET_PAIR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WM_MEDIA_TYPE {
    pub majortype: ::windows_sys::core::GUID,
    pub subtype: ::windows_sys::core::GUID,
    pub bFixedSizeSamples: super::super::Foundation::BOOL,
    pub bTemporalCompression: super::super::Foundation::BOOL,
    pub lSampleSize: u32,
    pub formattype: ::windows_sys::core::GUID,
    pub pUnk: ::windows_sys::core::IUnknown,
    pub cbFormat: u32,
    pub pbFormat: *mut u8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WM_MEDIA_TYPE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WM_MEDIA_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WM_PICTURE {
    pub pwszMIMEType: ::windows_sys::core::PWSTR,
    pub bPictureType: u8,
    pub pwszDescription: ::windows_sys::core::PWSTR,
    pub dwDataLen: u32,
    pub pbData: *mut u8,
}
impl ::core::marker::Copy for WM_PICTURE {}
impl ::core::clone::Clone for WM_PICTURE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WM_PORT_NUMBER_RANGE {
    pub wPortBegin: u16,
    pub wPortEnd: u16,
}
impl ::core::marker::Copy for WM_PORT_NUMBER_RANGE {}
impl ::core::clone::Clone for WM_PORT_NUMBER_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WM_READER_CLIENTINFO {
    pub cbSize: u32,
    pub wszLang: ::windows_sys::core::PWSTR,
    pub wszBrowserUserAgent: ::windows_sys::core::PWSTR,
    pub wszBrowserWebPage: ::windows_sys::core::PWSTR,
    pub qwReserved: u64,
    pub pReserved: *mut super::super::Foundation::LPARAM,
    pub wszHostExe: ::windows_sys::core::PWSTR,
    pub qwHostVersion: u64,
    pub wszPlayerUserAgent: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WM_READER_CLIENTINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WM_READER_CLIENTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WM_READER_STATISTICS {
    pub cbSize: u32,
    pub dwBandwidth: u32,
    pub cPacketsReceived: u32,
    pub cPacketsRecovered: u32,
    pub cPacketsLost: u32,
    pub wQuality: u16,
}
impl ::core::marker::Copy for WM_READER_STATISTICS {}
impl ::core::clone::Clone for WM_READER_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WM_STREAM_PRIORITY_RECORD {
    pub wStreamNumber: u16,
    pub fMandatory: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WM_STREAM_PRIORITY_RECORD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WM_STREAM_PRIORITY_RECORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WM_STREAM_TYPE_INFO {
    pub guidMajorType: ::windows_sys::core::GUID,
    pub cbFormat: u32,
}
impl ::core::marker::Copy for WM_STREAM_TYPE_INFO {}
impl ::core::clone::Clone for WM_STREAM_TYPE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WM_SYNCHRONISED_LYRICS {
    pub bTimeStampFormat: u8,
    pub bContentType: u8,
    pub pwszContentDescriptor: ::windows_sys::core::PWSTR,
    pub dwLyricsLen: u32,
    pub pbLyrics: *mut u8,
}
impl ::core::marker::Copy for WM_SYNCHRONISED_LYRICS {}
impl ::core::clone::Clone for WM_SYNCHRONISED_LYRICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WM_USER_TEXT {
    pub pwszDescription: ::windows_sys::core::PWSTR,
    pub pwszText: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for WM_USER_TEXT {}
impl ::core::clone::Clone for WM_USER_TEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WM_USER_WEB_URL {
    pub pwszDescription: ::windows_sys::core::PWSTR,
    pub pwszURL: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for WM_USER_WEB_URL {}
impl ::core::clone::Clone for WM_USER_WEB_URL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WM_WRITER_STATISTICS {
    pub qwSampleCount: u64,
    pub qwByteCount: u64,
    pub qwDroppedSampleCount: u64,
    pub qwDroppedByteCount: u64,
    pub dwCurrentBitrate: u32,
    pub dwAverageBitrate: u32,
    pub dwExpectedBitrate: u32,
    pub dwCurrentSampleRate: u32,
    pub dwAverageSampleRate: u32,
    pub dwExpectedSampleRate: u32,
}
impl ::core::marker::Copy for WM_WRITER_STATISTICS {}
impl ::core::clone::Clone for WM_WRITER_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WM_WRITER_STATISTICS_EX {
    pub dwBitratePlusOverhead: u32,
    pub dwCurrentSampleDropRateInQueue: u32,
    pub dwCurrentSampleDropRateInCodec: u32,
    pub dwCurrentSampleDropRateInMultiplexer: u32,
    pub dwTotalSampleDropsInQueue: u32,
    pub dwTotalSampleDropsInCodec: u32,
    pub dwTotalSampleDropsInMultiplexer: u32,
}
impl ::core::marker::Copy for WM_WRITER_STATISTICS_EX {}
impl ::core::clone::Clone for WM_WRITER_STATISTICS_EX {
    fn clone(&self) -> Self {
        *self
    }
}
