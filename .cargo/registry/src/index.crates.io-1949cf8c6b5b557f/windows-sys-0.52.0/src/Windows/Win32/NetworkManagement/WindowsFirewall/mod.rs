::windows_targets::link!("netshell.dll" "system" fn NcFreeNetconProperties(pprops : *mut NETCON_PROPERTIES) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("netshell.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NcIsValidConnectionName(pszwname : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" fn NetworkIsolationDiagnoseConnectFailureAndGetInfo(wszservername : ::windows_sys::core::PCWSTR, netisoerror : *mut NETISO_ERROR_TYPE) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NetworkIsolationEnumAppContainers(flags : u32, pdwnumpublicappcs : *mut u32, pppublicappcs : *mut *mut INET_FIREWALL_APP_CONTAINER) -> u32);
#[cfg(feature = "Win32_System_Ole")]
::windows_targets::link!("firewallapi.dll" "system" #[doc = "Required features: `\"Win32_System_Ole\"`"] fn NetworkIsolationEnumerateAppContainerRules(newenum : *mut super::super::System::Ole:: IEnumVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NetworkIsolationFreeAppContainers(ppublicappcs : *const INET_FIREWALL_APP_CONTAINER) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NetworkIsolationGetAppContainerConfig(pdwnumpublicappcs : *mut u32, appcontainersids : *mut *mut super::super::Security:: SID_AND_ATTRIBUTES) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("firewallapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NetworkIsolationGetEnterpriseIdAsync(wszservername : ::windows_sys::core::PCWSTR, dwflags : u32, context : *const ::core::ffi::c_void, callback : PNETISO_EDP_ID_CALLBACK_FN, hoperation : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("firewallapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NetworkIsolationGetEnterpriseIdClose(hoperation : super::super::Foundation:: HANDLE, bwaitforoperation : super::super::Foundation:: BOOL) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NetworkIsolationRegisterForAppContainerChanges(flags : u32, callback : PAC_CHANGES_CALLBACK_FN, context : *const ::core::ffi::c_void, registrationobject : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn NetworkIsolationSetAppContainerConfig(dwnumpublicappcs : u32, appcontainersids : *const super::super::Security:: SID_AND_ATTRIBUTES) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NetworkIsolationSetupAppContainerBinaries(applicationcontainersid : super::super::Foundation:: PSID, packagefullname : ::windows_sys::core::PCWSTR, packagefolder : ::windows_sys::core::PCWSTR, displayname : ::windows_sys::core::PCWSTR, bbinariesfullycomputed : super::super::Foundation:: BOOL, binaries : *const ::windows_sys::core::PCWSTR, binariescount : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-net-isolation-l1-1-0.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NetworkIsolationUnregisterForAppContainerChanges(registrationobject : super::super::Foundation:: HANDLE) -> u32);
pub type IDynamicPortMapping = *mut ::core::ffi::c_void;
pub type IDynamicPortMappingCollection = *mut ::core::ffi::c_void;
pub type IEnumNetConnection = *mut ::core::ffi::c_void;
pub type IEnumNetSharingEveryConnection = *mut ::core::ffi::c_void;
pub type IEnumNetSharingPortMapping = *mut ::core::ffi::c_void;
pub type IEnumNetSharingPrivateConnection = *mut ::core::ffi::c_void;
pub type IEnumNetSharingPublicConnection = *mut ::core::ffi::c_void;
pub type INATEventManager = *mut ::core::ffi::c_void;
pub type INATExternalIPAddressCallback = *mut ::core::ffi::c_void;
pub type INATNumberOfEntriesCallback = *mut ::core::ffi::c_void;
pub type INetConnection = *mut ::core::ffi::c_void;
pub type INetConnectionConnectUi = *mut ::core::ffi::c_void;
pub type INetConnectionManager = *mut ::core::ffi::c_void;
pub type INetConnectionProps = *mut ::core::ffi::c_void;
pub type INetFwAuthorizedApplication = *mut ::core::ffi::c_void;
pub type INetFwAuthorizedApplications = *mut ::core::ffi::c_void;
pub type INetFwIcmpSettings = *mut ::core::ffi::c_void;
pub type INetFwMgr = *mut ::core::ffi::c_void;
pub type INetFwOpenPort = *mut ::core::ffi::c_void;
pub type INetFwOpenPorts = *mut ::core::ffi::c_void;
pub type INetFwPolicy = *mut ::core::ffi::c_void;
pub type INetFwPolicy2 = *mut ::core::ffi::c_void;
pub type INetFwProduct = *mut ::core::ffi::c_void;
pub type INetFwProducts = *mut ::core::ffi::c_void;
pub type INetFwProfile = *mut ::core::ffi::c_void;
pub type INetFwRemoteAdminSettings = *mut ::core::ffi::c_void;
pub type INetFwRule = *mut ::core::ffi::c_void;
pub type INetFwRule2 = *mut ::core::ffi::c_void;
pub type INetFwRule3 = *mut ::core::ffi::c_void;
pub type INetFwRules = *mut ::core::ffi::c_void;
pub type INetFwService = *mut ::core::ffi::c_void;
pub type INetFwServiceRestriction = *mut ::core::ffi::c_void;
pub type INetFwServices = *mut ::core::ffi::c_void;
pub type INetSharingConfiguration = *mut ::core::ffi::c_void;
pub type INetSharingEveryConnectionCollection = *mut ::core::ffi::c_void;
pub type INetSharingManager = *mut ::core::ffi::c_void;
pub type INetSharingPortMapping = *mut ::core::ffi::c_void;
pub type INetSharingPortMappingCollection = *mut ::core::ffi::c_void;
pub type INetSharingPortMappingProps = *mut ::core::ffi::c_void;
pub type INetSharingPrivateConnectionCollection = *mut ::core::ffi::c_void;
pub type INetSharingPublicConnectionCollection = *mut ::core::ffi::c_void;
pub type IStaticPortMapping = *mut ::core::ffi::c_void;
pub type IStaticPortMappingCollection = *mut ::core::ffi::c_void;
pub type IUPnPNAT = *mut ::core::ffi::c_void;
pub const FW_DYNAMIC_KEYWORD_ADDRESS_ENUM_FLAGS_ALL: FW_DYNAMIC_KEYWORD_ADDRESS_ENUM_FLAGS = 3i32;
pub const FW_DYNAMIC_KEYWORD_ADDRESS_ENUM_FLAGS_AUTO_RESOLVE: FW_DYNAMIC_KEYWORD_ADDRESS_ENUM_FLAGS = 1i32;
pub const FW_DYNAMIC_KEYWORD_ADDRESS_ENUM_FLAGS_NON_AUTO_RESOLVE: FW_DYNAMIC_KEYWORD_ADDRESS_ENUM_FLAGS = 2i32;
pub const FW_DYNAMIC_KEYWORD_ADDRESS_FLAGS_AUTO_RESOLVE: FW_DYNAMIC_KEYWORD_ADDRESS_FLAGS = 1i32;
pub const FW_DYNAMIC_KEYWORD_ORIGIN_INVALID: FW_DYNAMIC_KEYWORD_ORIGIN_TYPE = 0i32;
pub const FW_DYNAMIC_KEYWORD_ORIGIN_LOCAL: FW_DYNAMIC_KEYWORD_ORIGIN_TYPE = 1i32;
pub const FW_DYNAMIC_KEYWORD_ORIGIN_MDM: FW_DYNAMIC_KEYWORD_ORIGIN_TYPE = 2i32;
pub const ICSSC_DEFAULT: SHARINGCONNECTION_ENUM_FLAGS = 0i32;
pub const ICSSC_ENABLED: SHARINGCONNECTION_ENUM_FLAGS = 1i32;
pub const ICSSHARINGTYPE_PRIVATE: SHARINGCONNECTIONTYPE = 1i32;
pub const ICSSHARINGTYPE_PUBLIC: SHARINGCONNECTIONTYPE = 0i32;
pub const ICSTT_IPADDRESS: ICS_TARGETTYPE = 1i32;
pub const ICSTT_NAME: ICS_TARGETTYPE = 0i32;
pub const INET_FIREWALL_AC_BINARY: INET_FIREWALL_AC_CREATION_TYPE = 2i32;
pub const INET_FIREWALL_AC_CHANGE_CREATE: INET_FIREWALL_AC_CHANGE_TYPE = 1i32;
pub const INET_FIREWALL_AC_CHANGE_DELETE: INET_FIREWALL_AC_CHANGE_TYPE = 2i32;
pub const INET_FIREWALL_AC_CHANGE_INVALID: INET_FIREWALL_AC_CHANGE_TYPE = 0i32;
pub const INET_FIREWALL_AC_CHANGE_MAX: INET_FIREWALL_AC_CHANGE_TYPE = 3i32;
pub const INET_FIREWALL_AC_MAX: INET_FIREWALL_AC_CREATION_TYPE = 4i32;
pub const INET_FIREWALL_AC_NONE: INET_FIREWALL_AC_CREATION_TYPE = 0i32;
pub const INET_FIREWALL_AC_PACKAGE_ID_ONLY: INET_FIREWALL_AC_CREATION_TYPE = 1i32;
pub const NCCF_ALLOW_DUPLICATION: NETCON_CHARACTERISTIC_FLAGS = 2i32;
pub const NCCF_ALLOW_REMOVAL: NETCON_CHARACTERISTIC_FLAGS = 4i32;
pub const NCCF_ALLOW_RENAME: NETCON_CHARACTERISTIC_FLAGS = 8i32;
pub const NCCF_ALL_USERS: NETCON_CHARACTERISTIC_FLAGS = 1i32;
pub const NCCF_BLUETOOTH_MASK: NETCON_CHARACTERISTIC_FLAGS = 983040i32;
pub const NCCF_BRANDED: NETCON_CHARACTERISTIC_FLAGS = 128i32;
pub const NCCF_BRIDGED: NETCON_CHARACTERISTIC_FLAGS = 512i32;
pub const NCCF_DEFAULT: NETCON_CHARACTERISTIC_FLAGS = 2048i32;
pub const NCCF_FIREWALLED: NETCON_CHARACTERISTIC_FLAGS = 1024i32;
pub const NCCF_HOMENET_CAPABLE: NETCON_CHARACTERISTIC_FLAGS = 4096i32;
pub const NCCF_HOSTED_NETWORK: NETCON_CHARACTERISTIC_FLAGS = 65536i32;
pub const NCCF_INCOMING_ONLY: NETCON_CHARACTERISTIC_FLAGS = 32i32;
pub const NCCF_LAN_MASK: NETCON_CHARACTERISTIC_FLAGS = 15728640i32;
pub const NCCF_NONE: NETCON_CHARACTERISTIC_FLAGS = 0i32;
pub const NCCF_OUTGOING_ONLY: NETCON_CHARACTERISTIC_FLAGS = 64i32;
pub const NCCF_QUARANTINED: NETCON_CHARACTERISTIC_FLAGS = 16384i32;
pub const NCCF_RESERVED: NETCON_CHARACTERISTIC_FLAGS = 32768i32;
pub const NCCF_SHARED: NETCON_CHARACTERISTIC_FLAGS = 256i32;
pub const NCCF_SHARED_PRIVATE: NETCON_CHARACTERISTIC_FLAGS = 8192i32;
pub const NCCF_VIRTUAL_STATION: NETCON_CHARACTERISTIC_FLAGS = 131072i32;
pub const NCCF_WIFI_DIRECT: NETCON_CHARACTERISTIC_FLAGS = 262144i32;
pub const NCME_DEFAULT: NETCONMGR_ENUM_FLAGS = 0i32;
pub const NCME_HIDDEN: NETCONMGR_ENUM_FLAGS = 1i32;
pub const NCM_BRIDGE: NETCON_MEDIATYPE = 7i32;
pub const NCM_DIRECT: NETCON_MEDIATYPE = 1i32;
pub const NCM_ISDN: NETCON_MEDIATYPE = 2i32;
pub const NCM_LAN: NETCON_MEDIATYPE = 3i32;
pub const NCM_NONE: NETCON_MEDIATYPE = 0i32;
pub const NCM_PHONE: NETCON_MEDIATYPE = 4i32;
pub const NCM_PPPOE: NETCON_MEDIATYPE = 6i32;
pub const NCM_SHAREDACCESSHOST_LAN: NETCON_MEDIATYPE = 8i32;
pub const NCM_SHAREDACCESSHOST_RAS: NETCON_MEDIATYPE = 9i32;
pub const NCM_TUNNEL: NETCON_MEDIATYPE = 5i32;
pub const NCS_ACTION_REQUIRED: NETCON_STATUS = 13i32;
pub const NCS_ACTION_REQUIRED_RETRY: NETCON_STATUS = 14i32;
pub const NCS_AUTHENTICATING: NETCON_STATUS = 8i32;
pub const NCS_AUTHENTICATION_FAILED: NETCON_STATUS = 10i32;
pub const NCS_AUTHENTICATION_SUCCEEDED: NETCON_STATUS = 9i32;
pub const NCS_CONNECTED: NETCON_STATUS = 2i32;
pub const NCS_CONNECTING: NETCON_STATUS = 1i32;
pub const NCS_CONNECT_FAILED: NETCON_STATUS = 15i32;
pub const NCS_CREDENTIALS_REQUIRED: NETCON_STATUS = 12i32;
pub const NCS_DISCONNECTED: NETCON_STATUS = 0i32;
pub const NCS_DISCONNECTING: NETCON_STATUS = 3i32;
pub const NCS_HARDWARE_DISABLED: NETCON_STATUS = 5i32;
pub const NCS_HARDWARE_MALFUNCTION: NETCON_STATUS = 6i32;
pub const NCS_HARDWARE_NOT_PRESENT: NETCON_STATUS = 4i32;
pub const NCS_INVALID_ADDRESS: NETCON_STATUS = 11i32;
pub const NCS_MEDIA_DISCONNECTED: NETCON_STATUS = 7i32;
pub const NCT_BRIDGE: NETCON_TYPE = 6i32;
pub const NCT_DIRECT_CONNECT: NETCON_TYPE = 0i32;
pub const NCT_INBOUND: NETCON_TYPE = 1i32;
pub const NCT_INTERNET: NETCON_TYPE = 2i32;
pub const NCT_LAN: NETCON_TYPE = 3i32;
pub const NCT_PHONE: NETCON_TYPE = 4i32;
pub const NCT_TUNNEL: NETCON_TYPE = 5i32;
pub const NCUC_DEFAULT: NETCONUI_CONNECT_FLAGS = 0i32;
pub const NCUC_ENABLE_DISABLE: NETCONUI_CONNECT_FLAGS = 2i32;
pub const NCUC_NO_UI: NETCONUI_CONNECT_FLAGS = 1i32;
pub const NETCON_MAX_NAME_LEN: u32 = 256u32;
pub const NETISO_ERROR_TYPE_INTERNET_CLIENT: NETISO_ERROR_TYPE = 2i32;
pub const NETISO_ERROR_TYPE_INTERNET_CLIENT_SERVER: NETISO_ERROR_TYPE = 3i32;
pub const NETISO_ERROR_TYPE_MAX: NETISO_ERROR_TYPE = 4i32;
pub const NETISO_ERROR_TYPE_NONE: NETISO_ERROR_TYPE = 0i32;
pub const NETISO_ERROR_TYPE_PRIVATE_NETWORK: NETISO_ERROR_TYPE = 1i32;
pub const NETISO_FLAG_FORCE_COMPUTE_BINARIES: NETISO_FLAG = 1i32;
pub const NETISO_FLAG_MAX: NETISO_FLAG = 2i32;
pub const NETISO_GEID_FOR_NEUTRAL_AWARE: u32 = 2u32;
pub const NETISO_GEID_FOR_WDAG: u32 = 1u32;
pub const NET_FW_ACTION_ALLOW: NET_FW_ACTION = 1i32;
pub const NET_FW_ACTION_BLOCK: NET_FW_ACTION = 0i32;
pub const NET_FW_ACTION_MAX: NET_FW_ACTION = 2i32;
pub const NET_FW_AUTHENTICATE_AND_ENCRYPT: NET_FW_AUTHENTICATE_TYPE = 4i32;
pub const NET_FW_AUTHENTICATE_AND_NEGOTIATE_ENCRYPTION: NET_FW_AUTHENTICATE_TYPE = 3i32;
pub const NET_FW_AUTHENTICATE_NONE: NET_FW_AUTHENTICATE_TYPE = 0i32;
pub const NET_FW_AUTHENTICATE_NO_ENCAPSULATION: NET_FW_AUTHENTICATE_TYPE = 1i32;
pub const NET_FW_AUTHENTICATE_WITH_INTEGRITY: NET_FW_AUTHENTICATE_TYPE = 2i32;
pub const NET_FW_EDGE_TRAVERSAL_TYPE_ALLOW: NET_FW_EDGE_TRAVERSAL_TYPE = 1i32;
pub const NET_FW_EDGE_TRAVERSAL_TYPE_DEFER_TO_APP: NET_FW_EDGE_TRAVERSAL_TYPE = 2i32;
pub const NET_FW_EDGE_TRAVERSAL_TYPE_DEFER_TO_USER: NET_FW_EDGE_TRAVERSAL_TYPE = 3i32;
pub const NET_FW_EDGE_TRAVERSAL_TYPE_DENY: NET_FW_EDGE_TRAVERSAL_TYPE = 0i32;
pub const NET_FW_IP_PROTOCOL_ANY: NET_FW_IP_PROTOCOL = 256i32;
pub const NET_FW_IP_PROTOCOL_TCP: NET_FW_IP_PROTOCOL = 6i32;
pub const NET_FW_IP_PROTOCOL_UDP: NET_FW_IP_PROTOCOL = 17i32;
pub const NET_FW_IP_VERSION_ANY: NET_FW_IP_VERSION = 2i32;
pub const NET_FW_IP_VERSION_MAX: NET_FW_IP_VERSION = 3i32;
pub const NET_FW_IP_VERSION_V4: NET_FW_IP_VERSION = 0i32;
pub const NET_FW_IP_VERSION_V6: NET_FW_IP_VERSION = 1i32;
pub const NET_FW_MODIFY_STATE_GP_OVERRIDE: NET_FW_MODIFY_STATE = 1i32;
pub const NET_FW_MODIFY_STATE_INBOUND_BLOCKED: NET_FW_MODIFY_STATE = 2i32;
pub const NET_FW_MODIFY_STATE_OK: NET_FW_MODIFY_STATE = 0i32;
pub const NET_FW_POLICY_EFFECTIVE: NET_FW_POLICY_TYPE = 2i32;
pub const NET_FW_POLICY_GROUP: NET_FW_POLICY_TYPE = 0i32;
pub const NET_FW_POLICY_LOCAL: NET_FW_POLICY_TYPE = 1i32;
pub const NET_FW_POLICY_TYPE_MAX: NET_FW_POLICY_TYPE = 3i32;
pub const NET_FW_PROFILE2_ALL: NET_FW_PROFILE_TYPE2 = 2147483647i32;
pub const NET_FW_PROFILE2_DOMAIN: NET_FW_PROFILE_TYPE2 = 1i32;
pub const NET_FW_PROFILE2_PRIVATE: NET_FW_PROFILE_TYPE2 = 2i32;
pub const NET_FW_PROFILE2_PUBLIC: NET_FW_PROFILE_TYPE2 = 4i32;
pub const NET_FW_PROFILE_CURRENT: NET_FW_PROFILE_TYPE = 2i32;
pub const NET_FW_PROFILE_DOMAIN: NET_FW_PROFILE_TYPE = 0i32;
pub const NET_FW_PROFILE_STANDARD: NET_FW_PROFILE_TYPE = 1i32;
pub const NET_FW_PROFILE_TYPE_MAX: NET_FW_PROFILE_TYPE = 3i32;
pub const NET_FW_RULE_CATEGORY_BOOT: NET_FW_RULE_CATEGORY = 0i32;
pub const NET_FW_RULE_CATEGORY_CONSEC: NET_FW_RULE_CATEGORY = 3i32;
pub const NET_FW_RULE_CATEGORY_FIREWALL: NET_FW_RULE_CATEGORY = 2i32;
pub const NET_FW_RULE_CATEGORY_MAX: NET_FW_RULE_CATEGORY = 4i32;
pub const NET_FW_RULE_CATEGORY_STEALTH: NET_FW_RULE_CATEGORY = 1i32;
pub const NET_FW_RULE_DIR_IN: NET_FW_RULE_DIRECTION = 1i32;
pub const NET_FW_RULE_DIR_MAX: NET_FW_RULE_DIRECTION = 3i32;
pub const NET_FW_RULE_DIR_OUT: NET_FW_RULE_DIRECTION = 2i32;
pub const NET_FW_SCOPE_ALL: NET_FW_SCOPE = 0i32;
pub const NET_FW_SCOPE_CUSTOM: NET_FW_SCOPE = 2i32;
pub const NET_FW_SCOPE_LOCAL_SUBNET: NET_FW_SCOPE = 1i32;
pub const NET_FW_SCOPE_MAX: NET_FW_SCOPE = 3i32;
pub const NET_FW_SERVICE_FILE_AND_PRINT: NET_FW_SERVICE_TYPE = 0i32;
pub const NET_FW_SERVICE_NONE: NET_FW_SERVICE_TYPE = 3i32;
pub const NET_FW_SERVICE_REMOTE_DESKTOP: NET_FW_SERVICE_TYPE = 2i32;
pub const NET_FW_SERVICE_TYPE_MAX: NET_FW_SERVICE_TYPE = 4i32;
pub const NET_FW_SERVICE_UPNP: NET_FW_SERVICE_TYPE = 1i32;
pub const NetFwAuthorizedApplication: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xec9846b3_2762_4a6b_a214_6acb603462d2);
pub const NetFwMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x304ce942_6e39_40d8_943a_b913c40c9cd4);
pub const NetFwOpenPort: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0ca545c6_37ad_4a6c_bf92_9f7610067ef5);
pub const NetFwPolicy2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe2b3c97f_6ae1_41ac_817a_f6f92166d7dd);
pub const NetFwProduct: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9d745ed8_c514_4d1d_bf42_751fed2d5ac7);
pub const NetFwProducts: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcc19079b_8272_4d73_bb70_cdb533527b61);
pub const NetFwRule: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2c5bc43e_3369_4c33_ab0c_be9469677af4);
pub const NetSharingManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c63c1ad_3956_4ff8_8486_40034758315b);
pub const S_OBJECT_NO_LONGER_VALID: ::windows_sys::core::HRESULT = 2i32;
pub const UPnPNAT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xae1e00aa_3fd5_403c_8a27_2bbdc30cd0e1);
pub type FW_DYNAMIC_KEYWORD_ADDRESS_ENUM_FLAGS = i32;
pub type FW_DYNAMIC_KEYWORD_ADDRESS_FLAGS = i32;
pub type FW_DYNAMIC_KEYWORD_ORIGIN_TYPE = i32;
pub type ICS_TARGETTYPE = i32;
pub type INET_FIREWALL_AC_CHANGE_TYPE = i32;
pub type INET_FIREWALL_AC_CREATION_TYPE = i32;
pub type NETCONMGR_ENUM_FLAGS = i32;
pub type NETCONUI_CONNECT_FLAGS = i32;
pub type NETCON_CHARACTERISTIC_FLAGS = i32;
pub type NETCON_MEDIATYPE = i32;
pub type NETCON_STATUS = i32;
pub type NETCON_TYPE = i32;
pub type NETISO_ERROR_TYPE = i32;
pub type NETISO_FLAG = i32;
pub type NET_FW_ACTION = i32;
pub type NET_FW_AUTHENTICATE_TYPE = i32;
pub type NET_FW_EDGE_TRAVERSAL_TYPE = i32;
pub type NET_FW_IP_PROTOCOL = i32;
pub type NET_FW_IP_VERSION = i32;
pub type NET_FW_MODIFY_STATE = i32;
pub type NET_FW_POLICY_TYPE = i32;
pub type NET_FW_PROFILE_TYPE = i32;
pub type NET_FW_PROFILE_TYPE2 = i32;
pub type NET_FW_RULE_CATEGORY = i32;
pub type NET_FW_RULE_DIRECTION = i32;
pub type NET_FW_SCOPE = i32;
pub type NET_FW_SERVICE_TYPE = i32;
pub type SHARINGCONNECTIONTYPE = i32;
pub type SHARINGCONNECTION_ENUM_FLAGS = i32;
#[repr(C)]
pub struct FW_DYNAMIC_KEYWORD_ADDRESS0 {
    pub id: ::windows_sys::core::GUID,
    pub keyword: ::windows_sys::core::PCWSTR,
    pub flags: u32,
    pub addresses: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for FW_DYNAMIC_KEYWORD_ADDRESS0 {}
impl ::core::clone::Clone for FW_DYNAMIC_KEYWORD_ADDRESS0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FW_DYNAMIC_KEYWORD_ADDRESS_DATA0 {
    pub dynamicKeywordAddress: FW_DYNAMIC_KEYWORD_ADDRESS0,
    pub next: *mut FW_DYNAMIC_KEYWORD_ADDRESS_DATA0,
    pub schemaVersion: u16,
    pub originType: FW_DYNAMIC_KEYWORD_ORIGIN_TYPE,
}
impl ::core::marker::Copy for FW_DYNAMIC_KEYWORD_ADDRESS_DATA0 {}
impl ::core::clone::Clone for FW_DYNAMIC_KEYWORD_ADDRESS_DATA0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INET_FIREWALL_AC_BINARIES {
    pub count: u32,
    pub binaries: *mut ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for INET_FIREWALL_AC_BINARIES {}
impl ::core::clone::Clone for INET_FIREWALL_AC_BINARIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct INET_FIREWALL_AC_CAPABILITIES {
    pub count: u32,
    pub capabilities: *mut super::super::Security::SID_AND_ATTRIBUTES,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for INET_FIREWALL_AC_CAPABILITIES {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for INET_FIREWALL_AC_CAPABILITIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct INET_FIREWALL_AC_CHANGE {
    pub changeType: INET_FIREWALL_AC_CHANGE_TYPE,
    pub createType: INET_FIREWALL_AC_CREATION_TYPE,
    pub appContainerSid: *mut super::super::Security::SID,
    pub userSid: *mut super::super::Security::SID,
    pub displayName: ::windows_sys::core::PWSTR,
    pub Anonymous: INET_FIREWALL_AC_CHANGE_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for INET_FIREWALL_AC_CHANGE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for INET_FIREWALL_AC_CHANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub union INET_FIREWALL_AC_CHANGE_0 {
    pub capabilities: INET_FIREWALL_AC_CAPABILITIES,
    pub binaries: INET_FIREWALL_AC_BINARIES,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for INET_FIREWALL_AC_CHANGE_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for INET_FIREWALL_AC_CHANGE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct INET_FIREWALL_APP_CONTAINER {
    pub appContainerSid: *mut super::super::Security::SID,
    pub userSid: *mut super::super::Security::SID,
    pub appContainerName: ::windows_sys::core::PWSTR,
    pub displayName: ::windows_sys::core::PWSTR,
    pub description: ::windows_sys::core::PWSTR,
    pub capabilities: INET_FIREWALL_AC_CAPABILITIES,
    pub binaries: INET_FIREWALL_AC_BINARIES,
    pub workingDirectory: ::windows_sys::core::PWSTR,
    pub packageFullName: ::windows_sys::core::PWSTR,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for INET_FIREWALL_APP_CONTAINER {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for INET_FIREWALL_APP_CONTAINER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NETCON_PROPERTIES {
    pub guidId: ::windows_sys::core::GUID,
    pub pszwName: ::windows_sys::core::PWSTR,
    pub pszwDeviceName: ::windows_sys::core::PWSTR,
    pub Status: NETCON_STATUS,
    pub MediaType: NETCON_MEDIATYPE,
    pub dwCharacter: u32,
    pub clsidThisObject: ::windows_sys::core::GUID,
    pub clsidUiObject: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for NETCON_PROPERTIES {}
impl ::core::clone::Clone for NETCON_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub type PAC_CHANGES_CALLBACK_FN = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, pchange: *const INET_FIREWALL_AC_CHANGE) -> ()>;
pub type PFN_FWADDDYNAMICKEYWORDADDRESS0 = ::core::option::Option<unsafe extern "system" fn(dynamickeywordaddress: *const FW_DYNAMIC_KEYWORD_ADDRESS0) -> u32>;
pub type PFN_FWDELETEDYNAMICKEYWORDADDRESS0 = ::core::option::Option<unsafe extern "system" fn(dynamickeywordaddressid: ::windows_sys::core::GUID) -> u32>;
pub type PFN_FWENUMDYNAMICKEYWORDADDRESSBYID0 = ::core::option::Option<unsafe extern "system" fn(dynamickeywordaddressid: ::windows_sys::core::GUID, dynamickeywordaddressdata: *mut *mut FW_DYNAMIC_KEYWORD_ADDRESS_DATA0) -> u32>;
pub type PFN_FWENUMDYNAMICKEYWORDADDRESSESBYTYPE0 = ::core::option::Option<unsafe extern "system" fn(flags: u32, dynamickeywordaddressdata: *mut *mut FW_DYNAMIC_KEYWORD_ADDRESS_DATA0) -> u32>;
pub type PFN_FWFREEDYNAMICKEYWORDADDRESSDATA0 = ::core::option::Option<unsafe extern "system" fn(dynamickeywordaddressdata: *const FW_DYNAMIC_KEYWORD_ADDRESS_DATA0) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_FWUPDATEDYNAMICKEYWORDADDRESS0 = ::core::option::Option<unsafe extern "system" fn(dynamickeywordaddressid: ::windows_sys::core::GUID, updatedaddresses: ::windows_sys::core::PCWSTR, append: super::super::Foundation::BOOL) -> u32>;
pub type PNETISO_EDP_ID_CALLBACK_FN = ::core::option::Option<unsafe extern "system" fn(context: *mut ::core::ffi::c_void, wszenterpriseid: ::windows_sys::core::PCWSTR, dwerr: u32) -> ()>;
