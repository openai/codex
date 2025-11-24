#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn QOSAddSocketToFlow(qoshandle : super::super::Foundation:: HANDLE, socket : super::super::Networking::WinSock:: SOCKET, destaddr : *const super::super::Networking::WinSock:: SOCKADDR, traffictype : QOS_TRAFFIC_TYPE, flags : u32, flowid : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn QOSCancel(qoshandle : super::super::Foundation:: HANDLE, overlapped : *const super::super::System::IO:: OVERLAPPED) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QOSCloseHandle(qoshandle : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QOSCreateHandle(version : *const QOS_VERSION, qoshandle : *mut super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QOSEnumerateFlows(qoshandle : super::super::Foundation:: HANDLE, size : *mut u32, buffer : *mut ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn QOSNotifyFlow(qoshandle : super::super::Foundation:: HANDLE, flowid : u32, operation : QOS_NOTIFY_FLOW, size : *mut u32, buffer : *mut ::core::ffi::c_void, flags : u32, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn QOSQueryFlow(qoshandle : super::super::Foundation:: HANDLE, flowid : u32, operation : QOS_QUERY_FLOW, size : *mut u32, buffer : *mut ::core::ffi::c_void, flags : u32, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn QOSRemoveSocketFromFlow(qoshandle : super::super::Foundation:: HANDLE, socket : super::super::Networking::WinSock:: SOCKET, flowid : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn QOSSetFlow(qoshandle : super::super::Foundation:: HANDLE, flowid : u32, operation : QOS_SET_FLOW, size : u32, buffer : *const ::core::ffi::c_void, flags : u32, overlapped : *mut super::super::System::IO:: OVERLAPPED) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn QOSStartTrackingClient(qoshandle : super::super::Foundation:: HANDLE, destaddr : *const super::super::Networking::WinSock:: SOCKADDR, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("qwave.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn QOSStopTrackingClient(qoshandle : super::super::Foundation:: HANDLE, destaddr : *const super::super::Networking::WinSock:: SOCKADDR, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcAddFilter(flowhandle : super::super::Foundation:: HANDLE, pgenericfilter : *const TC_GEN_FILTER, pfilterhandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn TcAddFlow(ifchandle : super::super::Foundation:: HANDLE, clflowctx : super::super::Foundation:: HANDLE, flags : u32, pgenericflow : *const TC_GEN_FLOW, pflowhandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcCloseInterface(ifchandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcDeleteFilter(filterhandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcDeleteFlow(flowhandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcDeregisterClient(clienthandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn TcEnumerateFlows(ifchandle : super::super::Foundation:: HANDLE, penumhandle : *mut super::super::Foundation:: HANDLE, pflowcount : *mut u32, pbufsize : *mut u32, buffer : *mut ENUMERATION_BUFFER) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"] fn TcEnumerateInterfaces(clienthandle : super::super::Foundation:: HANDLE, pbuffersize : *mut u32, interfacebuffer : *mut TC_IFC_DESCRIPTOR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcGetFlowNameA(flowhandle : super::super::Foundation:: HANDLE, strsize : u32, pflowname : ::windows_sys::core::PSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcGetFlowNameW(flowhandle : super::super::Foundation:: HANDLE, strsize : u32, pflowname : ::windows_sys::core::PWSTR) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn TcModifyFlow(flowhandle : super::super::Foundation:: HANDLE, pgenericflow : *const TC_GEN_FLOW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcOpenInterfaceA(pinterfacename : ::windows_sys::core::PCSTR, clienthandle : super::super::Foundation:: HANDLE, clifcctx : super::super::Foundation:: HANDLE, pifchandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcOpenInterfaceW(pinterfacename : ::windows_sys::core::PCWSTR, clienthandle : super::super::Foundation:: HANDLE, clifcctx : super::super::Foundation:: HANDLE, pifchandle : *mut super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("traffic.dll" "system" fn TcQueryFlowA(pflowname : ::windows_sys::core::PCSTR, pguidparam : *const ::windows_sys::core::GUID, pbuffersize : *mut u32, buffer : *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("traffic.dll" "system" fn TcQueryFlowW(pflowname : ::windows_sys::core::PCWSTR, pguidparam : *const ::windows_sys::core::GUID, pbuffersize : *mut u32, buffer : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcQueryInterface(ifchandle : super::super::Foundation:: HANDLE, pguidparam : *const ::windows_sys::core::GUID, notifychange : super::super::Foundation:: BOOLEAN, pbuffersize : *mut u32, buffer : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcRegisterClient(tciversion : u32, clregctx : super::super::Foundation:: HANDLE, clienthandlerlist : *const TCI_CLIENT_FUNC_LIST, pclienthandle : *mut super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("traffic.dll" "system" fn TcSetFlowA(pflowname : ::windows_sys::core::PCSTR, pguidparam : *const ::windows_sys::core::GUID, buffersize : u32, buffer : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("traffic.dll" "system" fn TcSetFlowW(pflowname : ::windows_sys::core::PCWSTR, pguidparam : *const ::windows_sys::core::GUID, buffersize : u32, buffer : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("traffic.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TcSetInterface(ifchandle : super::super::Foundation:: HANDLE, pguidparam : *const ::windows_sys::core::GUID, buffersize : u32, buffer : *const ::core::ffi::c_void) -> u32);
pub const ABLE_TO_RECV_RSVP: u32 = 50002u32;
pub const ADM_CTRL_FAILED: u32 = 3u32;
pub const AD_FLAG_BREAK_BIT: u32 = 1u32;
pub const ALLOWED_TO_SEND_DATA: u32 = 50001u32;
pub const ANY_DEST_ADDR: u32 = 4294967295u32;
pub const CONTROLLED_DELAY_SERV: u32 = 4u32;
pub const CONTROLLED_LOAD_SERV: u32 = 5u32;
pub const CREDENTIAL_SUB_TYPE_ASCII_ID: u32 = 1u32;
pub const CREDENTIAL_SUB_TYPE_KERBEROS_TKT: u32 = 3u32;
pub const CREDENTIAL_SUB_TYPE_PGP_CERT: u32 = 5u32;
pub const CREDENTIAL_SUB_TYPE_UNICODE_ID: u32 = 2u32;
pub const CREDENTIAL_SUB_TYPE_X509_V3_CERT: u32 = 4u32;
pub const CURRENT_TCI_VERSION: u32 = 2u32;
pub const DD_TCP_DEVICE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("\\Device\\Tcp");
pub const DUP_RESULTS: u32 = 4u32;
pub const END_TO_END_QOSABILITY: u32 = 50006u32;
pub const ERROR_ADDRESS_TYPE_NOT_SUPPORTED: u32 = 7511u32;
pub const ERROR_DS_MAPPING_EXISTS: u32 = 7518u32;
pub const ERROR_DUPLICATE_FILTER: u32 = 7509u32;
pub const ERROR_FILTER_CONFLICT: u32 = 7510u32;
pub const ERROR_INCOMPATABLE_QOS: u32 = 7513u32;
pub const ERROR_INCOMPATIBLE_TCI_VERSION: u32 = 7501u32;
pub const ERROR_INVALID_ADDRESS_TYPE: u32 = 7508u32;
pub const ERROR_INVALID_DIFFSERV_FLOW: u32 = 7517u32;
pub const ERROR_INVALID_DS_CLASS: u32 = 7520u32;
pub const ERROR_INVALID_FLOW_MODE: u32 = 7516u32;
pub const ERROR_INVALID_PEAK_RATE: u32 = 7504u32;
pub const ERROR_INVALID_QOS_PRIORITY: u32 = 7506u32;
pub const ERROR_INVALID_SD_MODE: u32 = 7505u32;
pub const ERROR_INVALID_SERVICE_TYPE: u32 = 7502u32;
pub const ERROR_INVALID_SHAPE_RATE: u32 = 7519u32;
pub const ERROR_INVALID_TOKEN_RATE: u32 = 7503u32;
pub const ERROR_INVALID_TRAFFIC_CLASS: u32 = 7507u32;
pub const ERROR_NO_MORE_INFO: u32 = 1u32;
pub const ERROR_SPECF_InPlace: u32 = 1u32;
pub const ERROR_SPECF_NotGuilty: u32 = 2u32;
pub const ERROR_TC_NOT_SUPPORTED: u32 = 7514u32;
pub const ERROR_TC_OBJECT_LENGTH_INVALID: u32 = 7515u32;
pub const ERROR_TC_SUPPORTED_OBJECTS_EXIST: u32 = 7512u32;
pub const ERROR_TOO_MANY_CLIENTS: u32 = 7521u32;
pub const ERR_FORWARD_OK: u32 = 32768u32;
pub const ERR_Usage_globl: u32 = 0u32;
pub const ERR_Usage_local: u32 = 16u32;
pub const ERR_Usage_serv: u32 = 17u32;
pub const ERR_global_mask: u32 = 4095u32;
pub const EXPIRED_CREDENTIAL: u32 = 4u32;
pub const FILTERSPECV4: FilterType = 1i32;
pub const FILTERSPECV4_GPI: FilterType = 4i32;
pub const FILTERSPECV6: FilterType = 2i32;
pub const FILTERSPECV6_FLOW: FilterType = 3i32;
pub const FILTERSPECV6_GPI: FilterType = 5i32;
pub const FILTERSPEC_END: FilterType = 6i32;
pub const FLOW_DURATION: u32 = 5u32;
pub const FORCE_IMMEDIATE_REFRESH: u32 = 1u32;
pub const FSCTL_TCP_BASE: u32 = 18u32;
pub const FVEB_UNLOCK_FLAG_AUK_OSFVEINFO: u32 = 512u32;
pub const FVEB_UNLOCK_FLAG_CACHED: u32 = 1u32;
pub const FVEB_UNLOCK_FLAG_EXTERNAL: u32 = 32u32;
pub const FVEB_UNLOCK_FLAG_MEDIA: u32 = 2u32;
pub const FVEB_UNLOCK_FLAG_NBP: u32 = 256u32;
pub const FVEB_UNLOCK_FLAG_NONE: u32 = 0u32;
pub const FVEB_UNLOCK_FLAG_PASSPHRASE: u32 = 128u32;
pub const FVEB_UNLOCK_FLAG_PIN: u32 = 16u32;
pub const FVEB_UNLOCK_FLAG_RECOVERY: u32 = 64u32;
pub const FVEB_UNLOCK_FLAG_TPM: u32 = 4u32;
pub const GENERAL_INFO: u32 = 1u32;
pub const GQOS_API: u32 = 56400u32;
pub const GQOS_ERRORCODE_UNKNOWN: u32 = 4294967295u32;
pub const GQOS_ERRORVALUE_UNKNOWN: u32 = 4294967295u32;
pub const GQOS_KERNEL_TC: u32 = 56700u32;
pub const GQOS_KERNEL_TC_SYS: u32 = 56500u32;
pub const GQOS_NET_ADMISSION: u32 = 56100u32;
pub const GQOS_NET_POLICY: u32 = 56200u32;
pub const GQOS_NO_ERRORCODE: u32 = 0u32;
pub const GQOS_NO_ERRORVALUE: u32 = 0u32;
pub const GQOS_RSVP: u32 = 56300u32;
pub const GQOS_RSVP_SYS: u32 = 56600u32;
pub const GUARANTEED_SERV: u32 = 2u32;
pub const GUAR_ADSPARM_C: i32 = 131i32;
pub const GUAR_ADSPARM_Csum: i32 = 135i32;
pub const GUAR_ADSPARM_Ctot: i32 = 133i32;
pub const GUAR_ADSPARM_D: i32 = 132i32;
pub const GUAR_ADSPARM_Dsum: i32 = 136i32;
pub const GUAR_ADSPARM_Dtot: i32 = 134i32;
pub const GUID_QOS_BESTEFFORT_BANDWIDTH: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xed885290_40ec_11d1_2c91_00aa00574915);
pub const GUID_QOS_ENABLE_AVG_STATS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbafb6d11_27c4_4801_a46f_ef8080c188c8);
pub const GUID_QOS_ENABLE_WINDOW_ADJUSTMENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaa966725_d3e9_4c55_b335_2a00279a1e64);
pub const GUID_QOS_FLOW_8021P_CONFORMING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x08c1e013_fcd2_11d2_be1e_00a0c99ee63b);
pub const GUID_QOS_FLOW_8021P_NONCONFORMING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x09023f91_fcd2_11d2_be1e_00a0c99ee63b);
pub const GUID_QOS_FLOW_COUNT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1147f880_40ed_11d1_2c91_00aa00574915);
pub const GUID_QOS_FLOW_IP_CONFORMING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x07f99a8b_fcd2_11d2_be1e_00a0c99ee63b);
pub const GUID_QOS_FLOW_IP_NONCONFORMING: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x087a5987_fcd2_11d2_be1e_00a0c99ee63b);
pub const GUID_QOS_FLOW_MODE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c82290a_515a_11d2_8e58_00c04fc9bfcb);
pub const GUID_QOS_ISSLOW_FLOW: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xabf273a4_ee07_11d2_be1b_00a0c99ee63b);
pub const GUID_QOS_LATENCY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfc408ef0_40ec_11d1_2c91_00aa00574915);
pub const GUID_QOS_MAX_OUTSTANDING_SENDS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x161ffa86_6120_11d1_2c91_00aa00574915);
pub const GUID_QOS_NON_BESTEFFORT_LIMIT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x185c44e0_40ed_11d1_2c91_00aa00574915);
pub const GUID_QOS_REMAINING_BANDWIDTH: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc4c51720_40ec_11d1_2c91_00aa00574915);
pub const GUID_QOS_STATISTICS_BUFFER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbb2c0980_e900_11d1_b07e_0080c71382bf);
pub const GUID_QOS_TIMER_RESOLUTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xba10cc88_f13e_11d2_be1b_00a0c99ee63b);
pub const HIGHLY_DELAY_SENSITIVE: u32 = 4294967294u32;
pub const IDENTITY_CHANGED: u32 = 5u32;
pub const IF_MIB_STATS_ID: u32 = 1u32;
pub const INFO_NOT_AVAILABLE: u32 = 4294967295u32;
pub const INSUFFICIENT_PRIVILEGES: u32 = 3u32;
pub const INTSERV_VERSION0: u32 = 0u32;
pub const INTSERV_VERS_MASK: u32 = 240u32;
pub const INV_LPM_HANDLE: u32 = 1u32;
pub const INV_REQ_HANDLE: u32 = 3u32;
pub const INV_RESULTS: u32 = 5u32;
pub const IP_INTFC_INFO_ID: u32 = 259u32;
pub const IP_MIB_ADDRTABLE_ENTRY_ID: u32 = 258u32;
pub const IP_MIB_STATS_ID: u32 = 1u32;
pub const ISPH_FLG_INV: u32 = 128u32;
pub const ISSH_BREAK_BIT: u32 = 128u32;
pub const IS_GUAR_RSPEC: i32 = 130i32;
pub const IS_WKP_COMPOSED_MTU: int_serv_wkp = 10i32;
pub const IS_WKP_HOP_CNT: int_serv_wkp = 4i32;
pub const IS_WKP_MIN_LATENCY: int_serv_wkp = 8i32;
pub const IS_WKP_PATH_BW: int_serv_wkp = 6i32;
pub const IS_WKP_Q_TSPEC: int_serv_wkp = 128i32;
pub const IS_WKP_TB_TSPEC: int_serv_wkp = 127i32;
pub const LINE_RATE: u32 = 50003u32;
pub const LOCAL_QOSABILITY: u32 = 50005u32;
pub const LOCAL_TRAFFIC_CONTROL: u32 = 50004u32;
pub const LPM_API_VERSION_1: u32 = 1u32;
pub const LPM_OK: u32 = 0u32;
pub const LPM_PE_ALL_TYPES: u32 = 0u32;
pub const LPM_PE_APP_IDENTITY: u32 = 3u32;
pub const LPM_PE_USER_IDENTITY: u32 = 2u32;
pub const LPM_RESULT_DEFER: u32 = 1u32;
pub const LPM_RESULT_READY: u32 = 0u32;
pub const LPM_TIME_OUT: u32 = 2u32;
pub const LPV_DONT_CARE: u32 = 65534u32;
pub const LPV_DROP_MSG: u32 = 65533u32;
pub const LPV_MAX_PRIORITY: u32 = 65280u32;
pub const LPV_MIN_PRIORITY: u32 = 1u32;
pub const LPV_REJECT: u32 = 65535u32;
pub const LPV_RESERVED: u32 = 0u32;
pub const MAX_HSP_UPGRADE_FILENAME_LENGTH: u32 = 64u32;
pub const MAX_PHYSADDR_SIZE: u32 = 8u32;
pub const MAX_STRING_LENGTH: u32 = 256u32;
pub const MODERATELY_DELAY_SENSITIVE: u32 = 4294967293u32;
pub const OSDEVICE_TYPE_BLOCKIO_CDROM: u32 = 65539u32;
pub const OSDEVICE_TYPE_BLOCKIO_FILE: u32 = 65541u32;
pub const OSDEVICE_TYPE_BLOCKIO_HARDDISK: u32 = 65537u32;
pub const OSDEVICE_TYPE_BLOCKIO_PARTITION: u32 = 65540u32;
pub const OSDEVICE_TYPE_BLOCKIO_RAMDISK: u32 = 65542u32;
pub const OSDEVICE_TYPE_BLOCKIO_REMOVABLEDISK: u32 = 65538u32;
pub const OSDEVICE_TYPE_BLOCKIO_VIRTUALHARDDISK: u32 = 65543u32;
pub const OSDEVICE_TYPE_CIMFS: u32 = 393216u32;
pub const OSDEVICE_TYPE_COMPOSITE: u32 = 327680u32;
pub const OSDEVICE_TYPE_SERIAL: u32 = 131072u32;
pub const OSDEVICE_TYPE_UDP: u32 = 196608u32;
pub const OSDEVICE_TYPE_UNKNOWN: u32 = 0u32;
pub const OSDEVICE_TYPE_VMBUS: u32 = 262144u32;
pub const Opt_Distinct: u32 = 8u32;
pub const Opt_Explicit: u32 = 2u32;
pub const Opt_Share_mask: u32 = 24u32;
pub const Opt_Shared: u32 = 16u32;
pub const Opt_SndSel_mask: u32 = 7u32;
pub const Opt_Wildcard: u32 = 1u32;
pub const PCM_VERSION_1: u32 = 1u32;
pub const PE_ATTRIB_TYPE_CREDENTIAL: u32 = 2u32;
pub const PE_ATTRIB_TYPE_POLICY_LOCATOR: u32 = 1u32;
pub const PE_TYPE_APPID: u32 = 3u32;
pub const POLICY_ERRV_CRAZY_FLOWSPEC: u32 = 57u32;
pub const POLICY_ERRV_EXPIRED_CREDENTIALS: u32 = 4u32;
pub const POLICY_ERRV_EXPIRED_USER_TOKEN: u32 = 51u32;
pub const POLICY_ERRV_GLOBAL_DEF_FLOW_COUNT: u32 = 1u32;
pub const POLICY_ERRV_GLOBAL_DEF_FLOW_DURATION: u32 = 9u32;
pub const POLICY_ERRV_GLOBAL_DEF_FLOW_RATE: u32 = 17u32;
pub const POLICY_ERRV_GLOBAL_DEF_PEAK_RATE: u32 = 25u32;
pub const POLICY_ERRV_GLOBAL_DEF_SUM_FLOW_RATE: u32 = 33u32;
pub const POLICY_ERRV_GLOBAL_DEF_SUM_PEAK_RATE: u32 = 41u32;
pub const POLICY_ERRV_GLOBAL_GRP_FLOW_COUNT: u32 = 2u32;
pub const POLICY_ERRV_GLOBAL_GRP_FLOW_DURATION: u32 = 10u32;
pub const POLICY_ERRV_GLOBAL_GRP_FLOW_RATE: u32 = 18u32;
pub const POLICY_ERRV_GLOBAL_GRP_PEAK_RATE: u32 = 26u32;
pub const POLICY_ERRV_GLOBAL_GRP_SUM_FLOW_RATE: u32 = 34u32;
pub const POLICY_ERRV_GLOBAL_GRP_SUM_PEAK_RATE: u32 = 42u32;
pub const POLICY_ERRV_GLOBAL_UNAUTH_USER_FLOW_COUNT: u32 = 4u32;
pub const POLICY_ERRV_GLOBAL_UNAUTH_USER_FLOW_DURATION: u32 = 12u32;
pub const POLICY_ERRV_GLOBAL_UNAUTH_USER_FLOW_RATE: u32 = 20u32;
pub const POLICY_ERRV_GLOBAL_UNAUTH_USER_PEAK_RATE: u32 = 28u32;
pub const POLICY_ERRV_GLOBAL_UNAUTH_USER_SUM_FLOW_RATE: u32 = 36u32;
pub const POLICY_ERRV_GLOBAL_UNAUTH_USER_SUM_PEAK_RATE: u32 = 44u32;
pub const POLICY_ERRV_GLOBAL_USER_FLOW_COUNT: u32 = 3u32;
pub const POLICY_ERRV_GLOBAL_USER_FLOW_DURATION: u32 = 11u32;
pub const POLICY_ERRV_GLOBAL_USER_FLOW_RATE: u32 = 19u32;
pub const POLICY_ERRV_GLOBAL_USER_PEAK_RATE: u32 = 27u32;
pub const POLICY_ERRV_GLOBAL_USER_SUM_FLOW_RATE: u32 = 35u32;
pub const POLICY_ERRV_GLOBAL_USER_SUM_PEAK_RATE: u32 = 43u32;
pub const POLICY_ERRV_IDENTITY_CHANGED: u32 = 5u32;
pub const POLICY_ERRV_INSUFFICIENT_PRIVILEGES: u32 = 3u32;
pub const POLICY_ERRV_NO_ACCEPTS: u32 = 55u32;
pub const POLICY_ERRV_NO_MEMORY: u32 = 56u32;
pub const POLICY_ERRV_NO_MORE_INFO: u32 = 1u32;
pub const POLICY_ERRV_NO_PRIVILEGES: u32 = 50u32;
pub const POLICY_ERRV_NO_RESOURCES: u32 = 52u32;
pub const POLICY_ERRV_PRE_EMPTED: u32 = 53u32;
pub const POLICY_ERRV_SUBNET_DEF_FLOW_COUNT: u32 = 5u32;
pub const POLICY_ERRV_SUBNET_DEF_FLOW_DURATION: u32 = 13u32;
pub const POLICY_ERRV_SUBNET_DEF_FLOW_RATE: u32 = 21u32;
pub const POLICY_ERRV_SUBNET_DEF_PEAK_RATE: u32 = 29u32;
pub const POLICY_ERRV_SUBNET_DEF_SUM_FLOW_RATE: u32 = 37u32;
pub const POLICY_ERRV_SUBNET_DEF_SUM_PEAK_RATE: u32 = 45u32;
pub const POLICY_ERRV_SUBNET_GRP_FLOW_COUNT: u32 = 6u32;
pub const POLICY_ERRV_SUBNET_GRP_FLOW_DURATION: u32 = 14u32;
pub const POLICY_ERRV_SUBNET_GRP_FLOW_RATE: u32 = 22u32;
pub const POLICY_ERRV_SUBNET_GRP_PEAK_RATE: u32 = 30u32;
pub const POLICY_ERRV_SUBNET_GRP_SUM_FLOW_RATE: u32 = 38u32;
pub const POLICY_ERRV_SUBNET_GRP_SUM_PEAK_RATE: u32 = 46u32;
pub const POLICY_ERRV_SUBNET_UNAUTH_USER_FLOW_COUNT: u32 = 8u32;
pub const POLICY_ERRV_SUBNET_UNAUTH_USER_FLOW_DURATION: u32 = 16u32;
pub const POLICY_ERRV_SUBNET_UNAUTH_USER_FLOW_RATE: u32 = 24u32;
pub const POLICY_ERRV_SUBNET_UNAUTH_USER_PEAK_RATE: u32 = 32u32;
pub const POLICY_ERRV_SUBNET_UNAUTH_USER_SUM_FLOW_RATE: u32 = 40u32;
pub const POLICY_ERRV_SUBNET_UNAUTH_USER_SUM_PEAK_RATE: u32 = 48u32;
pub const POLICY_ERRV_SUBNET_USER_FLOW_COUNT: u32 = 7u32;
pub const POLICY_ERRV_SUBNET_USER_FLOW_DURATION: u32 = 15u32;
pub const POLICY_ERRV_SUBNET_USER_FLOW_RATE: u32 = 23u32;
pub const POLICY_ERRV_SUBNET_USER_PEAK_RATE: u32 = 31u32;
pub const POLICY_ERRV_SUBNET_USER_SUM_FLOW_RATE: u32 = 39u32;
pub const POLICY_ERRV_SUBNET_USER_SUM_PEAK_RATE: u32 = 47u32;
pub const POLICY_ERRV_UNKNOWN: u32 = 0u32;
pub const POLICY_ERRV_UNKNOWN_USER: u32 = 49u32;
pub const POLICY_ERRV_UNSUPPORTED_CREDENTIAL_TYPE: u32 = 2u32;
pub const POLICY_ERRV_USER_CHANGED: u32 = 54u32;
pub const POLICY_LOCATOR_SUB_TYPE_ASCII_DN: u32 = 1u32;
pub const POLICY_LOCATOR_SUB_TYPE_ASCII_DN_ENC: u32 = 3u32;
pub const POLICY_LOCATOR_SUB_TYPE_UNICODE_DN: u32 = 2u32;
pub const POLICY_LOCATOR_SUB_TYPE_UNICODE_DN_ENC: u32 = 4u32;
pub const POSITIVE_INFINITY_RATE: u32 = 4294967294u32;
pub const PREDICTIVE_SERV: u32 = 3u32;
pub const QOSFlowRateCongestion: QOS_FLOWRATE_REASON = 2i32;
pub const QOSFlowRateContentChange: QOS_FLOWRATE_REASON = 1i32;
pub const QOSFlowRateHigherContentEncoding: QOS_FLOWRATE_REASON = 3i32;
pub const QOSFlowRateNotApplicable: QOS_FLOWRATE_REASON = 0i32;
pub const QOSFlowRateUserCaused: QOS_FLOWRATE_REASON = 4i32;
pub const QOSNotifyAvailable: QOS_NOTIFY_FLOW = 2i32;
pub const QOSNotifyCongested: QOS_NOTIFY_FLOW = 0i32;
pub const QOSNotifyUncongested: QOS_NOTIFY_FLOW = 1i32;
pub const QOSQueryFlowFundamentals: QOS_QUERY_FLOW = 0i32;
pub const QOSQueryOutgoingRate: QOS_QUERY_FLOW = 2i32;
pub const QOSQueryPacketPriority: QOS_QUERY_FLOW = 1i32;
pub const QOSSPBASE: u32 = 50000u32;
pub const QOSSP_ERR_BASE: u32 = 56000u32;
pub const QOSSetOutgoingDSCPValue: QOS_SET_FLOW = 2i32;
pub const QOSSetOutgoingRate: QOS_SET_FLOW = 1i32;
pub const QOSSetTrafficType: QOS_SET_FLOW = 0i32;
pub const QOSShapeAndMark: QOS_SHAPING = 1i32;
pub const QOSShapeOnly: QOS_SHAPING = 0i32;
pub const QOSTrafficTypeAudioVideo: QOS_TRAFFIC_TYPE = 3i32;
pub const QOSTrafficTypeBackground: QOS_TRAFFIC_TYPE = 1i32;
pub const QOSTrafficTypeBestEffort: QOS_TRAFFIC_TYPE = 0i32;
pub const QOSTrafficTypeControl: QOS_TRAFFIC_TYPE = 5i32;
pub const QOSTrafficTypeExcellentEffort: QOS_TRAFFIC_TYPE = 2i32;
pub const QOSTrafficTypeVoice: QOS_TRAFFIC_TYPE = 4i32;
pub const QOSUseNonConformantMarkings: QOS_SHAPING = 2i32;
pub const QOS_GENERAL_ID_BASE: u32 = 2000u32;
pub const QOS_MAX_OBJECT_STRING_LENGTH: u32 = 256u32;
pub const QOS_NON_ADAPTIVE_FLOW: u32 = 2u32;
pub const QOS_NOT_SPECIFIED: u32 = 4294967295u32;
pub const QOS_OUTGOING_DEFAULT_MINIMUM_BANDWIDTH: u32 = 4294967295u32;
pub const QOS_QUERYFLOW_FRESH: u32 = 1u32;
pub const QOS_TRAFFIC_GENERAL_ID_BASE: u32 = 4000u32;
pub const QUALITATIVE_SERV: u32 = 6u32;
pub const RCVD_PATH_TEAR: u32 = 1u32;
pub const RCVD_RESV_TEAR: u32 = 2u32;
pub const RESOURCES_ALLOCATED: u32 = 1u32;
pub const RESOURCES_MODIFIED: u32 = 2u32;
pub const RSVP_DEFAULT_STYLE: u32 = 0u32;
pub const RSVP_Err_ADMISSION: u32 = 1u32;
pub const RSVP_Err_AMBIG_FILTER: u32 = 9u32;
pub const RSVP_Err_API_ERROR: u32 = 20u32;
pub const RSVP_Err_BAD_DSTPORT: u32 = 7u32;
pub const RSVP_Err_BAD_SNDPORT: u32 = 8u32;
pub const RSVP_Err_BAD_STYLE: u32 = 5u32;
pub const RSVP_Err_NONE: u32 = 0u32;
pub const RSVP_Err_NO_PATH: u32 = 3u32;
pub const RSVP_Err_NO_SENDER: u32 = 4u32;
pub const RSVP_Err_POLICY: u32 = 2u32;
pub const RSVP_Err_PREEMPTED: u32 = 12u32;
pub const RSVP_Err_RSVP_SYS_ERROR: u32 = 23u32;
pub const RSVP_Err_TC_ERROR: u32 = 21u32;
pub const RSVP_Err_TC_SYS_ERROR: u32 = 22u32;
pub const RSVP_Err_UNKNOWN_CTYPE: u32 = 14u32;
pub const RSVP_Err_UNKNOWN_STYLE: u32 = 6u32;
pub const RSVP_Err_UNKN_OBJ_CLASS: u32 = 13u32;
pub const RSVP_Erv_API: u32 = 2u32;
pub const RSVP_Erv_Bandwidth: u32 = 2u32;
pub const RSVP_Erv_Bucket_szie: u32 = 32770u32;
pub const RSVP_Erv_Conflict_Serv: u32 = 1u32;
pub const RSVP_Erv_Crazy_Flowspec: u32 = 3u32;
pub const RSVP_Erv_Crazy_Tspec: u32 = 4u32;
pub const RSVP_Erv_DelayBnd: u32 = 1u32;
pub const RSVP_Erv_Flow_Rate: u32 = 32769u32;
pub const RSVP_Erv_MEMORY: u32 = 1u32;
pub const RSVP_Erv_MTU: u32 = 3u32;
pub const RSVP_Erv_Min_Policied_size: u32 = 32772u32;
pub const RSVP_Erv_No_Serv: u32 = 2u32;
pub const RSVP_Erv_Nonev: u32 = 0u32;
pub const RSVP_Erv_Other: u32 = 0u32;
pub const RSVP_Erv_Peak_Rate: u32 = 32771u32;
pub const RSVP_FIXED_FILTER_STYLE: u32 = 2u32;
pub const RSVP_OBJECT_ID_BASE: u32 = 1000u32;
pub const RSVP_PATH: u32 = 1u32;
pub const RSVP_PATH_ERR: u32 = 3u32;
pub const RSVP_PATH_TEAR: u32 = 5u32;
pub const RSVP_RESV: u32 = 2u32;
pub const RSVP_RESV_ERR: u32 = 4u32;
pub const RSVP_RESV_TEAR: u32 = 6u32;
pub const RSVP_SHARED_EXPLICIT_STYLE: u32 = 3u32;
pub const RSVP_WILDCARD_STYLE: u32 = 1u32;
pub const SERVICETYPE_BESTEFFORT: u32 = 1u32;
pub const SERVICETYPE_CONTROLLEDLOAD: u32 = 2u32;
pub const SERVICETYPE_GENERAL_INFORMATION: u32 = 5u32;
pub const SERVICETYPE_GUARANTEED: u32 = 3u32;
pub const SERVICETYPE_NETWORK_CONTROL: u32 = 10u32;
pub const SERVICETYPE_NETWORK_UNAVAILABLE: u32 = 4u32;
pub const SERVICETYPE_NOCHANGE: u32 = 6u32;
pub const SERVICETYPE_NONCONFORMING: u32 = 9u32;
pub const SERVICETYPE_NOTRAFFIC: u32 = 0u32;
pub const SERVICETYPE_QUALITATIVE: u32 = 13u32;
pub const SERVICE_BESTEFFORT: u32 = 2147549184u32;
pub const SERVICE_CONTROLLEDLOAD: u32 = 2147614720u32;
pub const SERVICE_GUARANTEED: u32 = 2147745792u32;
pub const SERVICE_NO_QOS_SIGNALING: u32 = 1073741824u32;
pub const SERVICE_NO_TRAFFIC_CONTROL: u32 = 2164260864u32;
pub const SERVICE_QUALITATIVE: u32 = 2149580800u32;
pub const SESSFLG_E_Police: u32 = 1u32;
pub const SIPAERROR_FIRMWAREFAILURE: u32 = 196609u32;
pub const SIPAERROR_INTERNALFAILURE: u32 = 196611u32;
pub const SIPAEVENTTYPE_AGGREGATION: u32 = 1073741824u32;
pub const SIPAEVENTTYPE_AUTHORITY: u32 = 393216u32;
pub const SIPAEVENTTYPE_CONTAINER: u32 = 65536u32;
pub const SIPAEVENTTYPE_DRTM: u32 = 786432u32;
pub const SIPAEVENTTYPE_ELAM: u32 = 589824u32;
pub const SIPAEVENTTYPE_ERROR: u32 = 196608u32;
pub const SIPAEVENTTYPE_INFORMATION: u32 = 131072u32;
pub const SIPAEVENTTYPE_KSR: u32 = 720896u32;
pub const SIPAEVENTTYPE_LOADEDMODULE: u32 = 458752u32;
pub const SIPAEVENTTYPE_NONMEASURED: u32 = 2147483648u32;
pub const SIPAEVENTTYPE_OSPARAMETER: u32 = 327680u32;
pub const SIPAEVENTTYPE_PREOSPARAMETER: u32 = 262144u32;
pub const SIPAEVENTTYPE_TRUSTPOINT: u32 = 524288u32;
pub const SIPAEVENTTYPE_VBS: u32 = 655360u32;
pub const SIPAEVENT_APPLICATION_RETURN: u32 = 131076u32;
pub const SIPAEVENT_APPLICATION_SVN: u32 = 131081u32;
pub const SIPAEVENT_AUTHENTICODEHASH: u32 = 458756u32;
pub const SIPAEVENT_AUTHORITYISSUER: u32 = 458757u32;
pub const SIPAEVENT_AUTHORITYPUBKEY: u32 = 393218u32;
pub const SIPAEVENT_AUTHORITYPUBLISHER: u32 = 458760u32;
pub const SIPAEVENT_AUTHORITYSERIAL: u32 = 458758u32;
pub const SIPAEVENT_AUTHORITYSHA1THUMBPRINT: u32 = 458761u32;
pub const SIPAEVENT_BITLOCKER_UNLOCK: u32 = 131077u32;
pub const SIPAEVENT_BOOTCOUNTER: u32 = 131074u32;
pub const SIPAEVENT_BOOTDEBUGGING: u32 = 262145u32;
pub const SIPAEVENT_BOOT_REVOCATION_LIST: u32 = 262146u32;
pub const SIPAEVENT_CODEINTEGRITY: u32 = 327682u32;
pub const SIPAEVENT_COUNTERID: u32 = 131079u32;
pub const SIPAEVENT_DATAEXECUTIONPREVENTION: u32 = 327684u32;
pub const SIPAEVENT_DRIVER_LOAD_POLICY: u32 = 327694u32;
pub const SIPAEVENT_DRTM_AMD_SMM_HASH: u32 = 786435u32;
pub const SIPAEVENT_DRTM_AMD_SMM_SIGNER_KEY: u32 = 786436u32;
pub const SIPAEVENT_DRTM_SMM_LEVEL: u32 = 786434u32;
pub const SIPAEVENT_DRTM_STATE_AUTH: u32 = 786433u32;
pub const SIPAEVENT_DUMPS_DISABLED: u32 = 327717u32;
pub const SIPAEVENT_DUMP_ENCRYPTION_ENABLED: u32 = 327718u32;
pub const SIPAEVENT_DUMP_ENCRYPTION_KEY_DIGEST: u32 = 327719u32;
pub const SIPAEVENT_ELAM_CONFIGURATION: u32 = 589826u32;
pub const SIPAEVENT_ELAM_KEYNAME: u32 = 589825u32;
pub const SIPAEVENT_ELAM_MEASURED: u32 = 589828u32;
pub const SIPAEVENT_ELAM_POLICY: u32 = 589827u32;
pub const SIPAEVENT_EVENTCOUNTER: u32 = 131078u32;
pub const SIPAEVENT_FILEPATH: u32 = 458753u32;
pub const SIPAEVENT_FLIGHTSIGNING: u32 = 327713u32;
pub const SIPAEVENT_HASHALGORITHMID: u32 = 458755u32;
pub const SIPAEVENT_HIBERNATION_DISABLED: u32 = 327716u32;
pub const SIPAEVENT_HYPERVISOR_BOOT_DMA_PROTECTION: u32 = 327728u32;
pub const SIPAEVENT_HYPERVISOR_DEBUG: u32 = 327693u32;
pub const SIPAEVENT_HYPERVISOR_IOMMU_POLICY: u32 = 327692u32;
pub const SIPAEVENT_HYPERVISOR_LAUNCH_TYPE: u32 = 327690u32;
pub const SIPAEVENT_HYPERVISOR_MMIO_NX_POLICY: u32 = 327696u32;
pub const SIPAEVENT_HYPERVISOR_MSR_FILTER_POLICY: u32 = 327697u32;
pub const SIPAEVENT_HYPERVISOR_PATH: u32 = 327691u32;
pub const SIPAEVENT_IMAGEBASE: u32 = 458759u32;
pub const SIPAEVENT_IMAGESIZE: u32 = 458754u32;
pub const SIPAEVENT_IMAGEVALIDATED: u32 = 458762u32;
pub const SIPAEVENT_INFORMATION: u32 = 131073u32;
pub const SIPAEVENT_KSR_SIGNATURE: u32 = 720897u32;
pub const SIPAEVENT_LSAISO_CONFIG: u32 = 327720u32;
pub const SIPAEVENT_MODULE_HSP: u32 = 458764u32;
pub const SIPAEVENT_MODULE_SVN: u32 = 458763u32;
pub const SIPAEVENT_MORBIT_API_STATUS: u32 = 131083u32;
pub const SIPAEVENT_MORBIT_NOT_CANCELABLE: u32 = 131080u32;
pub const SIPAEVENT_NOAUTHORITY: u32 = 393217u32;
pub const SIPAEVENT_OSDEVICE: u32 = 327688u32;
pub const SIPAEVENT_OSKERNELDEBUG: u32 = 327681u32;
pub const SIPAEVENT_OS_REVOCATION_LIST: u32 = 327699u32;
pub const SIPAEVENT_PAGEFILE_ENCRYPTION_ENABLED: u32 = 327714u32;
pub const SIPAEVENT_PHYSICALADDRESSEXTENSION: u32 = 327687u32;
pub const SIPAEVENT_SAFEMODE: u32 = 327685u32;
pub const SIPAEVENT_SBCP_INFO: u32 = 327721u32;
pub const SIPAEVENT_SI_POLICY: u32 = 327695u32;
pub const SIPAEVENT_SMT_STATUS: u32 = 327700u32;
pub const SIPAEVENT_SVN_CHAIN_STATUS: u32 = 131082u32;
pub const SIPAEVENT_SYSTEMROOT: u32 = 327689u32;
pub const SIPAEVENT_TESTSIGNING: u32 = 327683u32;
pub const SIPAEVENT_TRANSFER_CONTROL: u32 = 131075u32;
pub const SIPAEVENT_VBS_DUMP_USES_AMEROOT: u32 = 655369u32;
pub const SIPAEVENT_VBS_HVCI_POLICY: u32 = 655367u32;
pub const SIPAEVENT_VBS_IOMMU_REQUIRED: u32 = 655363u32;
pub const SIPAEVENT_VBS_MANDATORY_ENFORCEMENT: u32 = 655366u32;
pub const SIPAEVENT_VBS_MICROSOFT_BOOT_CHAIN_REQUIRED: u32 = 655368u32;
pub const SIPAEVENT_VBS_MMIO_NX_REQUIRED: u32 = 655364u32;
pub const SIPAEVENT_VBS_MSR_FILTERING_REQUIRED: u32 = 655365u32;
pub const SIPAEVENT_VBS_SECUREBOOT_REQUIRED: u32 = 655362u32;
pub const SIPAEVENT_VBS_VSM_NOSECRETS_ENFORCED: u32 = 655370u32;
pub const SIPAEVENT_VBS_VSM_REQUIRED: u32 = 655361u32;
pub const SIPAEVENT_VSM_IDKS_INFO: u32 = 327715u32;
pub const SIPAEVENT_VSM_IDK_INFO: u32 = 327712u32;
pub const SIPAEVENT_VSM_LAUNCH_TYPE: u32 = 327698u32;
pub const SIPAEVENT_WINPE: u32 = 327686u32;
pub const SIPAEV_ACTION: u32 = 5u32;
pub const SIPAEV_AMD_SL_EVENT_BASE: u32 = 32768u32;
pub const SIPAEV_AMD_SL_LOAD: u32 = 32769u32;
pub const SIPAEV_AMD_SL_LOAD_1: u32 = 32774u32;
pub const SIPAEV_AMD_SL_PSP_FW_SPLT: u32 = 32770u32;
pub const SIPAEV_AMD_SL_PUB_KEY: u32 = 32772u32;
pub const SIPAEV_AMD_SL_SEPARATOR: u32 = 32775u32;
pub const SIPAEV_AMD_SL_SVN: u32 = 32773u32;
pub const SIPAEV_AMD_SL_TSME_RB_FUSE: u32 = 32771u32;
pub const SIPAEV_COMPACT_HASH: u32 = 12u32;
pub const SIPAEV_CPU_MICROCODE: u32 = 9u32;
pub const SIPAEV_EFI_ACTION: u32 = 2147483655u32;
pub const SIPAEV_EFI_BOOT_SERVICES_APPLICATION: u32 = 2147483651u32;
pub const SIPAEV_EFI_BOOT_SERVICES_DRIVER: u32 = 2147483652u32;
pub const SIPAEV_EFI_EVENT_BASE: u32 = 2147483648u32;
pub const SIPAEV_EFI_GPT_EVENT: u32 = 2147483654u32;
pub const SIPAEV_EFI_HANDOFF_TABLES: u32 = 2147483657u32;
pub const SIPAEV_EFI_HANDOFF_TABLES2: u32 = 2147483659u32;
pub const SIPAEV_EFI_HCRTM_EVENT: u32 = 2147483664u32;
pub const SIPAEV_EFI_PLATFORM_FIRMWARE_BLOB: u32 = 2147483656u32;
pub const SIPAEV_EFI_PLATFORM_FIRMWARE_BLOB2: u32 = 2147483658u32;
pub const SIPAEV_EFI_RUNTIME_SERVICES_DRIVER: u32 = 2147483653u32;
pub const SIPAEV_EFI_SPDM_FIRMWARE_BLOB: u32 = 2147483873u32;
pub const SIPAEV_EFI_SPDM_FIRMWARE_CONFIG: u32 = 2147483874u32;
pub const SIPAEV_EFI_VARIABLE_AUTHORITY: u32 = 2147483872u32;
pub const SIPAEV_EFI_VARIABLE_BOOT: u32 = 2147483650u32;
pub const SIPAEV_EFI_VARIABLE_BOOT2: u32 = 2147483660u32;
pub const SIPAEV_EFI_VARIABLE_DRIVER_CONFIG: u32 = 2147483649u32;
pub const SIPAEV_EVENT_TAG: u32 = 6u32;
pub const SIPAEV_IPL: u32 = 13u32;
pub const SIPAEV_IPL_PARTITION_DATA: u32 = 14u32;
pub const SIPAEV_NONHOST_CODE: u32 = 15u32;
pub const SIPAEV_NONHOST_CONFIG: u32 = 16u32;
pub const SIPAEV_NONHOST_INFO: u32 = 17u32;
pub const SIPAEV_NO_ACTION: u32 = 3u32;
pub const SIPAEV_OMIT_BOOT_DEVICE_EVENTS: u32 = 18u32;
pub const SIPAEV_PLATFORM_CONFIG_FLAGS: u32 = 10u32;
pub const SIPAEV_POST_CODE: u32 = 1u32;
pub const SIPAEV_PREBOOT_CERT: u32 = 0u32;
pub const SIPAEV_SEPARATOR: u32 = 4u32;
pub const SIPAEV_S_CRTM_CONTENTS: u32 = 7u32;
pub const SIPAEV_S_CRTM_VERSION: u32 = 8u32;
pub const SIPAEV_TABLE_OF_DEVICES: u32 = 11u32;
pub const SIPAEV_TXT_BIOSAC_REG_DATA: u32 = 1034u32;
pub const SIPAEV_TXT_BOOT_POL_HASH: u32 = 1050u32;
pub const SIPAEV_TXT_BPM_HASH: u32 = 1047u32;
pub const SIPAEV_TXT_BPM_INFO_HASH: u32 = 1049u32;
pub const SIPAEV_TXT_CAP_VALUE: u32 = 1279u32;
pub const SIPAEV_TXT_COLD_BOOT_BIOS_HASH: u32 = 1045u32;
pub const SIPAEV_TXT_COMBINED_HASH: u32 = 1027u32;
pub const SIPAEV_TXT_CPU_SCRTM_STAT: u32 = 1035u32;
pub const SIPAEV_TXT_ELEMENTS_HASH: u32 = 1037u32;
pub const SIPAEV_TXT_EVENT_BASE: u32 = 1024u32;
pub const SIPAEV_TXT_HASH_START: u32 = 1026u32;
pub const SIPAEV_TXT_KM_HASH: u32 = 1046u32;
pub const SIPAEV_TXT_KM_INFO_HASH: u32 = 1048u32;
pub const SIPAEV_TXT_LCP_AUTHORITIES_HASH: u32 = 1043u32;
pub const SIPAEV_TXT_LCP_CONTROL_HASH: u32 = 1036u32;
pub const SIPAEV_TXT_LCP_DETAILS_HASH: u32 = 1042u32;
pub const SIPAEV_TXT_LCP_HASH: u32 = 1041u32;
pub const SIPAEV_TXT_MLE_HASH: u32 = 1028u32;
pub const SIPAEV_TXT_NV_INFO_HASH: u32 = 1044u32;
pub const SIPAEV_TXT_OSSINITDATA_CAP_HASH: u32 = 1039u32;
pub const SIPAEV_TXT_PCR_MAPPING: u32 = 1025u32;
pub const SIPAEV_TXT_RANDOM_VALUE: u32 = 1278u32;
pub const SIPAEV_TXT_SINIT_PUBKEY_HASH: u32 = 1040u32;
pub const SIPAEV_TXT_STM_HASH: u32 = 1038u32;
pub const SIPAEV_UNUSED: u32 = 2u32;
pub const SIPAHDRSIGNATURE: u32 = 1279476311u32;
pub const SIPAKSRHDRSIGNATURE: u32 = 1297240907u32;
pub const SIPALOGVERSION: u32 = 1u32;
pub const STATE_TIMEOUT: u32 = 4u32;
pub const TCBASE: u32 = 7500u32;
pub const TC_NONCONF_BORROW: u32 = 0u32;
pub const TC_NONCONF_BORROW_PLUS: u32 = 3u32;
pub const TC_NONCONF_DISCARD: u32 = 2u32;
pub const TC_NONCONF_SHAPE: u32 = 1u32;
pub const TC_NOTIFY_FLOW_CLOSE: u32 = 5u32;
pub const TC_NOTIFY_IFC_CHANGE: u32 = 3u32;
pub const TC_NOTIFY_IFC_CLOSE: u32 = 2u32;
pub const TC_NOTIFY_IFC_UP: u32 = 1u32;
pub const TC_NOTIFY_PARAM_CHANGED: u32 = 4u32;
pub const UNSUPPORTED_CREDENTIAL_TYPE: u32 = 2u32;
pub const WBCL_DIGEST_ALG_BITMAP_SHA3_256: u32 = 32u32;
pub const WBCL_DIGEST_ALG_BITMAP_SHA3_384: u32 = 64u32;
pub const WBCL_DIGEST_ALG_BITMAP_SHA3_512: u32 = 128u32;
pub const WBCL_DIGEST_ALG_BITMAP_SHA_1: u32 = 1u32;
pub const WBCL_DIGEST_ALG_BITMAP_SHA_2_256: u32 = 2u32;
pub const WBCL_DIGEST_ALG_BITMAP_SHA_2_384: u32 = 4u32;
pub const WBCL_DIGEST_ALG_BITMAP_SHA_2_512: u32 = 8u32;
pub const WBCL_DIGEST_ALG_BITMAP_SM3_256: u32 = 16u32;
pub const WBCL_DIGEST_ALG_ID_SHA3_256: u32 = 39u32;
pub const WBCL_DIGEST_ALG_ID_SHA3_384: u32 = 40u32;
pub const WBCL_DIGEST_ALG_ID_SHA3_512: u32 = 41u32;
pub const WBCL_DIGEST_ALG_ID_SHA_1: u32 = 4u32;
pub const WBCL_DIGEST_ALG_ID_SHA_2_256: u32 = 11u32;
pub const WBCL_DIGEST_ALG_ID_SHA_2_384: u32 = 12u32;
pub const WBCL_DIGEST_ALG_ID_SHA_2_512: u32 = 13u32;
pub const WBCL_DIGEST_ALG_ID_SM3_256: u32 = 18u32;
pub const WBCL_HASH_LEN_SHA1: u32 = 20u32;
pub const WBCL_MAX_HSP_UPGRADE_HASH_LEN: u32 = 64u32;
pub const class_ADSPEC: u32 = 13u32;
pub const class_CONFIRM: u32 = 15u32;
pub const class_ERROR_SPEC: u32 = 6u32;
pub const class_FILTER_SPEC: u32 = 10u32;
pub const class_FLOWSPEC: u32 = 9u32;
pub const class_INTEGRITY: u32 = 4u32;
pub const class_IS_FLOWSPEC: u32 = 9u32;
pub const class_MAX: u32 = 15u32;
pub const class_NULL: u32 = 0u32;
pub const class_POLICY_DATA: u32 = 14u32;
pub const class_RSVP_HOP: u32 = 3u32;
pub const class_SCOPE: u32 = 7u32;
pub const class_SENDER_TEMPLATE: u32 = 11u32;
pub const class_SENDER_TSPEC: u32 = 12u32;
pub const class_SESSION: u32 = 1u32;
pub const class_SESSION_GROUP: u32 = 2u32;
pub const class_STYLE: u32 = 8u32;
pub const class_TIME_VALUES: u32 = 5u32;
pub const ctype_ADSPEC_INTSERV: u32 = 2u32;
pub const ctype_ERROR_SPEC_ipv4: u32 = 1u32;
pub const ctype_FILTER_SPEC_ipv4: u32 = 1u32;
pub const ctype_FILTER_SPEC_ipv4GPI: u32 = 4u32;
pub const ctype_FLOWSPEC_Intserv0: u32 = 2u32;
pub const ctype_POLICY_DATA: u32 = 1u32;
pub const ctype_RSVP_HOP_ipv4: u32 = 1u32;
pub const ctype_SCOPE_list_ipv4: u32 = 1u32;
pub const ctype_SENDER_TEMPLATE_ipv4: u32 = 1u32;
pub const ctype_SENDER_TEMPLATE_ipv4GPI: u32 = 4u32;
pub const ctype_SENDER_TSPEC: u32 = 2u32;
pub const ctype_SESSION_ipv4: u32 = 1u32;
pub const ctype_SESSION_ipv4GPI: u32 = 3u32;
pub const ctype_STYLE: u32 = 1u32;
pub const ioctl_code: u32 = 1u32;
pub const mCOMPANY: u32 = 402653184u32;
pub const mIOC_IN: u32 = 2147483648u32;
pub const mIOC_OUT: u32 = 1073741824u32;
pub const mIOC_VENDOR: u32 = 67108864u32;
pub type FilterType = i32;
pub type QOS_FLOWRATE_REASON = i32;
pub type QOS_NOTIFY_FLOW = i32;
pub type QOS_QUERY_FLOW = i32;
pub type QOS_SET_FLOW = i32;
pub type QOS_SHAPING = i32;
pub type QOS_TRAFFIC_TYPE = i32;
pub type int_serv_wkp = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct ADDRESS_LIST_DESCRIPTOR {
    pub MediaType: u32,
    pub AddressList: super::Ndis::NETWORK_ADDRESS_LIST,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for ADDRESS_LIST_DESCRIPTOR {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for ADDRESS_LIST_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADSPEC {
    pub adspec_header: RsvpObjHdr,
    pub adspec_body: IS_ADSPEC_BODY,
}
impl ::core::marker::Copy for ADSPEC {}
impl ::core::clone::Clone for ADSPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AD_GENERAL_PARAMS {
    pub IntServAwareHopCount: u32,
    pub PathBandwidthEstimate: u32,
    pub MinimumLatency: u32,
    pub PathMTU: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for AD_GENERAL_PARAMS {}
impl ::core::clone::Clone for AD_GENERAL_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AD_GUARANTEED {
    pub CTotal: u32,
    pub DTotal: u32,
    pub CSum: u32,
    pub DSum: u32,
}
impl ::core::marker::Copy for AD_GUARANTEED {}
impl ::core::clone::Clone for AD_GUARANTEED {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CONTROL_SERVICE {
    pub Length: u32,
    pub Service: u32,
    pub Overrides: AD_GENERAL_PARAMS,
    pub Anonymous: CONTROL_SERVICE_0,
}
impl ::core::marker::Copy for CONTROL_SERVICE {}
impl ::core::clone::Clone for CONTROL_SERVICE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CONTROL_SERVICE_0 {
    pub Guaranteed: AD_GUARANTEED,
    pub ParamBuffer: [PARAM_BUFFER; 1],
}
impl ::core::marker::Copy for CONTROL_SERVICE_0 {}
impl ::core::clone::Clone for CONTROL_SERVICE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CtrlLoadFlowspec {
    pub CL_spec_serv_hdr: IntServServiceHdr,
    pub CL_spec_parm_hdr: IntServParmHdr,
    pub CL_spec_parms: GenTspecParms,
}
impl ::core::marker::Copy for CtrlLoadFlowspec {}
impl ::core::clone::Clone for CtrlLoadFlowspec {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct ENUMERATION_BUFFER {
    pub Length: u32,
    pub OwnerProcessId: u32,
    pub FlowNameLength: u16,
    pub FlowName: [u16; 256],
    pub pFlow: *mut TC_GEN_FLOW,
    pub NumberOfFilters: u32,
    pub GenericFilter: [TC_GEN_FILTER; 1],
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for ENUMERATION_BUFFER {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for ENUMERATION_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct ERROR_SPEC {
    pub errs_header: RsvpObjHdr,
    pub errs_u: ERROR_SPEC_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for ERROR_SPEC {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for ERROR_SPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union ERROR_SPEC_0 {
    pub errs_ipv4: Error_Spec_IPv4,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for ERROR_SPEC_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for ERROR_SPEC_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct Error_Spec_IPv4 {
    pub errs_errnode: super::super::Networking::WinSock::IN_ADDR,
    pub errs_flags: u8,
    pub errs_code: u8,
    pub errs_value: u16,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for Error_Spec_IPv4 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for Error_Spec_IPv4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct FILTER_SPEC {
    pub filt_header: RsvpObjHdr,
    pub filt_u: FILTER_SPEC_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for FILTER_SPEC {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for FILTER_SPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union FILTER_SPEC_0 {
    pub filt_ipv4: Filter_Spec_IPv4,
    pub filt_ipv4gpi: Filter_Spec_IPv4GPI,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for FILTER_SPEC_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for FILTER_SPEC_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct FLOWDESCRIPTOR {
    pub FlowSpec: super::super::Networking::WinSock::FLOWSPEC,
    pub NumFilters: u32,
    pub FilterList: *mut RSVP_FILTERSPEC,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for FLOWDESCRIPTOR {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for FLOWDESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct FLOW_DESC {
    pub u1: FLOW_DESC_0,
    pub u2: FLOW_DESC_1,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for FLOW_DESC {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for FLOW_DESC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union FLOW_DESC_0 {
    pub stspec: *mut SENDER_TSPEC,
    pub isflow: *mut IS_FLOWSPEC,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for FLOW_DESC_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for FLOW_DESC_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union FLOW_DESC_1 {
    pub stemp: *mut FILTER_SPEC,
    pub fspec: *mut FILTER_SPEC,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for FLOW_DESC_1 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for FLOW_DESC_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct Filter_Spec_IPv4 {
    pub filt_ipaddr: super::super::Networking::WinSock::IN_ADDR,
    pub filt_unused: u16,
    pub filt_port: u16,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for Filter_Spec_IPv4 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for Filter_Spec_IPv4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct Filter_Spec_IPv4GPI {
    pub filt_ipaddr: super::super::Networking::WinSock::IN_ADDR,
    pub filt_gpi: u32,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for Filter_Spec_IPv4GPI {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for Filter_Spec_IPv4GPI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct Gads_parms_t {
    pub Gads_serv_hdr: IntServServiceHdr,
    pub Gads_Ctot_hdr: IntServParmHdr,
    pub Gads_Ctot: u32,
    pub Gads_Dtot_hdr: IntServParmHdr,
    pub Gads_Dtot: u32,
    pub Gads_Csum_hdr: IntServParmHdr,
    pub Gads_Csum: u32,
    pub Gads_Dsum_hdr: IntServParmHdr,
    pub Gads_Dsum: u32,
}
impl ::core::marker::Copy for Gads_parms_t {}
impl ::core::clone::Clone for Gads_parms_t {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GenAdspecParams {
    pub gen_parm_hdr: IntServServiceHdr,
    pub gen_parm_hopcnt_hdr: IntServParmHdr,
    pub gen_parm_hopcnt: u32,
    pub gen_parm_pathbw_hdr: IntServParmHdr,
    pub gen_parm_path_bw: f32,
    pub gen_parm_minlat_hdr: IntServParmHdr,
    pub gen_parm_min_latency: u32,
    pub gen_parm_compmtu_hdr: IntServParmHdr,
    pub gen_parm_composed_MTU: u32,
}
impl ::core::marker::Copy for GenAdspecParams {}
impl ::core::clone::Clone for GenAdspecParams {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GenTspec {
    pub gen_Tspec_serv_hdr: IntServServiceHdr,
    pub gen_Tspec_parm_hdr: IntServParmHdr,
    pub gen_Tspec_parms: GenTspecParms,
}
impl ::core::marker::Copy for GenTspec {}
impl ::core::clone::Clone for GenTspec {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GenTspecParms {
    pub TB_Tspec_r: f32,
    pub TB_Tspec_b: f32,
    pub TB_Tspec_p: f32,
    pub TB_Tspec_m: u32,
    pub TB_Tspec_M: u32,
}
impl ::core::marker::Copy for GenTspecParms {}
impl ::core::clone::Clone for GenTspecParms {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GuarFlowSpec {
    pub Guar_serv_hdr: IntServServiceHdr,
    pub Guar_Tspec_hdr: IntServParmHdr,
    pub Guar_Tspec_parms: GenTspecParms,
    pub Guar_Rspec_hdr: IntServParmHdr,
    pub Guar_Rspec: GuarRspec,
}
impl ::core::marker::Copy for GuarFlowSpec {}
impl ::core::clone::Clone for GuarFlowSpec {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GuarRspec {
    pub Guar_R: f32,
    pub Guar_S: u32,
}
impl ::core::marker::Copy for GuarRspec {}
impl ::core::clone::Clone for GuarRspec {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct HSP_UPGRADE_IMAGEDATA {
    pub hashAlgID: u16,
    pub digestSize: u16,
    pub digest: [u8; 64],
    pub fileName: [u16; 64],
}
impl ::core::marker::Copy for HSP_UPGRADE_IMAGEDATA {}
impl ::core::clone::Clone for HSP_UPGRADE_IMAGEDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IDPE_ATTR {
    pub PeAttribLength: u16,
    pub PeAttribType: u8,
    pub PeAttribSubType: u8,
    pub PeAttribValue: [u8; 4],
}
impl ::core::marker::Copy for IDPE_ATTR {}
impl ::core::clone::Clone for IDPE_ATTR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ID_ERROR_OBJECT {
    pub usIdErrLength: u16,
    pub ucAType: u8,
    pub ucSubType: u8,
    pub usReserved: u16,
    pub usIdErrorValue: u16,
    pub ucIdErrData: [u8; 4],
}
impl ::core::marker::Copy for ID_ERROR_OBJECT {}
impl ::core::clone::Clone for ID_ERROR_OBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IN_ADDR_IPV4 {
    pub Addr: u32,
    pub AddrBytes: [u8; 4],
}
impl ::core::marker::Copy for IN_ADDR_IPV4 {}
impl ::core::clone::Clone for IN_ADDR_IPV4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IN_ADDR_IPV6 {
    pub Addr: [u8; 16],
}
impl ::core::marker::Copy for IN_ADDR_IPV6 {}
impl ::core::clone::Clone for IN_ADDR_IPV6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IPX_PATTERN {
    pub Src: IPX_PATTERN_0,
    pub Dest: IPX_PATTERN_0,
}
impl ::core::marker::Copy for IPX_PATTERN {}
impl ::core::clone::Clone for IPX_PATTERN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IPX_PATTERN_0 {
    pub NetworkAddress: u32,
    pub NodeAddress: [u8; 6],
    pub Socket: u16,
}
impl ::core::marker::Copy for IPX_PATTERN_0 {}
impl ::core::clone::Clone for IPX_PATTERN_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IP_PATTERN {
    pub Reserved1: u32,
    pub Reserved2: u32,
    pub SrcAddr: u32,
    pub DstAddr: u32,
    pub S_un: IP_PATTERN_0,
    pub ProtocolId: u8,
    pub Reserved3: [u8; 3],
}
impl ::core::marker::Copy for IP_PATTERN {}
impl ::core::clone::Clone for IP_PATTERN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IP_PATTERN_0 {
    pub S_un_ports: IP_PATTERN_0_1,
    pub S_un_icmp: IP_PATTERN_0_0,
    pub S_Spi: u32,
}
impl ::core::marker::Copy for IP_PATTERN_0 {}
impl ::core::clone::Clone for IP_PATTERN_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IP_PATTERN_0_0 {
    pub s_type: u8,
    pub s_code: u8,
    pub filler: u16,
}
impl ::core::marker::Copy for IP_PATTERN_0_0 {}
impl ::core::clone::Clone for IP_PATTERN_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IP_PATTERN_0_1 {
    pub s_srcport: u16,
    pub s_dstport: u16,
}
impl ::core::marker::Copy for IP_PATTERN_0_1 {}
impl ::core::clone::Clone for IP_PATTERN_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IS_ADSPEC_BODY {
    pub adspec_mh: IntServMainHdr,
    pub adspec_genparms: GenAdspecParams,
}
impl ::core::marker::Copy for IS_ADSPEC_BODY {}
impl ::core::clone::Clone for IS_ADSPEC_BODY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IS_FLOWSPEC {
    pub flow_header: RsvpObjHdr,
    pub flow_body: IntServFlowSpec,
}
impl ::core::marker::Copy for IS_FLOWSPEC {}
impl ::core::clone::Clone for IS_FLOWSPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IntServFlowSpec {
    pub spec_mh: IntServMainHdr,
    pub spec_u: IntServFlowSpec_0,
}
impl ::core::marker::Copy for IntServFlowSpec {}
impl ::core::clone::Clone for IntServFlowSpec {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IntServFlowSpec_0 {
    pub CL_spec: CtrlLoadFlowspec,
    pub G_spec: GuarFlowSpec,
    pub Q_spec: QualAppFlowSpec,
}
impl ::core::marker::Copy for IntServFlowSpec_0 {}
impl ::core::clone::Clone for IntServFlowSpec_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IntServMainHdr {
    pub ismh_version: u8,
    pub ismh_unused: u8,
    pub ismh_len32b: u16,
}
impl ::core::marker::Copy for IntServMainHdr {}
impl ::core::clone::Clone for IntServMainHdr {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IntServParmHdr {
    pub isph_parm_num: u8,
    pub isph_flags: u8,
    pub isph_len32b: u16,
}
impl ::core::marker::Copy for IntServParmHdr {}
impl ::core::clone::Clone for IntServParmHdr {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IntServServiceHdr {
    pub issh_service: u8,
    pub issh_flags: u8,
    pub issh_len32b: u16,
}
impl ::core::marker::Copy for IntServServiceHdr {}
impl ::core::clone::Clone for IntServServiceHdr {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IntServTspecBody {
    pub st_mh: IntServMainHdr,
    pub tspec_u: IntServTspecBody_0,
}
impl ::core::marker::Copy for IntServTspecBody {}
impl ::core::clone::Clone for IntServTspecBody {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IntServTspecBody_0 {
    pub gen_stspec: GenTspec,
    pub qual_stspec: QualTspec,
}
impl ::core::marker::Copy for IntServTspecBody_0 {}
impl ::core::clone::Clone for IntServTspecBody_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct LPMIPTABLE {
    pub ulIfIndex: u32,
    pub MediaType: u32,
    pub IfIpAddr: super::super::Networking::WinSock::IN_ADDR,
    pub IfNetMask: super::super::Networking::WinSock::IN_ADDR,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for LPMIPTABLE {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for LPMIPTABLE {
    fn clone(&self) -> Self {
        *self
    }
}
pub type LPM_HANDLE = isize;
#[repr(C)]
pub struct LPM_INIT_INFO {
    pub PcmVersionNumber: u32,
    pub ResultTimeLimit: u32,
    pub ConfiguredLpmCount: i32,
    pub AllocMemory: PALLOCMEM,
    pub FreeMemory: PFREEMEM,
    pub PcmAdmitResultCallback: CBADMITRESULT,
    pub GetRsvpObjectsCallback: CBGETRSVPOBJECTS,
}
impl ::core::marker::Copy for LPM_INIT_INFO {}
impl ::core::clone::Clone for LPM_INIT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PARAM_BUFFER {
    pub ParameterId: u32,
    pub Length: u32,
    pub Buffer: [u8; 1],
}
impl ::core::marker::Copy for PARAM_BUFFER {}
impl ::core::clone::Clone for PARAM_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POLICY_DATA {
    pub PolicyObjHdr: RsvpObjHdr,
    pub usPeOffset: u16,
    pub usReserved: u16,
}
impl ::core::marker::Copy for POLICY_DATA {}
impl ::core::clone::Clone for POLICY_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POLICY_DECISION {
    pub lpvResult: u32,
    pub wPolicyErrCode: u16,
    pub wPolicyErrValue: u16,
}
impl ::core::marker::Copy for POLICY_DECISION {}
impl ::core::clone::Clone for POLICY_DECISION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POLICY_ELEMENT {
    pub usPeLength: u16,
    pub usPeType: u16,
    pub ucPeData: [u8; 4],
}
impl ::core::marker::Copy for POLICY_ELEMENT {}
impl ::core::clone::Clone for POLICY_ELEMENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct QOS_DESTADDR {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub SocketAddress: *const super::super::Networking::WinSock::SOCKADDR,
    pub SocketAddressLength: u32,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for QOS_DESTADDR {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for QOS_DESTADDR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_DIFFSERV {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub DSFieldCount: u32,
    pub DiffservRule: [u8; 1],
}
impl ::core::marker::Copy for QOS_DIFFSERV {}
impl ::core::clone::Clone for QOS_DIFFSERV {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_DIFFSERV_RULE {
    pub InboundDSField: u8,
    pub ConformingOutboundDSField: u8,
    pub NonConformingOutboundDSField: u8,
    pub ConformingUserPriority: u8,
    pub NonConformingUserPriority: u8,
}
impl ::core::marker::Copy for QOS_DIFFSERV_RULE {}
impl ::core::clone::Clone for QOS_DIFFSERV_RULE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_DS_CLASS {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub DSField: u32,
}
impl ::core::marker::Copy for QOS_DS_CLASS {}
impl ::core::clone::Clone for QOS_DS_CLASS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_FLOWRATE_OUTGOING {
    pub Bandwidth: u64,
    pub ShapingBehavior: QOS_SHAPING,
    pub Reason: QOS_FLOWRATE_REASON,
}
impl ::core::marker::Copy for QOS_FLOWRATE_OUTGOING {}
impl ::core::clone::Clone for QOS_FLOWRATE_OUTGOING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct QOS_FLOW_FUNDAMENTALS {
    pub BottleneckBandwidthSet: super::super::Foundation::BOOL,
    pub BottleneckBandwidth: u64,
    pub AvailableBandwidthSet: super::super::Foundation::BOOL,
    pub AvailableBandwidth: u64,
    pub RTTSet: super::super::Foundation::BOOL,
    pub RTT: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for QOS_FLOW_FUNDAMENTALS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for QOS_FLOW_FUNDAMENTALS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_FRIENDLY_NAME {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub FriendlyName: [u16; 256],
}
impl ::core::marker::Copy for QOS_FRIENDLY_NAME {}
impl ::core::clone::Clone for QOS_FRIENDLY_NAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_OBJECT_HDR {
    pub ObjectType: u32,
    pub ObjectLength: u32,
}
impl ::core::marker::Copy for QOS_OBJECT_HDR {}
impl ::core::clone::Clone for QOS_OBJECT_HDR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_PACKET_PRIORITY {
    pub ConformantDSCPValue: u32,
    pub NonConformantDSCPValue: u32,
    pub ConformantL2Value: u32,
    pub NonConformantL2Value: u32,
}
impl ::core::marker::Copy for QOS_PACKET_PRIORITY {}
impl ::core::clone::Clone for QOS_PACKET_PRIORITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_SD_MODE {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub ShapeDiscardMode: u32,
}
impl ::core::marker::Copy for QOS_SD_MODE {}
impl ::core::clone::Clone for QOS_SD_MODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_SHAPING_RATE {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub ShapingRate: u32,
}
impl ::core::marker::Copy for QOS_SHAPING_RATE {}
impl ::core::clone::Clone for QOS_SHAPING_RATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_TCP_TRAFFIC {
    pub ObjectHdr: QOS_OBJECT_HDR,
}
impl ::core::marker::Copy for QOS_TCP_TRAFFIC {}
impl ::core::clone::Clone for QOS_TCP_TRAFFIC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_TRAFFIC_CLASS {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub TrafficClass: u32,
}
impl ::core::marker::Copy for QOS_TRAFFIC_CLASS {}
impl ::core::clone::Clone for QOS_TRAFFIC_CLASS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QOS_VERSION {
    pub MajorVersion: u16,
    pub MinorVersion: u16,
}
impl ::core::marker::Copy for QOS_VERSION {}
impl ::core::clone::Clone for QOS_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QualAppFlowSpec {
    pub Q_spec_serv_hdr: IntServServiceHdr,
    pub Q_spec_parm_hdr: IntServParmHdr,
    pub Q_spec_parms: QualTspecParms,
}
impl ::core::marker::Copy for QualAppFlowSpec {}
impl ::core::clone::Clone for QualAppFlowSpec {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QualTspec {
    pub qual_Tspec_serv_hdr: IntServServiceHdr,
    pub qual_Tspec_parm_hdr: IntServParmHdr,
    pub qual_Tspec_parms: QualTspecParms,
}
impl ::core::marker::Copy for QualTspec {}
impl ::core::clone::Clone for QualTspec {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct QualTspecParms {
    pub TB_Tspec_M: u32,
}
impl ::core::marker::Copy for QualTspecParms {}
impl ::core::clone::Clone for QualTspecParms {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESV_STYLE {
    pub style_header: RsvpObjHdr,
    pub style_word: u32,
}
impl ::core::marker::Copy for RESV_STYLE {}
impl ::core::clone::Clone for RESV_STYLE {
    fn clone(&self) -> Self {
        *self
    }
}
pub type RHANDLE = isize;
#[repr(C)]
pub struct RSVP_ADSPEC {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub GeneralParams: AD_GENERAL_PARAMS,
    pub NumberOfServices: u32,
    pub Services: [CONTROL_SERVICE; 1],
}
impl ::core::marker::Copy for RSVP_ADSPEC {}
impl ::core::clone::Clone for RSVP_ADSPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_FILTERSPEC {
    pub Type: FilterType,
    pub Anonymous: RSVP_FILTERSPEC_0,
}
impl ::core::marker::Copy for RSVP_FILTERSPEC {}
impl ::core::clone::Clone for RSVP_FILTERSPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union RSVP_FILTERSPEC_0 {
    pub FilterSpecV4: RSVP_FILTERSPEC_V4,
    pub FilterSpecV6: RSVP_FILTERSPEC_V6,
    pub FilterSpecV6Flow: RSVP_FILTERSPEC_V6_FLOW,
    pub FilterSpecV4Gpi: RSVP_FILTERSPEC_V4_GPI,
    pub FilterSpecV6Gpi: RSVP_FILTERSPEC_V6_GPI,
}
impl ::core::marker::Copy for RSVP_FILTERSPEC_0 {}
impl ::core::clone::Clone for RSVP_FILTERSPEC_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_FILTERSPEC_V4 {
    pub Address: IN_ADDR_IPV4,
    pub Unused: u16,
    pub Port: u16,
}
impl ::core::marker::Copy for RSVP_FILTERSPEC_V4 {}
impl ::core::clone::Clone for RSVP_FILTERSPEC_V4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_FILTERSPEC_V4_GPI {
    pub Address: IN_ADDR_IPV4,
    pub GeneralPortId: u32,
}
impl ::core::marker::Copy for RSVP_FILTERSPEC_V4_GPI {}
impl ::core::clone::Clone for RSVP_FILTERSPEC_V4_GPI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_FILTERSPEC_V6 {
    pub Address: IN_ADDR_IPV6,
    pub UnUsed: u16,
    pub Port: u16,
}
impl ::core::marker::Copy for RSVP_FILTERSPEC_V6 {}
impl ::core::clone::Clone for RSVP_FILTERSPEC_V6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_FILTERSPEC_V6_FLOW {
    pub Address: IN_ADDR_IPV6,
    pub UnUsed: u8,
    pub FlowLabel: [u8; 3],
}
impl ::core::marker::Copy for RSVP_FILTERSPEC_V6_FLOW {}
impl ::core::clone::Clone for RSVP_FILTERSPEC_V6_FLOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_FILTERSPEC_V6_GPI {
    pub Address: IN_ADDR_IPV6,
    pub GeneralPortId: u32,
}
impl ::core::marker::Copy for RSVP_FILTERSPEC_V6_GPI {}
impl ::core::clone::Clone for RSVP_FILTERSPEC_V6_GPI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RSVP_HOP {
    pub hop_header: RsvpObjHdr,
    pub hop_u: RSVP_HOP_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_HOP {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_HOP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union RSVP_HOP_0 {
    pub hop_ipv4: Rsvp_Hop_IPv4,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_HOP_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_HOP_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RSVP_MSG_OBJS {
    pub RsvpMsgType: i32,
    pub pRsvpSession: *mut RSVP_SESSION,
    pub pRsvpFromHop: *mut RSVP_HOP,
    pub pRsvpToHop: *mut RSVP_HOP,
    pub pResvStyle: *mut RESV_STYLE,
    pub pRsvpScope: *mut RSVP_SCOPE,
    pub FlowDescCount: i32,
    pub pFlowDescs: *mut FLOW_DESC,
    pub PdObjectCount: i32,
    pub ppPdObjects: *mut *mut POLICY_DATA,
    pub pErrorSpec: *mut ERROR_SPEC,
    pub pAdspec: *mut ADSPEC,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_MSG_OBJS {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_MSG_OBJS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_POLICY {
    pub Len: u16,
    pub Type: u16,
    pub Info: [u8; 4],
}
impl ::core::marker::Copy for RSVP_POLICY {}
impl ::core::clone::Clone for RSVP_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_POLICY_INFO {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub NumPolicyElement: u32,
    pub PolicyElement: [RSVP_POLICY; 1],
}
impl ::core::marker::Copy for RSVP_POLICY_INFO {}
impl ::core::clone::Clone for RSVP_POLICY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RSVP_RESERVE_INFO {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub Style: u32,
    pub ConfirmRequest: u32,
    pub PolicyElementList: *mut RSVP_POLICY_INFO,
    pub NumFlowDesc: u32,
    pub FlowDescList: *mut FLOWDESCRIPTOR,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_RESERVE_INFO {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_RESERVE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RSVP_SCOPE {
    pub scopl_header: RsvpObjHdr,
    pub scope_u: RSVP_SCOPE_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_SCOPE {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_SCOPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union RSVP_SCOPE_0 {
    pub scopl_ipv4: Scope_list_ipv4,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_SCOPE_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_SCOPE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RSVP_SESSION {
    pub sess_header: RsvpObjHdr,
    pub sess_u: RSVP_SESSION_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_SESSION {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_SESSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union RSVP_SESSION_0 {
    pub sess_ipv4: Session_IPv4,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RSVP_SESSION_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RSVP_SESSION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RSVP_STATUS_INFO {
    pub ObjectHdr: QOS_OBJECT_HDR,
    pub StatusCode: u32,
    pub ExtendedStatus1: u32,
    pub ExtendedStatus2: u32,
}
impl ::core::marker::Copy for RSVP_STATUS_INFO {}
impl ::core::clone::Clone for RSVP_STATUS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RsvpObjHdr {
    pub obj_length: u16,
    pub obj_class: u8,
    pub obj_ctype: u8,
}
impl ::core::marker::Copy for RsvpObjHdr {}
impl ::core::clone::Clone for RsvpObjHdr {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct Rsvp_Hop_IPv4 {
    pub hop_ipaddr: super::super::Networking::WinSock::IN_ADDR,
    pub hop_LIH: u32,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for Rsvp_Hop_IPv4 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for Rsvp_Hop_IPv4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SENDER_TSPEC {
    pub stspec_header: RsvpObjHdr,
    pub stspec_body: IntServTspecBody,
}
impl ::core::marker::Copy for SENDER_TSPEC {}
impl ::core::clone::Clone for SENDER_TSPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct SIPAEVENT_KSR_SIGNATURE_PAYLOAD {
    pub SignAlgID: u32,
    pub SignatureLength: u32,
    pub Signature: [u8; 1],
}
impl ::core::marker::Copy for SIPAEVENT_KSR_SIGNATURE_PAYLOAD {}
impl ::core::clone::Clone for SIPAEVENT_KSR_SIGNATURE_PAYLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct SIPAEVENT_REVOCATION_LIST_PAYLOAD {
    pub CreationTime: i64,
    pub DigestLength: u32,
    pub HashAlgID: u16,
    pub Digest: [u8; 1],
}
impl ::core::marker::Copy for SIPAEVENT_REVOCATION_LIST_PAYLOAD {}
impl ::core::clone::Clone for SIPAEVENT_REVOCATION_LIST_PAYLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct SIPAEVENT_SBCP_INFO_PAYLOAD_V1 {
    pub PayloadVersion: u32,
    pub VarDataOffset: u32,
    pub HashAlgID: u16,
    pub DigestLength: u16,
    pub Options: u32,
    pub SignersCount: u32,
    pub VarData: [u8; 1],
}
impl ::core::marker::Copy for SIPAEVENT_SBCP_INFO_PAYLOAD_V1 {}
impl ::core::clone::Clone for SIPAEVENT_SBCP_INFO_PAYLOAD_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct SIPAEVENT_SI_POLICY_PAYLOAD {
    pub PolicyVersion: u64,
    pub PolicyNameLength: u16,
    pub HashAlgID: u16,
    pub DigestLength: u32,
    pub VarLengthData: [u8; 1],
}
impl ::core::marker::Copy for SIPAEVENT_SI_POLICY_PAYLOAD {}
impl ::core::clone::Clone for SIPAEVENT_SI_POLICY_PAYLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct SIPAEVENT_VSM_IDK_INFO_PAYLOAD {
    pub KeyAlgID: u32,
    pub Anonymous: SIPAEVENT_VSM_IDK_INFO_PAYLOAD_0,
}
impl ::core::marker::Copy for SIPAEVENT_VSM_IDK_INFO_PAYLOAD {}
impl ::core::clone::Clone for SIPAEVENT_VSM_IDK_INFO_PAYLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union SIPAEVENT_VSM_IDK_INFO_PAYLOAD_0 {
    pub RsaKeyInfo: SIPAEVENT_VSM_IDK_RSA_INFO,
}
impl ::core::marker::Copy for SIPAEVENT_VSM_IDK_INFO_PAYLOAD_0 {}
impl ::core::clone::Clone for SIPAEVENT_VSM_IDK_INFO_PAYLOAD_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct SIPAEVENT_VSM_IDK_RSA_INFO {
    pub KeyBitLength: u32,
    pub PublicExpLengthBytes: u32,
    pub ModulusSizeBytes: u32,
    pub PublicKeyData: [u8; 1],
}
impl ::core::marker::Copy for SIPAEVENT_VSM_IDK_RSA_INFO {}
impl ::core::clone::Clone for SIPAEVENT_VSM_IDK_RSA_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct Scope_list_ipv4 {
    pub scopl_ipaddr: [super::super::Networking::WinSock::IN_ADDR; 1],
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for Scope_list_ipv4 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for Scope_list_ipv4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct Session_IPv4 {
    pub sess_destaddr: super::super::Networking::WinSock::IN_ADDR,
    pub sess_protid: u8,
    pub sess_flags: u8,
    pub sess_destport: u16,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for Session_IPv4 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for Session_IPv4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct TCG_PCClientPCREventStruct {
    pub pcrIndex: u32,
    pub eventType: u32,
    pub digest: [u8; 20],
    pub eventDataSize: u32,
    pub event: [u8; 1],
}
impl ::core::marker::Copy for TCG_PCClientPCREventStruct {}
impl ::core::clone::Clone for TCG_PCClientPCREventStruct {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct TCG_PCClientTaggedEventStruct {
    pub EventID: u32,
    pub EventDataSize: u32,
    pub EventData: [u8; 1],
}
impl ::core::marker::Copy for TCG_PCClientTaggedEventStruct {}
impl ::core::clone::Clone for TCG_PCClientTaggedEventStruct {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct TCI_CLIENT_FUNC_LIST {
    pub ClNotifyHandler: TCI_NOTIFY_HANDLER,
    pub ClAddFlowCompleteHandler: TCI_ADD_FLOW_COMPLETE_HANDLER,
    pub ClModifyFlowCompleteHandler: TCI_MOD_FLOW_COMPLETE_HANDLER,
    pub ClDeleteFlowCompleteHandler: TCI_DEL_FLOW_COMPLETE_HANDLER,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for TCI_CLIENT_FUNC_LIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for TCI_CLIENT_FUNC_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TC_GEN_FILTER {
    pub AddressType: u16,
    pub PatternSize: u32,
    pub Pattern: *mut ::core::ffi::c_void,
    pub Mask: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for TC_GEN_FILTER {}
impl ::core::clone::Clone for TC_GEN_FILTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct TC_GEN_FLOW {
    pub SendingFlowspec: super::super::Networking::WinSock::FLOWSPEC,
    pub ReceivingFlowspec: super::super::Networking::WinSock::FLOWSPEC,
    pub TcObjectsLength: u32,
    pub TcObjects: [QOS_OBJECT_HDR; 1],
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for TC_GEN_FLOW {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for TC_GEN_FLOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct TC_IFC_DESCRIPTOR {
    pub Length: u32,
    pub pInterfaceName: ::windows_sys::core::PWSTR,
    pub pInterfaceID: ::windows_sys::core::PWSTR,
    pub AddressListDesc: ADDRESS_LIST_DESCRIPTOR,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for TC_IFC_DESCRIPTOR {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for TC_IFC_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct TC_SUPPORTED_INFO_BUFFER {
    pub InstanceIDLength: u16,
    pub InstanceID: [u16; 256],
    pub InterfaceLuid: u64,
    pub AddrListDesc: ADDRESS_LIST_DESCRIPTOR,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for TC_SUPPORTED_INFO_BUFFER {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for TC_SUPPORTED_INFO_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WBCL_Iterator {
    pub firstElementPtr: *mut ::core::ffi::c_void,
    pub logSize: u32,
    pub currentElementPtr: *mut ::core::ffi::c_void,
    pub currentElementSize: u32,
    pub digestSize: u16,
    pub logFormat: u16,
    pub numberOfDigests: u32,
    pub digestSizes: *mut ::core::ffi::c_void,
    pub supportedAlgorithms: u32,
    pub hashAlgorithm: u16,
}
impl ::core::marker::Copy for WBCL_Iterator {}
impl ::core::clone::Clone for WBCL_Iterator {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WBCL_LogHdr {
    pub signature: u32,
    pub version: u32,
    pub entries: u32,
    pub length: u32,
}
impl ::core::marker::Copy for WBCL_LogHdr {}
impl ::core::clone::Clone for WBCL_LogHdr {
    fn clone(&self) -> Self {
        *self
    }
}
pub type CBADMITRESULT = ::core::option::Option<unsafe extern "system" fn(lpmhandle: LPM_HANDLE, requesthandle: RHANDLE, ulpcmactionflags: u32, lpmerror: i32, policydecisionscount: i32, ppolicydecisions: *mut POLICY_DECISION) -> *mut u32>;
pub type CBGETRSVPOBJECTS = ::core::option::Option<unsafe extern "system" fn(lpmhandle: LPM_HANDLE, requesthandle: RHANDLE, lpmerror: i32, rsvpobjectscount: i32, pprsvpobjects: *mut *mut RsvpObjHdr) -> *mut u32>;
pub type PALLOCMEM = ::core::option::Option<unsafe extern "system" fn(size: u32) -> *mut ::core::ffi::c_void>;
pub type PFREEMEM = ::core::option::Option<unsafe extern "system" fn(pv: *mut ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TCI_ADD_FLOW_COMPLETE_HANDLER = ::core::option::Option<unsafe extern "system" fn(clflowctx: super::super::Foundation::HANDLE, status: u32) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TCI_DEL_FLOW_COMPLETE_HANDLER = ::core::option::Option<unsafe extern "system" fn(clflowctx: super::super::Foundation::HANDLE, status: u32) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TCI_MOD_FLOW_COMPLETE_HANDLER = ::core::option::Option<unsafe extern "system" fn(clflowctx: super::super::Foundation::HANDLE, status: u32) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TCI_NOTIFY_HANDLER = ::core::option::Option<unsafe extern "system" fn(clregctx: super::super::Foundation::HANDLE, clifcctx: super::super::Foundation::HANDLE, event: u32, subcode: super::super::Foundation::HANDLE, bufsize: u32, buffer: *const ::core::ffi::c_void) -> ()>;
