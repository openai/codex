#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappprxy.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerBeginSession(dwflags : u32, eaptype : EAP_METHOD_TYPE, pattributearray : *const EAP_ATTRIBUTES, htokenimpersonateuser : super::super::Foundation:: HANDLE, dwsizeofconnectiondata : u32, pconnectiondata : *const u8, dwsizeofuserdata : u32, puserdata : *const u8, dwmaxsendpacketsize : u32, pconnectionid : *const ::windows_sys::core::GUID, func : NotificationHandler, pcontextdata : *mut ::core::ffi::c_void, psessionid : *mut u32, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerClearConnection(pconnectionid : *mut ::windows_sys::core::GUID, ppeaperror : *mut *mut EAP_ERROR) -> u32);
#[cfg(all(feature = "Win32_Data_Xml_MsXml", feature = "Win32_System_Com"))]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Data_Xml_MsXml\"`, `\"Win32_System_Com\"`"] fn EapHostPeerConfigBlob2Xml(dwflags : u32, eapmethodtype : EAP_METHOD_TYPE, dwsizeofconfigin : u32, pconfigin : *const u8, ppconfigdoc : *mut super::super::Data::Xml::MsXml:: IXMLDOMDocument2, ppeaperror : *mut *mut EAP_ERROR) -> u32);
#[cfg(all(feature = "Win32_Data_Xml_MsXml", feature = "Win32_System_Com"))]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Data_Xml_MsXml\"`, `\"Win32_System_Com\"`"] fn EapHostPeerConfigXml2Blob(dwflags : u32, pconfigdoc : super::super::Data::Xml::MsXml:: IXMLDOMNode, pdwsizeofconfigout : *mut u32, ppconfigout : *mut *mut u8, peapmethodtype : *mut EAP_METHOD_TYPE, ppeaperror : *mut *mut EAP_ERROR) -> u32);
#[cfg(all(feature = "Win32_Data_Xml_MsXml", feature = "Win32_System_Com"))]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Data_Xml_MsXml\"`, `\"Win32_System_Com\"`"] fn EapHostPeerCredentialsXml2Blob(dwflags : u32, pcredentialsdoc : super::super::Data::Xml::MsXml:: IXMLDOMNode, dwsizeofconfigin : u32, pconfigin : *const u8, pdwsizeofcredentialsout : *mut u32, ppcredentialsout : *mut *mut u8, peapmethodtype : *mut EAP_METHOD_TYPE, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerEndSession(sessionhandle : u32, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerFreeEapError(peaperror : *mut EAP_ERROR) -> ());
::windows_targets::link!("eappcfg.dll" "system" fn EapHostPeerFreeErrorMemory(peaperror : *mut EAP_ERROR) -> ());
::windows_targets::link!("eappcfg.dll" "system" fn EapHostPeerFreeMemory(pdata : *mut u8) -> ());
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerFreeRuntimeMemory(pdata : *mut u8) -> ());
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerGetAuthStatus(sessionhandle : u32, authparam : EapHostPeerAuthParams, pcbauthdata : *mut u32, ppauthdata : *mut *mut u8, ppeaperror : *mut *mut EAP_ERROR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappprxy.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerGetDataToUnplumbCredentials(pconnectionidthatlastsavedcreds : *mut ::windows_sys::core::GUID, phcredentialimpersonationtoken : *mut isize, sessionhandle : u32, ppeaperror : *mut *mut EAP_ERROR, fsavetocredman : *mut super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerGetEncryptedPassword(dwsizeofpassword : u32, szpassword : ::windows_sys::core::PCWSTR, ppszencpassword : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappprxy.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerGetIdentity(dwversion : u32, dwflags : u32, eapmethodtype : EAP_METHOD_TYPE, dwsizeofconnectiondata : u32, pconnectiondata : *const u8, dwsizeofuserdata : u32, puserdata : *const u8, htokenimpersonateuser : super::super::Foundation:: HANDLE, pfinvokeui : *mut super::super::Foundation:: BOOL, pdwsizeofuserdataout : *mut u32, ppuserdataout : *mut *mut u8, ppwszidentity : *mut ::windows_sys::core::PWSTR, ppeaperror : *mut *mut EAP_ERROR, ppvreserved : *mut *mut u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerGetMethodProperties(dwversion : u32, dwflags : u32, eapmethodtype : EAP_METHOD_TYPE, huserimpersonationtoken : super::super::Foundation:: HANDLE, dweapconndatasize : u32, pbeapconndata : *const u8, dwuserdatasize : u32, pbuserdata : *const u8, pmethodpropertyarray : *mut EAP_METHOD_PROPERTY_ARRAY, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappcfg.dll" "system" fn EapHostPeerGetMethods(peapmethodinfoarray : *mut EAP_METHOD_INFO_ARRAY, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerGetResponseAttributes(sessionhandle : u32, pattribs : *mut EAP_ATTRIBUTES, ppeaperror : *mut *mut EAP_ERROR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappprxy.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerGetResult(sessionhandle : u32, reason : EapHostPeerMethodResultReason, ppresult : *mut EapHostPeerMethodResult, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerGetSendPacket(sessionhandle : u32, pcbsendpacket : *mut u32, ppsendpacket : *mut *mut u8, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerGetUIContext(sessionhandle : u32, pdwsizeofuicontextdata : *mut u32, ppuicontextdata : *mut *mut u8, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerInitialize() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerInvokeConfigUI(hwndparent : super::super::Foundation:: HWND, dwflags : u32, eapmethodtype : EAP_METHOD_TYPE, dwsizeofconfigin : u32, pconfigin : *const u8, pdwsizeofconfigout : *mut u32, ppconfigout : *mut *mut u8, ppeaperror : *mut *mut EAP_ERROR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerInvokeIdentityUI(dwversion : u32, eapmethodtype : EAP_METHOD_TYPE, dwflags : u32, hwndparent : super::super::Foundation:: HWND, dwsizeofconnectiondata : u32, pconnectiondata : *const u8, dwsizeofuserdata : u32, puserdata : *const u8, pdwsizeofuserdataout : *mut u32, ppuserdataout : *mut *mut u8, ppwszidentity : *mut ::windows_sys::core::PWSTR, ppeaperror : *mut *mut EAP_ERROR, ppvreserved : *mut *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerInvokeInteractiveUI(hwndparent : super::super::Foundation:: HWND, dwsizeofuicontextdata : u32, puicontextdata : *const u8, pdwsizeofdatafrominteractiveui : *mut u32, ppdatafrominteractiveui : *mut *mut u8, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerProcessReceivedPacket(sessionhandle : u32, cbreceivepacket : u32, preceivepacket : *const u8, peapoutput : *mut EapHostPeerResponseAction, ppeaperror : *mut *mut EAP_ERROR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerQueryCredentialInputFields(huserimpersonationtoken : super::super::Foundation:: HANDLE, eapmethodtype : EAP_METHOD_TYPE, dwflags : u32, dweapconndatasize : u32, pbeapconndata : *const u8, peapconfiginputfieldarray : *mut EAP_CONFIG_INPUT_FIELD_ARRAY, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappcfg.dll" "system" fn EapHostPeerQueryInteractiveUIInputFields(dwversion : u32, dwflags : u32, dwsizeofuicontextdata : u32, puicontextdata : *const u8, peapinteractiveuidata : *mut EAP_INTERACTIVE_UI_DATA, ppeaperror : *mut *mut EAP_ERROR, ppvreserved : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("eappcfg.dll" "system" fn EapHostPeerQueryUIBlobFromInteractiveUIInputFields(dwversion : u32, dwflags : u32, dwsizeofuicontextdata : u32, puicontextdata : *const u8, peapinteractiveuidata : *const EAP_INTERACTIVE_UI_DATA, pdwsizeofdatafrominteractiveui : *mut u32, ppdatafrominteractiveui : *mut *mut u8, ppeaperror : *mut *mut EAP_ERROR, ppvreserved : *mut *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("eappcfg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EapHostPeerQueryUserBlobFromCredentialInputFields(huserimpersonationtoken : super::super::Foundation:: HANDLE, eapmethodtype : EAP_METHOD_TYPE, dwflags : u32, dweapconndatasize : u32, pbeapconndata : *const u8, peapconfiginputfieldarray : *const EAP_CONFIG_INPUT_FIELD_ARRAY, pdwuserblobsize : *mut u32, ppbuserblob : *mut *mut u8, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerSetResponseAttributes(sessionhandle : u32, pattribs : *const EAP_ATTRIBUTES, peapoutput : *mut EapHostPeerResponseAction, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerSetUIContext(sessionhandle : u32, dwsizeofuicontextdata : u32, puicontextdata : *const u8, peapoutput : *mut EapHostPeerResponseAction, ppeaperror : *mut *mut EAP_ERROR) -> u32);
::windows_targets::link!("eappprxy.dll" "system" fn EapHostPeerUninitialize() -> ());
pub type IAccountingProviderConfig = *mut ::core::ffi::c_void;
pub type IAuthenticationProviderConfig = *mut ::core::ffi::c_void;
pub type IEAPProviderConfig = *mut ::core::ffi::c_void;
pub type IEAPProviderConfig2 = *mut ::core::ffi::c_void;
pub type IEAPProviderConfig3 = *mut ::core::ffi::c_void;
pub type IRouterProtocolConfig = *mut ::core::ffi::c_void;
pub const CERTIFICATE_HASH_LENGTH: u32 = 20u32;
pub const EAPACTION_Authenticate: PPP_EAP_ACTION = 1i32;
pub const EAPACTION_Done: PPP_EAP_ACTION = 2i32;
pub const EAPACTION_IndicateIdentity: PPP_EAP_ACTION = 8i32;
pub const EAPACTION_IndicateTLV: PPP_EAP_ACTION = 7i32;
pub const EAPACTION_NoAction: PPP_EAP_ACTION = 0i32;
pub const EAPACTION_Send: PPP_EAP_ACTION = 4i32;
pub const EAPACTION_SendAndDone: PPP_EAP_ACTION = 3i32;
pub const EAPACTION_SendWithTimeout: PPP_EAP_ACTION = 5i32;
pub const EAPACTION_SendWithTimeoutInteractive: PPP_EAP_ACTION = 6i32;
pub const EAPCODE_Failure: u32 = 4u32;
pub const EAPCODE_Request: u32 = 1u32;
pub const EAPCODE_Response: u32 = 2u32;
pub const EAPCODE_Success: u32 = 3u32;
pub const EAPHOST_METHOD_API_VERSION: u32 = 1u32;
pub const EAPHOST_PEER_API_VERSION: u32 = 1u32;
pub const EAP_AUTHENTICATOR_SEND_TIMEOUT_BASIC: EAP_AUTHENTICATOR_SEND_TIMEOUT = 1i32;
pub const EAP_AUTHENTICATOR_SEND_TIMEOUT_INTERACTIVE: EAP_AUTHENTICATOR_SEND_TIMEOUT = 2i32;
pub const EAP_AUTHENTICATOR_SEND_TIMEOUT_NONE: EAP_AUTHENTICATOR_SEND_TIMEOUT = 0i32;
pub const EAP_AUTHENTICATOR_VALUENAME_CONFIGUI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AuthenticatorConfigUIPath");
pub const EAP_AUTHENTICATOR_VALUENAME_DLL_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AuthenticatorDllPath");
pub const EAP_AUTHENTICATOR_VALUENAME_FRIENDLY_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AuthenticatorFriendlyName");
pub const EAP_AUTHENTICATOR_VALUENAME_PROPERTIES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Properties");
pub const EAP_CERTIFICATE_CREDENTIAL: EapCredentialType = 3i32;
pub const EAP_CONFIG_INPUT_FIELD_PROPS_DEFAULT: u32 = 0u32;
pub const EAP_CONFIG_INPUT_FIELD_PROPS_NON_DISPLAYABLE: u32 = 1u32;
pub const EAP_CONFIG_INPUT_FIELD_PROPS_NON_PERSIST: u32 = 2u32;
pub const EAP_CREDENTIAL_VERSION: u32 = 1u32;
pub const EAP_EMPTY_CREDENTIAL: EapCredentialType = 0i32;
pub const EAP_E_AUTHENTICATION_FAILED: u32 = 2151809045u32;
pub const EAP_E_CERT_STORE_INACCESSIBLE: u32 = 2151809040u32;
pub const EAP_E_EAPHOST_EAPQEC_INACCESSIBLE: u32 = 2151809043u32;
pub const EAP_E_EAPHOST_FIRST: i32 = -2143158272i32;
pub const EAP_E_EAPHOST_IDENTITY_UNKNOWN: u32 = 2151809044u32;
pub const EAP_E_EAPHOST_LAST: i32 = -2143158017i32;
pub const EAP_E_EAPHOST_METHOD_INVALID_PACKET: u32 = 2151809047u32;
pub const EAP_E_EAPHOST_METHOD_NOT_INSTALLED: u32 = 2151809041u32;
pub const EAP_E_EAPHOST_METHOD_OPERATION_NOT_SUPPORTED: u32 = 2151809056u32;
pub const EAP_E_EAPHOST_REMOTE_INVALID_PACKET: u32 = 2151809048u32;
pub const EAP_E_EAPHOST_THIRDPARTY_METHOD_HOST_RESET: u32 = 2151809042u32;
pub const EAP_E_EAPHOST_XML_MALFORMED: u32 = 2151809049u32;
pub const EAP_E_METHOD_CONFIG_DOES_NOT_SUPPORT_SSO: u32 = 2151809050u32;
pub const EAP_E_NO_SMART_CARD_READER: u32 = 2151809299u32;
pub const EAP_E_SERVER_CERT_EXPIRED: u32 = 2151809538u32;
pub const EAP_E_SERVER_CERT_INVALID: u32 = 2151809537u32;
pub const EAP_E_SERVER_CERT_NOT_FOUND: u32 = 2151809536u32;
pub const EAP_E_SERVER_CERT_OTHER_ERROR: u32 = 2151809540u32;
pub const EAP_E_SERVER_CERT_REVOKED: u32 = 2151809539u32;
pub const EAP_E_SERVER_FIRST: i32 = -2143157760i32;
pub const EAP_E_SERVER_LAST: i32 = -2143157505i32;
pub const EAP_E_SERVER_ROOT_CERT_FIRST: i32 = -2143157248i32;
pub const EAP_E_SERVER_ROOT_CERT_INVALID: u32 = 2151810049u32;
pub const EAP_E_SERVER_ROOT_CERT_LAST: i32 = -2143156993i32;
pub const EAP_E_SERVER_ROOT_CERT_NAME_REQUIRED: u32 = 2151810054u32;
pub const EAP_E_SERVER_ROOT_CERT_NOT_FOUND: u32 = 2151810048u32;
pub const EAP_E_SIM_NOT_VALID: u32 = 2151810304u32;
pub const EAP_E_USER_CERT_EXPIRED: u32 = 2151809282u32;
pub const EAP_E_USER_CERT_INVALID: u32 = 2151809281u32;
pub const EAP_E_USER_CERT_NOT_FOUND: u32 = 2151809280u32;
pub const EAP_E_USER_CERT_OTHER_ERROR: u32 = 2151809284u32;
pub const EAP_E_USER_CERT_REJECTED: u32 = 2151809285u32;
pub const EAP_E_USER_CERT_REVOKED: u32 = 2151809283u32;
pub const EAP_E_USER_CREDENTIALS_REJECTED: u32 = 2151809297u32;
pub const EAP_E_USER_FIRST: i32 = -2143158016i32;
pub const EAP_E_USER_LAST: i32 = -2143157761i32;
pub const EAP_E_USER_NAME_PASSWORD_REJECTED: u32 = 2151809298u32;
pub const EAP_E_USER_ROOT_CERT_EXPIRED: u32 = 2151809794u32;
pub const EAP_E_USER_ROOT_CERT_FIRST: i32 = -2143157504i32;
pub const EAP_E_USER_ROOT_CERT_INVALID: u32 = 2151809793u32;
pub const EAP_E_USER_ROOT_CERT_LAST: i32 = -2143157249i32;
pub const EAP_E_USER_ROOT_CERT_NOT_FOUND: u32 = 2151809792u32;
pub const EAP_FLAG_CONFG_READONLY: u32 = 524288u32;
pub const EAP_FLAG_FULL_AUTH: u32 = 4096u32;
pub const EAP_FLAG_GUEST_ACCESS: u32 = 64u32;
pub const EAP_FLAG_LOGON: u32 = 4u32;
pub const EAP_FLAG_MACHINE_AUTH: u32 = 32u32;
pub const EAP_FLAG_NON_INTERACTIVE: u32 = 2u32;
pub const EAP_FLAG_ONLY_EAP_TLS: u32 = 16777216u32;
pub const EAP_FLAG_PREFER_ALT_CREDENTIALS: u32 = 8192u32;
pub const EAP_FLAG_PREVIEW: u32 = 8u32;
pub const EAP_FLAG_PRE_LOGON: u32 = 131072u32;
pub const EAP_FLAG_RESUME_FROM_HIBERNATE: u32 = 512u32;
pub const EAP_FLAG_Reserved1: u32 = 1u32;
pub const EAP_FLAG_Reserved2: u32 = 16u32;
pub const EAP_FLAG_Reserved3: u32 = 128u32;
pub const EAP_FLAG_Reserved4: u32 = 256u32;
pub const EAP_FLAG_Reserved5: u32 = 1024u32;
pub const EAP_FLAG_Reserved6: u32 = 2048u32;
pub const EAP_FLAG_Reserved7: u32 = 16384u32;
pub const EAP_FLAG_Reserved8: u32 = 1048576u32;
pub const EAP_FLAG_Reserved9: u32 = 4194304u32;
pub const EAP_FLAG_SERVER_VALIDATION_REQUIRED: u32 = 33554432u32;
pub const EAP_FLAG_SUPRESS_UI: u32 = 65536u32;
pub const EAP_FLAG_USER_AUTH: u32 = 262144u32;
pub const EAP_FLAG_VPN: u32 = 8388608u32;
pub const EAP_GROUP_MASK: i32 = 65280i32;
pub const EAP_INTERACTIVE_UI_DATA_VERSION: u32 = 1u32;
pub const EAP_INVALID_PACKET: u32 = 2151809048u32;
pub const EAP_I_EAPHOST_EAP_NEGOTIATION_FAILED: u32 = 1078067222u32;
pub const EAP_I_EAPHOST_FIRST: i32 = -2143158272i32;
pub const EAP_I_EAPHOST_LAST: i32 = -2143158017i32;
pub const EAP_I_USER_ACCOUNT_OTHER_ERROR: u32 = 1078067472u32;
pub const EAP_I_USER_FIRST: i32 = 1078067456i32;
pub const EAP_I_USER_LAST: i32 = 1078067711i32;
pub const EAP_METHOD_AUTHENTICATOR_CONFIG_IS_IDENTITY_PRIVACY: u32 = 1u32;
pub const EAP_METHOD_AUTHENTICATOR_RESPONSE_AUTHENTICATE: EAP_METHOD_AUTHENTICATOR_RESPONSE_ACTION = 4i32;
pub const EAP_METHOD_AUTHENTICATOR_RESPONSE_DISCARD: EAP_METHOD_AUTHENTICATOR_RESPONSE_ACTION = 0i32;
pub const EAP_METHOD_AUTHENTICATOR_RESPONSE_HANDLE_IDENTITY: EAP_METHOD_AUTHENTICATOR_RESPONSE_ACTION = 5i32;
pub const EAP_METHOD_AUTHENTICATOR_RESPONSE_RESPOND: EAP_METHOD_AUTHENTICATOR_RESPONSE_ACTION = 3i32;
pub const EAP_METHOD_AUTHENTICATOR_RESPONSE_RESULT: EAP_METHOD_AUTHENTICATOR_RESPONSE_ACTION = 2i32;
pub const EAP_METHOD_AUTHENTICATOR_RESPONSE_SEND: EAP_METHOD_AUTHENTICATOR_RESPONSE_ACTION = 1i32;
pub const EAP_METHOD_INVALID_PACKET: u32 = 2151809047u32;
pub const EAP_PEER_FLAG_GUEST_ACCESS: u32 = 64u32;
pub const EAP_PEER_FLAG_HEALTH_STATE_CHANGE: u32 = 32768u32;
pub const EAP_PEER_VALUENAME_CONFIGUI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerConfigUIPath");
pub const EAP_PEER_VALUENAME_DLL_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerDllPath");
pub const EAP_PEER_VALUENAME_FRIENDLY_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerFriendlyName");
pub const EAP_PEER_VALUENAME_IDENTITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerIdentityPath");
pub const EAP_PEER_VALUENAME_INTERACTIVEUI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerInteractiveUIPath");
pub const EAP_PEER_VALUENAME_INVOKE_NAMEDLG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerInvokeUsernameDialog");
pub const EAP_PEER_VALUENAME_INVOKE_PWDDLG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerInvokePasswordDialog");
pub const EAP_PEER_VALUENAME_PROPERTIES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Properties");
pub const EAP_PEER_VALUENAME_REQUIRE_CONFIGUI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PeerRequireConfigUI");
pub const EAP_REGISTRY_LOCATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("System\\CurrentControlSet\\Services\\EapHost\\Methods");
pub const EAP_SIM_CREDENTIAL: EapCredentialType = 4i32;
pub const EAP_UI_INPUT_FIELD_PROPS_DEFAULT: u32 = 0u32;
pub const EAP_UI_INPUT_FIELD_PROPS_NON_DISPLAYABLE: u32 = 1u32;
pub const EAP_UI_INPUT_FIELD_PROPS_NON_PERSIST: u32 = 2u32;
pub const EAP_UI_INPUT_FIELD_PROPS_READ_ONLY: u32 = 4u32;
pub const EAP_USERNAME_PASSWORD_CREDENTIAL: EapCredentialType = 1i32;
pub const EAP_VALUENAME_PROPERTIES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Properties");
pub const EAP_WINLOGON_CREDENTIAL: EapCredentialType = 2i32;
pub const EapCodeFailure: EapCode = 4i32;
pub const EapCodeMaximum: EapCode = 4i32;
pub const EapCodeMinimum: EapCode = 1i32;
pub const EapCodeRequest: EapCode = 1i32;
pub const EapCodeResponse: EapCode = 2i32;
pub const EapCodeSuccess: EapCode = 3i32;
pub const EapConfigInputEdit: EAP_CONFIG_INPUT_FIELD_TYPE = 6i32;
pub const EapConfigInputNetworkPassword: EAP_CONFIG_INPUT_FIELD_TYPE = 3i32;
pub const EapConfigInputNetworkUsername: EAP_CONFIG_INPUT_FIELD_TYPE = 2i32;
pub const EapConfigInputPSK: EAP_CONFIG_INPUT_FIELD_TYPE = 5i32;
pub const EapConfigInputPassword: EAP_CONFIG_INPUT_FIELD_TYPE = 1i32;
pub const EapConfigInputPin: EAP_CONFIG_INPUT_FIELD_TYPE = 4i32;
pub const EapConfigInputUsername: EAP_CONFIG_INPUT_FIELD_TYPE = 0i32;
pub const EapConfigSmartCardError: EAP_CONFIG_INPUT_FIELD_TYPE = 8i32;
pub const EapConfigSmartCardUsername: EAP_CONFIG_INPUT_FIELD_TYPE = 7i32;
pub const EapCredExpiryReq: EAP_INTERACTIVE_UI_DATA_TYPE = 2i32;
pub const EapCredExpiryResp: EAP_INTERACTIVE_UI_DATA_TYPE = 3i32;
pub const EapCredLogonReq: EAP_INTERACTIVE_UI_DATA_TYPE = 4i32;
pub const EapCredLogonResp: EAP_INTERACTIVE_UI_DATA_TYPE = 5i32;
pub const EapCredReq: EAP_INTERACTIVE_UI_DATA_TYPE = 0i32;
pub const EapCredResp: EAP_INTERACTIVE_UI_DATA_TYPE = 1i32;
pub const EapHostAuthFailed: EAPHOST_AUTH_STATUS = 6i32;
pub const EapHostAuthIdentityExchange: EAPHOST_AUTH_STATUS = 2i32;
pub const EapHostAuthInProgress: EAPHOST_AUTH_STATUS = 4i32;
pub const EapHostAuthNegotiatingType: EAPHOST_AUTH_STATUS = 3i32;
pub const EapHostAuthNotStarted: EAPHOST_AUTH_STATUS = 1i32;
pub const EapHostAuthSucceeded: EAPHOST_AUTH_STATUS = 5i32;
pub const EapHostInvalidSession: EAPHOST_AUTH_STATUS = 0i32;
pub const EapHostNapInfo: EapHostPeerAuthParams = 4i32;
pub const EapHostPeerAuthStatus: EapHostPeerAuthParams = 1i32;
pub const EapHostPeerIdentity: EapHostPeerAuthParams = 2i32;
pub const EapHostPeerIdentityExtendedInfo: EapHostPeerAuthParams = 3i32;
pub const EapHostPeerMethodResultAltSuccessReceived: EapHostPeerMethodResultReason = 1i32;
pub const EapHostPeerMethodResultFromMethod: EapHostPeerMethodResultReason = 3i32;
pub const EapHostPeerMethodResultTimeout: EapHostPeerMethodResultReason = 2i32;
pub const EapHostPeerResponseDiscard: EapHostPeerResponseAction = 0i32;
pub const EapHostPeerResponseInvokeUi: EapHostPeerResponseAction = 3i32;
pub const EapHostPeerResponseNone: EapHostPeerResponseAction = 6i32;
pub const EapHostPeerResponseRespond: EapHostPeerResponseAction = 4i32;
pub const EapHostPeerResponseResult: EapHostPeerResponseAction = 2i32;
pub const EapHostPeerResponseSend: EapHostPeerResponseAction = 1i32;
pub const EapHostPeerResponseStartAuthentication: EapHostPeerResponseAction = 5i32;
pub const EapPeerMethodResponseActionDiscard: EapPeerMethodResponseAction = 0i32;
pub const EapPeerMethodResponseActionInvokeUI: EapPeerMethodResponseAction = 3i32;
pub const EapPeerMethodResponseActionNone: EapPeerMethodResponseAction = 5i32;
pub const EapPeerMethodResponseActionRespond: EapPeerMethodResponseAction = 4i32;
pub const EapPeerMethodResponseActionResult: EapPeerMethodResponseAction = 2i32;
pub const EapPeerMethodResponseActionSend: EapPeerMethodResponseAction = 1i32;
pub const EapPeerMethodResultFailure: EapPeerMethodResultReason = 3i32;
pub const EapPeerMethodResultSuccess: EapPeerMethodResultReason = 2i32;
pub const EapPeerMethodResultUnknown: EapPeerMethodResultReason = 1i32;
pub const FACILITY_EAP_MESSAGE: u32 = 2114u32;
pub const GUID_EapHost_Cause_CertStoreInaccessible: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000004);
pub const GUID_EapHost_Cause_EapNegotiationFailed: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000001c);
pub const GUID_EapHost_Cause_EapQecInaccessible: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000312);
pub const GUID_EapHost_Cause_Generic_AuthFailure: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000104);
pub const GUID_EapHost_Cause_IdentityUnknown: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000204);
pub const GUID_EapHost_Cause_MethodDLLNotFound: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000001);
pub const GUID_EapHost_Cause_MethodDoesNotSupportOperation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000001e);
pub const GUID_EapHost_Cause_Method_Config_Does_Not_Support_Sso: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xda18bd32_004f_41fa_ae08_0bc85e5845ac);
pub const GUID_EapHost_Cause_No_SmartCardReader_Found: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000002b);
pub const GUID_EapHost_Cause_Server_CertExpired: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000005);
pub const GUID_EapHost_Cause_Server_CertInvalid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000006);
pub const GUID_EapHost_Cause_Server_CertNotFound: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000007);
pub const GUID_EapHost_Cause_Server_CertOtherError: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000108);
pub const GUID_EapHost_Cause_Server_CertRevoked: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000008);
pub const GUID_EapHost_Cause_Server_Root_CertNameRequired: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000012);
pub const GUID_EapHost_Cause_Server_Root_CertNotFound: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000112);
pub const GUID_EapHost_Cause_SimNotValid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000304);
pub const GUID_EapHost_Cause_ThirdPartyMethod_Host_Reset: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000212);
pub const GUID_EapHost_Cause_User_Account_OtherProblem: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000010e);
pub const GUID_EapHost_Cause_User_CertExpired: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000009);
pub const GUID_EapHost_Cause_User_CertInvalid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000000a);
pub const GUID_EapHost_Cause_User_CertNotFound: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000000b);
pub const GUID_EapHost_Cause_User_CertOtherError: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000000c);
pub const GUID_EapHost_Cause_User_CertRejected: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000000d);
pub const GUID_EapHost_Cause_User_CertRevoked: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000000e);
pub const GUID_EapHost_Cause_User_CredsRejected: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000020e);
pub const GUID_EapHost_Cause_User_Root_CertExpired: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000000f);
pub const GUID_EapHost_Cause_User_Root_CertInvalid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000010);
pub const GUID_EapHost_Cause_User_Root_CertNotFound: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000011);
pub const GUID_EapHost_Cause_XmlMalformed: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000001d);
pub const GUID_EapHost_Default: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x00000000_0000_0000_0000_000000000000);
pub const GUID_EapHost_Help_ObtainingCerts: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf535eea3_1bdd_46ca_a2fc_a6655939b7e8);
pub const GUID_EapHost_Help_Troubleshooting: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x33307acf_0698_41ba_b014_ea0a2eb8d0a8);
pub const GUID_EapHost_Repair_ContactAdmin_AuthFailure: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000001f);
pub const GUID_EapHost_Repair_ContactAdmin_CertNameAbsent: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000029);
pub const GUID_EapHost_Repair_ContactAdmin_CertStoreInaccessible: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000024);
pub const GUID_EapHost_Repair_ContactAdmin_IdentityUnknown: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000020);
pub const GUID_EapHost_Repair_ContactAdmin_InvalidUserAccount: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000025);
pub const GUID_EapHost_Repair_ContactAdmin_InvalidUserCert: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000002c);
pub const GUID_EapHost_Repair_ContactAdmin_MethodNotFound: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000022);
pub const GUID_EapHost_Repair_ContactAdmin_NegotiationFailed: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000021);
pub const GUID_EapHost_Repair_ContactAdmin_NoSmartCardReader: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000002a);
pub const GUID_EapHost_Repair_ContactAdmin_RootCertInvalid: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000026);
pub const GUID_EapHost_Repair_ContactAdmin_RootCertNotFound: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000027);
pub const GUID_EapHost_Repair_ContactAdmin_RootExpired: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000028);
pub const GUID_EapHost_Repair_ContactSysadmin: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000002);
pub const GUID_EapHost_Repair_Method_Not_Support_Sso: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000002d);
pub const GUID_EapHost_Repair_No_ValidSim_Found: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000002e);
pub const GUID_EapHost_Repair_RestartNap: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000023);
pub const GUID_EapHost_Repair_Retry_Authentication: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000011b);
pub const GUID_EapHost_Repair_Server_ClientSelectServerCert: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000018);
pub const GUID_EapHost_Repair_User_AuthFailure: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d800000019);
pub const GUID_EapHost_Repair_User_GetNewCert: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000001a);
pub const GUID_EapHost_Repair_User_SelectValidCert: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9612fc67_6150_4209_a85e_a8d80000001b);
pub const ISOLATION_STATE_IN_PROBATION: ISOLATION_STATE = 2i32;
pub const ISOLATION_STATE_NOT_RESTRICTED: ISOLATION_STATE = 1i32;
pub const ISOLATION_STATE_RESTRICTED_ACCESS: ISOLATION_STATE = 3i32;
pub const ISOLATION_STATE_UNKNOWN: ISOLATION_STATE = 0i32;
pub const MAXEAPCODE: u32 = 4u32;
pub const MAX_EAP_CONFIG_INPUT_FIELD_LENGTH: u32 = 256u32;
pub const MAX_EAP_CONFIG_INPUT_FIELD_VALUE_LENGTH: u32 = 1024u32;
pub const NCRYPT_PIN_CACHE_PIN_BYTE_LENGTH: u32 = 90u32;
pub const RAS_EAP_FLAG_8021X_AUTH: u32 = 128u32;
pub const RAS_EAP_FLAG_ALTERNATIVE_USER_DB: u32 = 2048u32;
pub const RAS_EAP_FLAG_CONFG_READONLY: u32 = 524288u32;
pub const RAS_EAP_FLAG_FIRST_LINK: u32 = 16u32;
pub const RAS_EAP_FLAG_GUEST_ACCESS: u32 = 64u32;
pub const RAS_EAP_FLAG_HOSTED_IN_PEAP: u32 = 256u32;
pub const RAS_EAP_FLAG_LOGON: u32 = 4u32;
pub const RAS_EAP_FLAG_MACHINE_AUTH: u32 = 32u32;
pub const RAS_EAP_FLAG_NON_INTERACTIVE: u32 = 2u32;
pub const RAS_EAP_FLAG_PEAP_FORCE_FULL_AUTH: u32 = 4096u32;
pub const RAS_EAP_FLAG_PEAP_UPFRONT: u32 = 1024u32;
pub const RAS_EAP_FLAG_PREVIEW: u32 = 8u32;
pub const RAS_EAP_FLAG_PRE_LOGON: u32 = 131072u32;
pub const RAS_EAP_FLAG_RESERVED: u32 = 1048576u32;
pub const RAS_EAP_FLAG_RESUME_FROM_HIBERNATE: u32 = 512u32;
pub const RAS_EAP_FLAG_ROUTER: u32 = 1u32;
pub const RAS_EAP_FLAG_SAVE_CREDMAN: u32 = 2097152u32;
pub const RAS_EAP_FLAG_SERVER_VALIDATION_REQUIRED: u32 = 33554432u32;
pub const RAS_EAP_REGISTRY_LOCATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("System\\CurrentControlSet\\Services\\Rasman\\PPP\\EAP");
pub const RAS_EAP_ROLE_AUTHENTICATEE: u32 = 2u32;
pub const RAS_EAP_ROLE_AUTHENTICATOR: u32 = 1u32;
pub const RAS_EAP_ROLE_EXCLUDE_IN_EAP: u32 = 4u32;
pub const RAS_EAP_ROLE_EXCLUDE_IN_PEAP: u32 = 8u32;
pub const RAS_EAP_ROLE_EXCLUDE_IN_VPN: u32 = 16u32;
pub const RAS_EAP_VALUENAME_CONFIGUI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ConfigUIPath");
pub const RAS_EAP_VALUENAME_CONFIG_CLSID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ConfigCLSID");
pub const RAS_EAP_VALUENAME_DEFAULT_DATA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ConfigData");
pub const RAS_EAP_VALUENAME_ENCRYPTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MPPEEncryptionSupported");
pub const RAS_EAP_VALUENAME_FILTER_INNERMETHODS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FilterInnerMethods");
pub const RAS_EAP_VALUENAME_FRIENDLY_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FriendlyName");
pub const RAS_EAP_VALUENAME_IDENTITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IdentityPath");
pub const RAS_EAP_VALUENAME_INTERACTIVEUI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InteractiveUIPath");
pub const RAS_EAP_VALUENAME_INVOKE_NAMEDLG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InvokeUsernameDialog");
pub const RAS_EAP_VALUENAME_INVOKE_PWDDLG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InvokePasswordDialog");
pub const RAS_EAP_VALUENAME_ISTUNNEL_METHOD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsTunnelMethod");
pub const RAS_EAP_VALUENAME_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Path");
pub const RAS_EAP_VALUENAME_PER_POLICY_CONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PerPolicyConfig");
pub const RAS_EAP_VALUENAME_REQUIRE_CONFIGUI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RequireConfigUI");
pub const RAS_EAP_VALUENAME_ROLES_SUPPORTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RolesSupported");
pub const RAS_EAP_VALUENAME_STANDALONE_SUPPORTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StandaloneSupported");
pub const eapPropCertifiedMethod: u32 = 4194304u32;
pub const eapPropChannelBinding: u32 = 65536u32;
pub const eapPropCipherSuiteNegotiation: u32 = 1u32;
pub const eapPropConfidentiality: u32 = 16u32;
pub const eapPropCryptoBinding: u32 = 8192u32;
pub const eapPropDictionaryAttackResistance: u32 = 2048u32;
pub const eapPropFastReconnect: u32 = 4096u32;
pub const eapPropFragmentation: u32 = 32768u32;
pub const eapPropHiddenMethod: u32 = 8388608u32;
pub const eapPropIdentityPrivacy: u32 = 67108864u32;
pub const eapPropIntegrity: u32 = 4u32;
pub const eapPropKeyDerivation: u32 = 32u32;
pub const eapPropKeyStrength1024: u32 = 1024u32;
pub const eapPropKeyStrength128: u32 = 128u32;
pub const eapPropKeyStrength256: u32 = 256u32;
pub const eapPropKeyStrength512: u32 = 512u32;
pub const eapPropKeyStrength64: u32 = 64u32;
pub const eapPropMachineAuth: u32 = 16777216u32;
pub const eapPropMethodChaining: u32 = 134217728u32;
pub const eapPropMppeEncryption: u32 = 524288u32;
pub const eapPropMutualAuth: u32 = 2u32;
pub const eapPropNap: u32 = 131072u32;
pub const eapPropReplayProtection: u32 = 8u32;
pub const eapPropReserved: u32 = 2147483648u32;
pub const eapPropSessionIndependence: u32 = 16384u32;
pub const eapPropSharedStateEquivalence: u32 = 268435456u32;
pub const eapPropStandalone: u32 = 262144u32;
pub const eapPropSupportsConfig: u32 = 2097152u32;
pub const eapPropTunnelMethod: u32 = 1048576u32;
pub const eapPropUserAuth: u32 = 33554432u32;
pub const eatARAPChallengeResponse: EAP_ATTRIBUTE_TYPE = 84i32;
pub const eatARAPFeatures: EAP_ATTRIBUTE_TYPE = 71i32;
pub const eatARAPGuestLogon: EAP_ATTRIBUTE_TYPE = 8096i32;
pub const eatARAPPassword: EAP_ATTRIBUTE_TYPE = 70i32;
pub const eatARAPSecurity: EAP_ATTRIBUTE_TYPE = 73i32;
pub const eatARAPSecurityData: EAP_ATTRIBUTE_TYPE = 74i32;
pub const eatARAPZoneAccess: EAP_ATTRIBUTE_TYPE = 72i32;
pub const eatAcctAuthentic: EAP_ATTRIBUTE_TYPE = 45i32;
pub const eatAcctDelayTime: EAP_ATTRIBUTE_TYPE = 41i32;
pub const eatAcctEventTimeStamp: EAP_ATTRIBUTE_TYPE = 55i32;
pub const eatAcctInputOctets: EAP_ATTRIBUTE_TYPE = 42i32;
pub const eatAcctInputPackets: EAP_ATTRIBUTE_TYPE = 47i32;
pub const eatAcctInterimInterval: EAP_ATTRIBUTE_TYPE = 85i32;
pub const eatAcctLinkCount: EAP_ATTRIBUTE_TYPE = 51i32;
pub const eatAcctMultiSessionId: EAP_ATTRIBUTE_TYPE = 50i32;
pub const eatAcctOutputOctets: EAP_ATTRIBUTE_TYPE = 43i32;
pub const eatAcctOutputPackets: EAP_ATTRIBUTE_TYPE = 48i32;
pub const eatAcctSessionId: EAP_ATTRIBUTE_TYPE = 44i32;
pub const eatAcctSessionTime: EAP_ATTRIBUTE_TYPE = 46i32;
pub const eatAcctStatusType: EAP_ATTRIBUTE_TYPE = 40i32;
pub const eatAcctTerminateCause: EAP_ATTRIBUTE_TYPE = 49i32;
pub const eatCallbackId: EAP_ATTRIBUTE_TYPE = 20i32;
pub const eatCallbackNumber: EAP_ATTRIBUTE_TYPE = 19i32;
pub const eatCalledStationId: EAP_ATTRIBUTE_TYPE = 30i32;
pub const eatCallingStationId: EAP_ATTRIBUTE_TYPE = 31i32;
pub const eatCertificateOID: EAP_ATTRIBUTE_TYPE = 8097i32;
pub const eatCertificateThumbprint: EAP_ATTRIBUTE_TYPE = 8250i32;
pub const eatClass: EAP_ATTRIBUTE_TYPE = 25i32;
pub const eatClearTextPassword: EAP_ATTRIBUTE_TYPE = 8107i32;
pub const eatConfigurationToken: EAP_ATTRIBUTE_TYPE = 78i32;
pub const eatConnectInfo: EAP_ATTRIBUTE_TYPE = 77i32;
pub const eatCredentialsChanged: EAP_ATTRIBUTE_TYPE = 8103i32;
pub const eatEAPConfiguration: EAP_ATTRIBUTE_TYPE = 8098i32;
pub const eatEAPMessage: EAP_ATTRIBUTE_TYPE = 79i32;
pub const eatEAPTLV: EAP_ATTRIBUTE_TYPE = 8102i32;
pub const eatEMSK: EAP_ATTRIBUTE_TYPE = 9003i32;
pub const eatFastRoamedSession: EAP_ATTRIBUTE_TYPE = 8100i32;
pub const eatFilterId: EAP_ATTRIBUTE_TYPE = 11i32;
pub const eatFramedAppleTalkLink: EAP_ATTRIBUTE_TYPE = 37i32;
pub const eatFramedAppleTalkNetwork: EAP_ATTRIBUTE_TYPE = 38i32;
pub const eatFramedAppleTalkZone: EAP_ATTRIBUTE_TYPE = 39i32;
pub const eatFramedCompression: EAP_ATTRIBUTE_TYPE = 13i32;
pub const eatFramedIPAddress: EAP_ATTRIBUTE_TYPE = 8i32;
pub const eatFramedIPNetmask: EAP_ATTRIBUTE_TYPE = 9i32;
pub const eatFramedIPXNetwork: EAP_ATTRIBUTE_TYPE = 23i32;
pub const eatFramedIPv6Pool: EAP_ATTRIBUTE_TYPE = 100i32;
pub const eatFramedIPv6Prefix: EAP_ATTRIBUTE_TYPE = 97i32;
pub const eatFramedIPv6Route: EAP_ATTRIBUTE_TYPE = 99i32;
pub const eatFramedInterfaceId: EAP_ATTRIBUTE_TYPE = 96i32;
pub const eatFramedMTU: EAP_ATTRIBUTE_TYPE = 12i32;
pub const eatFramedProtocol: EAP_ATTRIBUTE_TYPE = 7i32;
pub const eatFramedRoute: EAP_ATTRIBUTE_TYPE = 22i32;
pub const eatFramedRouting: EAP_ATTRIBUTE_TYPE = 10i32;
pub const eatIdleTimeout: EAP_ATTRIBUTE_TYPE = 28i32;
pub const eatInnerEapMethodType: EAP_ATTRIBUTE_TYPE = 8104i32;
pub const eatLoginIPHost: EAP_ATTRIBUTE_TYPE = 14i32;
pub const eatLoginIPv6Host: EAP_ATTRIBUTE_TYPE = 98i32;
pub const eatLoginLATGroup: EAP_ATTRIBUTE_TYPE = 36i32;
pub const eatLoginLATNode: EAP_ATTRIBUTE_TYPE = 35i32;
pub const eatLoginLATPort: EAP_ATTRIBUTE_TYPE = 63i32;
pub const eatLoginLATService: EAP_ATTRIBUTE_TYPE = 34i32;
pub const eatLoginService: EAP_ATTRIBUTE_TYPE = 15i32;
pub const eatLoginTCPPort: EAP_ATTRIBUTE_TYPE = 16i32;
pub const eatMD5CHAPChallenge: EAP_ATTRIBUTE_TYPE = 60i32;
pub const eatMD5CHAPPassword: EAP_ATTRIBUTE_TYPE = 3i32;
pub const eatMethodId: EAP_ATTRIBUTE_TYPE = 9002i32;
pub const eatMinimum: EAP_ATTRIBUTE_TYPE = 0i32;
pub const eatNASIPAddress: EAP_ATTRIBUTE_TYPE = 4i32;
pub const eatNASIPv6Address: EAP_ATTRIBUTE_TYPE = 95i32;
pub const eatNASIdentifier: EAP_ATTRIBUTE_TYPE = 32i32;
pub const eatNASPort: EAP_ATTRIBUTE_TYPE = 5i32;
pub const eatNASPortType: EAP_ATTRIBUTE_TYPE = 61i32;
pub const eatPEAPEmbeddedEAPTypeId: EAP_ATTRIBUTE_TYPE = 8099i32;
pub const eatPEAPFastRoamedSession: EAP_ATTRIBUTE_TYPE = 8100i32;
pub const eatPasswordRetry: EAP_ATTRIBUTE_TYPE = 75i32;
pub const eatPeerId: EAP_ATTRIBUTE_TYPE = 9000i32;
pub const eatPortLimit: EAP_ATTRIBUTE_TYPE = 62i32;
pub const eatPrompt: EAP_ATTRIBUTE_TYPE = 76i32;
pub const eatProxyState: EAP_ATTRIBUTE_TYPE = 33i32;
pub const eatQuarantineSoH: EAP_ATTRIBUTE_TYPE = 8150i32;
pub const eatReplyMessage: EAP_ATTRIBUTE_TYPE = 18i32;
pub const eatReserved: EAP_ATTRIBUTE_TYPE = -1i32;
pub const eatServerId: EAP_ATTRIBUTE_TYPE = 9001i32;
pub const eatServiceType: EAP_ATTRIBUTE_TYPE = 6i32;
pub const eatSessionId: EAP_ATTRIBUTE_TYPE = 9004i32;
pub const eatSessionTimeout: EAP_ATTRIBUTE_TYPE = 27i32;
pub const eatSignature: EAP_ATTRIBUTE_TYPE = 80i32;
pub const eatState: EAP_ATTRIBUTE_TYPE = 24i32;
pub const eatTerminationAction: EAP_ATTRIBUTE_TYPE = 29i32;
pub const eatTunnelClientEndpoint: EAP_ATTRIBUTE_TYPE = 66i32;
pub const eatTunnelMediumType: EAP_ATTRIBUTE_TYPE = 65i32;
pub const eatTunnelServerEndpoint: EAP_ATTRIBUTE_TYPE = 67i32;
pub const eatTunnelType: EAP_ATTRIBUTE_TYPE = 64i32;
pub const eatUnassigned17: EAP_ATTRIBUTE_TYPE = 17i32;
pub const eatUnassigned21: EAP_ATTRIBUTE_TYPE = 21i32;
pub const eatUserName: EAP_ATTRIBUTE_TYPE = 1i32;
pub const eatUserPassword: EAP_ATTRIBUTE_TYPE = 2i32;
pub const eatVendorSpecific: EAP_ATTRIBUTE_TYPE = 26i32;
pub const emptLegacyMethodPropertyFlag: EAP_METHOD_PROPERTY_TYPE = 31i32;
pub const emptPropCertifiedMethod: EAP_METHOD_PROPERTY_TYPE = 22i32;
pub const emptPropChannelBinding: EAP_METHOD_PROPERTY_TYPE = 16i32;
pub const emptPropCipherSuiteNegotiation: EAP_METHOD_PROPERTY_TYPE = 0i32;
pub const emptPropConfidentiality: EAP_METHOD_PROPERTY_TYPE = 4i32;
pub const emptPropCryptoBinding: EAP_METHOD_PROPERTY_TYPE = 13i32;
pub const emptPropDictionaryAttackResistance: EAP_METHOD_PROPERTY_TYPE = 11i32;
pub const emptPropFastReconnect: EAP_METHOD_PROPERTY_TYPE = 12i32;
pub const emptPropFragmentation: EAP_METHOD_PROPERTY_TYPE = 15i32;
pub const emptPropHiddenMethod: EAP_METHOD_PROPERTY_TYPE = 23i32;
pub const emptPropIdentityPrivacy: EAP_METHOD_PROPERTY_TYPE = 26i32;
pub const emptPropIntegrity: EAP_METHOD_PROPERTY_TYPE = 2i32;
pub const emptPropKeyDerivation: EAP_METHOD_PROPERTY_TYPE = 5i32;
pub const emptPropKeyStrength1024: EAP_METHOD_PROPERTY_TYPE = 10i32;
pub const emptPropKeyStrength128: EAP_METHOD_PROPERTY_TYPE = 7i32;
pub const emptPropKeyStrength256: EAP_METHOD_PROPERTY_TYPE = 8i32;
pub const emptPropKeyStrength512: EAP_METHOD_PROPERTY_TYPE = 9i32;
pub const emptPropKeyStrength64: EAP_METHOD_PROPERTY_TYPE = 6i32;
pub const emptPropMachineAuth: EAP_METHOD_PROPERTY_TYPE = 24i32;
pub const emptPropMethodChaining: EAP_METHOD_PROPERTY_TYPE = 27i32;
pub const emptPropMppeEncryption: EAP_METHOD_PROPERTY_TYPE = 19i32;
pub const emptPropMutualAuth: EAP_METHOD_PROPERTY_TYPE = 1i32;
pub const emptPropNap: EAP_METHOD_PROPERTY_TYPE = 17i32;
pub const emptPropReplayProtection: EAP_METHOD_PROPERTY_TYPE = 3i32;
pub const emptPropSessionIndependence: EAP_METHOD_PROPERTY_TYPE = 14i32;
pub const emptPropSharedStateEquivalence: EAP_METHOD_PROPERTY_TYPE = 28i32;
pub const emptPropStandalone: EAP_METHOD_PROPERTY_TYPE = 18i32;
pub const emptPropSupportsConfig: EAP_METHOD_PROPERTY_TYPE = 21i32;
pub const emptPropTunnelMethod: EAP_METHOD_PROPERTY_TYPE = 20i32;
pub const emptPropUserAuth: EAP_METHOD_PROPERTY_TYPE = 25i32;
pub const emptPropVendorSpecific: EAP_METHOD_PROPERTY_TYPE = 255i32;
pub const empvtBool: EAP_METHOD_PROPERTY_VALUE_TYPE = 0i32;
pub const empvtDword: EAP_METHOD_PROPERTY_VALUE_TYPE = 1i32;
pub const empvtString: EAP_METHOD_PROPERTY_VALUE_TYPE = 2i32;
pub const raatARAPChallenge: u32 = 33u32;
pub const raatARAPChallengeResponse: RAS_AUTH_ATTRIBUTE_TYPE = 84i32;
pub const raatARAPFeatures: RAS_AUTH_ATTRIBUTE_TYPE = 71i32;
pub const raatARAPGuestLogon: RAS_AUTH_ATTRIBUTE_TYPE = 8096i32;
pub const raatARAPNewPassword: u32 = 20u32;
pub const raatARAPOldPassword: u32 = 19u32;
pub const raatARAPPassword: RAS_AUTH_ATTRIBUTE_TYPE = 70i32;
pub const raatARAPPasswordChangeReason: u32 = 21u32;
pub const raatARAPSecurity: RAS_AUTH_ATTRIBUTE_TYPE = 73i32;
pub const raatARAPSecurityData: RAS_AUTH_ATTRIBUTE_TYPE = 74i32;
pub const raatARAPZoneAccess: RAS_AUTH_ATTRIBUTE_TYPE = 72i32;
pub const raatAcctAuthentic: RAS_AUTH_ATTRIBUTE_TYPE = 45i32;
pub const raatAcctDelayTime: RAS_AUTH_ATTRIBUTE_TYPE = 41i32;
pub const raatAcctEventTimeStamp: RAS_AUTH_ATTRIBUTE_TYPE = 55i32;
pub const raatAcctInputOctets: RAS_AUTH_ATTRIBUTE_TYPE = 42i32;
pub const raatAcctInputPackets: RAS_AUTH_ATTRIBUTE_TYPE = 47i32;
pub const raatAcctInterimInterval: RAS_AUTH_ATTRIBUTE_TYPE = 85i32;
pub const raatAcctLinkCount: RAS_AUTH_ATTRIBUTE_TYPE = 51i32;
pub const raatAcctMultiSessionId: RAS_AUTH_ATTRIBUTE_TYPE = 50i32;
pub const raatAcctOutputOctets: RAS_AUTH_ATTRIBUTE_TYPE = 43i32;
pub const raatAcctOutputPackets: RAS_AUTH_ATTRIBUTE_TYPE = 48i32;
pub const raatAcctSessionId: RAS_AUTH_ATTRIBUTE_TYPE = 44i32;
pub const raatAcctSessionTime: RAS_AUTH_ATTRIBUTE_TYPE = 46i32;
pub const raatAcctStatusType: RAS_AUTH_ATTRIBUTE_TYPE = 40i32;
pub const raatAcctTerminateCause: RAS_AUTH_ATTRIBUTE_TYPE = 49i32;
pub const raatCallbackId: RAS_AUTH_ATTRIBUTE_TYPE = 20i32;
pub const raatCallbackNumber: RAS_AUTH_ATTRIBUTE_TYPE = 19i32;
pub const raatCalledStationId: RAS_AUTH_ATTRIBUTE_TYPE = 30i32;
pub const raatCallingStationId: RAS_AUTH_ATTRIBUTE_TYPE = 31i32;
pub const raatCertificateOID: RAS_AUTH_ATTRIBUTE_TYPE = 8097i32;
pub const raatCertificateThumbprint: RAS_AUTH_ATTRIBUTE_TYPE = 8250i32;
pub const raatClass: RAS_AUTH_ATTRIBUTE_TYPE = 25i32;
pub const raatConfigurationToken: RAS_AUTH_ATTRIBUTE_TYPE = 78i32;
pub const raatConnectInfo: RAS_AUTH_ATTRIBUTE_TYPE = 77i32;
pub const raatCredentialsChanged: RAS_AUTH_ATTRIBUTE_TYPE = 8103i32;
pub const raatEAPConfiguration: RAS_AUTH_ATTRIBUTE_TYPE = 8098i32;
pub const raatEAPMessage: RAS_AUTH_ATTRIBUTE_TYPE = 79i32;
pub const raatEAPTLV: RAS_AUTH_ATTRIBUTE_TYPE = 8102i32;
pub const raatEMSK: RAS_AUTH_ATTRIBUTE_TYPE = 9003i32;
pub const raatFastRoamedSession: RAS_AUTH_ATTRIBUTE_TYPE = 8100i32;
pub const raatFilterId: RAS_AUTH_ATTRIBUTE_TYPE = 11i32;
pub const raatFramedAppleTalkLink: RAS_AUTH_ATTRIBUTE_TYPE = 37i32;
pub const raatFramedAppleTalkNetwork: RAS_AUTH_ATTRIBUTE_TYPE = 38i32;
pub const raatFramedAppleTalkZone: RAS_AUTH_ATTRIBUTE_TYPE = 39i32;
pub const raatFramedCompression: RAS_AUTH_ATTRIBUTE_TYPE = 13i32;
pub const raatFramedIPAddress: RAS_AUTH_ATTRIBUTE_TYPE = 8i32;
pub const raatFramedIPNetmask: RAS_AUTH_ATTRIBUTE_TYPE = 9i32;
pub const raatFramedIPXNetwork: RAS_AUTH_ATTRIBUTE_TYPE = 23i32;
pub const raatFramedIPv6Pool: RAS_AUTH_ATTRIBUTE_TYPE = 100i32;
pub const raatFramedIPv6Prefix: RAS_AUTH_ATTRIBUTE_TYPE = 97i32;
pub const raatFramedIPv6Route: RAS_AUTH_ATTRIBUTE_TYPE = 99i32;
pub const raatFramedInterfaceId: RAS_AUTH_ATTRIBUTE_TYPE = 96i32;
pub const raatFramedMTU: RAS_AUTH_ATTRIBUTE_TYPE = 12i32;
pub const raatFramedProtocol: RAS_AUTH_ATTRIBUTE_TYPE = 7i32;
pub const raatFramedRoute: RAS_AUTH_ATTRIBUTE_TYPE = 22i32;
pub const raatFramedRouting: RAS_AUTH_ATTRIBUTE_TYPE = 10i32;
pub const raatIdleTimeout: RAS_AUTH_ATTRIBUTE_TYPE = 28i32;
pub const raatInnerEAPTypeId: RAS_AUTH_ATTRIBUTE_TYPE = 8099i32;
pub const raatLoginIPHost: RAS_AUTH_ATTRIBUTE_TYPE = 14i32;
pub const raatLoginIPv6Host: RAS_AUTH_ATTRIBUTE_TYPE = 98i32;
pub const raatLoginLATGroup: RAS_AUTH_ATTRIBUTE_TYPE = 36i32;
pub const raatLoginLATNode: RAS_AUTH_ATTRIBUTE_TYPE = 35i32;
pub const raatLoginLATPort: RAS_AUTH_ATTRIBUTE_TYPE = 63i32;
pub const raatLoginLATService: RAS_AUTH_ATTRIBUTE_TYPE = 34i32;
pub const raatLoginService: RAS_AUTH_ATTRIBUTE_TYPE = 15i32;
pub const raatLoginTCPPort: RAS_AUTH_ATTRIBUTE_TYPE = 16i32;
pub const raatMD5CHAPChallenge: RAS_AUTH_ATTRIBUTE_TYPE = 60i32;
pub const raatMD5CHAPPassword: RAS_AUTH_ATTRIBUTE_TYPE = 3i32;
pub const raatMethodId: RAS_AUTH_ATTRIBUTE_TYPE = 9002i32;
pub const raatMinimum: RAS_AUTH_ATTRIBUTE_TYPE = 0i32;
pub const raatNASIPAddress: RAS_AUTH_ATTRIBUTE_TYPE = 4i32;
pub const raatNASIPv6Address: RAS_AUTH_ATTRIBUTE_TYPE = 95i32;
pub const raatNASIdentifier: RAS_AUTH_ATTRIBUTE_TYPE = 32i32;
pub const raatNASPort: RAS_AUTH_ATTRIBUTE_TYPE = 5i32;
pub const raatNASPortType: RAS_AUTH_ATTRIBUTE_TYPE = 61i32;
pub const raatPEAPEmbeddedEAPTypeId: RAS_AUTH_ATTRIBUTE_TYPE = 8099i32;
pub const raatPEAPFastRoamedSession: RAS_AUTH_ATTRIBUTE_TYPE = 8100i32;
pub const raatPasswordRetry: RAS_AUTH_ATTRIBUTE_TYPE = 75i32;
pub const raatPeerId: RAS_AUTH_ATTRIBUTE_TYPE = 9000i32;
pub const raatPortLimit: RAS_AUTH_ATTRIBUTE_TYPE = 62i32;
pub const raatPrompt: RAS_AUTH_ATTRIBUTE_TYPE = 76i32;
pub const raatProxyState: RAS_AUTH_ATTRIBUTE_TYPE = 33i32;
pub const raatReplyMessage: RAS_AUTH_ATTRIBUTE_TYPE = 18i32;
pub const raatReserved: RAS_AUTH_ATTRIBUTE_TYPE = -1i32;
pub const raatServerId: RAS_AUTH_ATTRIBUTE_TYPE = 9001i32;
pub const raatServiceType: RAS_AUTH_ATTRIBUTE_TYPE = 6i32;
pub const raatSessionId: RAS_AUTH_ATTRIBUTE_TYPE = 9004i32;
pub const raatSessionTimeout: RAS_AUTH_ATTRIBUTE_TYPE = 27i32;
pub const raatSignature: RAS_AUTH_ATTRIBUTE_TYPE = 80i32;
pub const raatState: RAS_AUTH_ATTRIBUTE_TYPE = 24i32;
pub const raatTerminationAction: RAS_AUTH_ATTRIBUTE_TYPE = 29i32;
pub const raatTunnelClientEndpoint: RAS_AUTH_ATTRIBUTE_TYPE = 66i32;
pub const raatTunnelMediumType: RAS_AUTH_ATTRIBUTE_TYPE = 65i32;
pub const raatTunnelServerEndpoint: RAS_AUTH_ATTRIBUTE_TYPE = 67i32;
pub const raatTunnelType: RAS_AUTH_ATTRIBUTE_TYPE = 64i32;
pub const raatUnassigned17: RAS_AUTH_ATTRIBUTE_TYPE = 17i32;
pub const raatUnassigned21: RAS_AUTH_ATTRIBUTE_TYPE = 21i32;
pub const raatUserName: RAS_AUTH_ATTRIBUTE_TYPE = 1i32;
pub const raatUserPassword: RAS_AUTH_ATTRIBUTE_TYPE = 2i32;
pub const raatVendorSpecific: RAS_AUTH_ATTRIBUTE_TYPE = 26i32;
pub type EAPHOST_AUTH_STATUS = i32;
pub type EAP_ATTRIBUTE_TYPE = i32;
pub type EAP_AUTHENTICATOR_SEND_TIMEOUT = i32;
pub type EAP_CONFIG_INPUT_FIELD_TYPE = i32;
pub type EAP_INTERACTIVE_UI_DATA_TYPE = i32;
pub type EAP_METHOD_AUTHENTICATOR_RESPONSE_ACTION = i32;
pub type EAP_METHOD_PROPERTY_TYPE = i32;
pub type EAP_METHOD_PROPERTY_VALUE_TYPE = i32;
pub type EapCode = i32;
pub type EapCredentialType = i32;
pub type EapHostPeerAuthParams = i32;
pub type EapHostPeerMethodResultReason = i32;
pub type EapHostPeerResponseAction = i32;
pub type EapPeerMethodResponseAction = i32;
pub type EapPeerMethodResultReason = i32;
pub type ISOLATION_STATE = i32;
pub type PPP_EAP_ACTION = i32;
pub type RAS_AUTH_ATTRIBUTE_TYPE = i32;
#[repr(C)]
pub struct EAPHOST_AUTH_INFO {
    pub status: EAPHOST_AUTH_STATUS,
    pub dwErrorCode: u32,
    pub dwReasonCode: u32,
}
impl ::core::marker::Copy for EAPHOST_AUTH_INFO {}
impl ::core::clone::Clone for EAPHOST_AUTH_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAPHOST_IDENTITY_UI_PARAMS {
    pub eapMethodType: EAP_METHOD_TYPE,
    pub dwFlags: u32,
    pub dwSizeofConnectionData: u32,
    pub pConnectionData: *mut u8,
    pub dwSizeofUserData: u32,
    pub pUserData: *mut u8,
    pub dwSizeofUserDataOut: u32,
    pub pUserDataOut: *mut u8,
    pub pwszIdentity: ::windows_sys::core::PWSTR,
    pub dwError: u32,
    pub pEapError: *mut EAP_ERROR,
}
impl ::core::marker::Copy for EAPHOST_IDENTITY_UI_PARAMS {}
impl ::core::clone::Clone for EAPHOST_IDENTITY_UI_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAPHOST_INTERACTIVE_UI_PARAMS {
    pub dwSizeofContextData: u32,
    pub pContextData: *mut u8,
    pub dwSizeofInteractiveUIData: u32,
    pub pInteractiveUIData: *mut u8,
    pub dwError: u32,
    pub pEapError: *mut EAP_ERROR,
}
impl ::core::marker::Copy for EAPHOST_INTERACTIVE_UI_PARAMS {}
impl ::core::clone::Clone for EAPHOST_INTERACTIVE_UI_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_ATTRIBUTE {
    pub eaType: EAP_ATTRIBUTE_TYPE,
    pub dwLength: u32,
    pub pValue: *mut u8,
}
impl ::core::marker::Copy for EAP_ATTRIBUTE {}
impl ::core::clone::Clone for EAP_ATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_ATTRIBUTES {
    pub dwNumberOfAttributes: u32,
    pub pAttribs: *mut EAP_ATTRIBUTE,
}
impl ::core::marker::Copy for EAP_ATTRIBUTES {}
impl ::core::clone::Clone for EAP_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_AUTHENTICATOR_METHOD_ROUTINES {
    pub dwSizeInBytes: u32,
    pub pEapType: *mut EAP_METHOD_TYPE,
    pub EapMethodAuthenticatorInitialize: isize,
    pub EapMethodAuthenticatorBeginSession: isize,
    pub EapMethodAuthenticatorUpdateInnerMethodParams: isize,
    pub EapMethodAuthenticatorReceivePacket: isize,
    pub EapMethodAuthenticatorSendPacket: isize,
    pub EapMethodAuthenticatorGetAttributes: isize,
    pub EapMethodAuthenticatorSetAttributes: isize,
    pub EapMethodAuthenticatorGetResult: isize,
    pub EapMethodAuthenticatorEndSession: isize,
    pub EapMethodAuthenticatorShutdown: isize,
}
impl ::core::marker::Copy for EAP_AUTHENTICATOR_METHOD_ROUTINES {}
impl ::core::clone::Clone for EAP_AUTHENTICATOR_METHOD_ROUTINES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_CONFIG_INPUT_FIELD_ARRAY {
    pub dwVersion: u32,
    pub dwNumberOfFields: u32,
    pub pFields: *mut EAP_CONFIG_INPUT_FIELD_DATA,
}
impl ::core::marker::Copy for EAP_CONFIG_INPUT_FIELD_ARRAY {}
impl ::core::clone::Clone for EAP_CONFIG_INPUT_FIELD_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_CONFIG_INPUT_FIELD_DATA {
    pub dwSize: u32,
    pub Type: EAP_CONFIG_INPUT_FIELD_TYPE,
    pub dwFlagProps: u32,
    pub pwszLabel: ::windows_sys::core::PWSTR,
    pub pwszData: ::windows_sys::core::PWSTR,
    pub dwMinDataLength: u32,
    pub dwMaxDataLength: u32,
}
impl ::core::marker::Copy for EAP_CONFIG_INPUT_FIELD_DATA {}
impl ::core::clone::Clone for EAP_CONFIG_INPUT_FIELD_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_CRED_EXPIRY_REQ {
    pub curCreds: EAP_CONFIG_INPUT_FIELD_ARRAY,
    pub newCreds: EAP_CONFIG_INPUT_FIELD_ARRAY,
}
impl ::core::marker::Copy for EAP_CRED_EXPIRY_REQ {}
impl ::core::clone::Clone for EAP_CRED_EXPIRY_REQ {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_ERROR {
    pub dwWinError: u32,
    pub r#type: EAP_METHOD_TYPE,
    pub dwReasonCode: u32,
    pub rootCauseGuid: ::windows_sys::core::GUID,
    pub repairGuid: ::windows_sys::core::GUID,
    pub helpLinkGuid: ::windows_sys::core::GUID,
    pub pRootCauseString: ::windows_sys::core::PWSTR,
    pub pRepairString: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for EAP_ERROR {}
impl ::core::clone::Clone for EAP_ERROR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_INTERACTIVE_UI_DATA {
    pub dwVersion: u32,
    pub dwSize: u32,
    pub dwDataType: EAP_INTERACTIVE_UI_DATA_TYPE,
    pub cbUiData: u32,
    pub pbUiData: EAP_UI_DATA_FORMAT,
}
impl ::core::marker::Copy for EAP_INTERACTIVE_UI_DATA {}
impl ::core::clone::Clone for EAP_INTERACTIVE_UI_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EAP_METHOD_AUTHENTICATOR_RESULT {
    pub fIsSuccess: super::super::Foundation::BOOL,
    pub dwFailureReason: u32,
    pub pAuthAttribs: *mut EAP_ATTRIBUTES,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EAP_METHOD_AUTHENTICATOR_RESULT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EAP_METHOD_AUTHENTICATOR_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_METHOD_INFO {
    pub eaptype: EAP_METHOD_TYPE,
    pub pwszAuthorName: ::windows_sys::core::PWSTR,
    pub pwszFriendlyName: ::windows_sys::core::PWSTR,
    pub eapProperties: u32,
    pub pInnerMethodInfo: *mut EAP_METHOD_INFO,
}
impl ::core::marker::Copy for EAP_METHOD_INFO {}
impl ::core::clone::Clone for EAP_METHOD_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_METHOD_INFO_ARRAY {
    pub dwNumberOfMethods: u32,
    pub pEapMethods: *mut EAP_METHOD_INFO,
}
impl ::core::marker::Copy for EAP_METHOD_INFO_ARRAY {}
impl ::core::clone::Clone for EAP_METHOD_INFO_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_METHOD_INFO_ARRAY_EX {
    pub dwNumberOfMethods: u32,
    pub pEapMethods: *mut EAP_METHOD_INFO_EX,
}
impl ::core::marker::Copy for EAP_METHOD_INFO_ARRAY_EX {}
impl ::core::clone::Clone for EAP_METHOD_INFO_ARRAY_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_METHOD_INFO_EX {
    pub eaptype: EAP_METHOD_TYPE,
    pub pwszAuthorName: ::windows_sys::core::PWSTR,
    pub pwszFriendlyName: ::windows_sys::core::PWSTR,
    pub eapProperties: u32,
    pub pInnerMethodInfoArray: *mut EAP_METHOD_INFO_ARRAY_EX,
}
impl ::core::marker::Copy for EAP_METHOD_INFO_EX {}
impl ::core::clone::Clone for EAP_METHOD_INFO_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EAP_METHOD_PROPERTY {
    pub eapMethodPropertyType: EAP_METHOD_PROPERTY_TYPE,
    pub eapMethodPropertyValueType: EAP_METHOD_PROPERTY_VALUE_TYPE,
    pub eapMethodPropertyValue: EAP_METHOD_PROPERTY_VALUE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EAP_METHOD_PROPERTY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EAP_METHOD_PROPERTY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EAP_METHOD_PROPERTY_ARRAY {
    pub dwNumberOfProperties: u32,
    pub pMethodProperty: *mut EAP_METHOD_PROPERTY,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EAP_METHOD_PROPERTY_ARRAY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EAP_METHOD_PROPERTY_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union EAP_METHOD_PROPERTY_VALUE {
    pub empvBool: EAP_METHOD_PROPERTY_VALUE_BOOL,
    pub empvDword: EAP_METHOD_PROPERTY_VALUE_DWORD,
    pub empvString: EAP_METHOD_PROPERTY_VALUE_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EAP_METHOD_PROPERTY_VALUE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EAP_METHOD_PROPERTY_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EAP_METHOD_PROPERTY_VALUE_BOOL {
    pub length: u32,
    pub value: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EAP_METHOD_PROPERTY_VALUE_BOOL {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EAP_METHOD_PROPERTY_VALUE_BOOL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_METHOD_PROPERTY_VALUE_DWORD {
    pub length: u32,
    pub value: u32,
}
impl ::core::marker::Copy for EAP_METHOD_PROPERTY_VALUE_DWORD {}
impl ::core::clone::Clone for EAP_METHOD_PROPERTY_VALUE_DWORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_METHOD_PROPERTY_VALUE_STRING {
    pub length: u32,
    pub value: *mut u8,
}
impl ::core::marker::Copy for EAP_METHOD_PROPERTY_VALUE_STRING {}
impl ::core::clone::Clone for EAP_METHOD_PROPERTY_VALUE_STRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_METHOD_TYPE {
    pub eapType: EAP_TYPE,
    pub dwAuthorId: u32,
}
impl ::core::marker::Copy for EAP_METHOD_TYPE {}
impl ::core::clone::Clone for EAP_METHOD_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_PEER_METHOD_ROUTINES {
    pub dwVersion: u32,
    pub pEapType: *mut EAP_TYPE,
    pub EapPeerInitialize: isize,
    pub EapPeerGetIdentity: isize,
    pub EapPeerBeginSession: isize,
    pub EapPeerSetCredentials: isize,
    pub EapPeerProcessRequestPacket: isize,
    pub EapPeerGetResponsePacket: isize,
    pub EapPeerGetResult: isize,
    pub EapPeerGetUIContext: isize,
    pub EapPeerSetUIContext: isize,
    pub EapPeerGetResponseAttributes: isize,
    pub EapPeerSetResponseAttributes: isize,
    pub EapPeerEndSession: isize,
    pub EapPeerShutdown: isize,
}
impl ::core::marker::Copy for EAP_PEER_METHOD_ROUTINES {}
impl ::core::clone::Clone for EAP_PEER_METHOD_ROUTINES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EAP_TYPE {
    pub r#type: u8,
    pub dwVendorId: u32,
    pub dwVendorType: u32,
}
impl ::core::marker::Copy for EAP_TYPE {}
impl ::core::clone::Clone for EAP_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union EAP_UI_DATA_FORMAT {
    pub credData: *mut EAP_CONFIG_INPUT_FIELD_ARRAY,
    pub credExpiryData: *mut EAP_CRED_EXPIRY_REQ,
    pub credLogonData: *mut EAP_CONFIG_INPUT_FIELD_ARRAY,
}
impl ::core::marker::Copy for EAP_UI_DATA_FORMAT {}
impl ::core::clone::Clone for EAP_UI_DATA_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EapCertificateCredential {
    pub certHash: [u8; 20],
    pub password: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for EapCertificateCredential {}
impl ::core::clone::Clone for EapCertificateCredential {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EapCredential {
    pub credType: EapCredentialType,
    pub credData: EapCredentialTypeData,
}
impl ::core::marker::Copy for EapCredential {}
impl ::core::clone::Clone for EapCredential {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union EapCredentialTypeData {
    pub username_password: EapUsernamePasswordCredential,
    pub certificate: EapCertificateCredential,
    pub sim: EapSimCredential,
}
impl ::core::marker::Copy for EapCredentialTypeData {}
impl ::core::clone::Clone for EapCredentialTypeData {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EapHostPeerMethodResult {
    pub fIsSuccess: super::super::Foundation::BOOL,
    pub dwFailureReasonCode: u32,
    pub fSaveConnectionData: super::super::Foundation::BOOL,
    pub dwSizeofConnectionData: u32,
    pub pConnectionData: *mut u8,
    pub fSaveUserData: super::super::Foundation::BOOL,
    pub dwSizeofUserData: u32,
    pub pUserData: *mut u8,
    pub pAttribArray: *mut EAP_ATTRIBUTES,
    pub isolationState: ISOLATION_STATE,
    pub pEapMethodInfo: *mut EAP_METHOD_INFO,
    pub pEapError: *mut EAP_ERROR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EapHostPeerMethodResult {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EapHostPeerMethodResult {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EapPacket {
    pub Code: u8,
    pub Id: u8,
    pub Length: [u8; 2],
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for EapPacket {}
impl ::core::clone::Clone for EapPacket {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EapPeerMethodOutput {
    pub action: EapPeerMethodResponseAction,
    pub fAllowNotifications: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EapPeerMethodOutput {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EapPeerMethodOutput {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct EapPeerMethodResult {
    pub fIsSuccess: super::super::Foundation::BOOL,
    pub dwFailureReasonCode: u32,
    pub fSaveConnectionData: super::super::Foundation::BOOL,
    pub dwSizeofConnectionData: u32,
    pub pConnectionData: *mut u8,
    pub fSaveUserData: super::super::Foundation::BOOL,
    pub dwSizeofUserData: u32,
    pub pUserData: *mut u8,
    pub pAttribArray: *mut EAP_ATTRIBUTES,
    pub pEapError: *mut EAP_ERROR,
    pub pNgcKerbTicket: *mut NgcTicketContext,
    pub fSaveToCredMan: super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for EapPeerMethodResult {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for EapPeerMethodResult {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EapSimCredential {
    pub iccID: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for EapSimCredential {}
impl ::core::clone::Clone for EapSimCredential {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EapUsernamePasswordCredential {
    pub username: ::windows_sys::core::PWSTR,
    pub password: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for EapUsernamePasswordCredential {}
impl ::core::clone::Clone for EapUsernamePasswordCredential {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LEGACY_IDENTITY_UI_PARAMS {
    pub eapType: u32,
    pub dwFlags: u32,
    pub dwSizeofConnectionData: u32,
    pub pConnectionData: *mut u8,
    pub dwSizeofUserData: u32,
    pub pUserData: *mut u8,
    pub dwSizeofUserDataOut: u32,
    pub pUserDataOut: *mut u8,
    pub pwszIdentity: ::windows_sys::core::PWSTR,
    pub dwError: u32,
}
impl ::core::marker::Copy for LEGACY_IDENTITY_UI_PARAMS {}
impl ::core::clone::Clone for LEGACY_IDENTITY_UI_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LEGACY_INTERACTIVE_UI_PARAMS {
    pub eapType: u32,
    pub dwSizeofContextData: u32,
    pub pContextData: *mut u8,
    pub dwSizeofInteractiveUIData: u32,
    pub pInteractiveUIData: *mut u8,
    pub dwError: u32,
}
impl ::core::marker::Copy for LEGACY_INTERACTIVE_UI_PARAMS {}
impl ::core::clone::Clone for LEGACY_INTERACTIVE_UI_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct NgcTicketContext {
    pub wszTicket: [u16; 45],
    pub hKey: super::Cryptography::NCRYPT_KEY_HANDLE,
    pub hImpersonateToken: super::super::Foundation::HANDLE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for NgcTicketContext {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for NgcTicketContext {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_EAP_INFO {
    pub dwSizeInBytes: u32,
    pub dwEapTypeId: u32,
    pub RasEapInitialize: isize,
    pub RasEapBegin: isize,
    pub RasEapEnd: isize,
    pub RasEapMakeMessage: isize,
}
impl ::core::marker::Copy for PPP_EAP_INFO {}
impl ::core::clone::Clone for PPP_EAP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PPP_EAP_INPUT {
    pub dwSizeInBytes: u32,
    pub fFlags: u32,
    pub fAuthenticator: super::super::Foundation::BOOL,
    pub pwszIdentity: ::windows_sys::core::PWSTR,
    pub pwszPassword: ::windows_sys::core::PWSTR,
    pub bInitialId: u8,
    pub pUserAttributes: *mut RAS_AUTH_ATTRIBUTE,
    pub fAuthenticationComplete: super::super::Foundation::BOOL,
    pub dwAuthResultCode: u32,
    pub hTokenImpersonateUser: super::super::Foundation::HANDLE,
    pub fSuccessPacketReceived: super::super::Foundation::BOOL,
    pub fDataReceivedFromInteractiveUI: super::super::Foundation::BOOL,
    pub pDataFromInteractiveUI: *mut u8,
    pub dwSizeOfDataFromInteractiveUI: u32,
    pub pConnectionData: *mut u8,
    pub dwSizeOfConnectionData: u32,
    pub pUserData: *mut u8,
    pub dwSizeOfUserData: u32,
    pub hReserved: super::super::Foundation::HANDLE,
    pub guidConnectionId: ::windows_sys::core::GUID,
    pub isVpn: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PPP_EAP_INPUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PPP_EAP_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct PPP_EAP_OUTPUT {
    pub dwSizeInBytes: u32,
    pub Action: PPP_EAP_ACTION,
    pub dwAuthResultCode: u32,
    pub pUserAttributes: *mut RAS_AUTH_ATTRIBUTE,
    pub fInvokeInteractiveUI: super::super::Foundation::BOOL,
    pub pUIContextData: *mut u8,
    pub dwSizeOfUIContextData: u32,
    pub fSaveConnectionData: super::super::Foundation::BOOL,
    pub pConnectionData: *mut u8,
    pub dwSizeOfConnectionData: u32,
    pub fSaveUserData: super::super::Foundation::BOOL,
    pub pUserData: *mut u8,
    pub dwSizeOfUserData: u32,
    pub pNgcKerbTicket: *mut NgcTicketContext,
    pub fSaveToCredMan: super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for PPP_EAP_OUTPUT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for PPP_EAP_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_EAP_PACKET {
    pub Code: u8,
    pub Id: u8,
    pub Length: [u8; 2],
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for PPP_EAP_PACKET {}
impl ::core::clone::Clone for PPP_EAP_PACKET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAS_AUTH_ATTRIBUTE {
    pub raaType: RAS_AUTH_ATTRIBUTE_TYPE,
    pub dwLength: u32,
    pub Value: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for RAS_AUTH_ATTRIBUTE {}
impl ::core::clone::Clone for RAS_AUTH_ATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
pub type NotificationHandler = ::core::option::Option<unsafe extern "system" fn(connectionid: ::windows_sys::core::GUID, pcontextdata: *mut ::core::ffi::c_void) -> ()>;
