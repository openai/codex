#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenPersonalTrustDBDialog(hwndparent : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenPersonalTrustDBDialogEx(hwndparent : super::super::Foundation:: HWND, dwflags : u32, pvreserved : *mut *mut ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn WTHelperCertCheckValidSignature(pprovdata : *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn WTHelperCertIsSelfSigned(dwencoding : u32, pcert : *mut super::Cryptography:: CERT_INFO) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn WTHelperGetProvCertFromChain(psgnr : *mut CRYPT_PROVIDER_SGNR, idxcert : u32) -> *mut CRYPT_PROVIDER_CERT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn WTHelperGetProvPrivateDataFromChain(pprovdata : *mut CRYPT_PROVIDER_DATA, pgproviderid : *mut ::windows_sys::core::GUID) -> *mut CRYPT_PROVIDER_PRIVDATA);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn WTHelperGetProvSignerFromChain(pprovdata : *mut CRYPT_PROVIDER_DATA, idxsigner : u32, fcountersigner : super::super::Foundation:: BOOL, idxcountersigner : u32) -> *mut CRYPT_PROVIDER_SGNR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn WTHelperProvDataFromStateData(hstatedata : super::super::Foundation:: HANDLE) -> *mut CRYPT_PROVIDER_DATA);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WinVerifyTrust(hwnd : super::super::Foundation:: HWND, pgactionid : *mut ::windows_sys::core::GUID, pwvtdata : *mut ::core::ffi::c_void) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn WinVerifyTrustEx(hwnd : super::super::Foundation:: HWND, pgactionid : *mut ::windows_sys::core::GUID, pwintrustdata : *mut WINTRUST_DATA) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WintrustAddActionID(pgactionid : *const ::windows_sys::core::GUID, fdwflags : u32, psprovinfo : *const CRYPT_REGISTER_ACTIONID) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WintrustAddDefaultForUsage(pszusageoid : ::windows_sys::core::PCSTR, psdefusage : *const CRYPT_PROVIDER_REGDEFUSAGE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WintrustGetDefaultForUsage(dwaction : WINTRUST_GET_DEFAULT_FOR_USAGE_ACTION, pszusageoid : ::windows_sys::core::PCSTR, psusage : *mut CRYPT_PROVIDER_DEFUSAGE) -> super::super::Foundation:: BOOL);
::windows_targets::link!("wintrust.dll" "system" fn WintrustGetRegPolicyFlags(pdwpolicyflags : *mut WINTRUST_POLICY_FLAGS) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn WintrustLoadFunctionPointers(pgactionid : *mut ::windows_sys::core::GUID, ppfns : *mut CRYPT_PROVIDER_FUNCTIONS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WintrustRemoveActionID(pgactionid : *const ::windows_sys::core::GUID) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WintrustSetDefaultIncludePEPageHashes(fincludepepagehashes : super::super::Foundation:: BOOL) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WintrustSetRegPolicyFlags(dwpolicyflags : WINTRUST_POLICY_FLAGS) -> super::super::Foundation:: BOOL);
pub const CAT_MEMBERINFO2_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.12.2.3");
pub const CAT_MEMBERINFO2_STRUCT: ::windows_sys::core::PCSTR = 2223i32 as _;
pub const CAT_MEMBERINFO_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.12.2.2");
pub const CAT_MEMBERINFO_STRUCT: ::windows_sys::core::PCSTR = 2222i32 as _;
pub const CAT_NAMEVALUE_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.12.2.1");
pub const CAT_NAMEVALUE_STRUCT: ::windows_sys::core::PCSTR = 2221i32 as _;
pub const CCPI_RESULT_ALLOW: u32 = 1u32;
pub const CCPI_RESULT_AUDIT: u32 = 3u32;
pub const CCPI_RESULT_DENY: u32 = 2u32;
pub const CERT_CONFIDENCE_AUTHIDEXT: u32 = 65536u32;
pub const CERT_CONFIDENCE_HIGHEST: u32 = 286330880u32;
pub const CERT_CONFIDENCE_HYGIENE: u32 = 4096u32;
pub const CERT_CONFIDENCE_SIG: u32 = 268435456u32;
pub const CERT_CONFIDENCE_TIME: u32 = 16777216u32;
pub const CERT_CONFIDENCE_TIMENEST: u32 = 1048576u32;
pub const CONFIG_CI_ACTION_VERIFY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6078065b_8f22_4b13_bd9b_5b762776f386);
pub const CPD_CHOICE_SIP: u32 = 1u32;
pub const CPD_RETURN_LOWER_QUALITY_CHAINS: u32 = 1048576u32;
pub const CPD_REVOCATION_CHECK_CHAIN: u32 = 262144u32;
pub const CPD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT: u32 = 524288u32;
pub const CPD_REVOCATION_CHECK_END_CERT: u32 = 131072u32;
pub const CPD_REVOCATION_CHECK_NONE: u32 = 65536u32;
pub const CPD_RFC3161v21: u32 = 2097152u32;
pub const CPD_UISTATE_MODE_ALLOW: u32 = 2u32;
pub const CPD_UISTATE_MODE_BLOCK: u32 = 1u32;
pub const CPD_UISTATE_MODE_MASK: u32 = 3u32;
pub const CPD_UISTATE_MODE_PROMPT: u32 = 0u32;
pub const CPD_USE_NT5_CHAIN_FLAG: u32 = 2147483648u32;
pub const DRIVER_ACTION_VERIFY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf750e6c3_38ee_11d1_85e5_00c04fc295ee);
pub const DRIVER_CLEANUPPOLICY_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverCleanupPolicy");
pub const DRIVER_FINALPOLPROV_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverFinalPolicy");
pub const DRIVER_INITPROV_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverInitializePolicy");
pub const DWACTION_ALLOCANDFILL: WINTRUST_GET_DEFAULT_FOR_USAGE_ACTION = 1u32;
pub const DWACTION_FREE: WINTRUST_GET_DEFAULT_FOR_USAGE_ACTION = 2u32;
pub const GENERIC_CHAIN_CERTTRUST_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GenericChainCertificateTrust");
pub const GENERIC_CHAIN_FINALPOLICY_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GenericChainFinalProv");
pub const HTTPSPROV_ACTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x573e31f8_aaba_11d0_8ccb_00c04fc295ee);
pub const HTTPS_CERTTRUST_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HTTPSCertificateTrust");
pub const HTTPS_CHKCERT_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HTTPSCheckCertProv");
pub const HTTPS_FINALPOLICY_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HTTPSFinalProv");
pub const INTENT_TO_SEAL_ATTRIBUTE_STRUCT: ::windows_sys::core::PCSTR = 2010i32 as _;
pub const OFFICESIGN_ACTION_VERIFY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5555c2cd_17fb_11d1_85c4_00c04fc295ee);
pub const OFFICE_CLEANUPPOLICY_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OfficeCleanupPolicy");
pub const OFFICE_INITPROV_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OfficeInitializePolicy");
pub const OFFICE_POLICY_PROVIDER_DLL_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WINTRUST.DLL");
pub const SEALING_SIGNATURE_ATTRIBUTE_STRUCT: ::windows_sys::core::PCSTR = 2011i32 as _;
pub const SEALING_TIMESTAMP_ATTRIBUTE_STRUCT: ::windows_sys::core::PCSTR = 2012i32 as _;
pub const SGNR_TYPE_TIMESTAMP: u32 = 16u32;
pub const SPC_CAB_DATA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.25");
pub const SPC_CAB_DATA_STRUCT: ::windows_sys::core::PCSTR = 2008i32 as _;
pub const SPC_CERT_EXTENSIONS_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.14");
pub const SPC_COMMERCIAL_SP_KEY_PURPOSE_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.22");
pub const SPC_COMMON_NAME_OBJID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("2.5.4.3");
pub const SPC_ENCRYPTED_DIGEST_RETRY_COUNT_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.6.2");
pub const SPC_FILE_LINK_CHOICE: u32 = 3u32;
pub const SPC_FINANCIAL_CRITERIA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.27");
pub const SPC_FINANCIAL_CRITERIA_STRUCT: ::windows_sys::core::PCSTR = 2002i32 as _;
pub const SPC_GLUE_RDN_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.25");
pub const SPC_INDIRECT_DATA_CONTENT_STRUCT: ::windows_sys::core::PCSTR = 2003i32 as _;
pub const SPC_INDIRECT_DATA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.4");
pub const SPC_INDIVIDUAL_SP_KEY_PURPOSE_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.21");
pub const SPC_JAVA_CLASS_DATA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.20");
pub const SPC_JAVA_CLASS_DATA_STRUCT: ::windows_sys::core::PCSTR = 2009i32 as _;
pub const SPC_LINK_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.28");
pub const SPC_LINK_STRUCT: ::windows_sys::core::PCSTR = 2005i32 as _;
pub const SPC_MINIMAL_CRITERIA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.26");
pub const SPC_MINIMAL_CRITERIA_STRUCT: ::windows_sys::core::PCSTR = 2001i32 as _;
pub const SPC_MONIKER_LINK_CHOICE: u32 = 2u32;
pub const SPC_NATURAL_AUTH_PLUGIN_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.96.1.1");
pub const SPC_PE_IMAGE_DATA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.15");
pub const SPC_PE_IMAGE_DATA_STRUCT: ::windows_sys::core::PCSTR = 2004i32 as _;
pub const SPC_PE_IMAGE_PAGE_HASHES_V1_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.3.1");
pub const SPC_PE_IMAGE_PAGE_HASHES_V2_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.3.2");
pub const SPC_RAW_FILE_DATA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.18");
pub const SPC_RELAXED_PE_MARKER_CHECK_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.6.1");
pub const SPC_SIGINFO_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.30");
pub const SPC_SIGINFO_STRUCT: ::windows_sys::core::PCSTR = 2130i32 as _;
pub const SPC_SP_AGENCY_INFO_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.10");
pub const SPC_SP_AGENCY_INFO_STRUCT: ::windows_sys::core::PCSTR = 2000i32 as _;
pub const SPC_SP_OPUS_INFO_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.12");
pub const SPC_SP_OPUS_INFO_STRUCT: ::windows_sys::core::PCSTR = 2007i32 as _;
pub const SPC_STATEMENT_TYPE_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.11");
pub const SPC_STATEMENT_TYPE_STRUCT: ::windows_sys::core::PCSTR = 2006i32 as _;
pub const SPC_STRUCTURED_STORAGE_DATA_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.1.19");
pub const SPC_TIME_STAMP_REQUEST_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.3.2.1");
pub const SPC_URL_LINK_CHOICE: u32 = 1u32;
pub const SPC_UUID_LENGTH: u32 = 16u32;
pub const SPC_WINDOWS_HELLO_COMPATIBILITY_OBJID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.10.41.1");
pub const SP_CHKCERT_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubCheckCert");
pub const SP_CLEANUPPOLICY_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubCleanup");
pub const SP_FINALPOLICY_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubAuthenticode");
pub const SP_GENERIC_CERT_INIT_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubDefCertInit");
pub const SP_INIT_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubInitialize");
pub const SP_OBJTRUST_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubLoadMessage");
pub const SP_POLICY_PROVIDER_DLL_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WINTRUST.DLL");
pub const SP_SIGTRUST_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubLoadSignature");
pub const SP_TESTDUMPPOLICY_FUNCTION_TEST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftpubDumpStructure");
pub const TRUSTERROR_MAX_STEPS: u32 = 38u32;
pub const TRUSTERROR_STEP_CATALOGFILE: u32 = 6u32;
pub const TRUSTERROR_STEP_CERTSTORE: u32 = 7u32;
pub const TRUSTERROR_STEP_FILEIO: u32 = 2u32;
pub const TRUSTERROR_STEP_FINAL_CERTCHKPROV: u32 = 35u32;
pub const TRUSTERROR_STEP_FINAL_CERTPROV: u32 = 34u32;
pub const TRUSTERROR_STEP_FINAL_INITPROV: u32 = 31u32;
pub const TRUSTERROR_STEP_FINAL_OBJPROV: u32 = 32u32;
pub const TRUSTERROR_STEP_FINAL_POLICYPROV: u32 = 36u32;
pub const TRUSTERROR_STEP_FINAL_SIGPROV: u32 = 33u32;
pub const TRUSTERROR_STEP_FINAL_UIPROV: u32 = 37u32;
pub const TRUSTERROR_STEP_FINAL_WVTINIT: u32 = 30u32;
pub const TRUSTERROR_STEP_MESSAGE: u32 = 8u32;
pub const TRUSTERROR_STEP_MSG_CERTCHAIN: u32 = 15u32;
pub const TRUSTERROR_STEP_MSG_COUNTERSIGCERT: u32 = 17u32;
pub const TRUSTERROR_STEP_MSG_COUNTERSIGINFO: u32 = 16u32;
pub const TRUSTERROR_STEP_MSG_INNERCNT: u32 = 11u32;
pub const TRUSTERROR_STEP_MSG_INNERCNTTYPE: u32 = 10u32;
pub const TRUSTERROR_STEP_MSG_SIGNERCERT: u32 = 14u32;
pub const TRUSTERROR_STEP_MSG_SIGNERCOUNT: u32 = 9u32;
pub const TRUSTERROR_STEP_MSG_SIGNERINFO: u32 = 13u32;
pub const TRUSTERROR_STEP_MSG_STORE: u32 = 12u32;
pub const TRUSTERROR_STEP_SIP: u32 = 3u32;
pub const TRUSTERROR_STEP_SIPSUBJINFO: u32 = 5u32;
pub const TRUSTERROR_STEP_VERIFY_MSGHASH: u32 = 18u32;
pub const TRUSTERROR_STEP_VERIFY_MSGINDIRECTDATA: u32 = 19u32;
pub const TRUSTERROR_STEP_WVTPARAMS: u32 = 0u32;
pub const WINTRUST_ACTION_GENERIC_CERT_VERIFY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x189a3842_3041_11d1_85e1_00c04fc295ee);
pub const WINTRUST_ACTION_GENERIC_CHAIN_VERIFY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfc451c16_ac75_11d1_b4b8_00c04fb66ea0);
pub const WINTRUST_ACTION_GENERIC_VERIFY_V2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00aac56b_cd44_11d0_8cc2_00c04fc295ee);
pub const WINTRUST_ACTION_TRUSTPROVIDER_TEST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x573e31f8_ddba_11d0_8ccb_00c04fc295ee);
pub const WINTRUST_CONFIG_REGPATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Software\\Microsoft\\Cryptography\\Wintrust\\Config");
pub const WINTRUST_MAX_HASH_BYTES_TO_MAP_DEFAULT: u32 = 1048576u32;
pub const WINTRUST_MAX_HASH_BYTES_TO_MAP_VALUE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaxHashBytesToMap");
pub const WINTRUST_MAX_HEADER_BYTES_TO_MAP_DEFAULT: u32 = 10485760u32;
pub const WINTRUST_MAX_HEADER_BYTES_TO_MAP_VALUE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaxHeaderBytesToMap");
pub const WIN_CERT_REVISION_1_0: u32 = 256u32;
pub const WIN_CERT_REVISION_2_0: u32 = 512u32;
pub const WIN_CERT_TYPE_PKCS_SIGNED_DATA: u32 = 2u32;
pub const WIN_CERT_TYPE_RESERVED_1: u32 = 3u32;
pub const WIN_CERT_TYPE_TS_STACK_SIGNED: u32 = 4u32;
pub const WIN_CERT_TYPE_X509: u32 = 1u32;
pub const WIN_SPUB_ACTION_NT_ACTIVATE_IMAGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8bc96b00_8da1_11cf_8736_00aa00a485eb);
pub const WIN_SPUB_ACTION_PUBLISHED_SOFTWARE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x64b9d180_8da2_11cf_8736_00aa00a485eb);
pub const WIN_SPUB_ACTION_TRUSTED_PUBLISHER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x66426730_8da1_11cf_8736_00aa00a485eb);
pub const WIN_TRUST_SUBJTYPE_CABINET: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd17c5374_a392_11cf_9df5_00aa00c184e0);
pub const WIN_TRUST_SUBJTYPE_CABINETEX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6f458114_c2f1_11cf_8a69_00aa006c3706);
pub const WIN_TRUST_SUBJTYPE_JAVA_CLASS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x08ad3990_8da1_11cf_8736_00aa00a485eb);
pub const WIN_TRUST_SUBJTYPE_JAVA_CLASSEX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6f458113_c2f1_11cf_8a69_00aa006c3706);
pub const WIN_TRUST_SUBJTYPE_OLE_STORAGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc257e740_8da0_11cf_8736_00aa00a485eb);
pub const WIN_TRUST_SUBJTYPE_PE_IMAGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x43c9a1e0_8da0_11cf_8736_00aa00a485eb);
pub const WIN_TRUST_SUBJTYPE_PE_IMAGEEX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6f458111_c2f1_11cf_8a69_00aa006c3706);
pub const WIN_TRUST_SUBJTYPE_RAW_FILE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x959dc450_8d9e_11cf_8736_00aa00a485eb);
pub const WIN_TRUST_SUBJTYPE_RAW_FILEEX: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6f458110_c2f1_11cf_8a69_00aa006c3706);
pub const WSS_CERTTRUST_SUPPORT: u32 = 4u32;
pub const WSS_GET_SECONDARY_SIG_COUNT: WINTRUST_SIGNATURE_SETTINGS_FLAGS = 2u32;
pub const WSS_INPUT_FLAG_MASK: u32 = 7u32;
pub const WSS_OBJTRUST_SUPPORT: u32 = 1u32;
pub const WSS_OUTPUT_FLAG_MASK: u32 = 3758096384u32;
pub const WSS_OUT_FILE_SUPPORTS_SEAL: u32 = 536870912u32;
pub const WSS_OUT_HAS_SEALING_INTENT: u32 = 1073741824u32;
pub const WSS_OUT_SEALING_STATUS_VERIFIED: u32 = 2147483648u32;
pub const WSS_SIGTRUST_SUPPORT: u32 = 2u32;
pub const WSS_VERIFY_SEALING: u32 = 4u32;
pub const WSS_VERIFY_SPECIFIC: WINTRUST_SIGNATURE_SETTINGS_FLAGS = 1u32;
pub const WTCI_DONT_OPEN_STORES: u32 = 1u32;
pub const WTCI_OPEN_ONLY_ROOT: u32 = 2u32;
pub const WTCI_USE_LOCAL_MACHINE: u32 = 4u32;
pub const WTD_CACHE_ONLY_URL_RETRIEVAL: WINTRUST_DATA_PROVIDER_FLAGS = 4096u32;
pub const WTD_CHOICE_BLOB: WINTRUST_DATA_UNION_CHOICE = 3u32;
pub const WTD_CHOICE_CATALOG: WINTRUST_DATA_UNION_CHOICE = 2u32;
pub const WTD_CHOICE_CERT: WINTRUST_DATA_UNION_CHOICE = 5u32;
pub const WTD_CHOICE_FILE: WINTRUST_DATA_UNION_CHOICE = 1u32;
pub const WTD_CHOICE_SIGNER: WINTRUST_DATA_UNION_CHOICE = 4u32;
pub const WTD_CODE_INTEGRITY_DRIVER_MODE: u32 = 32768u32;
pub const WTD_DISABLE_MD2_MD4: WINTRUST_DATA_PROVIDER_FLAGS = 8192u32;
pub const WTD_HASH_ONLY_FLAG: WINTRUST_DATA_PROVIDER_FLAGS = 512u32;
pub const WTD_LIFETIME_SIGNING_FLAG: WINTRUST_DATA_PROVIDER_FLAGS = 2048u32;
pub const WTD_MOTW: WINTRUST_DATA_PROVIDER_FLAGS = 16384u32;
pub const WTD_NO_IE4_CHAIN_FLAG: WINTRUST_DATA_PROVIDER_FLAGS = 2u32;
pub const WTD_NO_POLICY_USAGE_FLAG: WINTRUST_DATA_PROVIDER_FLAGS = 4u32;
pub const WTD_PROV_FLAGS_MASK: u32 = 65535u32;
pub const WTD_REVOCATION_CHECK_CHAIN: WINTRUST_DATA_PROVIDER_FLAGS = 64u32;
pub const WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT: WINTRUST_DATA_PROVIDER_FLAGS = 128u32;
pub const WTD_REVOCATION_CHECK_END_CERT: WINTRUST_DATA_PROVIDER_FLAGS = 32u32;
pub const WTD_REVOCATION_CHECK_NONE: WINTRUST_DATA_PROVIDER_FLAGS = 16u32;
pub const WTD_REVOKE_NONE: WINTRUST_DATA_REVOCATION_CHECKS = 0u32;
pub const WTD_REVOKE_WHOLECHAIN: WINTRUST_DATA_REVOCATION_CHECKS = 1u32;
pub const WTD_SAFER_FLAG: WINTRUST_DATA_PROVIDER_FLAGS = 256u32;
pub const WTD_STATEACTION_AUTO_CACHE: WINTRUST_DATA_STATE_ACTION = 3u32;
pub const WTD_STATEACTION_AUTO_CACHE_FLUSH: WINTRUST_DATA_STATE_ACTION = 4u32;
pub const WTD_STATEACTION_CLOSE: WINTRUST_DATA_STATE_ACTION = 2u32;
pub const WTD_STATEACTION_IGNORE: WINTRUST_DATA_STATE_ACTION = 0u32;
pub const WTD_STATEACTION_VERIFY: WINTRUST_DATA_STATE_ACTION = 1u32;
pub const WTD_UICONTEXT_EXECUTE: WINTRUST_DATA_UICONTEXT = 0u32;
pub const WTD_UICONTEXT_INSTALL: WINTRUST_DATA_UICONTEXT = 1u32;
pub const WTD_UI_ALL: WINTRUST_DATA_UICHOICE = 1u32;
pub const WTD_UI_NOBAD: WINTRUST_DATA_UICHOICE = 3u32;
pub const WTD_UI_NOGOOD: WINTRUST_DATA_UICHOICE = 4u32;
pub const WTD_UI_NONE: WINTRUST_DATA_UICHOICE = 2u32;
pub const WTD_USE_DEFAULT_OSVER_CHECK: WINTRUST_DATA_PROVIDER_FLAGS = 1024u32;
pub const WTD_USE_IE4_TRUST_FLAG: WINTRUST_DATA_PROVIDER_FLAGS = 1u32;
pub const WTPF_ALLOWONLYPERTRUST: WINTRUST_POLICY_FLAGS = 262144u32;
pub const WTPF_IGNOREEXPIRATION: WINTRUST_POLICY_FLAGS = 256u32;
pub const WTPF_IGNOREREVOCATIONONTS: WINTRUST_POLICY_FLAGS = 131072u32;
pub const WTPF_IGNOREREVOKATION: WINTRUST_POLICY_FLAGS = 512u32;
pub const WTPF_OFFLINEOKNBU_COM: WINTRUST_POLICY_FLAGS = 8192u32;
pub const WTPF_OFFLINEOKNBU_IND: WINTRUST_POLICY_FLAGS = 4096u32;
pub const WTPF_OFFLINEOK_COM: WINTRUST_POLICY_FLAGS = 2048u32;
pub const WTPF_OFFLINEOK_IND: WINTRUST_POLICY_FLAGS = 1024u32;
pub const WTPF_TESTCANBEVALID: WINTRUST_POLICY_FLAGS = 128u32;
pub const WTPF_TRUSTTEST: WINTRUST_POLICY_FLAGS = 32u32;
pub const WTPF_VERIFY_V1_OFF: WINTRUST_POLICY_FLAGS = 65536u32;
pub const WT_ADD_ACTION_ID_RET_RESULT_FLAG: u32 = 1u32;
pub const WT_CURRENT_VERSION: u32 = 512u32;
pub const WT_PROVIDER_CERTTRUST_FUNCTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WintrustCertificateTrust");
pub const WT_PROVIDER_DLL_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WINTRUST.DLL");
pub const WT_TRUSTDBDIALOG_NO_UI_FLAG: u32 = 1u32;
pub const WT_TRUSTDBDIALOG_ONLY_PUB_TAB_FLAG: u32 = 2u32;
pub const WT_TRUSTDBDIALOG_WRITE_IEAK_STORE_FLAG: u32 = 512u32;
pub const WT_TRUSTDBDIALOG_WRITE_LEGACY_REG_FLAG: u32 = 256u32;
pub const szOID_ENHANCED_HASH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.5.1");
pub const szOID_INTENT_TO_SEAL: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.4.2");
pub const szOID_NESTED_SIGNATURE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.4.1");
pub const szOID_PKCS_9_SEQUENCE_NUMBER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113549.1.9.25.4");
pub const szOID_SEALING_SIGNATURE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.4.3");
pub const szOID_SEALING_TIMESTAMP: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.4.4");
pub const szOID_TRUSTED_CLIENT_AUTH_CA_LIST: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.2.2");
pub const szOID_TRUSTED_CODESIGNING_CA_LIST: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.2.1");
pub const szOID_TRUSTED_SERVER_AUTH_CA_LIST: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.2.2.3");
pub type WINTRUST_DATA_PROVIDER_FLAGS = u32;
pub type WINTRUST_DATA_REVOCATION_CHECKS = u32;
pub type WINTRUST_DATA_STATE_ACTION = u32;
pub type WINTRUST_DATA_UICHOICE = u32;
pub type WINTRUST_DATA_UICONTEXT = u32;
pub type WINTRUST_DATA_UNION_CHOICE = u32;
pub type WINTRUST_GET_DEFAULT_FOR_USAGE_ACTION = u32;
pub type WINTRUST_POLICY_FLAGS = u32;
pub type WINTRUST_SIGNATURE_SETTINGS_FLAGS = u32;
#[repr(C)]
pub struct CAT_MEMBERINFO {
    pub pwszSubjGuid: ::windows_sys::core::PWSTR,
    pub dwCertVersion: u32,
}
impl ::core::marker::Copy for CAT_MEMBERINFO {}
impl ::core::clone::Clone for CAT_MEMBERINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CAT_MEMBERINFO2 {
    pub SubjectGuid: ::windows_sys::core::GUID,
    pub dwCertVersion: u32,
}
impl ::core::marker::Copy for CAT_MEMBERINFO2 {}
impl ::core::clone::Clone for CAT_MEMBERINFO2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct CAT_NAMEVALUE {
    pub pwszTag: ::windows_sys::core::PWSTR,
    pub fdwFlags: u32,
    pub Value: super::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for CAT_NAMEVALUE {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for CAT_NAMEVALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct CONFIG_CI_PROV_INFO {
    pub cbSize: u32,
    pub dwPolicies: u32,
    pub pPolicies: *mut super::Cryptography::CRYPT_INTEGER_BLOB,
    pub result: CONFIG_CI_PROV_INFO_RESULT,
    pub dwScenario: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for CONFIG_CI_PROV_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for CONFIG_CI_PROV_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CONFIG_CI_PROV_INFO_RESULT {
    pub hr: ::windows_sys::core::HRESULT,
    pub dwResult: u32,
    pub dwPolicyIndex: u32,
    pub fIsExplicitDeny: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CONFIG_CI_PROV_INFO_RESULT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CONFIG_CI_PROV_INFO_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct CRYPT_PROVIDER_CERT {
    pub cbStruct: u32,
    pub pCert: *const super::Cryptography::CERT_CONTEXT,
    pub fCommercial: super::super::Foundation::BOOL,
    pub fTrustedRoot: super::super::Foundation::BOOL,
    pub fSelfSigned: super::super::Foundation::BOOL,
    pub fTestCert: super::super::Foundation::BOOL,
    pub dwRevokedReason: u32,
    pub dwConfidence: u32,
    pub dwError: u32,
    pub pTrustListContext: *mut super::Cryptography::CTL_CONTEXT,
    pub fTrustListSignerCert: super::super::Foundation::BOOL,
    pub pCtlContext: *mut super::Cryptography::CTL_CONTEXT,
    pub dwCtlError: u32,
    pub fIsCyclic: super::super::Foundation::BOOL,
    pub pChainElement: *mut super::Cryptography::CERT_CHAIN_ELEMENT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for CRYPT_PROVIDER_CERT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for CRYPT_PROVIDER_CERT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub struct CRYPT_PROVIDER_DATA {
    pub cbStruct: u32,
    pub pWintrustData: *mut WINTRUST_DATA,
    pub fOpenedFile: super::super::Foundation::BOOL,
    pub hWndParent: super::super::Foundation::HWND,
    pub pgActionID: *mut ::windows_sys::core::GUID,
    pub hProv: usize,
    pub dwError: u32,
    pub dwRegSecuritySettings: u32,
    pub dwRegPolicySettings: u32,
    pub psPfns: *mut CRYPT_PROVIDER_FUNCTIONS,
    pub cdwTrustStepErrors: u32,
    pub padwTrustStepErrors: *mut u32,
    pub chStores: u32,
    pub pahStores: *mut super::Cryptography::HCERTSTORE,
    pub dwEncoding: u32,
    pub hMsg: *mut ::core::ffi::c_void,
    pub csSigners: u32,
    pub pasSigners: *mut CRYPT_PROVIDER_SGNR,
    pub csProvPrivData: u32,
    pub pasProvPrivData: *mut CRYPT_PROVIDER_PRIVDATA,
    pub dwSubjectChoice: u32,
    pub Anonymous: CRYPT_PROVIDER_DATA_0,
    pub pszUsageOID: ::windows_sys::core::PSTR,
    pub fRecallWithState: super::super::Foundation::BOOL,
    pub sftSystemTime: super::super::Foundation::FILETIME,
    pub pszCTLSignerUsageOID: ::windows_sys::core::PSTR,
    pub dwProvFlags: u32,
    pub dwFinalError: u32,
    pub pRequestUsage: *mut super::Cryptography::CERT_USAGE_MATCH,
    pub dwTrustPubSettings: u32,
    pub dwUIStateFlags: u32,
    pub pSigState: *mut CRYPT_PROVIDER_SIGSTATE,
    pub pSigSettings: *mut WINTRUST_SIGNATURE_SETTINGS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for CRYPT_PROVIDER_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for CRYPT_PROVIDER_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub union CRYPT_PROVIDER_DATA_0 {
    pub pPDSip: *mut PROVDATA_SIP,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for CRYPT_PROVIDER_DATA_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for CRYPT_PROVIDER_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CRYPT_PROVIDER_DEFUSAGE {
    pub cbStruct: u32,
    pub gActionID: ::windows_sys::core::GUID,
    pub pDefPolicyCallbackData: *mut ::core::ffi::c_void,
    pub pDefSIPClientData: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for CRYPT_PROVIDER_DEFUSAGE {}
impl ::core::clone::Clone for CRYPT_PROVIDER_DEFUSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub struct CRYPT_PROVIDER_FUNCTIONS {
    pub cbStruct: u32,
    pub pfnAlloc: PFN_CPD_MEM_ALLOC,
    pub pfnFree: PFN_CPD_MEM_FREE,
    pub pfnAddStore2Chain: PFN_CPD_ADD_STORE,
    pub pfnAddSgnr2Chain: PFN_CPD_ADD_SGNR,
    pub pfnAddCert2Chain: PFN_CPD_ADD_CERT,
    pub pfnAddPrivData2Chain: PFN_CPD_ADD_PRIVDATA,
    pub pfnInitialize: PFN_PROVIDER_INIT_CALL,
    pub pfnObjectTrust: PFN_PROVIDER_OBJTRUST_CALL,
    pub pfnSignatureTrust: PFN_PROVIDER_SIGTRUST_CALL,
    pub pfnCertificateTrust: PFN_PROVIDER_CERTTRUST_CALL,
    pub pfnFinalPolicy: PFN_PROVIDER_FINALPOLICY_CALL,
    pub pfnCertCheckPolicy: PFN_PROVIDER_CERTCHKPOLICY_CALL,
    pub pfnTestFinalPolicy: PFN_PROVIDER_TESTFINALPOLICY_CALL,
    pub psUIpfns: *mut CRYPT_PROVUI_FUNCS,
    pub pfnCleanupPolicy: PFN_PROVIDER_CLEANUP_CALL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for CRYPT_PROVIDER_FUNCTIONS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for CRYPT_PROVIDER_FUNCTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CRYPT_PROVIDER_PRIVDATA {
    pub cbStruct: u32,
    pub gProviderID: ::windows_sys::core::GUID,
    pub cbProvData: u32,
    pub pvProvData: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for CRYPT_PROVIDER_PRIVDATA {}
impl ::core::clone::Clone for CRYPT_PROVIDER_PRIVDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CRYPT_PROVIDER_REGDEFUSAGE {
    pub cbStruct: u32,
    pub pgActionID: *mut ::windows_sys::core::GUID,
    pub pwszDllName: ::windows_sys::core::PWSTR,
    pub pwszLoadCallbackDataFunctionName: ::windows_sys::core::PSTR,
    pub pwszFreeCallbackDataFunctionName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for CRYPT_PROVIDER_REGDEFUSAGE {}
impl ::core::clone::Clone for CRYPT_PROVIDER_REGDEFUSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct CRYPT_PROVIDER_SGNR {
    pub cbStruct: u32,
    pub sftVerifyAsOf: super::super::Foundation::FILETIME,
    pub csCertChain: u32,
    pub pasCertChain: *mut CRYPT_PROVIDER_CERT,
    pub dwSignerType: u32,
    pub psSigner: *mut super::Cryptography::CMSG_SIGNER_INFO,
    pub dwError: u32,
    pub csCounterSigners: u32,
    pub pasCounterSigners: *mut CRYPT_PROVIDER_SGNR,
    pub pChainContext: *mut super::Cryptography::CERT_CHAIN_CONTEXT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for CRYPT_PROVIDER_SGNR {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for CRYPT_PROVIDER_SGNR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct CRYPT_PROVIDER_SIGSTATE {
    pub cbStruct: u32,
    pub rhSecondarySigs: *mut *mut ::core::ffi::c_void,
    pub hPrimarySig: *mut ::core::ffi::c_void,
    pub fFirstAttemptMade: super::super::Foundation::BOOL,
    pub fNoMoreSigs: super::super::Foundation::BOOL,
    pub cSecondarySigs: u32,
    pub dwCurrentIndex: u32,
    pub fSupportMultiSig: super::super::Foundation::BOOL,
    pub dwCryptoPolicySupport: u32,
    pub iAttemptCount: u32,
    pub fCheckedSealing: super::super::Foundation::BOOL,
    pub pSealingSignature: *mut SEALING_SIGNATURE_ATTRIBUTE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for CRYPT_PROVIDER_SIGSTATE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for CRYPT_PROVIDER_SIGSTATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CRYPT_PROVUI_DATA {
    pub cbStruct: u32,
    pub dwFinalError: u32,
    pub pYesButtonText: ::windows_sys::core::PWSTR,
    pub pNoButtonText: ::windows_sys::core::PWSTR,
    pub pMoreInfoButtonText: ::windows_sys::core::PWSTR,
    pub pAdvancedLinkText: ::windows_sys::core::PWSTR,
    pub pCopyActionText: ::windows_sys::core::PWSTR,
    pub pCopyActionTextNoTS: ::windows_sys::core::PWSTR,
    pub pCopyActionTextNotSigned: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for CRYPT_PROVUI_DATA {}
impl ::core::clone::Clone for CRYPT_PROVUI_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub struct CRYPT_PROVUI_FUNCS {
    pub cbStruct: u32,
    pub psUIData: *mut CRYPT_PROVUI_DATA,
    pub pfnOnMoreInfoClick: PFN_PROVUI_CALL,
    pub pfnOnMoreInfoClickDefault: PFN_PROVUI_CALL,
    pub pfnOnAdvancedClick: PFN_PROVUI_CALL,
    pub pfnOnAdvancedClickDefault: PFN_PROVUI_CALL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for CRYPT_PROVUI_FUNCS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for CRYPT_PROVUI_FUNCS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CRYPT_REGISTER_ACTIONID {
    pub cbStruct: u32,
    pub sInitProvider: CRYPT_TRUST_REG_ENTRY,
    pub sObjectProvider: CRYPT_TRUST_REG_ENTRY,
    pub sSignatureProvider: CRYPT_TRUST_REG_ENTRY,
    pub sCertificateProvider: CRYPT_TRUST_REG_ENTRY,
    pub sCertificatePolicyProvider: CRYPT_TRUST_REG_ENTRY,
    pub sFinalPolicyProvider: CRYPT_TRUST_REG_ENTRY,
    pub sTestPolicyProvider: CRYPT_TRUST_REG_ENTRY,
    pub sCleanupProvider: CRYPT_TRUST_REG_ENTRY,
}
impl ::core::marker::Copy for CRYPT_REGISTER_ACTIONID {}
impl ::core::clone::Clone for CRYPT_REGISTER_ACTIONID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CRYPT_TRUST_REG_ENTRY {
    pub cbStruct: u32,
    pub pwszDLLName: ::windows_sys::core::PWSTR,
    pub pwszFunctionName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for CRYPT_TRUST_REG_ENTRY {}
impl ::core::clone::Clone for CRYPT_TRUST_REG_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct DRIVER_VER_INFO {
    pub cbStruct: u32,
    pub dwReserved1: usize,
    pub dwReserved2: usize,
    pub dwPlatform: u32,
    pub dwVersion: u32,
    pub wszVersion: [u16; 260],
    pub wszSignedBy: [u16; 260],
    pub pcSignerCertContext: *const super::Cryptography::CERT_CONTEXT,
    pub sOSVersionLow: DRIVER_VER_MAJORMINOR,
    pub sOSVersionHigh: DRIVER_VER_MAJORMINOR,
    pub dwBuildNumberLow: u32,
    pub dwBuildNumberHigh: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for DRIVER_VER_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for DRIVER_VER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRIVER_VER_MAJORMINOR {
    pub dwMajor: u32,
    pub dwMinor: u32,
}
impl ::core::marker::Copy for DRIVER_VER_MAJORMINOR {}
impl ::core::clone::Clone for DRIVER_VER_MAJORMINOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct INTENT_TO_SEAL_ATTRIBUTE {
    pub version: u32,
    pub seal: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for INTENT_TO_SEAL_ATTRIBUTE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for INTENT_TO_SEAL_ATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub struct PROVDATA_SIP {
    pub cbStruct: u32,
    pub gSubject: ::windows_sys::core::GUID,
    pub pSip: *mut super::Cryptography::Sip::SIP_DISPATCH_INFO,
    pub pCATSip: *mut super::Cryptography::Sip::SIP_DISPATCH_INFO,
    pub psSipSubjectInfo: *mut super::Cryptography::Sip::SIP_SUBJECTINFO,
    pub psSipCATSubjectInfo: *mut super::Cryptography::Sip::SIP_SUBJECTINFO,
    pub psIndirectData: *mut super::Cryptography::Sip::SIP_INDIRECT_DATA,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for PROVDATA_SIP {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for PROVDATA_SIP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SEALING_SIGNATURE_ATTRIBUTE {
    pub version: u32,
    pub signerIndex: u32,
    pub signatureAlgorithm: super::Cryptography::CRYPT_ALGORITHM_IDENTIFIER,
    pub encryptedDigest: super::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SEALING_SIGNATURE_ATTRIBUTE {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SEALING_SIGNATURE_ATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SEALING_TIMESTAMP_ATTRIBUTE {
    pub version: u32,
    pub signerIndex: u32,
    pub sealTimeStampToken: super::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SEALING_TIMESTAMP_ATTRIBUTE {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SEALING_TIMESTAMP_ATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SPC_FINANCIAL_CRITERIA {
    pub fFinancialInfoAvailable: super::super::Foundation::BOOL,
    pub fMeetsCriteria: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SPC_FINANCIAL_CRITERIA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SPC_FINANCIAL_CRITERIA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SPC_IMAGE {
    pub pImageLink: *mut SPC_LINK,
    pub Bitmap: super::Cryptography::CRYPT_INTEGER_BLOB,
    pub Metafile: super::Cryptography::CRYPT_INTEGER_BLOB,
    pub EnhancedMetafile: super::Cryptography::CRYPT_INTEGER_BLOB,
    pub GifFile: super::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_IMAGE {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_IMAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SPC_INDIRECT_DATA_CONTENT {
    pub Data: super::Cryptography::CRYPT_ATTRIBUTE_TYPE_VALUE,
    pub DigestAlgorithm: super::Cryptography::CRYPT_ALGORITHM_IDENTIFIER,
    pub Digest: super::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_INDIRECT_DATA_CONTENT {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_INDIRECT_DATA_CONTENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SPC_LINK {
    pub dwLinkChoice: u32,
    pub Anonymous: SPC_LINK_0,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_LINK {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_LINK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub union SPC_LINK_0 {
    pub pwszUrl: ::windows_sys::core::PWSTR,
    pub Moniker: SPC_SERIALIZED_OBJECT,
    pub pwszFile: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_LINK_0 {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_LINK_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SPC_PE_IMAGE_DATA {
    pub Flags: super::Cryptography::CRYPT_BIT_BLOB,
    pub pFile: *mut SPC_LINK,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_PE_IMAGE_DATA {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_PE_IMAGE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SPC_SERIALIZED_OBJECT {
    pub ClassId: [u8; 16],
    pub SerializedData: super::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_SERIALIZED_OBJECT {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_SERIALIZED_OBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SPC_SIGINFO {
    pub dwSipVersion: u32,
    pub gSIPGuid: ::windows_sys::core::GUID,
    pub dwReserved1: u32,
    pub dwReserved2: u32,
    pub dwReserved3: u32,
    pub dwReserved4: u32,
    pub dwReserved5: u32,
}
impl ::core::marker::Copy for SPC_SIGINFO {}
impl ::core::clone::Clone for SPC_SIGINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SPC_SP_AGENCY_INFO {
    pub pPolicyInformation: *mut SPC_LINK,
    pub pwszPolicyDisplayText: ::windows_sys::core::PWSTR,
    pub pLogoImage: *mut SPC_IMAGE,
    pub pLogoLink: *mut SPC_LINK,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_SP_AGENCY_INFO {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_SP_AGENCY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct SPC_SP_OPUS_INFO {
    pub pwszProgramName: ::windows_sys::core::PCWSTR,
    pub pMoreInfo: *mut SPC_LINK,
    pub pPublisherInfo: *mut SPC_LINK,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for SPC_SP_OPUS_INFO {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for SPC_SP_OPUS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SPC_STATEMENT_TYPE {
    pub cKeyPurposeId: u32,
    pub rgpszKeyPurposeId: *mut ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for SPC_STATEMENT_TYPE {}
impl ::core::clone::Clone for SPC_STATEMENT_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WINTRUST_BLOB_INFO {
    pub cbStruct: u32,
    pub gSubject: ::windows_sys::core::GUID,
    pub pcwszDisplayName: ::windows_sys::core::PCWSTR,
    pub cbMemObject: u32,
    pub pbMemObject: *mut u8,
    pub cbMemSignedMsg: u32,
    pub pbMemSignedMsg: *mut u8,
}
impl ::core::marker::Copy for WINTRUST_BLOB_INFO {}
impl ::core::clone::Clone for WINTRUST_BLOB_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct WINTRUST_CATALOG_INFO {
    pub cbStruct: u32,
    pub dwCatalogVersion: u32,
    pub pcwszCatalogFilePath: ::windows_sys::core::PCWSTR,
    pub pcwszMemberTag: ::windows_sys::core::PCWSTR,
    pub pcwszMemberFilePath: ::windows_sys::core::PCWSTR,
    pub hMemberFile: super::super::Foundation::HANDLE,
    pub pbCalculatedFileHash: *mut u8,
    pub cbCalculatedFileHash: u32,
    pub pcCatalogContext: *mut super::Cryptography::CTL_CONTEXT,
    pub hCatAdmin: isize,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WINTRUST_CATALOG_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WINTRUST_CATALOG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct WINTRUST_CERT_INFO {
    pub cbStruct: u32,
    pub pcwszDisplayName: ::windows_sys::core::PCWSTR,
    pub psCertContext: *mut super::Cryptography::CERT_CONTEXT,
    pub chStores: u32,
    pub pahStores: *mut super::Cryptography::HCERTSTORE,
    pub dwFlags: u32,
    pub psftVerifyAsOf: *mut super::super::Foundation::FILETIME,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WINTRUST_CERT_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WINTRUST_CERT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct WINTRUST_DATA {
    pub cbStruct: u32,
    pub pPolicyCallbackData: *mut ::core::ffi::c_void,
    pub pSIPClientData: *mut ::core::ffi::c_void,
    pub dwUIChoice: WINTRUST_DATA_UICHOICE,
    pub fdwRevocationChecks: WINTRUST_DATA_REVOCATION_CHECKS,
    pub dwUnionChoice: WINTRUST_DATA_UNION_CHOICE,
    pub Anonymous: WINTRUST_DATA_0,
    pub dwStateAction: WINTRUST_DATA_STATE_ACTION,
    pub hWVTStateData: super::super::Foundation::HANDLE,
    pub pwszURLReference: ::windows_sys::core::PWSTR,
    pub dwProvFlags: WINTRUST_DATA_PROVIDER_FLAGS,
    pub dwUIContext: WINTRUST_DATA_UICONTEXT,
    pub pSignatureSettings: *mut WINTRUST_SIGNATURE_SETTINGS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WINTRUST_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WINTRUST_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub union WINTRUST_DATA_0 {
    pub pFile: *mut WINTRUST_FILE_INFO,
    pub pCatalog: *mut WINTRUST_CATALOG_INFO,
    pub pBlob: *mut WINTRUST_BLOB_INFO,
    pub pSgnr: *mut WINTRUST_SGNR_INFO,
    pub pCert: *mut WINTRUST_CERT_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WINTRUST_DATA_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WINTRUST_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WINTRUST_FILE_INFO {
    pub cbStruct: u32,
    pub pcwszFilePath: ::windows_sys::core::PCWSTR,
    pub hFile: super::super::Foundation::HANDLE,
    pub pgKnownSubject: *mut ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WINTRUST_FILE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WINTRUST_FILE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct WINTRUST_SGNR_INFO {
    pub cbStruct: u32,
    pub pcwszDisplayName: ::windows_sys::core::PCWSTR,
    pub psSignerInfo: *mut super::Cryptography::CMSG_SIGNER_INFO,
    pub chStores: u32,
    pub pahStores: *mut super::Cryptography::HCERTSTORE,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for WINTRUST_SGNR_INFO {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for WINTRUST_SGNR_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct WINTRUST_SIGNATURE_SETTINGS {
    pub cbStruct: u32,
    pub dwIndex: u32,
    pub dwFlags: WINTRUST_SIGNATURE_SETTINGS_FLAGS,
    pub cSecondarySigs: u32,
    pub dwVerifiedSigIndex: u32,
    pub pCryptoPolicy: *mut super::Cryptography::CERT_STRONG_SIGN_PARA,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for WINTRUST_SIGNATURE_SETTINGS {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for WINTRUST_SIGNATURE_SETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WIN_CERTIFICATE {
    pub dwLength: u32,
    pub wRevision: u16,
    pub wCertificateType: u16,
    pub bCertificate: [u8; 1],
}
impl ::core::marker::Copy for WIN_CERTIFICATE {}
impl ::core::clone::Clone for WIN_CERTIFICATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WIN_SPUB_TRUSTED_PUBLISHER_DATA {
    pub hClientToken: super::super::Foundation::HANDLE,
    pub lpCertificate: *mut WIN_CERTIFICATE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WIN_SPUB_TRUSTED_PUBLISHER_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WIN_SPUB_TRUSTED_PUBLISHER_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WIN_TRUST_ACTDATA_CONTEXT_WITH_SUBJECT {
    pub hClientToken: super::super::Foundation::HANDLE,
    pub SubjectType: *mut ::windows_sys::core::GUID,
    pub Subject: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WIN_TRUST_ACTDATA_CONTEXT_WITH_SUBJECT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WIN_TRUST_ACTDATA_CONTEXT_WITH_SUBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WIN_TRUST_ACTDATA_SUBJECT_ONLY {
    pub SubjectType: *mut ::windows_sys::core::GUID,
    pub Subject: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for WIN_TRUST_ACTDATA_SUBJECT_ONLY {}
impl ::core::clone::Clone for WIN_TRUST_ACTDATA_SUBJECT_ONLY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WIN_TRUST_SUBJECT_FILE {
    pub hFile: super::super::Foundation::HANDLE,
    pub lpPath: ::windows_sys::core::PCWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WIN_TRUST_SUBJECT_FILE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WIN_TRUST_SUBJECT_FILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WIN_TRUST_SUBJECT_FILE_AND_DISPLAY {
    pub hFile: super::super::Foundation::HANDLE,
    pub lpPath: ::windows_sys::core::PCWSTR,
    pub lpDisplayName: ::windows_sys::core::PCWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WIN_TRUST_SUBJECT_FILE_AND_DISPLAY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WIN_TRUST_SUBJECT_FILE_AND_DISPLAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct WTD_GENERIC_CHAIN_POLICY_CREATE_INFO {
    pub Anonymous: WTD_GENERIC_CHAIN_POLICY_CREATE_INFO_0,
    pub hChainEngine: super::Cryptography::HCERTCHAINENGINE,
    pub pChainPara: *mut super::Cryptography::CERT_CHAIN_PARA,
    pub dwFlags: u32,
    pub pvReserved: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for WTD_GENERIC_CHAIN_POLICY_CREATE_INFO {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for WTD_GENERIC_CHAIN_POLICY_CREATE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub union WTD_GENERIC_CHAIN_POLICY_CREATE_INFO_0 {
    pub cbStruct: u32,
    pub cbSize: u32,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for WTD_GENERIC_CHAIN_POLICY_CREATE_INFO_0 {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for WTD_GENERIC_CHAIN_POLICY_CREATE_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub struct WTD_GENERIC_CHAIN_POLICY_DATA {
    pub Anonymous: WTD_GENERIC_CHAIN_POLICY_DATA_0,
    pub pSignerChainInfo: *mut WTD_GENERIC_CHAIN_POLICY_CREATE_INFO,
    pub pCounterSignerChainInfo: *mut WTD_GENERIC_CHAIN_POLICY_CREATE_INFO,
    pub pfnPolicyCallback: PFN_WTD_GENERIC_CHAIN_POLICY_CALLBACK,
    pub pvPolicyArg: *mut ::core::ffi::c_void,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for WTD_GENERIC_CHAIN_POLICY_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for WTD_GENERIC_CHAIN_POLICY_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub union WTD_GENERIC_CHAIN_POLICY_DATA_0 {
    pub cbStruct: u32,
    pub cbSize: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for WTD_GENERIC_CHAIN_POLICY_DATA_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for WTD_GENERIC_CHAIN_POLICY_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO {
    pub Anonymous: WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO_0,
    pub pChainContext: *mut super::Cryptography::CERT_CHAIN_CONTEXT,
    pub dwSignerType: u32,
    pub pMsgSignerInfo: *mut super::Cryptography::CMSG_SIGNER_INFO,
    pub dwError: u32,
    pub cCounterSigner: u32,
    pub rgpCounterSigner: *mut *mut WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub union WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO_0 {
    pub cbStruct: u32,
    pub cbSize: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_ALLOCANDFILLDEFUSAGE = ::core::option::Option<unsafe extern "system" fn(pszusageoid: ::windows_sys::core::PCSTR, psdefusage: *const CRYPT_PROVIDER_DEFUSAGE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_CPD_ADD_CERT = ::core::option::Option<unsafe extern "system" fn(pprovdata: *const CRYPT_PROVIDER_DATA, idxsigner: u32, fcountersigner: super::super::Foundation::BOOL, idxcountersigner: u32, pcert2add: *const super::Cryptography::CERT_CONTEXT) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_CPD_ADD_PRIVDATA = ::core::option::Option<unsafe extern "system" fn(pprovdata: *const CRYPT_PROVIDER_DATA, pprivdata2add: *const CRYPT_PROVIDER_PRIVDATA) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_CPD_ADD_SGNR = ::core::option::Option<unsafe extern "system" fn(pprovdata: *const CRYPT_PROVIDER_DATA, fcountersigner: super::super::Foundation::BOOL, idxsigner: u32, psgnr2add: *const CRYPT_PROVIDER_SGNR) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_CPD_ADD_STORE = ::core::option::Option<unsafe extern "system" fn(pprovdata: *const CRYPT_PROVIDER_DATA, hstore2add: super::Cryptography::HCERTSTORE) -> super::super::Foundation::BOOL>;
pub type PFN_CPD_MEM_ALLOC = ::core::option::Option<unsafe extern "system" fn(cbsize: u32) -> *mut ::core::ffi::c_void>;
pub type PFN_CPD_MEM_FREE = ::core::option::Option<unsafe extern "system" fn(pvmem2free: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_FREEDEFUSAGE = ::core::option::Option<unsafe extern "system" fn(pszusageoid: ::windows_sys::core::PCSTR, psdefusage: *const CRYPT_PROVIDER_DEFUSAGE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_CERTCHKPOLICY_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *const CRYPT_PROVIDER_DATA, idxsigner: u32, fcountersignerchain: super::super::Foundation::BOOL, idxcountersigner: u32) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_CERTTRUST_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_CLEANUP_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_FINALPOLICY_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_INIT_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_OBJTRUST_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_SIGTRUST_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVIDER_TESTFINALPOLICY_CALL = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_PROVUI_CALL = ::core::option::Option<unsafe extern "system" fn(hwndsecuritydialog: super::super::Foundation::HWND, pprovdata: *const CRYPT_PROVIDER_DATA) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Catalog\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Catalog", feature = "Win32_Security_Cryptography_Sip"))]
pub type PFN_WTD_GENERIC_CHAIN_POLICY_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pprovdata: *mut CRYPT_PROVIDER_DATA, dwsteperror: u32, dwregpolicysettings: u32, csigner: u32, rgpsigner: *mut *mut WTD_GENERIC_CHAIN_POLICY_SIGNER_INFO, pvpolicyarg: *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
