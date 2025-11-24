::windows_targets::link!("wsdapi.dll" "system" fn WSDAllocateLinkedMemory(pparent : *mut ::core::ffi::c_void, cbsize : usize) -> *mut ::core::ffi::c_void);
::windows_targets::link!("wsdapi.dll" "system" fn WSDAttachLinkedMemory(pparent : *mut ::core::ffi::c_void, pchild : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDeviceHost(pszlocalid : ::windows_sys::core::PCWSTR, pcontext : IWSDXMLContext, ppdevicehost : *mut IWSDDeviceHost) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDeviceHost2(pszlocalid : ::windows_sys::core::PCWSTR, pcontext : IWSDXMLContext, pconfigparams : *const WSD_CONFIG_PARAM, dwconfigparamcount : u32, ppdevicehost : *mut IWSDDeviceHost) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDeviceHostAdvanced(pszlocalid : ::windows_sys::core::PCWSTR, pcontext : IWSDXMLContext, pphostaddresses : *const IWSDAddress, dwhostaddresscount : u32, ppdevicehost : *mut IWSDDeviceHost) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDeviceProxy(pszdeviceid : ::windows_sys::core::PCWSTR, pszlocalid : ::windows_sys::core::PCWSTR, pcontext : IWSDXMLContext, ppdeviceproxy : *mut IWSDDeviceProxy) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDeviceProxy2(pszdeviceid : ::windows_sys::core::PCWSTR, pszlocalid : ::windows_sys::core::PCWSTR, pcontext : IWSDXMLContext, pconfigparams : *const WSD_CONFIG_PARAM, dwconfigparamcount : u32, ppdeviceproxy : *mut IWSDDeviceProxy) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDeviceProxyAdvanced(pszdeviceid : ::windows_sys::core::PCWSTR, pdeviceaddress : IWSDAddress, pszlocalid : ::windows_sys::core::PCWSTR, pcontext : IWSDXMLContext, ppdeviceproxy : *mut IWSDDeviceProxy) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDiscoveryProvider(pcontext : IWSDXMLContext, ppprovider : *mut IWSDiscoveryProvider) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDiscoveryProvider2(pcontext : IWSDXMLContext, pconfigparams : *const WSD_CONFIG_PARAM, dwconfigparamcount : u32, ppprovider : *mut IWSDiscoveryProvider) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDiscoveryPublisher(pcontext : IWSDXMLContext, pppublisher : *mut IWSDiscoveryPublisher) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateDiscoveryPublisher2(pcontext : IWSDXMLContext, pconfigparams : *const WSD_CONFIG_PARAM, dwconfigparamcount : u32, pppublisher : *mut IWSDiscoveryPublisher) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateHttpAddress(ppaddress : *mut IWSDHttpAddress) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateHttpMessageParameters(pptxparams : *mut IWSDHttpMessageParameters) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateOutboundAttachment(ppattachment : *mut IWSDOutboundAttachment) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateUdpAddress(ppaddress : *mut IWSDUdpAddress) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDCreateUdpMessageParameters(pptxparams : *mut IWSDUdpMessageParameters) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDDetachLinkedMemory(pvoid : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("wsdapi.dll" "system" fn WSDFreeLinkedMemory(pvoid : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("wsdapi.dll" "system" fn WSDGenerateFault(pszcode : ::windows_sys::core::PCWSTR, pszsubcode : ::windows_sys::core::PCWSTR, pszreason : ::windows_sys::core::PCWSTR, pszdetail : ::windows_sys::core::PCWSTR, pcontext : IWSDXMLContext, ppfault : *mut *mut WSD_SOAP_FAULT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDGenerateFaultEx(pcode : *const WSDXML_NAME, psubcode : *const WSDXML_NAME, preasons : *const WSD_LOCALIZED_STRING_LIST, pszdetail : ::windows_sys::core::PCWSTR, ppfault : *mut *mut WSD_SOAP_FAULT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDGetConfigurationOption(dwoption : u32, pvoid : *mut ::core::ffi::c_void, cboutbuffer : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDSetConfigurationOption(dwoption : u32, pvoid : *const ::core::ffi::c_void, cbinbuffer : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDUriDecode(source : ::windows_sys::core::PCWSTR, cchsource : u32, destout : *mut ::windows_sys::core::PWSTR, cchdestout : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDUriEncode(source : ::windows_sys::core::PCWSTR, cchsource : u32, destout : *mut ::windows_sys::core::PWSTR, cchdestout : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDXMLAddChild(pparent : *mut WSDXML_ELEMENT, pchild : *mut WSDXML_ELEMENT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDXMLAddSibling(pfirst : *mut WSDXML_ELEMENT, psecond : *mut WSDXML_ELEMENT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDXMLBuildAnyForSingleElement(pelementname : *mut WSDXML_NAME, psztext : ::windows_sys::core::PCWSTR, ppany : *mut *mut WSDXML_ELEMENT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDXMLCleanupElement(pany : *mut WSDXML_ELEMENT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDXMLCreateContext(ppcontext : *mut IWSDXMLContext) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDXMLGetNameFromBuiltinNamespace(psznamespace : ::windows_sys::core::PCWSTR, pszname : ::windows_sys::core::PCWSTR, ppname : *mut *mut WSDXML_NAME) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wsdapi.dll" "system" fn WSDXMLGetValueFromAny(psznamespace : ::windows_sys::core::PCWSTR, pszname : ::windows_sys::core::PCWSTR, pany : *mut WSDXML_ELEMENT, ppszvalue : *mut ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
pub type IWSDAddress = *mut ::core::ffi::c_void;
pub type IWSDAsyncCallback = *mut ::core::ffi::c_void;
pub type IWSDAsyncResult = *mut ::core::ffi::c_void;
pub type IWSDAttachment = *mut ::core::ffi::c_void;
pub type IWSDDeviceHost = *mut ::core::ffi::c_void;
pub type IWSDDeviceHostNotify = *mut ::core::ffi::c_void;
pub type IWSDDeviceProxy = *mut ::core::ffi::c_void;
pub type IWSDEndpointProxy = *mut ::core::ffi::c_void;
pub type IWSDEventingStatus = *mut ::core::ffi::c_void;
pub type IWSDHttpAddress = *mut ::core::ffi::c_void;
pub type IWSDHttpAuthParameters = *mut ::core::ffi::c_void;
pub type IWSDHttpMessageParameters = *mut ::core::ffi::c_void;
pub type IWSDInboundAttachment = *mut ::core::ffi::c_void;
pub type IWSDMessageParameters = *mut ::core::ffi::c_void;
pub type IWSDMetadataExchange = *mut ::core::ffi::c_void;
pub type IWSDOutboundAttachment = *mut ::core::ffi::c_void;
pub type IWSDSSLClientCertificate = *mut ::core::ffi::c_void;
pub type IWSDScopeMatchingRule = *mut ::core::ffi::c_void;
pub type IWSDServiceMessaging = *mut ::core::ffi::c_void;
pub type IWSDServiceProxy = *mut ::core::ffi::c_void;
pub type IWSDServiceProxyEventing = *mut ::core::ffi::c_void;
pub type IWSDSignatureProperty = *mut ::core::ffi::c_void;
pub type IWSDTransportAddress = *mut ::core::ffi::c_void;
pub type IWSDUdpAddress = *mut ::core::ffi::c_void;
pub type IWSDUdpMessageParameters = *mut ::core::ffi::c_void;
pub type IWSDXMLContext = *mut ::core::ffi::c_void;
pub type IWSDiscoveredService = *mut ::core::ffi::c_void;
pub type IWSDiscoveryProvider = *mut ::core::ffi::c_void;
pub type IWSDiscoveryProviderNotify = *mut ::core::ffi::c_void;
pub type IWSDiscoveryPublisher = *mut ::core::ffi::c_void;
pub type IWSDiscoveryPublisherNotify = *mut ::core::ffi::c_void;
pub const DirectedDiscovery: DeviceDiscoveryMechanism = 1i32;
pub const MulticastDiscovery: DeviceDiscoveryMechanism = 0i32;
pub const ONE_WAY: WSDUdpMessageType = 0i32;
pub const OpAnyElement: WSDXML_OP = 6i32;
pub const OpAnyElements: WSDXML_OP = 7i32;
pub const OpAnyNumber: WSDXML_OP = 17i32;
pub const OpAnyText: WSDXML_OP = 8i32;
pub const OpAnything: WSDXML_OP = 16i32;
pub const OpAttribute_: WSDXML_OP = 9i32;
pub const OpBeginAll: WSDXML_OP = 14i32;
pub const OpBeginAnyElement: WSDXML_OP = 3i32;
pub const OpBeginChoice: WSDXML_OP = 10i32;
pub const OpBeginElement_: WSDXML_OP = 2i32;
pub const OpBeginSequence: WSDXML_OP = 12i32;
pub const OpElement_: WSDXML_OP = 5i32;
pub const OpEndAll: WSDXML_OP = 15i32;
pub const OpEndChoice: WSDXML_OP = 11i32;
pub const OpEndElement: WSDXML_OP = 4i32;
pub const OpEndOfTable: WSDXML_OP = 1i32;
pub const OpEndSequence: WSDXML_OP = 13i32;
pub const OpFormatBool_: WSDXML_OP = 20i32;
pub const OpFormatDateTime_: WSDXML_OP = 40i32;
pub const OpFormatDom_: WSDXML_OP = 30i32;
pub const OpFormatDouble_: WSDXML_OP = 42i32;
pub const OpFormatDuration_: WSDXML_OP = 39i32;
pub const OpFormatDynamicType_: WSDXML_OP = 37i32;
pub const OpFormatFloat_: WSDXML_OP = 41i32;
pub const OpFormatInt16_: WSDXML_OP = 22i32;
pub const OpFormatInt32_: WSDXML_OP = 23i32;
pub const OpFormatInt64_: WSDXML_OP = 24i32;
pub const OpFormatInt8_: WSDXML_OP = 21i32;
pub const OpFormatListInsertTail_: WSDXML_OP = 35i32;
pub const OpFormatLookupType_: WSDXML_OP = 38i32;
pub const OpFormatMax: WSDXML_OP = 46i32;
pub const OpFormatName_: WSDXML_OP = 34i32;
pub const OpFormatStruct_: WSDXML_OP = 31i32;
pub const OpFormatType_: WSDXML_OP = 36i32;
pub const OpFormatUInt16_: WSDXML_OP = 26i32;
pub const OpFormatUInt32_: WSDXML_OP = 27i32;
pub const OpFormatUInt64_: WSDXML_OP = 28i32;
pub const OpFormatUInt8_: WSDXML_OP = 25i32;
pub const OpFormatUnicodeString_: WSDXML_OP = 29i32;
pub const OpFormatUri_: WSDXML_OP = 32i32;
pub const OpFormatUuidUri_: WSDXML_OP = 33i32;
pub const OpFormatXMLDeclaration_: WSDXML_OP = 45i32;
pub const OpNone: WSDXML_OP = 0i32;
pub const OpOneOrMore: WSDXML_OP = 18i32;
pub const OpOptional: WSDXML_OP = 19i32;
pub const OpProcess_: WSDXML_OP = 43i32;
pub const OpQualifiedAttribute_: WSDXML_OP = 44i32;
pub const SecureDirectedDiscovery: DeviceDiscoveryMechanism = 2i32;
pub const TWO_WAY: WSDUdpMessageType = 1i32;
pub const WSDAPI_ADDRESSFAMILY_IPV4: u32 = 1u32;
pub const WSDAPI_ADDRESSFAMILY_IPV6: u32 = 2u32;
pub const WSDAPI_COMPACTSIG_ACCEPT_ALL_MESSAGES: u32 = 1u32;
pub const WSDAPI_OPTION_MAX_INBOUND_MESSAGE_SIZE: u32 = 1u32;
pub const WSDAPI_OPTION_TRACE_XML_TO_DEBUGGER: u32 = 2u32;
pub const WSDAPI_OPTION_TRACE_XML_TO_FILE: u32 = 3u32;
pub const WSDAPI_SSL_CERT_APPLY_DEFAULT_CHECKS: u32 = 0u32;
pub const WSDAPI_SSL_CERT_IGNORE_EXPIRY: u32 = 2u32;
pub const WSDAPI_SSL_CERT_IGNORE_INVALID_CN: u32 = 16u32;
pub const WSDAPI_SSL_CERT_IGNORE_REVOCATION: u32 = 1u32;
pub const WSDAPI_SSL_CERT_IGNORE_UNKNOWN_CA: u32 = 8u32;
pub const WSDAPI_SSL_CERT_IGNORE_WRONG_USAGE: u32 = 4u32;
pub const WSDET_INCOMING_FAULT: WSDEventType = 2i32;
pub const WSDET_INCOMING_MESSAGE: WSDEventType = 1i32;
pub const WSDET_NONE: WSDEventType = 0i32;
pub const WSDET_RESPONSE_TIMEOUT: WSDEventType = 4i32;
pub const WSDET_TRANSMISSION_FAILURE: WSDEventType = 3i32;
pub const WSD_CONFIG_DEVICE_ADDRESSES: WSD_CONFIG_PARAM_TYPE = 10i32;
pub const WSD_CONFIG_HOSTING_ADDRESSES: WSD_CONFIG_PARAM_TYPE = 9i32;
pub const WSD_CONFIG_MAX_INBOUND_MESSAGE_SIZE: WSD_CONFIG_PARAM_TYPE = 1i32;
pub const WSD_CONFIG_MAX_OUTBOUND_MESSAGE_SIZE: WSD_CONFIG_PARAM_TYPE = 2i32;
pub const WSD_DEFAULT_EVENTING_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("http://*:5357/");
pub const WSD_DEFAULT_HOSTING_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("http://*:5357/");
pub const WSD_DEFAULT_SECURE_HOSTING_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("https://*:5358/");
pub const WSD_PT_ALL: WSD_PROTOCOL_TYPE = 255i32;
pub const WSD_PT_HTTP: WSD_PROTOCOL_TYPE = 2i32;
pub const WSD_PT_HTTPS: WSD_PROTOCOL_TYPE = 4i32;
pub const WSD_PT_NONE: WSD_PROTOCOL_TYPE = 0i32;
pub const WSD_PT_UDP: WSD_PROTOCOL_TYPE = 1i32;
pub const WSD_SECURITY_COMPACTSIG_SIGNING_CERT: WSD_CONFIG_PARAM_TYPE = 7i32;
pub const WSD_SECURITY_COMPACTSIG_VALIDATION: WSD_CONFIG_PARAM_TYPE = 8i32;
pub const WSD_SECURITY_HTTP_AUTH_SCHEME_NEGOTIATE: u32 = 1u32;
pub const WSD_SECURITY_HTTP_AUTH_SCHEME_NTLM: u32 = 2u32;
pub const WSD_SECURITY_REQUIRE_CLIENT_CERT_OR_HTTP_CLIENT_AUTH: WSD_CONFIG_PARAM_TYPE = 12i32;
pub const WSD_SECURITY_REQUIRE_HTTP_CLIENT_AUTH: WSD_CONFIG_PARAM_TYPE = 11i32;
pub const WSD_SECURITY_SSL_CERT_FOR_CLIENT_AUTH: WSD_CONFIG_PARAM_TYPE = 3i32;
pub const WSD_SECURITY_SSL_CLIENT_CERT_VALIDATION: WSD_CONFIG_PARAM_TYPE = 5i32;
pub const WSD_SECURITY_SSL_NEGOTIATE_CLIENT_CERT: WSD_CONFIG_PARAM_TYPE = 6i32;
pub const WSD_SECURITY_SSL_SERVER_CERT_VALIDATION: WSD_CONFIG_PARAM_TYPE = 4i32;
pub const WSD_SECURITY_USE_HTTP_CLIENT_AUTH: WSD_CONFIG_PARAM_TYPE = 13i32;
pub type DeviceDiscoveryMechanism = i32;
pub type WSDEventType = i32;
pub type WSDUdpMessageType = i32;
pub type WSDXML_OP = i32;
pub type WSD_CONFIG_PARAM_TYPE = i32;
pub type WSD_PROTOCOL_TYPE = i32;
#[repr(C)]
pub struct REQUESTBODY_GetStatus {
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for REQUESTBODY_GetStatus {}
impl ::core::clone::Clone for REQUESTBODY_GetStatus {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct REQUESTBODY_Renew {
    pub Expires: *mut WSD_EVENTING_EXPIRES,
    pub Any: *mut WSDXML_ELEMENT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for REQUESTBODY_Renew {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for REQUESTBODY_Renew {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct REQUESTBODY_Subscribe {
    pub EndTo: *mut WSD_ENDPOINT_REFERENCE,
    pub Delivery: *mut WSD_EVENTING_DELIVERY_MODE,
    pub Expires: *mut WSD_EVENTING_EXPIRES,
    pub Filter: *mut WSD_EVENTING_FILTER,
    pub Any: *mut WSDXML_ELEMENT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for REQUESTBODY_Subscribe {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for REQUESTBODY_Subscribe {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct REQUESTBODY_Unsubscribe {
    pub any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for REQUESTBODY_Unsubscribe {}
impl ::core::clone::Clone for REQUESTBODY_Unsubscribe {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESPONSEBODY_GetMetadata {
    pub Metadata: *mut WSD_METADATA_SECTION_LIST,
}
impl ::core::marker::Copy for RESPONSEBODY_GetMetadata {}
impl ::core::clone::Clone for RESPONSEBODY_GetMetadata {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESPONSEBODY_GetStatus {
    pub expires: *mut WSD_EVENTING_EXPIRES,
    pub any: *mut WSDXML_ELEMENT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESPONSEBODY_GetStatus {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESPONSEBODY_GetStatus {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESPONSEBODY_Renew {
    pub expires: *mut WSD_EVENTING_EXPIRES,
    pub any: *mut WSDXML_ELEMENT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESPONSEBODY_Renew {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESPONSEBODY_Renew {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESPONSEBODY_Subscribe {
    pub SubscriptionManager: *mut WSD_ENDPOINT_REFERENCE,
    pub expires: *mut WSD_EVENTING_EXPIRES,
    pub any: *mut WSDXML_ELEMENT,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESPONSEBODY_Subscribe {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESPONSEBODY_Subscribe {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESPONSEBODY_SubscriptionEnd {
    pub SubscriptionManager: *mut WSD_ENDPOINT_REFERENCE,
    pub Status: ::windows_sys::core::PCWSTR,
    pub Reason: *mut WSD_LOCALIZED_STRING,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for RESPONSEBODY_SubscriptionEnd {}
impl ::core::clone::Clone for RESPONSEBODY_SubscriptionEnd {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDUdpRetransmitParams {
    pub ulSendDelay: u32,
    pub ulRepeat: u32,
    pub ulRepeatMinDelay: u32,
    pub ulRepeatMaxDelay: u32,
    pub ulRepeatUpperDelay: u32,
}
impl ::core::marker::Copy for WSDUdpRetransmitParams {}
impl ::core::clone::Clone for WSDUdpRetransmitParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_ATTRIBUTE {
    pub Element: *mut WSDXML_ELEMENT,
    pub Next: *mut WSDXML_ATTRIBUTE,
    pub Name: *mut WSDXML_NAME,
    pub Value: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for WSDXML_ATTRIBUTE {}
impl ::core::clone::Clone for WSDXML_ATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_ELEMENT {
    pub Node: WSDXML_NODE,
    pub Name: *mut WSDXML_NAME,
    pub FirstAttribute: *mut WSDXML_ATTRIBUTE,
    pub FirstChild: *mut WSDXML_NODE,
    pub PrefixMappings: *mut WSDXML_PREFIX_MAPPING,
}
impl ::core::marker::Copy for WSDXML_ELEMENT {}
impl ::core::clone::Clone for WSDXML_ELEMENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_ELEMENT_LIST {
    pub Next: *mut WSDXML_ELEMENT_LIST,
    pub Element: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSDXML_ELEMENT_LIST {}
impl ::core::clone::Clone for WSDXML_ELEMENT_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_NAME {
    pub Space: *mut WSDXML_NAMESPACE,
    pub LocalName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for WSDXML_NAME {}
impl ::core::clone::Clone for WSDXML_NAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_NAMESPACE {
    pub Uri: ::windows_sys::core::PCWSTR,
    pub PreferredPrefix: ::windows_sys::core::PCWSTR,
    pub Names: *mut WSDXML_NAME,
    pub NamesCount: u16,
    pub Encoding: u16,
}
impl ::core::marker::Copy for WSDXML_NAMESPACE {}
impl ::core::clone::Clone for WSDXML_NAMESPACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_NODE {
    pub Type: i32,
    pub Parent: *mut WSDXML_ELEMENT,
    pub Next: *mut WSDXML_NODE,
}
impl WSDXML_NODE {
    pub const ElementType: i32 = 0i32;
    pub const TextType: i32 = 1i32;
}
impl ::core::marker::Copy for WSDXML_NODE {}
impl ::core::clone::Clone for WSDXML_NODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_PREFIX_MAPPING {
    pub Refs: u32,
    pub Next: *mut WSDXML_PREFIX_MAPPING,
    pub Space: *mut WSDXML_NAMESPACE,
    pub Prefix: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for WSDXML_PREFIX_MAPPING {}
impl ::core::clone::Clone for WSDXML_PREFIX_MAPPING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_TEXT {
    pub Node: WSDXML_NODE,
    pub Text: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for WSDXML_TEXT {}
impl ::core::clone::Clone for WSDXML_TEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSDXML_TYPE {
    pub Uri: ::windows_sys::core::PCWSTR,
    pub Table: *const u8,
}
impl ::core::marker::Copy for WSDXML_TYPE {}
impl ::core::clone::Clone for WSDXML_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_APP_SEQUENCE {
    pub InstanceId: u64,
    pub SequenceId: ::windows_sys::core::PCWSTR,
    pub MessageNumber: u64,
}
impl ::core::marker::Copy for WSD_APP_SEQUENCE {}
impl ::core::clone::Clone for WSD_APP_SEQUENCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_BYE {
    pub EndpointReference: *mut WSD_ENDPOINT_REFERENCE,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_BYE {}
impl ::core::clone::Clone for WSD_BYE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_CONFIG_ADDRESSES {
    pub addresses: *mut IWSDAddress,
    pub dwAddressCount: u32,
}
impl ::core::marker::Copy for WSD_CONFIG_ADDRESSES {}
impl ::core::clone::Clone for WSD_CONFIG_ADDRESSES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_CONFIG_PARAM {
    pub configParamType: WSD_CONFIG_PARAM_TYPE,
    pub pConfigData: *mut ::core::ffi::c_void,
    pub dwConfigDataSize: u32,
}
impl ::core::marker::Copy for WSD_CONFIG_PARAM {}
impl ::core::clone::Clone for WSD_CONFIG_PARAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WSD_DATETIME {
    pub isPositive: super::super::Foundation::BOOL,
    pub year: u32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u32,
    pub TZIsLocal: super::super::Foundation::BOOL,
    pub TZIsPositive: super::super::Foundation::BOOL,
    pub TZHour: u8,
    pub TZMinute: u8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WSD_DATETIME {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WSD_DATETIME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WSD_DURATION {
    pub isPositive: super::super::Foundation::BOOL,
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WSD_DURATION {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WSD_DURATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_ENDPOINT_REFERENCE {
    pub Address: ::windows_sys::core::PCWSTR,
    pub ReferenceProperties: WSD_REFERENCE_PROPERTIES,
    pub ReferenceParameters: WSD_REFERENCE_PARAMETERS,
    pub PortType: *mut WSDXML_NAME,
    pub ServiceName: *mut WSDXML_NAME,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_ENDPOINT_REFERENCE {}
impl ::core::clone::Clone for WSD_ENDPOINT_REFERENCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_ENDPOINT_REFERENCE_LIST {
    pub Next: *mut WSD_ENDPOINT_REFERENCE_LIST,
    pub Element: *mut WSD_ENDPOINT_REFERENCE,
}
impl ::core::marker::Copy for WSD_ENDPOINT_REFERENCE_LIST {}
impl ::core::clone::Clone for WSD_ENDPOINT_REFERENCE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_EVENT {
    pub Hr: ::windows_sys::core::HRESULT,
    pub EventType: u32,
    pub DispatchTag: ::windows_sys::core::PWSTR,
    pub HandlerContext: WSD_HANDLER_CONTEXT,
    pub Soap: *mut WSD_SOAP_MESSAGE,
    pub Operation: *mut WSD_OPERATION,
    pub MessageParameters: IWSDMessageParameters,
}
impl ::core::marker::Copy for WSD_EVENT {}
impl ::core::clone::Clone for WSD_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_EVENTING_DELIVERY_MODE {
    pub Mode: ::windows_sys::core::PCWSTR,
    pub Push: *mut WSD_EVENTING_DELIVERY_MODE_PUSH,
    pub Data: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for WSD_EVENTING_DELIVERY_MODE {}
impl ::core::clone::Clone for WSD_EVENTING_DELIVERY_MODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_EVENTING_DELIVERY_MODE_PUSH {
    pub NotifyTo: *mut WSD_ENDPOINT_REFERENCE,
}
impl ::core::marker::Copy for WSD_EVENTING_DELIVERY_MODE_PUSH {}
impl ::core::clone::Clone for WSD_EVENTING_DELIVERY_MODE_PUSH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WSD_EVENTING_EXPIRES {
    pub Duration: *mut WSD_DURATION,
    pub DateTime: *mut WSD_DATETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WSD_EVENTING_EXPIRES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WSD_EVENTING_EXPIRES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_EVENTING_FILTER {
    pub Dialect: ::windows_sys::core::PCWSTR,
    pub FilterAction: *mut WSD_EVENTING_FILTER_ACTION,
    pub Data: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for WSD_EVENTING_FILTER {}
impl ::core::clone::Clone for WSD_EVENTING_FILTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_EVENTING_FILTER_ACTION {
    pub Actions: *mut WSD_URI_LIST,
}
impl ::core::marker::Copy for WSD_EVENTING_FILTER_ACTION {}
impl ::core::clone::Clone for WSD_EVENTING_FILTER_ACTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_HANDLER_CONTEXT {
    pub Handler: PWSD_SOAP_MESSAGE_HANDLER,
    pub PVoid: *mut ::core::ffi::c_void,
    pub Unknown: ::windows_sys::core::IUnknown,
}
impl ::core::marker::Copy for WSD_HANDLER_CONTEXT {}
impl ::core::clone::Clone for WSD_HANDLER_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_HEADER_RELATESTO {
    pub RelationshipType: *mut WSDXML_NAME,
    pub MessageID: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for WSD_HEADER_RELATESTO {}
impl ::core::clone::Clone for WSD_HEADER_RELATESTO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_HELLO {
    pub EndpointReference: *mut WSD_ENDPOINT_REFERENCE,
    pub Types: *mut WSD_NAME_LIST,
    pub Scopes: *mut WSD_SCOPES,
    pub XAddrs: *mut WSD_URI_LIST,
    pub MetadataVersion: u64,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_HELLO {}
impl ::core::clone::Clone for WSD_HELLO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_HOST_METADATA {
    pub Host: *mut WSD_SERVICE_METADATA,
    pub Hosted: *mut WSD_SERVICE_METADATA_LIST,
}
impl ::core::marker::Copy for WSD_HOST_METADATA {}
impl ::core::clone::Clone for WSD_HOST_METADATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_LOCALIZED_STRING {
    pub lang: ::windows_sys::core::PCWSTR,
    pub String: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for WSD_LOCALIZED_STRING {}
impl ::core::clone::Clone for WSD_LOCALIZED_STRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_LOCALIZED_STRING_LIST {
    pub Next: *mut WSD_LOCALIZED_STRING_LIST,
    pub Element: *mut WSD_LOCALIZED_STRING,
}
impl ::core::marker::Copy for WSD_LOCALIZED_STRING_LIST {}
impl ::core::clone::Clone for WSD_LOCALIZED_STRING_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_METADATA_SECTION {
    pub Dialect: ::windows_sys::core::PCWSTR,
    pub Identifier: ::windows_sys::core::PCWSTR,
    pub Data: *mut ::core::ffi::c_void,
    pub MetadataReference: *mut WSD_ENDPOINT_REFERENCE,
    pub Location: ::windows_sys::core::PCWSTR,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_METADATA_SECTION {}
impl ::core::clone::Clone for WSD_METADATA_SECTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_METADATA_SECTION_LIST {
    pub Next: *mut WSD_METADATA_SECTION_LIST,
    pub Element: *mut WSD_METADATA_SECTION,
}
impl ::core::marker::Copy for WSD_METADATA_SECTION_LIST {}
impl ::core::clone::Clone for WSD_METADATA_SECTION_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_NAME_LIST {
    pub Next: *mut WSD_NAME_LIST,
    pub Element: *mut WSDXML_NAME,
}
impl ::core::marker::Copy for WSD_NAME_LIST {}
impl ::core::clone::Clone for WSD_NAME_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_OPERATION {
    pub RequestType: *mut WSDXML_TYPE,
    pub ResponseType: *mut WSDXML_TYPE,
    pub RequestStubFunction: WSD_STUB_FUNCTION,
}
impl ::core::marker::Copy for WSD_OPERATION {}
impl ::core::clone::Clone for WSD_OPERATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_PORT_TYPE {
    pub EncodedName: u32,
    pub OperationCount: u32,
    pub Operations: *mut WSD_OPERATION,
    pub ProtocolType: WSD_PROTOCOL_TYPE,
}
impl ::core::marker::Copy for WSD_PORT_TYPE {}
impl ::core::clone::Clone for WSD_PORT_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_PROBE {
    pub Types: *mut WSD_NAME_LIST,
    pub Scopes: *mut WSD_SCOPES,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_PROBE {}
impl ::core::clone::Clone for WSD_PROBE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_PROBE_MATCH {
    pub EndpointReference: *mut WSD_ENDPOINT_REFERENCE,
    pub Types: *mut WSD_NAME_LIST,
    pub Scopes: *mut WSD_SCOPES,
    pub XAddrs: *mut WSD_URI_LIST,
    pub MetadataVersion: u64,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_PROBE_MATCH {}
impl ::core::clone::Clone for WSD_PROBE_MATCH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_PROBE_MATCHES {
    pub ProbeMatch: *mut WSD_PROBE_MATCH_LIST,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_PROBE_MATCHES {}
impl ::core::clone::Clone for WSD_PROBE_MATCHES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_PROBE_MATCH_LIST {
    pub Next: *mut WSD_PROBE_MATCH_LIST,
    pub Element: *mut WSD_PROBE_MATCH,
}
impl ::core::marker::Copy for WSD_PROBE_MATCH_LIST {}
impl ::core::clone::Clone for WSD_PROBE_MATCH_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_REFERENCE_PARAMETERS {
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_REFERENCE_PARAMETERS {}
impl ::core::clone::Clone for WSD_REFERENCE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_REFERENCE_PROPERTIES {
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_REFERENCE_PROPERTIES {}
impl ::core::clone::Clone for WSD_REFERENCE_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_RELATIONSHIP_METADATA {
    pub Type: ::windows_sys::core::PCWSTR,
    pub Data: *mut WSD_HOST_METADATA,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_RELATIONSHIP_METADATA {}
impl ::core::clone::Clone for WSD_RELATIONSHIP_METADATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_RESOLVE {
    pub EndpointReference: *mut WSD_ENDPOINT_REFERENCE,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_RESOLVE {}
impl ::core::clone::Clone for WSD_RESOLVE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_RESOLVE_MATCH {
    pub EndpointReference: *mut WSD_ENDPOINT_REFERENCE,
    pub Types: *mut WSD_NAME_LIST,
    pub Scopes: *mut WSD_SCOPES,
    pub XAddrs: *mut WSD_URI_LIST,
    pub MetadataVersion: u64,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_RESOLVE_MATCH {}
impl ::core::clone::Clone for WSD_RESOLVE_MATCH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_RESOLVE_MATCHES {
    pub ResolveMatch: *mut WSD_RESOLVE_MATCH,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_RESOLVE_MATCHES {}
impl ::core::clone::Clone for WSD_RESOLVE_MATCHES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SCOPES {
    pub MatchBy: ::windows_sys::core::PCWSTR,
    pub Scopes: *mut WSD_URI_LIST,
}
impl ::core::marker::Copy for WSD_SCOPES {}
impl ::core::clone::Clone for WSD_SCOPES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct WSD_SECURITY_CERT_VALIDATION {
    pub certMatchArray: *mut *mut super::super::Security::Cryptography::CERT_CONTEXT,
    pub dwCertMatchArrayCount: u32,
    pub hCertMatchStore: super::super::Security::Cryptography::HCERTSTORE,
    pub hCertIssuerStore: super::super::Security::Cryptography::HCERTSTORE,
    pub dwCertCheckOptions: u32,
    pub pszCNGHashAlgId: ::windows_sys::core::PCWSTR,
    pub pbCertHash: *mut u8,
    pub dwCertHashSize: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WSD_SECURITY_CERT_VALIDATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WSD_SECURITY_CERT_VALIDATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct WSD_SECURITY_CERT_VALIDATION_V1 {
    pub certMatchArray: *mut *mut super::super::Security::Cryptography::CERT_CONTEXT,
    pub dwCertMatchArrayCount: u32,
    pub hCertMatchStore: super::super::Security::Cryptography::HCERTSTORE,
    pub hCertIssuerStore: super::super::Security::Cryptography::HCERTSTORE,
    pub dwCertCheckOptions: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WSD_SECURITY_CERT_VALIDATION_V1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WSD_SECURITY_CERT_VALIDATION_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct WSD_SECURITY_SIGNATURE_VALIDATION {
    pub signingCertArray: *mut *mut super::super::Security::Cryptography::CERT_CONTEXT,
    pub dwSigningCertArrayCount: u32,
    pub hSigningCertStore: super::super::Security::Cryptography::HCERTSTORE,
    pub dwFlags: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for WSD_SECURITY_SIGNATURE_VALIDATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for WSD_SECURITY_SIGNATURE_VALIDATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SERVICE_METADATA {
    pub EndpointReference: *mut WSD_ENDPOINT_REFERENCE_LIST,
    pub Types: *mut WSD_NAME_LIST,
    pub ServiceId: ::windows_sys::core::PCWSTR,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_SERVICE_METADATA {}
impl ::core::clone::Clone for WSD_SERVICE_METADATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SERVICE_METADATA_LIST {
    pub Next: *mut WSD_SERVICE_METADATA_LIST,
    pub Element: *mut WSD_SERVICE_METADATA,
}
impl ::core::marker::Copy for WSD_SERVICE_METADATA_LIST {}
impl ::core::clone::Clone for WSD_SERVICE_METADATA_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SOAP_FAULT {
    pub Code: *mut WSD_SOAP_FAULT_CODE,
    pub Reason: *mut WSD_SOAP_FAULT_REASON,
    pub Node: ::windows_sys::core::PCWSTR,
    pub Role: ::windows_sys::core::PCWSTR,
    pub Detail: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_SOAP_FAULT {}
impl ::core::clone::Clone for WSD_SOAP_FAULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SOAP_FAULT_CODE {
    pub Value: *mut WSDXML_NAME,
    pub Subcode: *mut WSD_SOAP_FAULT_SUBCODE,
}
impl ::core::marker::Copy for WSD_SOAP_FAULT_CODE {}
impl ::core::clone::Clone for WSD_SOAP_FAULT_CODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SOAP_FAULT_REASON {
    pub Text: *mut WSD_LOCALIZED_STRING_LIST,
}
impl ::core::marker::Copy for WSD_SOAP_FAULT_REASON {}
impl ::core::clone::Clone for WSD_SOAP_FAULT_REASON {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SOAP_FAULT_SUBCODE {
    pub Value: *mut WSDXML_NAME,
    pub Subcode: *mut WSD_SOAP_FAULT_SUBCODE,
}
impl ::core::marker::Copy for WSD_SOAP_FAULT_SUBCODE {}
impl ::core::clone::Clone for WSD_SOAP_FAULT_SUBCODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SOAP_HEADER {
    pub To: ::windows_sys::core::PCWSTR,
    pub Action: ::windows_sys::core::PCWSTR,
    pub MessageID: ::windows_sys::core::PCWSTR,
    pub RelatesTo: WSD_HEADER_RELATESTO,
    pub ReplyTo: *mut WSD_ENDPOINT_REFERENCE,
    pub From: *mut WSD_ENDPOINT_REFERENCE,
    pub FaultTo: *mut WSD_ENDPOINT_REFERENCE,
    pub AppSequence: *mut WSD_APP_SEQUENCE,
    pub AnyHeaders: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_SOAP_HEADER {}
impl ::core::clone::Clone for WSD_SOAP_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_SOAP_MESSAGE {
    pub Header: WSD_SOAP_HEADER,
    pub Body: *mut ::core::ffi::c_void,
    pub BodyType: *mut WSDXML_TYPE,
}
impl ::core::marker::Copy for WSD_SOAP_MESSAGE {}
impl ::core::clone::Clone for WSD_SOAP_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WSD_SYNCHRONOUS_RESPONSE_CONTEXT {
    pub hr: ::windows_sys::core::HRESULT,
    pub eventHandle: super::super::Foundation::HANDLE,
    pub messageParameters: IWSDMessageParameters,
    pub results: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WSD_SYNCHRONOUS_RESPONSE_CONTEXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WSD_SYNCHRONOUS_RESPONSE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_THIS_DEVICE_METADATA {
    pub FriendlyName: *mut WSD_LOCALIZED_STRING_LIST,
    pub FirmwareVersion: ::windows_sys::core::PCWSTR,
    pub SerialNumber: ::windows_sys::core::PCWSTR,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_THIS_DEVICE_METADATA {}
impl ::core::clone::Clone for WSD_THIS_DEVICE_METADATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_THIS_MODEL_METADATA {
    pub Manufacturer: *mut WSD_LOCALIZED_STRING_LIST,
    pub ManufacturerUrl: ::windows_sys::core::PCWSTR,
    pub ModelName: *mut WSD_LOCALIZED_STRING_LIST,
    pub ModelNumber: ::windows_sys::core::PCWSTR,
    pub ModelUrl: ::windows_sys::core::PCWSTR,
    pub PresentationUrl: ::windows_sys::core::PCWSTR,
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_THIS_MODEL_METADATA {}
impl ::core::clone::Clone for WSD_THIS_MODEL_METADATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_UNKNOWN_LOOKUP {
    pub Any: *mut WSDXML_ELEMENT,
}
impl ::core::marker::Copy for WSD_UNKNOWN_LOOKUP {}
impl ::core::clone::Clone for WSD_UNKNOWN_LOOKUP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WSD_URI_LIST {
    pub Next: *mut WSD_URI_LIST,
    pub Element: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for WSD_URI_LIST {}
impl ::core::clone::Clone for WSD_URI_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PWSD_SOAP_MESSAGE_HANDLER = ::core::option::Option<unsafe extern "system" fn(thisunknown: ::windows_sys::core::IUnknown, event: *mut WSD_EVENT) -> ::windows_sys::core::HRESULT>;
pub type WSD_STUB_FUNCTION = ::core::option::Option<unsafe extern "system" fn(server: ::windows_sys::core::IUnknown, session: IWSDServiceMessaging, event: *mut WSD_EVENT) -> ::windows_sys::core::HRESULT>;
