#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WFDCancelOpenSession(hsessionhandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WFDCloseHandle(hclienthandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WFDCloseSession(hsessionhandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WFDOpenHandle(dwclientversion : u32, pdwnegotiatedversion : *mut u32, phclienthandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WFDOpenLegacySession(hclienthandle : super::super::Foundation:: HANDLE, plegacymacaddress : *const *const u8, phsessionhandle : *mut super::super::Foundation:: HANDLE, pguidsessioninterface : *mut ::windows_sys::core::GUID) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WFDStartOpenSession(hclienthandle : super::super::Foundation:: HANDLE, pdeviceaddress : *const *const u8, pvcontext : *const ::core::ffi::c_void, pfncallback : WFD_OPEN_SESSION_COMPLETE_CALLBACK, phsessionhandle : *mut super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("wlanapi.dll" "system" fn WFDUpdateDeviceVisibility(pdeviceaddress : *const *const u8) -> u32);
::windows_targets::link!("wlanapi.dll" "system" fn WlanAllocateMemory(dwmemorysize : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanCloseHandle(hclienthandle : super::super::Foundation:: HANDLE, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"] fn WlanConnect(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, pconnectionparameters : *const WLAN_CONNECTION_PARAMETERS, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"] fn WlanConnect2(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, pconnectionparameters : *const WLAN_CONNECTION_PARAMETERS_V2, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanDeleteProfile(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanDeviceServiceCommand(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, pdeviceserviceguid : *const ::windows_sys::core::GUID, dwopcode : u32, dwinbuffersize : u32, pinbuffer : *const ::core::ffi::c_void, dwoutbuffersize : u32, poutbuffer : *mut ::core::ffi::c_void, pdwbytesreturned : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanDisconnect(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanEnumInterfaces(hclienthandle : super::super::Foundation:: HANDLE, preserved : *const ::core::ffi::c_void, ppinterfacelist : *mut *mut WLAN_INTERFACE_INFO_LIST) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanExtractPsdIEDataList(hclienthandle : super::super::Foundation:: HANDLE, dwiedatasize : u32, prawiedata : *const u8, strformat : ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void, pppsdiedatalist : *mut *mut WLAN_RAW_DATA_LIST) -> u32);
::windows_targets::link!("wlanapi.dll" "system" fn WlanFreeMemory(pmemory : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetAvailableNetworkList(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, dwflags : u32, preserved : *const ::core::ffi::c_void, ppavailablenetworklist : *mut *mut WLAN_AVAILABLE_NETWORK_LIST) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetAvailableNetworkList2(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, dwflags : u32, preserved : *const ::core::ffi::c_void, ppavailablenetworklist : *mut *mut WLAN_AVAILABLE_NETWORK_LIST_V2) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetFilterList(hclienthandle : super::super::Foundation:: HANDLE, wlanfilterlisttype : WLAN_FILTER_LIST_TYPE, preserved : *const ::core::ffi::c_void, ppnetworklist : *mut *mut DOT11_NETWORK_LIST) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetInterfaceCapability(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, preserved : *const ::core::ffi::c_void, ppcapability : *mut *mut WLAN_INTERFACE_CAPABILITY) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetNetworkBssList(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, pdot11ssid : *const DOT11_SSID, dot11bsstype : DOT11_BSS_TYPE, bsecurityenabled : super::super::Foundation:: BOOL, preserved : *const ::core::ffi::c_void, ppwlanbsslist : *mut *mut WLAN_BSS_LIST) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetProfile(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void, pstrprofilexml : *mut ::windows_sys::core::PWSTR, pdwflags : *mut u32, pdwgrantedaccess : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetProfileCustomUserData(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void, pdwdatasize : *mut u32, ppdata : *mut *mut u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetProfileList(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, preserved : *const ::core::ffi::c_void, ppprofilelist : *mut *mut WLAN_PROFILE_INFO_LIST) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetSecuritySettings(hclienthandle : super::super::Foundation:: HANDLE, securableobject : WLAN_SECURABLE_OBJECT, pvaluetype : *mut WLAN_OPCODE_VALUE_TYPE, pstrcurrentsddl : *mut ::windows_sys::core::PWSTR, pdwgrantedaccess : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanGetSupportedDeviceServices(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, ppdevsvcguidlist : *mut *mut WLAN_DEVICE_SERVICE_GUID_LIST) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkForceStart(hclienthandle : super::super::Foundation:: HANDLE, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkForceStop(hclienthandle : super::super::Foundation:: HANDLE, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkInitSettings(hclienthandle : super::super::Foundation:: HANDLE, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkQueryProperty(hclienthandle : super::super::Foundation:: HANDLE, opcode : WLAN_HOSTED_NETWORK_OPCODE, pdwdatasize : *mut u32, ppvdata : *mut *mut ::core::ffi::c_void, pwlanopcodevaluetype : *mut WLAN_OPCODE_VALUE_TYPE, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkQuerySecondaryKey(hclienthandle : super::super::Foundation:: HANDLE, pdwkeylength : *mut u32, ppuckeydata : *mut *mut u8, pbispassphrase : *mut super::super::Foundation:: BOOL, pbpersistent : *mut super::super::Foundation:: BOOL, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkQueryStatus(hclienthandle : super::super::Foundation:: HANDLE, ppwlanhostednetworkstatus : *mut *mut WLAN_HOSTED_NETWORK_STATUS, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkRefreshSecuritySettings(hclienthandle : super::super::Foundation:: HANDLE, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkSetProperty(hclienthandle : super::super::Foundation:: HANDLE, opcode : WLAN_HOSTED_NETWORK_OPCODE, dwdatasize : u32, pvdata : *const ::core::ffi::c_void, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkSetSecondaryKey(hclienthandle : super::super::Foundation:: HANDLE, dwkeylength : u32, puckeydata : *const u8, bispassphrase : super::super::Foundation:: BOOL, bpersistent : super::super::Foundation:: BOOL, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkStartUsing(hclienthandle : super::super::Foundation:: HANDLE, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanHostedNetworkStopUsing(hclienthandle : super::super::Foundation:: HANDLE, pfailreason : *mut WLAN_HOSTED_NETWORK_REASON, pvreserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanIhvControl(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, r#type : WLAN_IHV_CONTROL_TYPE, dwinbuffersize : u32, pinbuffer : *const ::core::ffi::c_void, dwoutbuffersize : u32, poutbuffer : *mut ::core::ffi::c_void, pdwbytesreturned : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanOpenHandle(dwclientversion : u32, preserved : *const ::core::ffi::c_void, pdwnegotiatedversion : *mut u32, phclienthandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanQueryAutoConfigParameter(hclienthandle : super::super::Foundation:: HANDLE, opcode : WLAN_AUTOCONF_OPCODE, preserved : *const ::core::ffi::c_void, pdwdatasize : *mut u32, ppdata : *mut *mut ::core::ffi::c_void, pwlanopcodevaluetype : *mut WLAN_OPCODE_VALUE_TYPE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanQueryInterface(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, opcode : WLAN_INTF_OPCODE, preserved : *const ::core::ffi::c_void, pdwdatasize : *mut u32, ppdata : *mut *mut ::core::ffi::c_void, pwlanopcodevaluetype : *mut WLAN_OPCODE_VALUE_TYPE) -> u32);
::windows_targets::link!("wlanapi.dll" "system" fn WlanReasonCodeToString(dwreasoncode : u32, dwbuffersize : u32, pstringbuffer : ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanRegisterDeviceServiceNotification(hclienthandle : super::super::Foundation:: HANDLE, pdevsvcguidlist : *const WLAN_DEVICE_SERVICE_GUID_LIST) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanRegisterNotification(hclienthandle : super::super::Foundation:: HANDLE, dwnotifsource : WLAN_NOTIFICATION_SOURCES, bignoreduplicate : super::super::Foundation:: BOOL, funccallback : WLAN_NOTIFICATION_CALLBACK, pcallbackcontext : *const ::core::ffi::c_void, preserved : *const ::core::ffi::c_void, pdwprevnotifsource : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanRegisterVirtualStationNotification(hclienthandle : super::super::Foundation:: HANDLE, bregister : super::super::Foundation:: BOOL, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanRenameProfile(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, stroldprofilename : ::windows_sys::core::PCWSTR, strnewprofilename : ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSaveTemporaryProfile(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, stralluserprofilesecurity : ::windows_sys::core::PCWSTR, dwflags : u32, boverwrite : super::super::Foundation:: BOOL, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanScan(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, pdot11ssid : *const DOT11_SSID, piedata : *const WLAN_RAW_DATA, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetAutoConfigParameter(hclienthandle : super::super::Foundation:: HANDLE, opcode : WLAN_AUTOCONF_OPCODE, dwdatasize : u32, pdata : *const ::core::ffi::c_void, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetFilterList(hclienthandle : super::super::Foundation:: HANDLE, wlanfilterlisttype : WLAN_FILTER_LIST_TYPE, pnetworklist : *const DOT11_NETWORK_LIST, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetInterface(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, opcode : WLAN_INTF_OPCODE, dwdatasize : u32, pdata : *const ::core::ffi::c_void, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetProfile(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, dwflags : u32, strprofilexml : ::windows_sys::core::PCWSTR, stralluserprofilesecurity : ::windows_sys::core::PCWSTR, boverwrite : super::super::Foundation:: BOOL, preserved : *const ::core::ffi::c_void, pdwreasoncode : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetProfileCustomUserData(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, dwdatasize : u32, pdata : *const u8, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"] fn WlanSetProfileEapUserData(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, eaptype : super::super::Security::ExtensibleAuthenticationProtocol:: EAP_METHOD_TYPE, dwflags : WLAN_SET_EAPHOST_FLAGS, dweapuserdatasize : u32, pbeapuserdata : *const u8, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetProfileEapXmlUserData(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, dwflags : WLAN_SET_EAPHOST_FLAGS, streapxmluserdata : ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetProfileList(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, dwitems : u32, strprofilenames : *const ::windows_sys::core::PCWSTR, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetProfilePosition(hclienthandle : super::super::Foundation:: HANDLE, pinterfaceguid : *const ::windows_sys::core::GUID, strprofilename : ::windows_sys::core::PCWSTR, dwposition : u32, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetPsdIEDataList(hclienthandle : super::super::Foundation:: HANDLE, strformat : ::windows_sys::core::PCWSTR, ppsdiedatalist : *const WLAN_RAW_DATA_LIST, preserved : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanSetSecuritySettings(hclienthandle : super::super::Foundation:: HANDLE, securableobject : WLAN_SECURABLE_OBJECT, strmodifiedsddl : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wlanui.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WlanUIEditProfile(dwclientversion : u32, wstrprofilename : ::windows_sys::core::PCWSTR, pinterfaceguid : *const ::windows_sys::core::GUID, hwnd : super::super::Foundation:: HWND, wlstartpage : WL_DISPLAY_PAGES, preserved : *const ::core::ffi::c_void, pwlanreasoncode : *mut u32) -> u32);
pub type IDot11AdHocInterface = *mut ::core::ffi::c_void;
pub type IDot11AdHocInterfaceNotificationSink = *mut ::core::ffi::c_void;
pub type IDot11AdHocManager = *mut ::core::ffi::c_void;
pub type IDot11AdHocManagerNotificationSink = *mut ::core::ffi::c_void;
pub type IDot11AdHocNetwork = *mut ::core::ffi::c_void;
pub type IDot11AdHocNetworkNotificationSink = *mut ::core::ffi::c_void;
pub type IDot11AdHocSecuritySettings = *mut ::core::ffi::c_void;
pub type IEnumDot11AdHocInterfaces = *mut ::core::ffi::c_void;
pub type IEnumDot11AdHocNetworks = *mut ::core::ffi::c_void;
pub type IEnumDot11AdHocSecuritySettings = *mut ::core::ffi::c_void;
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_AccessPointBssid: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 19 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_ChallengeAep: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 21 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_DevnodeAep: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 23 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_HostName_ResolutionMode: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 25 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_PinSupported: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 29 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_RtspTcpConnectionParametersSupported: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 30 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_SinkHostName: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 20 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_SinkIpAddress: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 26 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_StreamSecuritySupported: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 18 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_InfraCast_Supported: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 17 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_ASPMSupport: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 8 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_ClockPowerManagementSupport: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 9 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_CurrentSpeedAndMode: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 2 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_DeviceIDMessagingCapable: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 4 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_ExtendedConfigAvailable: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 6 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_ExtendedPCIConfigOpRegionSupport: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 7 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_MSISupport: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 11 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_NativePciExpressControl: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 17 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_PCIExpressAERControl: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 15 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_PCIExpressCapabilityControl: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 16 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_PCIExpressNativeHotPlugControl: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 12 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_PCIExpressNativePMEControl: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 14 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_PCISegmentGroupsSupport: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 10 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_SHPCNativeHotPlugControl: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 13 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_SecondaryBusWidth: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 5 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_SecondaryInterface: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 1 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_SupportedSpeedsAndModes: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 3 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_PciRootBus_SystemMsiSupport: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xd817fc28_793e_4b9e_9970_469d8be63073), pid: 18 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirectServices_AdvertisementId: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x31b37743_7c5e_4005_93e6_e953f92b82e9), pid: 5 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirectServices_RequestServiceInformation: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x31b37743_7c5e_4005_93e6_e953f92b82e9), pid: 7 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirectServices_ServiceAddress: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x31b37743_7c5e_4005_93e6_e953f92b82e9), pid: 2 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirectServices_ServiceConfigMethods: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x31b37743_7c5e_4005_93e6_e953f92b82e9), pid: 6 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirectServices_ServiceInformation: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x31b37743_7c5e_4005_93e6_e953f92b82e9), pid: 4 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirectServices_ServiceName: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x31b37743_7c5e_4005_93e6_e953f92b82e9), pid: 3 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_DeviceAddress: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 1 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_DeviceAddressCopy: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 13 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_FoundWsbService: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 24 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_GroupId: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 4 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_InformationElements: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 12 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_InterfaceAddress: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 2 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_InterfaceGuid: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 3 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_IsConnected: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 5 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_IsDMGCapable: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 22 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_IsLegacyDevice: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 7 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_IsMiracastLCPSupported: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 9 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_IsRecentlyAssociated: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 14 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_IsVisible: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 6 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_LinkQuality: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 28 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_MiracastVersion: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 8 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_Miracast_SessionMgmtControlPort: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 31 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_NoMiracastAutoProject: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 16 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_RtspTcpConnectionParametersSupported: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 32 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_Service_Aeps: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 15 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_Services: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 10 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_SupportedChannelList: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 11 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFiDirect_TransientAssociation: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0x1506935d_e3e7_450f_8637_82233ebe5f6e), pid: 27 };
#[doc = "Required features: `\"Win32_Devices_Properties\"`"]
#[cfg(feature = "Win32_Devices_Properties")]
pub const DEVPKEY_WiFi_InterfaceGuid: super::super::Devices::Properties::DEVPROPKEY = super::super::Devices::Properties::DEVPROPKEY { fmtid: ::windows_sys::core::GUID::from_u128(0xef1167eb_cbfc_4341_a568_a7c91a68982c), pid: 2 };
pub const DISCOVERY_FILTER_BITMASK_ANY: u32 = 15u32;
pub const DISCOVERY_FILTER_BITMASK_DEVICE: u32 = 1u32;
pub const DISCOVERY_FILTER_BITMASK_GO: u32 = 2u32;
pub const DOT11EXT_PSK_MAX_LENGTH: u32 = 64u32;
pub const DOT11_ADDITIONAL_IE_REVISION_1: u32 = 1u32;
pub const DOT11_ADHOC_AUTH_ALGO_80211_OPEN: DOT11_ADHOC_AUTH_ALGORITHM = 1i32;
pub const DOT11_ADHOC_AUTH_ALGO_INVALID: DOT11_ADHOC_AUTH_ALGORITHM = -1i32;
pub const DOT11_ADHOC_AUTH_ALGO_RSNA_PSK: DOT11_ADHOC_AUTH_ALGORITHM = 7i32;
pub const DOT11_ADHOC_CIPHER_ALGO_CCMP: DOT11_ADHOC_CIPHER_ALGORITHM = 4i32;
pub const DOT11_ADHOC_CIPHER_ALGO_INVALID: DOT11_ADHOC_CIPHER_ALGORITHM = -1i32;
pub const DOT11_ADHOC_CIPHER_ALGO_NONE: DOT11_ADHOC_CIPHER_ALGORITHM = 0i32;
pub const DOT11_ADHOC_CIPHER_ALGO_WEP: DOT11_ADHOC_CIPHER_ALGORITHM = 257i32;
pub const DOT11_ADHOC_CONNECT_FAIL_DOMAIN_MISMATCH: DOT11_ADHOC_CONNECT_FAIL_REASON = 0i32;
pub const DOT11_ADHOC_CONNECT_FAIL_OTHER: DOT11_ADHOC_CONNECT_FAIL_REASON = 2i32;
pub const DOT11_ADHOC_CONNECT_FAIL_PASSPHRASE_MISMATCH: DOT11_ADHOC_CONNECT_FAIL_REASON = 1i32;
pub const DOT11_ADHOC_NETWORK_CONNECTION_STATUS_CONNECTED: DOT11_ADHOC_NETWORK_CONNECTION_STATUS = 13i32;
pub const DOT11_ADHOC_NETWORK_CONNECTION_STATUS_CONNECTING: DOT11_ADHOC_NETWORK_CONNECTION_STATUS = 12i32;
pub const DOT11_ADHOC_NETWORK_CONNECTION_STATUS_DISCONNECTED: DOT11_ADHOC_NETWORK_CONNECTION_STATUS = 11i32;
pub const DOT11_ADHOC_NETWORK_CONNECTION_STATUS_FORMED: DOT11_ADHOC_NETWORK_CONNECTION_STATUS = 14i32;
pub const DOT11_ADHOC_NETWORK_CONNECTION_STATUS_INVALID: DOT11_ADHOC_NETWORK_CONNECTION_STATUS = 0i32;
pub const DOT11_ANQP_QUERY_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_ASSOCIATION_COMPLETION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_ASSOCIATION_COMPLETION_PARAMETERS_REVISION_2: u32 = 2u32;
pub const DOT11_ASSOCIATION_INFO_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_ASSOCIATION_PARAMS_REVISION_1: u32 = 1u32;
pub const DOT11_ASSOCIATION_START_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_ASSOC_ERROR_SOURCE_OS: u32 = 0u32;
pub const DOT11_ASSOC_ERROR_SOURCE_OTHER: u32 = 255u32;
pub const DOT11_ASSOC_ERROR_SOURCE_REMOTE: u32 = 1u32;
pub const DOT11_ASSOC_STATUS_SUCCESS: u32 = 0u32;
pub const DOT11_AUTH_ALGORITHM_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_AUTH_ALGO_80211_OPEN: DOT11_AUTH_ALGORITHM = 1i32;
pub const DOT11_AUTH_ALGO_80211_SHARED_KEY: DOT11_AUTH_ALGORITHM = 2i32;
pub const DOT11_AUTH_ALGO_IHV_END: DOT11_AUTH_ALGORITHM = -1i32;
pub const DOT11_AUTH_ALGO_IHV_START: DOT11_AUTH_ALGORITHM = -2147483648i32;
pub const DOT11_AUTH_ALGO_MICHAEL: u32 = 1u32;
pub const DOT11_AUTH_ALGO_OWE: DOT11_AUTH_ALGORITHM = 10i32;
pub const DOT11_AUTH_ALGO_RSNA: DOT11_AUTH_ALGORITHM = 6i32;
pub const DOT11_AUTH_ALGO_RSNA_PSK: DOT11_AUTH_ALGORITHM = 7i32;
pub const DOT11_AUTH_ALGO_WPA: DOT11_AUTH_ALGORITHM = 3i32;
pub const DOT11_AUTH_ALGO_WPA3: DOT11_AUTH_ALGORITHM = 8i32;
pub const DOT11_AUTH_ALGO_WPA3_ENT: DOT11_AUTH_ALGORITHM = 11i32;
pub const DOT11_AUTH_ALGO_WPA3_ENT_192: DOT11_AUTH_ALGORITHM = 8i32;
pub const DOT11_AUTH_ALGO_WPA3_SAE: DOT11_AUTH_ALGORITHM = 9i32;
pub const DOT11_AUTH_ALGO_WPA_NONE: DOT11_AUTH_ALGORITHM = 5i32;
pub const DOT11_AUTH_ALGO_WPA_PSK: DOT11_AUTH_ALGORITHM = 4i32;
pub const DOT11_AUTH_CIPHER_PAIR_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_AVAILABLE_CHANNEL_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_AVAILABLE_FREQUENCY_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_BSSID_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_BSS_ENTRY_BYTE_ARRAY_REVISION_1: u32 = 1u32;
pub const DOT11_CAN_SUSTAIN_AP_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_CAN_SUSTAIN_AP_REASON_IHV_END: u32 = 4294967295u32;
pub const DOT11_CAN_SUSTAIN_AP_REASON_IHV_START: u32 = 4278190080u32;
pub const DOT11_CAPABILITY_CHANNEL_AGILITY: u32 = 128u32;
pub const DOT11_CAPABILITY_DSSSOFDM: u32 = 8192u32;
pub const DOT11_CAPABILITY_INFO_CF_POLLABLE: u32 = 4u32;
pub const DOT11_CAPABILITY_INFO_CF_POLL_REQ: u32 = 8u32;
pub const DOT11_CAPABILITY_INFO_ESS: u32 = 1u32;
pub const DOT11_CAPABILITY_INFO_IBSS: u32 = 2u32;
pub const DOT11_CAPABILITY_INFO_PRIVACY: u32 = 16u32;
pub const DOT11_CAPABILITY_PBCC: u32 = 64u32;
pub const DOT11_CAPABILITY_SHORT_PREAMBLE: u32 = 32u32;
pub const DOT11_CAPABILITY_SHORT_SLOT_TIME: u32 = 1024u32;
pub const DOT11_CCA_MODE_CS_ONLY: u32 = 2u32;
pub const DOT11_CCA_MODE_CS_WITH_TIMER: u32 = 8u32;
pub const DOT11_CCA_MODE_ED_ONLY: u32 = 1u32;
pub const DOT11_CCA_MODE_ED_and_CS: u32 = 4u32;
pub const DOT11_CCA_MODE_HRCS_AND_ED: u32 = 16u32;
pub const DOT11_CIPHER_ALGORITHM_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_CIPHER_ALGO_BIP: DOT11_CIPHER_ALGORITHM = 6i32;
pub const DOT11_CIPHER_ALGO_BIP_CMAC_256: DOT11_CIPHER_ALGORITHM = 13i32;
pub const DOT11_CIPHER_ALGO_BIP_GMAC_128: DOT11_CIPHER_ALGORITHM = 11i32;
pub const DOT11_CIPHER_ALGO_BIP_GMAC_256: DOT11_CIPHER_ALGORITHM = 12i32;
pub const DOT11_CIPHER_ALGO_CCMP: DOT11_CIPHER_ALGORITHM = 4i32;
pub const DOT11_CIPHER_ALGO_CCMP_256: DOT11_CIPHER_ALGORITHM = 10i32;
pub const DOT11_CIPHER_ALGO_GCMP: DOT11_CIPHER_ALGORITHM = 8i32;
pub const DOT11_CIPHER_ALGO_GCMP_256: DOT11_CIPHER_ALGORITHM = 9i32;
pub const DOT11_CIPHER_ALGO_IHV_END: DOT11_CIPHER_ALGORITHM = -1i32;
pub const DOT11_CIPHER_ALGO_IHV_START: DOT11_CIPHER_ALGORITHM = -2147483648i32;
pub const DOT11_CIPHER_ALGO_NONE: DOT11_CIPHER_ALGORITHM = 0i32;
pub const DOT11_CIPHER_ALGO_RSN_USE_GROUP: DOT11_CIPHER_ALGORITHM = 256i32;
pub const DOT11_CIPHER_ALGO_TKIP: DOT11_CIPHER_ALGORITHM = 2i32;
pub const DOT11_CIPHER_ALGO_WEP: DOT11_CIPHER_ALGORITHM = 257i32;
pub const DOT11_CIPHER_ALGO_WEP104: DOT11_CIPHER_ALGORITHM = 5i32;
pub const DOT11_CIPHER_ALGO_WEP40: DOT11_CIPHER_ALGORITHM = 1i32;
pub const DOT11_CIPHER_ALGO_WPA_USE_GROUP: DOT11_CIPHER_ALGORITHM = 256i32;
pub const DOT11_CIPHER_DEFAULT_KEY_VALUE_REVISION_1: u32 = 1u32;
pub const DOT11_CIPHER_KEY_MAPPING_KEY_VALUE_BYTE_ARRAY_REVISION_1: u32 = 1u32;
pub const DOT11_CONF_ALGO_TKIP: u32 = 2u32;
pub const DOT11_CONF_ALGO_WEP_RC4: u32 = 1u32;
pub const DOT11_CONNECTION_COMPLETION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_CONNECTION_START_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_CONNECTION_STATUS_SUCCESS: u32 = 0u32;
pub const DOT11_COUNTRY_OR_REGION_STRING_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_DATA_RATE_MAPPING_TABLE_REVISION_1: u32 = 1u32;
pub const DOT11_DEVICE_ENTRY_BYTE_ARRAY_REVISION_1: u32 = 1u32;
pub const DOT11_DIR_BOTH: DOT11_DIRECTION = 3i32;
pub const DOT11_DIR_INBOUND: DOT11_DIRECTION = 1i32;
pub const DOT11_DIR_OUTBOUND: DOT11_DIRECTION = 2i32;
pub const DOT11_DISASSOCIATE_PEER_REQUEST_REVISION_1: u32 = 1u32;
pub const DOT11_DISASSOCIATION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_DS_CHANGED: DOT11_DS_INFO = 0i32;
pub const DOT11_DS_UNCHANGED: DOT11_DS_INFO = 1i32;
pub const DOT11_DS_UNKNOWN: DOT11_DS_INFO = 2i32;
pub const DOT11_ENCAP_802_1H: u32 = 2u32;
pub const DOT11_ENCAP_RFC_1042: u32 = 1u32;
pub const DOT11_EXEMPT_ALWAYS: u32 = 1u32;
pub const DOT11_EXEMPT_BOTH: u32 = 3u32;
pub const DOT11_EXEMPT_MULTICAST: u32 = 2u32;
pub const DOT11_EXEMPT_NO_EXEMPTION: u32 = 0u32;
pub const DOT11_EXEMPT_ON_KEY_MAPPING_KEY_UNAVAILABLE: u32 = 2u32;
pub const DOT11_EXEMPT_UNICAST: u32 = 1u32;
pub const DOT11_EXTAP_ATTRIBUTES_REVISION_1: u32 = 1u32;
pub const DOT11_EXTAP_RECV_CONTEXT_REVISION_1: u32 = 1u32;
pub const DOT11_EXTAP_SEND_CONTEXT_REVISION_1: u32 = 1u32;
pub const DOT11_EXTSTA_ATTRIBUTES_REVISION_1: u32 = 1u32;
pub const DOT11_EXTSTA_ATTRIBUTES_REVISION_2: u32 = 2u32;
pub const DOT11_EXTSTA_ATTRIBUTES_REVISION_3: u32 = 3u32;
pub const DOT11_EXTSTA_ATTRIBUTES_REVISION_4: u32 = 4u32;
pub const DOT11_EXTSTA_ATTRIBUTES_SAFEMODE_CERTIFIED: u32 = 2u32;
pub const DOT11_EXTSTA_ATTRIBUTES_SAFEMODE_OID_SUPPORTED: u32 = 1u32;
pub const DOT11_EXTSTA_ATTRIBUTES_SAFEMODE_RESERVED: u32 = 12u32;
pub const DOT11_EXTSTA_CAPABILITY_REVISION_1: u32 = 1u32;
pub const DOT11_EXTSTA_RECV_CONTEXT_REVISION_1: u32 = 1u32;
pub const DOT11_EXTSTA_SEND_CONTEXT_REVISION_1: u32 = 1u32;
pub const DOT11_FLAGS_80211B_CHANNEL_AGILITY: u32 = 4u32;
pub const DOT11_FLAGS_80211B_PBCC: u32 = 2u32;
pub const DOT11_FLAGS_80211B_SHORT_PREAMBLE: u32 = 1u32;
pub const DOT11_FLAGS_80211G_BARKER_PREAMBLE_MODE: u32 = 128u32;
pub const DOT11_FLAGS_80211G_DSSS_OFDM: u32 = 16u32;
pub const DOT11_FLAGS_80211G_NON_ERP_PRESENT: u32 = 64u32;
pub const DOT11_FLAGS_80211G_USE_PROTECTION: u32 = 32u32;
pub const DOT11_FLAGS_PS_ON: u32 = 8u32;
pub const DOT11_FREQUENCY_BANDS_LOWER: u32 = 1u32;
pub const DOT11_FREQUENCY_BANDS_MIDDLE: u32 = 2u32;
pub const DOT11_FREQUENCY_BANDS_UPPER: u32 = 4u32;
pub const DOT11_GO_NEGOTIATION_CONFIRMATION_SEND_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_GO_NEGOTIATION_REQUEST_SEND_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_GO_NEGOTIATION_RESPONSE_SEND_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_HESSID_LENGTH: u32 = 6u32;
pub const DOT11_HR_CCA_MODE_CS_AND_ED: u32 = 4u32;
pub const DOT11_HR_CCA_MODE_CS_ONLY: u32 = 2u32;
pub const DOT11_HR_CCA_MODE_CS_WITH_TIMER: u32 = 8u32;
pub const DOT11_HR_CCA_MODE_ED_ONLY: u32 = 1u32;
pub const DOT11_HR_CCA_MODE_HRCS_AND_ED: u32 = 16u32;
pub const DOT11_HW_DEFRAGMENTATION_SUPPORTED: u32 = 8u32;
pub const DOT11_HW_FRAGMENTATION_SUPPORTED: u32 = 4u32;
pub const DOT11_HW_MSDU_AUTH_SUPPORTED_RX: u32 = 32u32;
pub const DOT11_HW_MSDU_AUTH_SUPPORTED_TX: u32 = 16u32;
pub const DOT11_HW_WEP_SUPPORTED_RX: u32 = 2u32;
pub const DOT11_HW_WEP_SUPPORTED_TX: u32 = 1u32;
pub const DOT11_IBSS_PARAMS_REVISION_1: u32 = 1u32;
pub const DOT11_INCOMING_ASSOC_COMPLETION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_INCOMING_ASSOC_DECISION_REVISION_1: u32 = 1u32;
pub const DOT11_INCOMING_ASSOC_DECISION_REVISION_2: u32 = 2u32;
pub const DOT11_INCOMING_ASSOC_REQUEST_RECEIVED_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_INCOMING_ASSOC_STARTED_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_INVALID_CHANNEL_NUMBER: u32 = 0u32;
pub const DOT11_INVITATION_REQUEST_SEND_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_INVITATION_RESPONSE_SEND_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_LINK_QUALITY_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_MAC_ADDRESS_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_MAC_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_MANUFACTURING_CALLBACK_REVISION_1: u32 = 1u32;
pub const DOT11_MANUFACTURING_SELF_TEST_TYPE_BT_COEXISTENCE: DOT11_MANUFACTURING_SELF_TEST_TYPE = 3i32;
pub const DOT11_MANUFACTURING_SELF_TEST_TYPE_INTERFACE: DOT11_MANUFACTURING_SELF_TEST_TYPE = 1i32;
pub const DOT11_MANUFACTURING_SELF_TEST_TYPE_RF_INTERFACE: DOT11_MANUFACTURING_SELF_TEST_TYPE = 2i32;
pub const DOT11_MANUFACTURING_TEST_REVISION_1: u32 = 1u32;
pub const DOT11_MAX_CHANNEL_HINTS: u32 = 4u32;
pub const DOT11_MAX_NUM_DEFAULT_KEY: u32 = 4u32;
pub const DOT11_MAX_NUM_DEFAULT_KEY_MFP: u32 = 6u32;
pub const DOT11_MAX_NUM_OF_FRAGMENTS: u32 = 16u32;
pub const DOT11_MAX_PDU_SIZE: u32 = 2346u32;
pub const DOT11_MAX_REQUESTED_SERVICE_INFORMATION_LENGTH: u32 = 255u32;
pub const DOT11_MIN_PDU_SIZE: u32 = 256u32;
pub const DOT11_MPDU_MAX_LENGTH_INDICATION_REVISION_1: u32 = 1u32;
pub const DOT11_MSONEX_FAILURE: DOT11_MSONEX_RESULT = 1i32;
pub const DOT11_MSONEX_IN_PROGRESS: DOT11_MSONEX_RESULT = 2i32;
pub const DOT11_MSONEX_SUCCESS: DOT11_MSONEX_RESULT = 0i32;
pub const DOT11_NLO_FLAG_SCAN_AT_SYSTEM_RESUME: u32 = 4u32;
pub const DOT11_NLO_FLAG_SCAN_ON_AOAC_PLATFORM: u32 = 2u32;
pub const DOT11_NLO_FLAG_STOP_NLO_INDICATION: u32 = 1u32;
pub const DOT11_OFFLOAD_NETWORK_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_OFFLOAD_NETWORK_STATUS_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_OI_MAX_LENGTH: u32 = 5u32;
pub const DOT11_OI_MIN_LENGTH: u32 = 3u32;
pub const DOT11_OPERATION_MODE_AP: u32 = 2u32;
pub const DOT11_OPERATION_MODE_EXTENSIBLE_AP: u32 = 8u32;
pub const DOT11_OPERATION_MODE_EXTENSIBLE_STATION: u32 = 4u32;
pub const DOT11_OPERATION_MODE_MANUFACTURING: u32 = 1073741824u32;
pub const DOT11_OPERATION_MODE_NETWORK_MONITOR: u32 = 2147483648u32;
pub const DOT11_OPERATION_MODE_STATION: u32 = 1u32;
pub const DOT11_OPERATION_MODE_UNKNOWN: u32 = 0u32;
pub const DOT11_OPERATION_MODE_WFD_CLIENT: u32 = 64u32;
pub const DOT11_OPERATION_MODE_WFD_DEVICE: u32 = 16u32;
pub const DOT11_OPERATION_MODE_WFD_GROUP_OWNER: u32 = 32u32;
pub const DOT11_PACKET_TYPE_ALL_MULTICAST_CTRL: u32 = 4096u32;
pub const DOT11_PACKET_TYPE_ALL_MULTICAST_DATA: u32 = 16384u32;
pub const DOT11_PACKET_TYPE_ALL_MULTICAST_MGMT: u32 = 8192u32;
pub const DOT11_PACKET_TYPE_BROADCAST_CTRL: u32 = 64u32;
pub const DOT11_PACKET_TYPE_BROADCAST_DATA: u32 = 256u32;
pub const DOT11_PACKET_TYPE_BROADCAST_MGMT: u32 = 128u32;
pub const DOT11_PACKET_TYPE_DIRECTED_CTRL: u32 = 1u32;
pub const DOT11_PACKET_TYPE_DIRECTED_DATA: u32 = 4u32;
pub const DOT11_PACKET_TYPE_DIRECTED_MGMT: u32 = 2u32;
pub const DOT11_PACKET_TYPE_MULTICAST_CTRL: u32 = 8u32;
pub const DOT11_PACKET_TYPE_MULTICAST_DATA: u32 = 32u32;
pub const DOT11_PACKET_TYPE_MULTICAST_MGMT: u32 = 16u32;
pub const DOT11_PACKET_TYPE_PROMISCUOUS_CTRL: u32 = 512u32;
pub const DOT11_PACKET_TYPE_PROMISCUOUS_DATA: u32 = 2048u32;
pub const DOT11_PACKET_TYPE_PROMISCUOUS_MGMT: u32 = 1024u32;
pub const DOT11_PEER_INFO_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_PHY_ATTRIBUTES_REVISION_1: u32 = 1u32;
pub const DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_PHY_ID_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_PHY_STATE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_PHY_TYPE_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_PMKID_CANDIDATE_LIST_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_PMKID_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_PORT_STATE_NOTIFICATION_REVISION_1: u32 = 1u32;
pub const DOT11_POWER_MGMT_AUTO_MODE_ENABLED_REVISION_1: u32 = 1u32;
pub const DOT11_POWER_MGMT_MODE_STATUS_INFO_REVISION_1: u32 = 1u32;
pub const DOT11_POWER_SAVE_LEVEL_FAST_PSP: u32 = 2u32;
pub const DOT11_POWER_SAVE_LEVEL_MAX_PSP: u32 = 1u32;
pub const DOT11_POWER_SAVING_FAST_PSP: u32 = 8u32;
pub const DOT11_POWER_SAVING_MAXIMUM_LEVEL: u32 = 24u32;
pub const DOT11_POWER_SAVING_MAX_PSP: u32 = 16u32;
pub const DOT11_POWER_SAVING_NO_POWER_SAVING: u32 = 0u32;
pub const DOT11_PRIORITY_CONTENTION: u32 = 0u32;
pub const DOT11_PRIORITY_CONTENTION_FREE: u32 = 1u32;
pub const DOT11_PRIVACY_EXEMPTION_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_PROVISION_DISCOVERY_REQUEST_SEND_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_PROVISION_DISCOVERY_RESPONSE_SEND_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_PSD_IE_MAX_DATA_SIZE: u32 = 240u32;
pub const DOT11_PSD_IE_MAX_ENTRY_NUMBER: u32 = 5u32;
pub const DOT11_QOS_PARAMS_REVISION_1: u32 = 1u32;
pub const DOT11_RATE_SET_MAX_LENGTH: u32 = 126u32;
pub const DOT11_RECEIVED_GO_NEGOTIATION_CONFIRMATION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_RECEIVED_GO_NEGOTIATION_REQUEST_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_RECEIVED_GO_NEGOTIATION_RESPONSE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_RECEIVED_INVITATION_REQUEST_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_RECEIVED_INVITATION_RESPONSE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_RECEIVED_PROVISION_DISCOVERY_REQUEST_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_RECEIVED_PROVISION_DISCOVERY_RESPONSE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_RECV_CONTEXT_REVISION_1: u32 = 1u32;
pub const DOT11_REG_DOMAIN_DOC: u32 = 32u32;
pub const DOT11_REG_DOMAIN_ETSI: u32 = 48u32;
pub const DOT11_REG_DOMAIN_FCC: u32 = 16u32;
pub const DOT11_REG_DOMAIN_FRANCE: u32 = 50u32;
pub const DOT11_REG_DOMAIN_MKK: u32 = 64u32;
pub const DOT11_REG_DOMAIN_OTHER: u32 = 0u32;
pub const DOT11_REG_DOMAIN_SPAIN: u32 = 49u32;
pub const DOT11_ROAMING_COMPLETION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_ROAMING_START_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_CONTEXT_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_GO_NEGOTIATION_CONFIRMATION_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_GO_NEGOTIATION_REQUEST_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_GO_NEGOTIATION_RESPONSE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_INVITATION_REQUEST_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_INVITATION_RESPONSE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_PROVISION_DISCOVERY_REQUEST_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SEND_PROVISION_DISCOVERY_RESPONSE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_SERVICE_CLASS_REORDERABLE_MULTICAST: u32 = 0u32;
pub const DOT11_SERVICE_CLASS_STRICTLY_ORDERED: u32 = 1u32;
pub const DOT11_SSID_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_SSID_MAX_LENGTH: u32 = 32u32;
pub const DOT11_STATISTICS_REVISION_1: u32 = 1u32;
pub const DOT11_STATUS_AP_JOIN_CONFIRM: u32 = 5u32;
pub const DOT11_STATUS_AUTH_FAILED: u32 = 131072u32;
pub const DOT11_STATUS_AUTH_NOT_VERIFIED: u32 = 32768u32;
pub const DOT11_STATUS_AUTH_VERIFIED: u32 = 65536u32;
pub const DOT11_STATUS_ENCRYPTION_FAILED: u32 = 512u32;
pub const DOT11_STATUS_EXCESSIVE_DATA_LENGTH: u32 = 256u32;
pub const DOT11_STATUS_GENERATE_AUTH_FAILED: u32 = 16384u32;
pub const DOT11_STATUS_ICV_VERIFIED: u32 = 2048u32;
pub const DOT11_STATUS_JOIN_CONFIRM: u32 = 2u32;
pub const DOT11_STATUS_MPDU_MAX_LENGTH_CHANGED: u32 = 6u32;
pub const DOT11_STATUS_PACKET_NOT_REASSEMBLED: u32 = 8192u32;
pub const DOT11_STATUS_PACKET_REASSEMBLED: u32 = 4096u32;
pub const DOT11_STATUS_PS_LIFETIME_EXPIRED: u32 = 262144u32;
pub const DOT11_STATUS_RESET_CONFIRM: u32 = 4u32;
pub const DOT11_STATUS_RETRY_LIMIT_EXCEEDED: u32 = 2u32;
pub const DOT11_STATUS_SCAN_CONFIRM: u32 = 1u32;
pub const DOT11_STATUS_START_CONFIRM: u32 = 3u32;
pub const DOT11_STATUS_SUCCESS: u32 = 1u32;
pub const DOT11_STATUS_UNAVAILABLE_BSS: u32 = 128u32;
pub const DOT11_STATUS_UNAVAILABLE_PRIORITY: u32 = 16u32;
pub const DOT11_STATUS_UNAVAILABLE_SERVICE_CLASS: u32 = 32u32;
pub const DOT11_STATUS_UNSUPPORTED_PRIORITY: u32 = 4u32;
pub const DOT11_STATUS_UNSUPPORTED_SERVICE_CLASS: u32 = 8u32;
pub const DOT11_STATUS_WEP_KEY_UNAVAILABLE: u32 = 1024u32;
pub const DOT11_STATUS_XMIT_MSDU_TIMER_EXPIRED: u32 = 64u32;
pub const DOT11_STOP_AP_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_STOP_AP_REASON_AP_ACTIVE: u32 = 3u32;
pub const DOT11_STOP_AP_REASON_CHANNEL_NOT_AVAILABLE: u32 = 2u32;
pub const DOT11_STOP_AP_REASON_FREQUENCY_NOT_AVAILABLE: u32 = 1u32;
pub const DOT11_STOP_AP_REASON_IHV_END: u32 = 4294967295u32;
pub const DOT11_STOP_AP_REASON_IHV_START: u32 = 4278190080u32;
pub const DOT11_TKIPMIC_FAILURE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_VWIFI_ATTRIBUTES_REVISION_1: u32 = 1u32;
pub const DOT11_VWIFI_COMBINATION_REVISION_1: u32 = 1u32;
pub const DOT11_VWIFI_COMBINATION_REVISION_2: u32 = 2u32;
pub const DOT11_VWIFI_COMBINATION_REVISION_3: u32 = 3u32;
pub const DOT11_WFD_ADDITIONAL_IE_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_APS2_SERVICE_TYPE_MAX_LENGTH: u32 = 21u32;
pub const DOT11_WFD_ASP2_INSTANCE_NAME_MAX_LENGTH: u32 = 63u32;
pub const DOT11_WFD_ATTRIBUTES_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_DEVICE_AUTO_AVAILABILITY: u32 = 16u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_CONCURRENT_OPERATION: u32 = 4u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_CONFIG_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_P2P_CLIENT_DISCOVERABILITY: u32 = 2u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_P2P_DEVICE_LIMIT: u32 = 16u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_P2P_INFRASTRUCTURE_MANAGED: u32 = 8u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_P2P_INVITATION_PROCEDURE: u32 = 32u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_RESERVED_6: u32 = 64u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_RESERVED_7: u32 = 128u32;
pub const DOT11_WFD_DEVICE_CAPABILITY_SERVICE_DISCOVERY: u32 = 1u32;
pub const DOT11_WFD_DEVICE_HIGH_AVAILABILITY: u32 = 24u32;
pub const DOT11_WFD_DEVICE_INFO_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_DEVICE_LISTEN_CHANNEL_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_DEVICE_NOT_DISCOVERABLE: u32 = 0u32;
pub const DOT11_WFD_DISCOVER_COMPLETE_MAX_LIST_SIZE: u32 = 128u32;
pub const DOT11_WFD_DISCOVER_COMPLETE_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_DISCOVER_REQUEST_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_GROUP_CAPABILITY_CROSS_CONNECTION_SUPPORTED: u32 = 16u32;
pub const DOT11_WFD_GROUP_CAPABILITY_EAPOL_KEY_IP_ADDRESS_ALLOCATION_SUPPORTED: u32 = 128u32;
pub const DOT11_WFD_GROUP_CAPABILITY_GROUP_LIMIT_REACHED: u32 = 4u32;
pub const DOT11_WFD_GROUP_CAPABILITY_GROUP_OWNER: u32 = 1u32;
pub const DOT11_WFD_GROUP_CAPABILITY_INTRABSS_DISTRIBUTION_SUPPORTED: u32 = 8u32;
pub const DOT11_WFD_GROUP_CAPABILITY_IN_GROUP_FORMATION: u32 = 64u32;
pub const DOT11_WFD_GROUP_CAPABILITY_NONE: u32 = 0u32;
pub const DOT11_WFD_GROUP_CAPABILITY_PERSISTENT_GROUP: u32 = 2u32;
pub const DOT11_WFD_GROUP_CAPABILITY_PERSISTENT_RECONNECT_SUPPORTED: u32 = 32u32;
pub const DOT11_WFD_GROUP_CAPABILITY_RESERVED_7: u32 = 128u32;
pub const DOT11_WFD_GROUP_JOIN_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG_REVISION_2: u32 = 2u32;
pub const DOT11_WFD_GROUP_START_PARAMETERS_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_MINOR_REASON_DISASSOCIATED_FROM_WLAN_CROSS_CONNECTION_POLICY: u32 = 1u32;
pub const DOT11_WFD_MINOR_REASON_DISASSOCIATED_INFRASTRUCTURE_MANAGED_POLICY: u32 = 4u32;
pub const DOT11_WFD_MINOR_REASON_DISASSOCIATED_NOT_MANAGED_INFRASTRUCTURE_CAPABLE: u32 = 2u32;
pub const DOT11_WFD_MINOR_REASON_DISASSOCIATED_WFD_COEXISTENCE_POLICY: u32 = 3u32;
pub const DOT11_WFD_MINOR_REASON_SUCCESS: u32 = 0u32;
pub const DOT11_WFD_SECONDARY_DEVICE_TYPE_LIST_REVISION_1: u32 = 1u32;
pub const DOT11_WFD_SERVICE_INFORMATION_MAX_LENGTH: u32 = 65535u32;
pub const DOT11_WFD_SERVICE_NAME_MAX_LENGTH: u32 = 255u32;
pub const DOT11_WFD_SESSION_INFO_MAX_LENGTH: u32 = 144u32;
pub const DOT11_WFD_STATUS_FAILED_INCOMPATIBLE_PARAMETERS: u32 = 2u32;
pub const DOT11_WFD_STATUS_FAILED_INCOMPATIBLE_PROVISIONING_METHOD: u32 = 10u32;
pub const DOT11_WFD_STATUS_FAILED_INFORMATION_IS_UNAVAILABLE: u32 = 1u32;
pub const DOT11_WFD_STATUS_FAILED_INVALID_PARAMETERS: u32 = 4u32;
pub const DOT11_WFD_STATUS_FAILED_LIMIT_REACHED: u32 = 3u32;
pub const DOT11_WFD_STATUS_FAILED_MATCHING_MAX_INTENT: u32 = 9u32;
pub const DOT11_WFD_STATUS_FAILED_NO_COMMON_CHANNELS: u32 = 7u32;
pub const DOT11_WFD_STATUS_FAILED_PREVIOUS_PROTOCOL_ERROR: u32 = 6u32;
pub const DOT11_WFD_STATUS_FAILED_REJECTED_BY_USER: u32 = 11u32;
pub const DOT11_WFD_STATUS_FAILED_UNABLE_TO_ACCOMODATE_REQUEST: u32 = 5u32;
pub const DOT11_WFD_STATUS_FAILED_UNKNOWN_WFD_GROUP: u32 = 8u32;
pub const DOT11_WFD_STATUS_SUCCESS: u32 = 0u32;
pub const DOT11_WFD_STATUS_SUCCESS_ACCEPTED_BY_USER: u32 = 12u32;
pub const DOT11_WME_PACKET: u32 = 256u32;
pub const DOT11_WPS_CONFIG_METHOD_DISPLAY: DOT11_WPS_CONFIG_METHOD = 8i32;
pub const DOT11_WPS_CONFIG_METHOD_KEYPAD: DOT11_WPS_CONFIG_METHOD = 256i32;
pub const DOT11_WPS_CONFIG_METHOD_NFC_INTERFACE: DOT11_WPS_CONFIG_METHOD = 64i32;
pub const DOT11_WPS_CONFIG_METHOD_NFC_TAG: DOT11_WPS_CONFIG_METHOD = 32i32;
pub const DOT11_WPS_CONFIG_METHOD_NULL: DOT11_WPS_CONFIG_METHOD = 0i32;
pub const DOT11_WPS_CONFIG_METHOD_PUSHBUTTON: DOT11_WPS_CONFIG_METHOD = 128i32;
pub const DOT11_WPS_CONFIG_METHOD_WFDS_DEFAULT: DOT11_WPS_CONFIG_METHOD = 4096i32;
pub const DOT11_WPS_DEVICE_NAME_MAX_LENGTH: u32 = 32u32;
pub const DOT11_WPS_MAX_MODEL_NAME_LENGTH: u32 = 32u32;
pub const DOT11_WPS_MAX_MODEL_NUMBER_LENGTH: u32 = 32u32;
pub const DOT11_WPS_MAX_PASSKEY_LENGTH: u32 = 8u32;
pub const DOT11_WPS_PASSWORD_ID_DEFAULT: DOT11_WPS_DEVICE_PASSWORD_ID = 0i32;
pub const DOT11_WPS_PASSWORD_ID_MACHINE_SPECIFIED: DOT11_WPS_DEVICE_PASSWORD_ID = 2i32;
pub const DOT11_WPS_PASSWORD_ID_NFC_CONNECTION_HANDOVER: DOT11_WPS_DEVICE_PASSWORD_ID = 7i32;
pub const DOT11_WPS_PASSWORD_ID_OOB_RANGE_MAX: DOT11_WPS_DEVICE_PASSWORD_ID = 65535i32;
pub const DOT11_WPS_PASSWORD_ID_OOB_RANGE_MIN: DOT11_WPS_DEVICE_PASSWORD_ID = 16i32;
pub const DOT11_WPS_PASSWORD_ID_PUSHBUTTON: DOT11_WPS_DEVICE_PASSWORD_ID = 4i32;
pub const DOT11_WPS_PASSWORD_ID_REGISTRAR_SPECIFIED: DOT11_WPS_DEVICE_PASSWORD_ID = 5i32;
pub const DOT11_WPS_PASSWORD_ID_REKEY: DOT11_WPS_DEVICE_PASSWORD_ID = 3i32;
pub const DOT11_WPS_PASSWORD_ID_USER_SPECIFIED: DOT11_WPS_DEVICE_PASSWORD_ID = 1i32;
pub const DOT11_WPS_PASSWORD_ID_WFD_SERVICES: DOT11_WPS_DEVICE_PASSWORD_ID = 8i32;
pub const DOT11_WPS_VERSION_1_0: u32 = 1u32;
pub const DOT11_WPS_VERSION_2_0: u32 = 2u32;
pub const DevProp_PciDevice_AcsCompatibleUpHierarchy_Enhanced: DEVPROP_PCIDEVICE_ACSCOMPATIBLEUPHIERARCHY = 4u32;
pub const DevProp_PciDevice_AcsCompatibleUpHierarchy_NoP2PSupported: DEVPROP_PCIDEVICE_ACSCOMPATIBLEUPHIERARCHY = 2u32;
pub const DevProp_PciDevice_AcsCompatibleUpHierarchy_NotSupported: DEVPROP_PCIDEVICE_ACSCOMPATIBLEUPHIERARCHY = 0u32;
pub const DevProp_PciDevice_AcsCompatibleUpHierarchy_SingleFunctionSupported: DEVPROP_PCIDEVICE_ACSCOMPATIBLEUPHIERARCHY = 1u32;
pub const DevProp_PciDevice_AcsCompatibleUpHierarchy_Supported: DEVPROP_PCIDEVICE_ACSCOMPATIBLEUPHIERARCHY = 3u32;
pub const DevProp_PciDevice_AcsSupport_Missing: DEVPROP_PCIDEVICE_ACSSUPPORT = 2u32;
pub const DevProp_PciDevice_AcsSupport_NotNeeded: DEVPROP_PCIDEVICE_ACSSUPPORT = 1u32;
pub const DevProp_PciDevice_AcsSupport_Present: DEVPROP_PCIDEVICE_ACSSUPPORT = 0u32;
pub const DevProp_PciDevice_BridgeType_PciConventional: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 6u32;
pub const DevProp_PciDevice_BridgeType_PciExpressDownstreamSwitchPort: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 10u32;
pub const DevProp_PciDevice_BridgeType_PciExpressEventCollector: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 14u32;
pub const DevProp_PciDevice_BridgeType_PciExpressRootPort: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 8u32;
pub const DevProp_PciDevice_BridgeType_PciExpressToPciXBridge: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 11u32;
pub const DevProp_PciDevice_BridgeType_PciExpressTreatedAsPci: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 13u32;
pub const DevProp_PciDevice_BridgeType_PciExpressUpstreamSwitchPort: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 9u32;
pub const DevProp_PciDevice_BridgeType_PciX: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 7u32;
pub const DevProp_PciDevice_BridgeType_PciXToExpressBridge: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 12u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode1_100Mhz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 2u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode1_133MHZ: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 3u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode1_66Mhz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 1u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode1_ECC_100Mhz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 6u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode1_ECC_133Mhz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 7u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode1_ECC_66Mhz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 5u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode2_266_100MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 10u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode2_266_133MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 11u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode2_266_66MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 9u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode2_533_100MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 14u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode2_533_133MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 15u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode2_533_66MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 13u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_PciX_Mode_Conventional_Pci: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 0u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_Pci_Conventional_33MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 0u32;
pub const DevProp_PciDevice_CurrentSpeedAndMode_Pci_Conventional_66MHz: DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = 1u32;
pub const DevProp_PciDevice_DeviceType_PciConventional: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 0u32;
pub const DevProp_PciDevice_DeviceType_PciExpressEndpoint: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 2u32;
pub const DevProp_PciDevice_DeviceType_PciExpressLegacyEndpoint: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 3u32;
pub const DevProp_PciDevice_DeviceType_PciExpressRootComplexIntegratedEndpoint: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 4u32;
pub const DevProp_PciDevice_DeviceType_PciExpressTreatedAsPci: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 5u32;
pub const DevProp_PciDevice_DeviceType_PciX: DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = 1u32;
pub const DevProp_PciDevice_InterruptType_LineBased: DEVPROP_PCIDEVICE_INTERRUPTTYPE = 1u32;
pub const DevProp_PciDevice_InterruptType_Msi: DEVPROP_PCIDEVICE_INTERRUPTTYPE = 2u32;
pub const DevProp_PciDevice_InterruptType_MsiX: DEVPROP_PCIDEVICE_INTERRUPTTYPE = 4u32;
pub const DevProp_PciDevice_SriovSupport_DidntGetVfBarSpace: DEVPROP_PCIDEVICE_SRIOVSUPPORT = 4u32;
pub const DevProp_PciDevice_SriovSupport_MissingAcs: DEVPROP_PCIDEVICE_SRIOVSUPPORT = 1u32;
pub const DevProp_PciDevice_SriovSupport_MissingPfDriver: DEVPROP_PCIDEVICE_SRIOVSUPPORT = 2u32;
pub const DevProp_PciDevice_SriovSupport_NoBusResource: DEVPROP_PCIDEVICE_SRIOVSUPPORT = 3u32;
pub const DevProp_PciDevice_SriovSupport_Ok: DEVPROP_PCIDEVICE_SRIOVSUPPORT = 0u32;
pub const DevProp_PciExpressDevice_LinkSpeed_Five_Gbps: DEVPROP_PCIEXPRESSDEVICE_LINKSPEED = 2u32;
pub const DevProp_PciExpressDevice_LinkSpeed_TwoAndHalf_Gbps: DEVPROP_PCIEXPRESSDEVICE_LINKSPEED = 1u32;
pub const DevProp_PciExpressDevice_LinkWidth_By_1: DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = 1u32;
pub const DevProp_PciExpressDevice_LinkWidth_By_12: DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = 12u32;
pub const DevProp_PciExpressDevice_LinkWidth_By_16: DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = 16u32;
pub const DevProp_PciExpressDevice_LinkWidth_By_2: DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = 2u32;
pub const DevProp_PciExpressDevice_LinkWidth_By_32: DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = 32u32;
pub const DevProp_PciExpressDevice_LinkWidth_By_4: DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = 4u32;
pub const DevProp_PciExpressDevice_LinkWidth_By_8: DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = 8u32;
pub const DevProp_PciExpressDevice_PayloadOrRequestSize_1024Bytes: DEVPROP_PCIEXPRESSDEVICE_PAYLOADORREQUESTSIZE = 3u32;
pub const DevProp_PciExpressDevice_PayloadOrRequestSize_128Bytes: DEVPROP_PCIEXPRESSDEVICE_PAYLOADORREQUESTSIZE = 0u32;
pub const DevProp_PciExpressDevice_PayloadOrRequestSize_2048Bytes: DEVPROP_PCIEXPRESSDEVICE_PAYLOADORREQUESTSIZE = 4u32;
pub const DevProp_PciExpressDevice_PayloadOrRequestSize_256Bytes: DEVPROP_PCIEXPRESSDEVICE_PAYLOADORREQUESTSIZE = 1u32;
pub const DevProp_PciExpressDevice_PayloadOrRequestSize_4096Bytes: DEVPROP_PCIEXPRESSDEVICE_PAYLOADORREQUESTSIZE = 5u32;
pub const DevProp_PciExpressDevice_PayloadOrRequestSize_512Bytes: DEVPROP_PCIEXPRESSDEVICE_PAYLOADORREQUESTSIZE = 2u32;
pub const DevProp_PciExpressDevice_Spec_Version_10: DEVPROP_PCIEXPRESSDEVICE_SPEC_VERSION = 1u32;
pub const DevProp_PciExpressDevice_Spec_Version_11: DEVPROP_PCIEXPRESSDEVICE_SPEC_VERSION = 2u32;
pub const DevProp_PciRootBus_BusWidth_32Bits: DEVPROP_PCIROOTBUS_BUSWIDTH = 0u32;
pub const DevProp_PciRootBus_BusWidth_64Bits: DEVPROP_PCIROOTBUS_BUSWIDTH = 1u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_Conventional_33Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 0u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_Conventional_66Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 1u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_266_Mode2_100Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 9u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_266_Mode2_133Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 10u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_266_Mode2_66Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 8u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_533_Mode2_100Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 12u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_533_Mode2_133Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 13u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_533_Mode2_66Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 11u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_Mode1_100Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 3u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_Mode1_133Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 4u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_Mode1_66Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 2u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_Mode1_ECC_100Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 6u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_Mode1_ECC_133Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 7u32;
pub const DevProp_PciRootBus_CurrentSpeedAndMode_Pci_X_Mode1_ECC_66Mhz: DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = 5u32;
pub const DevProp_PciRootBus_SecondaryInterface_PciConventional: DEVPROP_PCIROOTBUS_SECONDARYINTERFACE = 0u32;
pub const DevProp_PciRootBus_SecondaryInterface_PciExpress: DEVPROP_PCIROOTBUS_SECONDARYINTERFACE = 3u32;
pub const DevProp_PciRootBus_SecondaryInterface_PciXMode1: DEVPROP_PCIROOTBUS_SECONDARYINTERFACE = 1u32;
pub const DevProp_PciRootBus_SecondaryInterface_PciXMode2: DEVPROP_PCIROOTBUS_SECONDARYINTERFACE = 2u32;
pub const DevProp_PciRootBus_SupportedSpeedsAndModes_Pci_Conventional_33Mhz: DEVPROP_PCIROOTBUS_SUPPORTEDSPEEDSANDMODES = 1u32;
pub const DevProp_PciRootBus_SupportedSpeedsAndModes_Pci_Conventional_66Mhz: DEVPROP_PCIROOTBUS_SUPPORTEDSPEEDSANDMODES = 2u32;
pub const DevProp_PciRootBus_SupportedSpeedsAndModes_Pci_X_133Mhz: DEVPROP_PCIROOTBUS_SUPPORTEDSPEEDSANDMODES = 8u32;
pub const DevProp_PciRootBus_SupportedSpeedsAndModes_Pci_X_266Mhz: DEVPROP_PCIROOTBUS_SUPPORTEDSPEEDSANDMODES = 16u32;
pub const DevProp_PciRootBus_SupportedSpeedsAndModes_Pci_X_533Mhz: DEVPROP_PCIROOTBUS_SUPPORTEDSPEEDSANDMODES = 32u32;
pub const DevProp_PciRootBus_SupportedSpeedsAndModes_Pci_X_66Mhz: DEVPROP_PCIROOTBUS_SUPPORTEDSPEEDSANDMODES = 4u32;
pub const Dot11AdHocManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdd06a84f_83bd_4d01_8ab9_2389fea0869e);
pub const GUID_AEPSERVICE_WIFIDIRECT_DEVICE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcc29827c_9caf_4928_99a9_18f7c2381389);
pub const GUID_DEVINTERFACE_ASP_INFRA_DEVICE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xff823995_7a72_4c80_8757_c67ee13d1a49);
pub const GUID_DEVINTERFACE_WIFIDIRECT_DEVICE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x439b20af_8955_405b_99f0_a62af0c68d43);
pub const IHV_INIT_FUNCTION_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Dot11ExtIhvInitService");
pub const IHV_INIT_VS_FUNCTION_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Dot11ExtIhvInitVirtualStation");
pub const IHV_VERSION_FUNCTION_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Dot11ExtIhvGetVersionInfo");
pub const IndicationTypeLinkQuality: DOT11EXT_IHV_INDICATION_TYPE = 4i32;
pub const IndicationTypeNicSpecificNotification: DOT11EXT_IHV_INDICATION_TYPE = 0i32;
pub const IndicationTypePhyStateChange: DOT11EXT_IHV_INDICATION_TYPE = 3i32;
pub const IndicationTypePmkidCandidateList: DOT11EXT_IHV_INDICATION_TYPE = 1i32;
pub const IndicationTypeTkipMicFailure: DOT11EXT_IHV_INDICATION_TYPE = 2i32;
pub const L2_NOTIFICATION_CODE_GROUP_SIZE: u32 = 4096u32;
pub const L2_NOTIFICATION_CODE_PUBLIC_BEGIN: u32 = 0u32;
pub const L2_NOTIFICATION_SOURCE_ALL: u32 = 65535u32;
pub const L2_NOTIFICATION_SOURCE_DOT3_AUTO_CONFIG: u32 = 1u32;
pub const L2_NOTIFICATION_SOURCE_NONE: u32 = 0u32;
pub const L2_NOTIFICATION_SOURCE_ONEX: u32 = 4u32;
pub const L2_NOTIFICATION_SOURCE_SECURITY: u32 = 2u32;
pub const L2_NOTIFICATION_SOURCE_WCM: u32 = 256u32;
pub const L2_NOTIFICATION_SOURCE_WCM_CSP: u32 = 512u32;
pub const L2_NOTIFICATION_SOURCE_WFD: u32 = 1024u32;
pub const L2_NOTIFICATION_SOURCE_WLAN_ACM: u32 = 8u32;
pub const L2_NOTIFICATION_SOURCE_WLAN_DEVICE_SERVICE: u32 = 2048u32;
pub const L2_NOTIFICATION_SOURCE_WLAN_HNWK: u32 = 128u32;
pub const L2_NOTIFICATION_SOURCE_WLAN_IHV: u32 = 64u32;
pub const L2_NOTIFICATION_SOURCE_WLAN_MSM: u32 = 16u32;
pub const L2_NOTIFICATION_SOURCE_WLAN_SECURITY: u32 = 32u32;
pub const L2_PROFILE_MAX_NAME_LENGTH: u32 = 256u32;
pub const L2_REASON_CODE_DOT11_AC_BASE: u32 = 131072u32;
pub const L2_REASON_CODE_DOT11_MSM_BASE: u32 = 196608u32;
pub const L2_REASON_CODE_DOT11_SECURITY_BASE: u32 = 262144u32;
pub const L2_REASON_CODE_DOT3_AC_BASE: u32 = 393216u32;
pub const L2_REASON_CODE_DOT3_MSM_BASE: u32 = 458752u32;
pub const L2_REASON_CODE_GEN_BASE: u32 = 65536u32;
pub const L2_REASON_CODE_GROUP_SIZE: u32 = 65536u32;
pub const L2_REASON_CODE_IHV_BASE: u32 = 589824u32;
pub const L2_REASON_CODE_ONEX_BASE: u32 = 327680u32;
pub const L2_REASON_CODE_PROFILE_BASE: u32 = 524288u32;
pub const L2_REASON_CODE_PROFILE_MISSING: u32 = 1u32;
pub const L2_REASON_CODE_RESERVED_BASE: u32 = 720896u32;
pub const L2_REASON_CODE_SUCCESS: u32 = 0u32;
pub const L2_REASON_CODE_UNKNOWN: u32 = 65537u32;
pub const L2_REASON_CODE_WIMAX_BASE: u32 = 655360u32;
pub const MAX_NUM_SUPPORTED_RATES: u32 = 8u32;
pub const MAX_NUM_SUPPORTED_RATES_V2: u32 = 255u32;
pub const MS_MAX_PROFILE_NAME_LENGTH: u32 = 256u32;
pub const MS_PROFILE_GROUP_POLICY: u32 = 1u32;
pub const MS_PROFILE_USER: u32 = 2u32;
pub const NDIS_PACKET_TYPE_802_11_ALL_MULTICAST_DATA: u32 = 4u32;
pub const NDIS_PACKET_TYPE_802_11_BROADCAST_DATA: u32 = 8u32;
pub const NDIS_PACKET_TYPE_802_11_DIRECTED_DATA: u32 = 1u32;
pub const NDIS_PACKET_TYPE_802_11_MULTICAST_DATA: u32 = 2u32;
pub const NDIS_PACKET_TYPE_802_11_PROMISCUOUS_DATA: u32 = 32u32;
pub const OID_DOT11_AP_JOIN_REQUEST: u32 = 218170205u32;
pub const OID_DOT11_ATIM_WINDOW: u32 = 218170122u32;
pub const OID_DOT11_BEACON_PERIOD: u32 = 218170139u32;
pub const OID_DOT11_CCA_MODE_SUPPORTED: u32 = 218170166u32;
pub const OID_DOT11_CCA_WATCHDOG_COUNT_MAX: u32 = 218170170u32;
pub const OID_DOT11_CCA_WATCHDOG_COUNT_MIN: u32 = 218170172u32;
pub const OID_DOT11_CCA_WATCHDOG_TIMER_MAX: u32 = 218170169u32;
pub const OID_DOT11_CCA_WATCHDOG_TIMER_MIN: u32 = 218170171u32;
pub const OID_DOT11_CFP_MAX_DURATION: u32 = 218170136u32;
pub const OID_DOT11_CFP_PERIOD: u32 = 218170135u32;
pub const OID_DOT11_CF_POLLABLE: u32 = 218170134u32;
pub const OID_DOT11_CHANNEL_AGILITY_ENABLED: u32 = 218170184u32;
pub const OID_DOT11_CHANNEL_AGILITY_PRESENT: u32 = 218170183u32;
pub const OID_DOT11_COUNTERS_ENTRY: u32 = 218170149u32;
pub const OID_DOT11_COUNTRY_STRING: u32 = 218170188u32;
pub const OID_DOT11_CURRENT_ADDRESS: u32 = 218171138u32;
pub const OID_DOT11_CURRENT_CCA_MODE: u32 = 218170167u32;
pub const OID_DOT11_CURRENT_CHANNEL: u32 = 218170165u32;
pub const OID_DOT11_CURRENT_CHANNEL_NUMBER: u32 = 218170159u32;
pub const OID_DOT11_CURRENT_DWELL_TIME: u32 = 218170161u32;
pub const OID_DOT11_CURRENT_FREQUENCY: u32 = 218170178u32;
pub const OID_DOT11_CURRENT_INDEX: u32 = 218170164u32;
pub const OID_DOT11_CURRENT_OFFLOAD_CAPABILITY: u32 = 218170113u32;
pub const OID_DOT11_CURRENT_OPERATION_MODE: u32 = 218170120u32;
pub const OID_DOT11_CURRENT_OPTIONAL_CAPABILITY: u32 = 218170131u32;
pub const OID_DOT11_CURRENT_PACKET_FILTER: u32 = 218170121u32;
pub const OID_DOT11_CURRENT_PATTERN: u32 = 218170163u32;
pub const OID_DOT11_CURRENT_PHY_TYPE: u32 = 218170124u32;
pub const OID_DOT11_CURRENT_REG_DOMAIN: u32 = 218170151u32;
pub const OID_DOT11_CURRENT_RX_ANTENNA: u32 = 218170155u32;
pub const OID_DOT11_CURRENT_SET: u32 = 218170162u32;
pub const OID_DOT11_CURRENT_TX_ANTENNA: u32 = 218170153u32;
pub const OID_DOT11_CURRENT_TX_POWER_LEVEL: u32 = 218170157u32;
pub const OID_DOT11_DEFAULT_WEP_OFFLOAD: u32 = 218170116u32;
pub const OID_DOT11_DEFAULT_WEP_UPLOAD: u32 = 218170117u32;
pub const OID_DOT11_DIVERSITY_SELECTION_RX: u32 = 218170176u32;
pub const OID_DOT11_DIVERSITY_SUPPORT: u32 = 218170154u32;
pub const OID_DOT11_DSSS_OFDM_OPTION_ENABLED: u32 = 218170209u32;
pub const OID_DOT11_DSSS_OFDM_OPTION_IMPLEMENTED: u32 = 218170208u32;
pub const OID_DOT11_DTIM_PERIOD: u32 = 218170140u32;
pub const OID_DOT11_ED_THRESHOLD: u32 = 218170168u32;
pub const OID_DOT11_EHCC_CAPABILITY_ENABLED: u32 = 218170193u32;
pub const OID_DOT11_EHCC_CAPABILITY_IMPLEMENTED: u32 = 218170192u32;
pub const OID_DOT11_EHCC_NUMBER_OF_CHANNELS_FAMILY_INDEX: u32 = 218170191u32;
pub const OID_DOT11_EHCC_PRIME_RADIX: u32 = 218170190u32;
pub const OID_DOT11_ERP_PBCC_OPTION_ENABLED: u32 = 218170207u32;
pub const OID_DOT11_ERP_PBCC_OPTION_IMPLEMENTED: u32 = 218170206u32;
pub const OID_DOT11_FRAGMENTATION_THRESHOLD: u32 = 218170146u32;
pub const OID_DOT11_FREQUENCY_BANDS_SUPPORTED: u32 = 218170180u32;
pub const OID_DOT11_HOPPING_PATTERN: u32 = 218170199u32;
pub const OID_DOT11_HOP_ALGORITHM_ADOPTED: u32 = 218170194u32;
pub const OID_DOT11_HOP_MODULUS: u32 = 218170197u32;
pub const OID_DOT11_HOP_OFFSET: u32 = 218170198u32;
pub const OID_DOT11_HOP_TIME: u32 = 218170158u32;
pub const OID_DOT11_HR_CCA_MODE_SUPPORTED: u32 = 218170185u32;
pub const OID_DOT11_JOIN_REQUEST: u32 = 218170125u32;
pub const OID_DOT11_LONG_RETRY_LIMIT: u32 = 218170145u32;
pub const OID_DOT11_MAC_ADDRESS: u32 = 218170142u32;
pub const OID_DOT11_MAXIMUM_LIST_SIZE: u32 = 218171141u32;
pub const OID_DOT11_MAX_DWELL_TIME: u32 = 218170160u32;
pub const OID_DOT11_MAX_MAC_ADDRESS_STATES: u32 = 218170212u32;
pub const OID_DOT11_MAX_RECEIVE_LIFETIME: u32 = 218170148u32;
pub const OID_DOT11_MAX_TRANSMIT_MSDU_LIFETIME: u32 = 218170147u32;
pub const OID_DOT11_MEDIUM_OCCUPANCY_LIMIT: u32 = 218170133u32;
pub const OID_DOT11_MPDU_MAX_LENGTH: u32 = 218170118u32;
pub const OID_DOT11_MULTICAST_LIST: u32 = 218171140u32;
pub const OID_DOT11_MULTI_DOMAIN_CAPABILITY: u32 = 218170189u32;
pub const OID_DOT11_MULTI_DOMAIN_CAPABILITY_ENABLED: u32 = 218170187u32;
pub const OID_DOT11_MULTI_DOMAIN_CAPABILITY_IMPLEMENTED: u32 = 218170186u32;
pub const OID_DOT11_NDIS_START: u32 = 218170112u32;
pub const OID_DOT11_NIC_POWER_STATE: u32 = 218170129u32;
pub const OID_DOT11_NIC_SPECIFIC_EXTENSION: u32 = 218170204u32;
pub const OID_DOT11_NUMBER_OF_HOPPING_SETS: u32 = 218170196u32;
pub const OID_DOT11_OFFLOAD_CAPABILITY: u32 = 218170112u32;
pub const OID_DOT11_OPERATIONAL_RATE_SET: u32 = 218170138u32;
pub const OID_DOT11_OPERATION_MODE_CAPABILITY: u32 = 218170119u32;
pub const OID_DOT11_OPTIONAL_CAPABILITY: u32 = 218170130u32;
pub const OID_DOT11_PBCC_OPTION_IMPLEMENTED: u32 = 218170182u32;
pub const OID_DOT11_PERMANENT_ADDRESS: u32 = 218171139u32;
pub const OID_DOT11_POWER_MGMT_MODE: u32 = 218170137u32;
pub const OID_DOT11_PRIVATE_OIDS_START: u32 = 218171136u32;
pub const OID_DOT11_QOS_TX_DURATION: u32 = 218170219u32;
pub const OID_DOT11_QOS_TX_MEDIUM_TIME: u32 = 218170220u32;
pub const OID_DOT11_QOS_TX_QUEUES_SUPPORTED: u32 = 218170218u32;
pub const OID_DOT11_RANDOM_TABLE_FIELD_NUMBER: u32 = 218170200u32;
pub const OID_DOT11_RANDOM_TABLE_FLAG: u32 = 218170195u32;
pub const OID_DOT11_RECV_SENSITIVITY_LIST: u32 = 218170213u32;
pub const OID_DOT11_REG_DOMAINS_SUPPORT_VALUE: u32 = 218170173u32;
pub const OID_DOT11_RESET_REQUEST: u32 = 218170128u32;
pub const OID_DOT11_RF_USAGE: u32 = 218170203u32;
pub const OID_DOT11_RSSI_RANGE: u32 = 218170202u32;
pub const OID_DOT11_RTS_THRESHOLD: u32 = 218170143u32;
pub const OID_DOT11_SCAN_REQUEST: u32 = 218170123u32;
pub const OID_DOT11_SHORT_PREAMBLE_OPTION_IMPLEMENTED: u32 = 218170181u32;
pub const OID_DOT11_SHORT_RETRY_LIMIT: u32 = 218170144u32;
pub const OID_DOT11_SHORT_SLOT_TIME_OPTION_ENABLED: u32 = 218170211u32;
pub const OID_DOT11_SHORT_SLOT_TIME_OPTION_IMPLEMENTED: u32 = 218170210u32;
pub const OID_DOT11_START_REQUEST: u32 = 218170126u32;
pub const OID_DOT11_STATION_ID: u32 = 218170132u32;
pub const OID_DOT11_SUPPORTED_DATA_RATES_VALUE: u32 = 218170177u32;
pub const OID_DOT11_SUPPORTED_DSSS_CHANNEL_LIST: u32 = 218170222u32;
pub const OID_DOT11_SUPPORTED_OFDM_FREQUENCY_LIST: u32 = 218170221u32;
pub const OID_DOT11_SUPPORTED_PHY_TYPES: u32 = 218170150u32;
pub const OID_DOT11_SUPPORTED_POWER_LEVELS: u32 = 218170156u32;
pub const OID_DOT11_SUPPORTED_RX_ANTENNA: u32 = 218170175u32;
pub const OID_DOT11_SUPPORTED_TX_ANTENNA: u32 = 218170174u32;
pub const OID_DOT11_TEMP_TYPE: u32 = 218170152u32;
pub const OID_DOT11_TI_THRESHOLD: u32 = 218170179u32;
pub const OID_DOT11_UPDATE_IE: u32 = 218170127u32;
pub const OID_DOT11_WEP_ICV_ERROR_COUNT: u32 = 218170141u32;
pub const OID_DOT11_WEP_OFFLOAD: u32 = 218170114u32;
pub const OID_DOT11_WEP_UPLOAD: u32 = 218170115u32;
pub const OID_DOT11_WME_AC_PARAMETERS: u32 = 218170216u32;
pub const OID_DOT11_WME_ENABLED: u32 = 218170215u32;
pub const OID_DOT11_WME_IMPLEMENTED: u32 = 218170214u32;
pub const OID_DOT11_WME_UPDATE_IE: u32 = 218170217u32;
pub const OID_DOT11_WPA_TSC: u32 = 218170201u32;
pub const ONEX_AUTHENTICATOR_NO_LONGER_PRESENT: ONEX_REASON_CODE = 327686i32;
pub const ONEX_EAP_FAILURE_RECEIVED: ONEX_REASON_CODE = 327685i32;
pub const ONEX_IDENTITY_NOT_FOUND: ONEX_REASON_CODE = 327682i32;
pub const ONEX_NO_RESPONSE_TO_IDENTITY: ONEX_REASON_CODE = 327687i32;
pub const ONEX_PROFILE_DISALLOWED_EAP_TYPE: ONEX_REASON_CODE = 327690i32;
pub const ONEX_PROFILE_EXPIRED_EXPLICIT_CREDENTIALS: ONEX_REASON_CODE = 327699i32;
pub const ONEX_PROFILE_INVALID_AUTH_MODE: ONEX_REASON_CODE = 327695i32;
pub const ONEX_PROFILE_INVALID_EAP_CONNECTION_PROPERTIES: ONEX_REASON_CODE = 327696i32;
pub const ONEX_PROFILE_INVALID_EAP_TYPE_OR_FLAG: ONEX_REASON_CODE = 327691i32;
pub const ONEX_PROFILE_INVALID_EXPLICIT_CREDENTIALS: ONEX_REASON_CODE = 327698i32;
pub const ONEX_PROFILE_INVALID_LENGTH: ONEX_REASON_CODE = 327689i32;
pub const ONEX_PROFILE_INVALID_ONEX_FLAGS: ONEX_REASON_CODE = 327692i32;
pub const ONEX_PROFILE_INVALID_SUPPLICANT_MODE: ONEX_REASON_CODE = 327694i32;
pub const ONEX_PROFILE_INVALID_TIMER_VALUE: ONEX_REASON_CODE = 327693i32;
pub const ONEX_PROFILE_VERSION_NOT_SUPPORTED: ONEX_REASON_CODE = 327688i32;
pub const ONEX_REASON_CODE_SUCCESS: ONEX_REASON_CODE = 0i32;
pub const ONEX_REASON_START: ONEX_REASON_CODE = 327680i32;
pub const ONEX_UI_CANCELLED: ONEX_REASON_CODE = 327697i32;
pub const ONEX_UI_DISABLED: ONEX_REASON_CODE = 327683i32;
pub const ONEX_UI_FAILURE: ONEX_REASON_CODE = 327684i32;
pub const ONEX_UI_NOT_PERMITTED: ONEX_REASON_CODE = 327700i32;
pub const ONEX_UNABLE_TO_IDENTIFY_USER: ONEX_REASON_CODE = 327681i32;
pub const OneXAuthFailure: ONEX_AUTH_STATUS = 4i32;
pub const OneXAuthIdentityExplicitUser: ONEX_AUTH_IDENTITY = 3i32;
pub const OneXAuthIdentityGuest: ONEX_AUTH_IDENTITY = 4i32;
pub const OneXAuthIdentityInvalid: ONEX_AUTH_IDENTITY = 5i32;
pub const OneXAuthIdentityMachine: ONEX_AUTH_IDENTITY = 1i32;
pub const OneXAuthIdentityNone: ONEX_AUTH_IDENTITY = 0i32;
pub const OneXAuthIdentityUser: ONEX_AUTH_IDENTITY = 2i32;
pub const OneXAuthInProgress: ONEX_AUTH_STATUS = 1i32;
pub const OneXAuthInvalid: ONEX_AUTH_STATUS = 5i32;
pub const OneXAuthNoAuthenticatorFound: ONEX_AUTH_STATUS = 2i32;
pub const OneXAuthNotStarted: ONEX_AUTH_STATUS = 0i32;
pub const OneXAuthSuccess: ONEX_AUTH_STATUS = 3i32;
pub const OneXEapMethodBackendSupportUnknown: ONEX_EAP_METHOD_BACKEND_SUPPORT = 0i32;
pub const OneXEapMethodBackendSupported: ONEX_EAP_METHOD_BACKEND_SUPPORT = 1i32;
pub const OneXEapMethodBackendUnsupported: ONEX_EAP_METHOD_BACKEND_SUPPORT = 2i32;
pub const OneXNotificationTypeAuthRestarted: ONEX_NOTIFICATION_TYPE = 2i32;
pub const OneXNotificationTypeEventInvalid: ONEX_NOTIFICATION_TYPE = 3i32;
pub const OneXNotificationTypeResultUpdate: ONEX_NOTIFICATION_TYPE = 1i32;
pub const OneXNumNotifications: ONEX_NOTIFICATION_TYPE = 3i32;
pub const OneXPublicNotificationBase: ONEX_NOTIFICATION_TYPE = 0i32;
pub const OneXRestartReasonAltCredsTrial: ONEX_AUTH_RESTART_REASON = 7i32;
pub const OneXRestartReasonInvalid: ONEX_AUTH_RESTART_REASON = 8i32;
pub const OneXRestartReasonMsmInitiated: ONEX_AUTH_RESTART_REASON = 1i32;
pub const OneXRestartReasonOneXAuthTimeout: ONEX_AUTH_RESTART_REASON = 3i32;
pub const OneXRestartReasonOneXConfigurationChanged: ONEX_AUTH_RESTART_REASON = 4i32;
pub const OneXRestartReasonOneXHeldStateTimeout: ONEX_AUTH_RESTART_REASON = 2i32;
pub const OneXRestartReasonOneXUserChanged: ONEX_AUTH_RESTART_REASON = 5i32;
pub const OneXRestartReasonPeerInitiated: ONEX_AUTH_RESTART_REASON = 0i32;
pub const OneXRestartReasonQuarantineStateChanged: ONEX_AUTH_RESTART_REASON = 6i32;
pub const WDIAG_IHV_WLAN_ID_FLAG_SECURITY_ENABLED: u32 = 1u32;
pub const WFDSVC_CONNECTION_CAPABILITY_CLIENT: u32 = 2u32;
pub const WFDSVC_CONNECTION_CAPABILITY_GO: u32 = 4u32;
pub const WFDSVC_CONNECTION_CAPABILITY_NEW: u32 = 1u32;
pub const WFD_API_VERSION: u32 = 1u32;
pub const WFD_API_VERSION_1_0: u32 = 1u32;
pub const WFD_ROLE_TYPE_CLIENT: WFD_ROLE_TYPE = 4i32;
pub const WFD_ROLE_TYPE_DEVICE: WFD_ROLE_TYPE = 1i32;
pub const WFD_ROLE_TYPE_GROUP_OWNER: WFD_ROLE_TYPE = 2i32;
pub const WFD_ROLE_TYPE_MAX: WFD_ROLE_TYPE = 5i32;
pub const WFD_ROLE_TYPE_NONE: WFD_ROLE_TYPE = 0i32;
pub const WLAN_API_VERSION: u32 = 2u32;
pub const WLAN_API_VERSION_1_0: u32 = 1u32;
pub const WLAN_API_VERSION_2_0: u32 = 2u32;
pub const WLAN_AVAILABLE_NETWORK_ANQP_SUPPORTED: u32 = 32u32;
pub const WLAN_AVAILABLE_NETWORK_AUTO_CONNECT_FAILED: u32 = 256u32;
pub const WLAN_AVAILABLE_NETWORK_CONNECTED: u32 = 1u32;
pub const WLAN_AVAILABLE_NETWORK_CONSOLE_USER_PROFILE: u32 = 4u32;
pub const WLAN_AVAILABLE_NETWORK_HAS_PROFILE: u32 = 2u32;
pub const WLAN_AVAILABLE_NETWORK_HOTSPOT2_DOMAIN: u32 = 64u32;
pub const WLAN_AVAILABLE_NETWORK_HOTSPOT2_ENABLED: u32 = 16u32;
pub const WLAN_AVAILABLE_NETWORK_HOTSPOT2_ROAMING: u32 = 128u32;
pub const WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_ADHOC_PROFILES: u32 = 1u32;
pub const WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_MANUAL_HIDDEN_PROFILES: u32 = 2u32;
pub const WLAN_AVAILABLE_NETWORK_INTERWORKING_SUPPORTED: u32 = 8u32;
pub const WLAN_CONNECTION_ADHOC_JOIN_ONLY: u32 = 2u32;
pub const WLAN_CONNECTION_EAPOL_PASSTHROUGH: u32 = 8u32;
pub const WLAN_CONNECTION_HIDDEN_NETWORK: u32 = 1u32;
pub const WLAN_CONNECTION_IGNORE_PRIVACY_BIT: u32 = 4u32;
pub const WLAN_CONNECTION_NOTIFICATION_ADHOC_NETWORK_FORMED: WLAN_CONNECTION_NOTIFICATION_FLAGS = 1u32;
pub const WLAN_CONNECTION_NOTIFICATION_CONSOLE_USER_PROFILE: WLAN_CONNECTION_NOTIFICATION_FLAGS = 4u32;
pub const WLAN_CONNECTION_PERSIST_DISCOVERY_PROFILE: u32 = 16u32;
pub const WLAN_CONNECTION_PERSIST_DISCOVERY_PROFILE_CONNECTION_MODE_AUTO: u32 = 32u32;
pub const WLAN_CONNECTION_PERSIST_DISCOVERY_PROFILE_OVERWRITE_EXISTING: u32 = 64u32;
pub const WLAN_MAX_NAME_LENGTH: u32 = 256u32;
pub const WLAN_MAX_PHY_INDEX: u32 = 64u32;
pub const WLAN_MAX_PHY_TYPE_NUMBER: u32 = 8u32;
pub const WLAN_NOTIFICATION_SOURCE_ACM: WLAN_NOTIFICATION_SOURCES = 8u32;
pub const WLAN_NOTIFICATION_SOURCE_ALL: WLAN_NOTIFICATION_SOURCES = 65535u32;
pub const WLAN_NOTIFICATION_SOURCE_DEVICE_SERVICE: WLAN_NOTIFICATION_SOURCES = 2048u32;
pub const WLAN_NOTIFICATION_SOURCE_HNWK: WLAN_NOTIFICATION_SOURCES = 128u32;
pub const WLAN_NOTIFICATION_SOURCE_IHV: WLAN_NOTIFICATION_SOURCES = 64u32;
pub const WLAN_NOTIFICATION_SOURCE_MSM: WLAN_NOTIFICATION_SOURCES = 16u32;
pub const WLAN_NOTIFICATION_SOURCE_NONE: WLAN_NOTIFICATION_SOURCES = 0u32;
pub const WLAN_NOTIFICATION_SOURCE_ONEX: WLAN_NOTIFICATION_SOURCES = 4u32;
pub const WLAN_NOTIFICATION_SOURCE_SECURITY: WLAN_NOTIFICATION_SOURCES = 32u32;
pub const WLAN_PROFILE_CONNECTION_MODE_AUTO: u32 = 131072u32;
pub const WLAN_PROFILE_CONNECTION_MODE_SET_BY_CLIENT: u32 = 65536u32;
pub const WLAN_PROFILE_GET_PLAINTEXT_KEY: u32 = 4u32;
pub const WLAN_PROFILE_GROUP_POLICY: u32 = 1u32;
pub const WLAN_PROFILE_USER: u32 = 2u32;
pub const WLAN_REASON_CODE_AC_BASE: u32 = 131072u32;
pub const WLAN_REASON_CODE_AC_CONNECT_BASE: u32 = 163840u32;
pub const WLAN_REASON_CODE_AC_END: u32 = 196607u32;
pub const WLAN_REASON_CODE_ADHOC_SECURITY_FAILURE: u32 = 229386u32;
pub const WLAN_REASON_CODE_AP_PROFILE_NOT_ALLOWED: u32 = 163856u32;
pub const WLAN_REASON_CODE_AP_PROFILE_NOT_ALLOWED_FOR_CLIENT: u32 = 163855u32;
pub const WLAN_REASON_CODE_AP_STARTING_FAILURE: u32 = 229395u32;
pub const WLAN_REASON_CODE_ASSOCIATION_FAILURE: u32 = 229378u32;
pub const WLAN_REASON_CODE_ASSOCIATION_TIMEOUT: u32 = 229379u32;
pub const WLAN_REASON_CODE_AUTO_AP_PROFILE_NOT_ALLOWED: u32 = 524313u32;
pub const WLAN_REASON_CODE_AUTO_CONNECTION_NOT_ALLOWED: u32 = 524314u32;
pub const WLAN_REASON_CODE_AUTO_SWITCH_SET_FOR_ADHOC: u32 = 524304u32;
pub const WLAN_REASON_CODE_AUTO_SWITCH_SET_FOR_MANUAL_CONNECTION: u32 = 524305u32;
pub const WLAN_REASON_CODE_BAD_MAX_NUMBER_OF_CLIENTS_FOR_AP: u32 = 524310u32;
pub const WLAN_REASON_CODE_BASE: u32 = 131072u32;
pub const WLAN_REASON_CODE_BSS_TYPE_NOT_ALLOWED: u32 = 163845u32;
pub const WLAN_REASON_CODE_BSS_TYPE_UNMATCH: u32 = 196611u32;
pub const WLAN_REASON_CODE_CONFLICT_SECURITY: u32 = 524299u32;
pub const WLAN_REASON_CODE_CONNECT_CALL_FAIL: u32 = 163849u32;
pub const WLAN_REASON_CODE_DATARATE_UNMATCH: u32 = 196613u32;
pub const WLAN_REASON_CODE_DISCONNECT_TIMEOUT: u32 = 229391u32;
pub const WLAN_REASON_CODE_DRIVER_DISCONNECTED: u32 = 229387u32;
pub const WLAN_REASON_CODE_DRIVER_OPERATION_FAILURE: u32 = 229388u32;
pub const WLAN_REASON_CODE_GP_DENIED: u32 = 163843u32;
pub const WLAN_REASON_CODE_HOTSPOT2_PROFILE_DENIED: u32 = 163857u32;
pub const WLAN_REASON_CODE_HOTSPOT2_PROFILE_NOT_ALLOWED: u32 = 524315u32;
pub const WLAN_REASON_CODE_IHV_CONNECTIVITY_NOT_SUPPORTED: u32 = 524309u32;
pub const WLAN_REASON_CODE_IHV_NOT_AVAILABLE: u32 = 229389u32;
pub const WLAN_REASON_CODE_IHV_NOT_RESPONDING: u32 = 229390u32;
pub const WLAN_REASON_CODE_IHV_OUI_MISMATCH: u32 = 524296u32;
pub const WLAN_REASON_CODE_IHV_OUI_MISSING: u32 = 524297u32;
pub const WLAN_REASON_CODE_IHV_SECURITY_NOT_SUPPORTED: u32 = 524295u32;
pub const WLAN_REASON_CODE_IHV_SECURITY_ONEX_MISSING: u32 = 524306u32;
pub const WLAN_REASON_CODE_IHV_SETTINGS_MISSING: u32 = 524298u32;
pub const WLAN_REASON_CODE_INTERNAL_FAILURE: u32 = 229392u32;
pub const WLAN_REASON_CODE_INVALID_ADHOC_CONNECTION_MODE: u32 = 524302u32;
pub const WLAN_REASON_CODE_INVALID_BSS_TYPE: u32 = 524301u32;
pub const WLAN_REASON_CODE_INVALID_CHANNEL: u32 = 524311u32;
pub const WLAN_REASON_CODE_INVALID_PHY_TYPE: u32 = 524293u32;
pub const WLAN_REASON_CODE_INVALID_PROFILE_NAME: u32 = 524291u32;
pub const WLAN_REASON_CODE_INVALID_PROFILE_SCHEMA: u32 = 524289u32;
pub const WLAN_REASON_CODE_INVALID_PROFILE_TYPE: u32 = 524292u32;
pub const WLAN_REASON_CODE_IN_BLOCKED_LIST: u32 = 163847u32;
pub const WLAN_REASON_CODE_IN_FAILED_LIST: u32 = 163846u32;
pub const WLAN_REASON_CODE_KEY_MISMATCH: u32 = 163853u32;
pub const WLAN_REASON_CODE_MSMSEC_AUTH_START_TIMEOUT: u32 = 294914u32;
pub const WLAN_REASON_CODE_MSMSEC_AUTH_SUCCESS_TIMEOUT: u32 = 294915u32;
pub const WLAN_REASON_CODE_MSMSEC_AUTH_WCN_COMPLETED: u32 = 294937u32;
pub const WLAN_REASON_CODE_MSMSEC_BASE: u32 = 262144u32;
pub const WLAN_REASON_CODE_MSMSEC_CANCELLED: u32 = 294929u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_DISCOVERY: u32 = 262165u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_MFP_NW_NIC: u32 = 262181u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_NETWORK: u32 = 262162u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_NIC: u32 = 262163u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_PROFILE: u32 = 262164u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_PROFILE_AUTH: u32 = 262174u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_PROFILE_CIPHER: u32 = 262175u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_PROFILE_SAFE_MODE_NIC: u32 = 262177u32;
pub const WLAN_REASON_CODE_MSMSEC_CAPABILITY_PROFILE_SAFE_MODE_NW: u32 = 262178u32;
pub const WLAN_REASON_CODE_MSMSEC_CONNECT_BASE: u32 = 294912u32;
pub const WLAN_REASON_CODE_MSMSEC_DOWNGRADE_DETECTED: u32 = 294931u32;
pub const WLAN_REASON_CODE_MSMSEC_END: u32 = 327679u32;
pub const WLAN_REASON_CODE_MSMSEC_FORCED_FAILURE: u32 = 294933u32;
pub const WLAN_REASON_CODE_MSMSEC_G1_MISSING_GRP_KEY: u32 = 294925u32;
pub const WLAN_REASON_CODE_MSMSEC_G1_MISSING_KEY_DATA: u32 = 294924u32;
pub const WLAN_REASON_CODE_MSMSEC_G1_MISSING_MGMT_GRP_KEY: u32 = 294939u32;
pub const WLAN_REASON_CODE_MSMSEC_KEY_FORMAT: u32 = 294930u32;
pub const WLAN_REASON_CODE_MSMSEC_KEY_START_TIMEOUT: u32 = 294916u32;
pub const WLAN_REASON_CODE_MSMSEC_KEY_SUCCESS_TIMEOUT: u32 = 294917u32;
pub const WLAN_REASON_CODE_MSMSEC_M2_MISSING_IE: u32 = 294936u32;
pub const WLAN_REASON_CODE_MSMSEC_M2_MISSING_KEY_DATA: u32 = 294935u32;
pub const WLAN_REASON_CODE_MSMSEC_M3_MISSING_GRP_KEY: u32 = 294920u32;
pub const WLAN_REASON_CODE_MSMSEC_M3_MISSING_IE: u32 = 294919u32;
pub const WLAN_REASON_CODE_MSMSEC_M3_MISSING_KEY_DATA: u32 = 294918u32;
pub const WLAN_REASON_CODE_MSMSEC_M3_MISSING_MGMT_GRP_KEY: u32 = 294938u32;
pub const WLAN_REASON_CODE_MSMSEC_M3_TOO_MANY_RSNIE: u32 = 294934u32;
pub const WLAN_REASON_CODE_MSMSEC_MAX: u32 = 327679u32;
pub const WLAN_REASON_CODE_MSMSEC_MIN: u32 = 262144u32;
pub const WLAN_REASON_CODE_MSMSEC_MIXED_CELL: u32 = 262169u32;
pub const WLAN_REASON_CODE_MSMSEC_NIC_FAILURE: u32 = 294928u32;
pub const WLAN_REASON_CODE_MSMSEC_NO_AUTHENTICATOR: u32 = 294927u32;
pub const WLAN_REASON_CODE_MSMSEC_NO_PAIRWISE_KEY: u32 = 294923u32;
pub const WLAN_REASON_CODE_MSMSEC_PEER_INDICATED_INSECURE: u32 = 294926u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_AUTH_TIMERS_INVALID: u32 = 262170u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_DUPLICATE_AUTH_CIPHER: u32 = 262151u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_AUTH_CIPHER: u32 = 262153u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_GKEY_INTV: u32 = 262171u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_KEY_INDEX: u32 = 262145u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_PMKCACHE_MODE: u32 = 262156u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_PMKCACHE_SIZE: u32 = 262157u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_PMKCACHE_TTL: u32 = 262158u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_PREAUTH_MODE: u32 = 262159u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_INVALID_PREAUTH_THROTTLE: u32 = 262160u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_KEYMATERIAL_CHAR: u32 = 262167u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_KEY_LENGTH: u32 = 262147u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_KEY_UNMAPPED_CHAR: u32 = 262173u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_NO_AUTH_CIPHER_SPECIFIED: u32 = 262149u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_ONEX_DISABLED: u32 = 262154u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_ONEX_ENABLED: u32 = 262155u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_PASSPHRASE_CHAR: u32 = 262166u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_PREAUTH_ONLY_ENABLED: u32 = 262161u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_PSK_LENGTH: u32 = 262148u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_PSK_PRESENT: u32 = 262146u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_RAWDATA_INVALID: u32 = 262152u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_SAFE_MODE: u32 = 262176u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_TOO_MANY_AUTH_CIPHER_SPECIFIED: u32 = 262150u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_UNSUPPORTED_AUTH: u32 = 262179u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_UNSUPPORTED_CIPHER: u32 = 262180u32;
pub const WLAN_REASON_CODE_MSMSEC_PROFILE_WRONG_KEYTYPE: u32 = 262168u32;
pub const WLAN_REASON_CODE_MSMSEC_PR_IE_MATCHING: u32 = 294921u32;
pub const WLAN_REASON_CODE_MSMSEC_PSK_MISMATCH_SUSPECTED: u32 = 294932u32;
pub const WLAN_REASON_CODE_MSMSEC_SEC_IE_MATCHING: u32 = 294922u32;
pub const WLAN_REASON_CODE_MSMSEC_TRANSITION_NETWORK: u32 = 262172u32;
pub const WLAN_REASON_CODE_MSMSEC_UI_REQUEST_FAILURE: u32 = 294913u32;
pub const WLAN_REASON_CODE_MSM_BASE: u32 = 196608u32;
pub const WLAN_REASON_CODE_MSM_CONNECT_BASE: u32 = 229376u32;
pub const WLAN_REASON_CODE_MSM_END: u32 = 262143u32;
pub const WLAN_REASON_CODE_MSM_SECURITY_MISSING: u32 = 524294u32;
pub const WLAN_REASON_CODE_NETWORK_NOT_AVAILABLE: u32 = 163851u32;
pub const WLAN_REASON_CODE_NETWORK_NOT_COMPATIBLE: u32 = 131073u32;
pub const WLAN_REASON_CODE_NON_BROADCAST_SET_FOR_ADHOC: u32 = 524303u32;
pub const WLAN_REASON_CODE_NOT_VISIBLE: u32 = 163842u32;
pub const WLAN_REASON_CODE_NO_AUTO_CONNECTION: u32 = 163841u32;
pub const WLAN_REASON_CODE_NO_VISIBLE_AP: u32 = 229396u32;
pub const WLAN_REASON_CODE_OPERATION_MODE_NOT_SUPPORTED: u32 = 524312u32;
pub const WLAN_REASON_CODE_PHY_TYPE_UNMATCH: u32 = 196612u32;
pub const WLAN_REASON_CODE_PRE_SECURITY_FAILURE: u32 = 229380u32;
pub const WLAN_REASON_CODE_PROFILE_BASE: u32 = 524288u32;
pub const WLAN_REASON_CODE_PROFILE_CHANGED_OR_DELETED: u32 = 163852u32;
pub const WLAN_REASON_CODE_PROFILE_CONNECT_BASE: u32 = 557056u32;
pub const WLAN_REASON_CODE_PROFILE_END: u32 = 589823u32;
pub const WLAN_REASON_CODE_PROFILE_MISSING: u32 = 524290u32;
pub const WLAN_REASON_CODE_PROFILE_NOT_COMPATIBLE: u32 = 131074u32;
pub const WLAN_REASON_CODE_PROFILE_SSID_INVALID: u32 = 524307u32;
pub const WLAN_REASON_CODE_RANGE_SIZE: u32 = 65536u32;
pub const WLAN_REASON_CODE_RESERVED_BASE: u32 = 720896u32;
pub const WLAN_REASON_CODE_RESERVED_END: u32 = 786431u32;
pub const WLAN_REASON_CODE_ROAMING_FAILURE: u32 = 229384u32;
pub const WLAN_REASON_CODE_ROAMING_SECURITY_FAILURE: u32 = 229385u32;
pub const WLAN_REASON_CODE_SCAN_CALL_FAIL: u32 = 163850u32;
pub const WLAN_REASON_CODE_SECURITY_FAILURE: u32 = 229382u32;
pub const WLAN_REASON_CODE_SECURITY_MISSING: u32 = 524300u32;
pub const WLAN_REASON_CODE_SECURITY_TIMEOUT: u32 = 229383u32;
pub const WLAN_REASON_CODE_SSID_LIST_TOO_LONG: u32 = 163848u32;
pub const WLAN_REASON_CODE_START_SECURITY_FAILURE: u32 = 229381u32;
pub const WLAN_REASON_CODE_SUCCESS: u32 = 0u32;
pub const WLAN_REASON_CODE_TOO_MANY_SECURITY_ATTEMPTS: u32 = 229394u32;
pub const WLAN_REASON_CODE_TOO_MANY_SSID: u32 = 524308u32;
pub const WLAN_REASON_CODE_UI_REQUEST_TIMEOUT: u32 = 229393u32;
pub const WLAN_REASON_CODE_UNKNOWN: u32 = 65537u32;
pub const WLAN_REASON_CODE_UNSUPPORTED_SECURITY_SET: u32 = 196610u32;
pub const WLAN_REASON_CODE_UNSUPPORTED_SECURITY_SET_BY_OS: u32 = 196609u32;
pub const WLAN_REASON_CODE_USER_CANCELLED: u32 = 229377u32;
pub const WLAN_REASON_CODE_USER_DENIED: u32 = 163844u32;
pub const WLAN_REASON_CODE_USER_NOT_RESPOND: u32 = 163854u32;
pub const WLAN_SECURABLE_OBJECT_COUNT: WLAN_SECURABLE_OBJECT = 17i32;
pub const WLAN_SET_EAPHOST_DATA_ALL_USERS: WLAN_SET_EAPHOST_FLAGS = 1u32;
pub const WLAN_UI_API_INITIAL_VERSION: u32 = 1u32;
pub const WLAN_UI_API_VERSION: u32 = 1u32;
pub const WLAdvPage: WL_DISPLAY_PAGES = 2i32;
pub const WLConnectionPage: WL_DISPLAY_PAGES = 0i32;
pub const WLSecurityPage: WL_DISPLAY_PAGES = 1i32;
pub const ch_description_type_center_frequency: CH_DESCRIPTION_TYPE = 2i32;
pub const ch_description_type_logical: CH_DESCRIPTION_TYPE = 1i32;
pub const ch_description_type_phy_specific: CH_DESCRIPTION_TYPE = 3i32;
pub const connection_phase_any: DOT11EXT_IHV_CONNECTION_PHASE = 0i32;
pub const connection_phase_initial_connection: DOT11EXT_IHV_CONNECTION_PHASE = 1i32;
pub const connection_phase_post_l3_connection: DOT11EXT_IHV_CONNECTION_PHASE = 2i32;
pub const dot11_AC_param_BE: DOT11_AC_PARAM = 0i32;
pub const dot11_AC_param_BK: DOT11_AC_PARAM = 1i32;
pub const dot11_AC_param_VI: DOT11_AC_PARAM = 2i32;
pub const dot11_AC_param_VO: DOT11_AC_PARAM = 3i32;
pub const dot11_AC_param_max: DOT11_AC_PARAM = 4i32;
pub const dot11_ANQP_query_result_access_issues: DOT11_ANQP_QUERY_RESULT = 7i32;
pub const dot11_ANQP_query_result_advertisement_protocol_not_supported_on_remote: DOT11_ANQP_QUERY_RESULT = 4i32;
pub const dot11_ANQP_query_result_advertisement_server_not_responding: DOT11_ANQP_QUERY_RESULT = 6i32;
pub const dot11_ANQP_query_result_failure: DOT11_ANQP_QUERY_RESULT = 1i32;
pub const dot11_ANQP_query_result_gas_protocol_failure: DOT11_ANQP_QUERY_RESULT = 5i32;
pub const dot11_ANQP_query_result_resources: DOT11_ANQP_QUERY_RESULT = 3i32;
pub const dot11_ANQP_query_result_success: DOT11_ANQP_QUERY_RESULT = 0i32;
pub const dot11_ANQP_query_result_timed_out: DOT11_ANQP_QUERY_RESULT = 2i32;
pub const dot11_BSS_type_any: DOT11_BSS_TYPE = 3i32;
pub const dot11_BSS_type_independent: DOT11_BSS_TYPE = 2i32;
pub const dot11_BSS_type_infrastructure: DOT11_BSS_TYPE = 1i32;
pub const dot11_assoc_state_auth_assoc: DOT11_ASSOCIATION_STATE = 3i32;
pub const dot11_assoc_state_auth_unassoc: DOT11_ASSOCIATION_STATE = 2i32;
pub const dot11_assoc_state_unauth_unassoc: DOT11_ASSOCIATION_STATE = 1i32;
pub const dot11_assoc_state_zero: DOT11_ASSOCIATION_STATE = 0i32;
pub const dot11_band_2p4g: DOT11_BAND = 1i32;
pub const dot11_band_4p9g: DOT11_BAND = 2i32;
pub const dot11_band_5g: DOT11_BAND = 3i32;
pub const dot11_diversity_support_dynamic: DOT11_DIVERSITY_SUPPORT = 3i32;
pub const dot11_diversity_support_fixedlist: DOT11_DIVERSITY_SUPPORT = 1i32;
pub const dot11_diversity_support_notsupported: DOT11_DIVERSITY_SUPPORT = 2i32;
pub const dot11_diversity_support_unknown: DOT11_DIVERSITY_SUPPORT = 0i32;
pub const dot11_hop_algo_current: DOT11_HOP_ALGO_ADOPTED = 0i32;
pub const dot11_hop_algo_hcc: DOT11_HOP_ALGO_ADOPTED = 2i32;
pub const dot11_hop_algo_hop_index: DOT11_HOP_ALGO_ADOPTED = 1i32;
pub const dot11_key_direction_both: DOT11_KEY_DIRECTION = 1i32;
pub const dot11_key_direction_inbound: DOT11_KEY_DIRECTION = 2i32;
pub const dot11_key_direction_outbound: DOT11_KEY_DIRECTION = 3i32;
pub const dot11_manufacturing_callback_IHV_end: DOT11_MANUFACTURING_CALLBACK_TYPE = -1i32;
pub const dot11_manufacturing_callback_IHV_start: DOT11_MANUFACTURING_CALLBACK_TYPE = -2147483648i32;
pub const dot11_manufacturing_callback_self_test_complete: DOT11_MANUFACTURING_CALLBACK_TYPE = 1i32;
pub const dot11_manufacturing_callback_sleep_complete: DOT11_MANUFACTURING_CALLBACK_TYPE = 2i32;
pub const dot11_manufacturing_callback_unknown: DOT11_MANUFACTURING_CALLBACK_TYPE = 0i32;
pub const dot11_manufacturing_test_IHV_end: DOT11_MANUFACTURING_TEST_TYPE = -1i32;
pub const dot11_manufacturing_test_IHV_start: DOT11_MANUFACTURING_TEST_TYPE = -2147483648i32;
pub const dot11_manufacturing_test_awake: DOT11_MANUFACTURING_TEST_TYPE = 9i32;
pub const dot11_manufacturing_test_query_adc: DOT11_MANUFACTURING_TEST_TYPE = 5i32;
pub const dot11_manufacturing_test_query_data: DOT11_MANUFACTURING_TEST_TYPE = 7i32;
pub const dot11_manufacturing_test_rx: DOT11_MANUFACTURING_TEST_TYPE = 3i32;
pub const dot11_manufacturing_test_self_query_result: DOT11_MANUFACTURING_TEST_TYPE = 2i32;
pub const dot11_manufacturing_test_self_start: DOT11_MANUFACTURING_TEST_TYPE = 1i32;
pub const dot11_manufacturing_test_set_data: DOT11_MANUFACTURING_TEST_TYPE = 6i32;
pub const dot11_manufacturing_test_sleep: DOT11_MANUFACTURING_TEST_TYPE = 8i32;
pub const dot11_manufacturing_test_tx: DOT11_MANUFACTURING_TEST_TYPE = 4i32;
pub const dot11_manufacturing_test_unknown: DOT11_MANUFACTURING_TEST_TYPE = 0i32;
pub const dot11_offload_type_auth: DOT11_OFFLOAD_TYPE = 2i32;
pub const dot11_offload_type_wep: DOT11_OFFLOAD_TYPE = 1i32;
pub const dot11_phy_type_IHV_end: DOT11_PHY_TYPE = -1i32;
pub const dot11_phy_type_IHV_start: DOT11_PHY_TYPE = -2147483648i32;
pub const dot11_phy_type_any: DOT11_PHY_TYPE = 0i32;
pub const dot11_phy_type_dmg: DOT11_PHY_TYPE = 9i32;
pub const dot11_phy_type_dsss: DOT11_PHY_TYPE = 2i32;
pub const dot11_phy_type_eht: DOT11_PHY_TYPE = 11i32;
pub const dot11_phy_type_erp: DOT11_PHY_TYPE = 6i32;
pub const dot11_phy_type_fhss: DOT11_PHY_TYPE = 1i32;
pub const dot11_phy_type_he: DOT11_PHY_TYPE = 10i32;
pub const dot11_phy_type_hrdsss: DOT11_PHY_TYPE = 5i32;
pub const dot11_phy_type_ht: DOT11_PHY_TYPE = 7i32;
pub const dot11_phy_type_irbaseband: DOT11_PHY_TYPE = 3i32;
pub const dot11_phy_type_ofdm: DOT11_PHY_TYPE = 4i32;
pub const dot11_phy_type_unknown: DOT11_PHY_TYPE = 0i32;
pub const dot11_phy_type_vht: DOT11_PHY_TYPE = 8i32;
pub const dot11_power_mode_active: DOT11_POWER_MODE = 1i32;
pub const dot11_power_mode_powersave: DOT11_POWER_MODE = 2i32;
pub const dot11_power_mode_reason_compliant_AP: DOT11_POWER_MODE_REASON = 3i32;
pub const dot11_power_mode_reason_compliant_WFD_device: DOT11_POWER_MODE_REASON = 4i32;
pub const dot11_power_mode_reason_legacy_WFD_device: DOT11_POWER_MODE_REASON = 2i32;
pub const dot11_power_mode_reason_no_change: DOT11_POWER_MODE_REASON = 0i32;
pub const dot11_power_mode_reason_noncompliant_AP: DOT11_POWER_MODE_REASON = 1i32;
pub const dot11_power_mode_reason_others: DOT11_POWER_MODE_REASON = 5i32;
pub const dot11_power_mode_unknown: DOT11_POWER_MODE = 0i32;
pub const dot11_radio_state_off: DOT11_RADIO_STATE = 2i32;
pub const dot11_radio_state_on: DOT11_RADIO_STATE = 1i32;
pub const dot11_radio_state_unknown: DOT11_RADIO_STATE = 0i32;
pub const dot11_reset_type_mac: DOT11_RESET_TYPE = 2i32;
pub const dot11_reset_type_phy: DOT11_RESET_TYPE = 1i32;
pub const dot11_reset_type_phy_and_mac: DOT11_RESET_TYPE = 3i32;
pub const dot11_scan_type_active: DOT11_SCAN_TYPE = 1i32;
pub const dot11_scan_type_auto: DOT11_SCAN_TYPE = 3i32;
pub const dot11_scan_type_forced: DOT11_SCAN_TYPE = -2147483648i32;
pub const dot11_scan_type_passive: DOT11_SCAN_TYPE = 2i32;
pub const dot11_temp_type_1: DOT11_TEMP_TYPE = 1i32;
pub const dot11_temp_type_2: DOT11_TEMP_TYPE = 2i32;
pub const dot11_temp_type_unknown: DOT11_TEMP_TYPE = 0i32;
pub const dot11_update_ie_op_create_replace: DOT11_UPDATE_IE_OP = 1i32;
pub const dot11_update_ie_op_delete: DOT11_UPDATE_IE_OP = 2i32;
pub const dot11_wfd_discover_type_auto: DOT11_WFD_DISCOVER_TYPE = 3i32;
pub const dot11_wfd_discover_type_find_only: DOT11_WFD_DISCOVER_TYPE = 2i32;
pub const dot11_wfd_discover_type_forced: DOT11_WFD_DISCOVER_TYPE = -2147483648i32;
pub const dot11_wfd_discover_type_scan_only: DOT11_WFD_DISCOVER_TYPE = 1i32;
pub const dot11_wfd_discover_type_scan_social_channels: DOT11_WFD_DISCOVER_TYPE = 4i32;
pub const dot11_wfd_scan_type_active: DOT11_WFD_SCAN_TYPE = 1i32;
pub const dot11_wfd_scan_type_auto: DOT11_WFD_SCAN_TYPE = 3i32;
pub const dot11_wfd_scan_type_passive: DOT11_WFD_SCAN_TYPE = 2i32;
pub const wlan_adhoc_network_state_connected: WLAN_ADHOC_NETWORK_STATE = 1i32;
pub const wlan_adhoc_network_state_formed: WLAN_ADHOC_NETWORK_STATE = 0i32;
pub const wlan_autoconf_opcode_allow_explicit_creds: WLAN_AUTOCONF_OPCODE = 4i32;
pub const wlan_autoconf_opcode_allow_virtual_station_extensibility: WLAN_AUTOCONF_OPCODE = 6i32;
pub const wlan_autoconf_opcode_block_period: WLAN_AUTOCONF_OPCODE = 5i32;
pub const wlan_autoconf_opcode_end: WLAN_AUTOCONF_OPCODE = 7i32;
pub const wlan_autoconf_opcode_only_use_gp_profiles_for_allowed_networks: WLAN_AUTOCONF_OPCODE = 3i32;
pub const wlan_autoconf_opcode_power_setting: WLAN_AUTOCONF_OPCODE = 2i32;
pub const wlan_autoconf_opcode_show_denied_networks: WLAN_AUTOCONF_OPCODE = 1i32;
pub const wlan_autoconf_opcode_start: WLAN_AUTOCONF_OPCODE = 0i32;
pub const wlan_connection_mode_auto: WLAN_CONNECTION_MODE = 4i32;
pub const wlan_connection_mode_discovery_secure: WLAN_CONNECTION_MODE = 2i32;
pub const wlan_connection_mode_discovery_unsecure: WLAN_CONNECTION_MODE = 3i32;
pub const wlan_connection_mode_invalid: WLAN_CONNECTION_MODE = 5i32;
pub const wlan_connection_mode_profile: WLAN_CONNECTION_MODE = 0i32;
pub const wlan_connection_mode_temporary_profile: WLAN_CONNECTION_MODE = 1i32;
pub const wlan_filter_list_type_gp_deny: WLAN_FILTER_LIST_TYPE = 1i32;
pub const wlan_filter_list_type_gp_permit: WLAN_FILTER_LIST_TYPE = 0i32;
pub const wlan_filter_list_type_user_deny: WLAN_FILTER_LIST_TYPE = 3i32;
pub const wlan_filter_list_type_user_permit: WLAN_FILTER_LIST_TYPE = 2i32;
pub const wlan_hosted_network_active: WLAN_HOSTED_NETWORK_STATE = 2i32;
pub const wlan_hosted_network_idle: WLAN_HOSTED_NETWORK_STATE = 1i32;
pub const wlan_hosted_network_opcode_connection_settings: WLAN_HOSTED_NETWORK_OPCODE = 0i32;
pub const wlan_hosted_network_opcode_enable: WLAN_HOSTED_NETWORK_OPCODE = 3i32;
pub const wlan_hosted_network_opcode_security_settings: WLAN_HOSTED_NETWORK_OPCODE = 1i32;
pub const wlan_hosted_network_opcode_station_profile: WLAN_HOSTED_NETWORK_OPCODE = 2i32;
pub const wlan_hosted_network_peer_state_authenticated: WLAN_HOSTED_NETWORK_PEER_AUTH_STATE = 1i32;
pub const wlan_hosted_network_peer_state_change: WLAN_HOSTED_NETWORK_NOTIFICATION_CODE = 4097i32;
pub const wlan_hosted_network_peer_state_invalid: WLAN_HOSTED_NETWORK_PEER_AUTH_STATE = 0i32;
pub const wlan_hosted_network_radio_state_change: WLAN_HOSTED_NETWORK_NOTIFICATION_CODE = 4098i32;
pub const wlan_hosted_network_reason_ap_start_failed: WLAN_HOSTED_NETWORK_REASON = 19i32;
pub const wlan_hosted_network_reason_bad_parameters: WLAN_HOSTED_NETWORK_REASON = 2i32;
pub const wlan_hosted_network_reason_client_abort: WLAN_HOSTED_NETWORK_REASON = 18i32;
pub const wlan_hosted_network_reason_crypt_error: WLAN_HOSTED_NETWORK_REASON = 8i32;
pub const wlan_hosted_network_reason_device_change: WLAN_HOSTED_NETWORK_REASON = 25i32;
pub const wlan_hosted_network_reason_elevation_required: WLAN_HOSTED_NETWORK_REASON = 5i32;
pub const wlan_hosted_network_reason_gp_denied: WLAN_HOSTED_NETWORK_REASON = 23i32;
pub const wlan_hosted_network_reason_impersonation: WLAN_HOSTED_NETWORK_REASON = 9i32;
pub const wlan_hosted_network_reason_incompatible_connection_started: WLAN_HOSTED_NETWORK_REASON = 15i32;
pub const wlan_hosted_network_reason_incompatible_connection_stopped: WLAN_HOSTED_NETWORK_REASON = 16i32;
pub const wlan_hosted_network_reason_insufficient_resources: WLAN_HOSTED_NETWORK_REASON = 4i32;
pub const wlan_hosted_network_reason_interface_available: WLAN_HOSTED_NETWORK_REASON = 11i32;
pub const wlan_hosted_network_reason_interface_unavailable: WLAN_HOSTED_NETWORK_REASON = 12i32;
pub const wlan_hosted_network_reason_miniport_started: WLAN_HOSTED_NETWORK_REASON = 14i32;
pub const wlan_hosted_network_reason_miniport_stopped: WLAN_HOSTED_NETWORK_REASON = 13i32;
pub const wlan_hosted_network_reason_peer_arrived: WLAN_HOSTED_NETWORK_REASON = 20i32;
pub const wlan_hosted_network_reason_peer_departed: WLAN_HOSTED_NETWORK_REASON = 21i32;
pub const wlan_hosted_network_reason_peer_timeout: WLAN_HOSTED_NETWORK_REASON = 22i32;
pub const wlan_hosted_network_reason_persistence_failed: WLAN_HOSTED_NETWORK_REASON = 7i32;
pub const wlan_hosted_network_reason_properties_change: WLAN_HOSTED_NETWORK_REASON = 26i32;
pub const wlan_hosted_network_reason_read_only: WLAN_HOSTED_NETWORK_REASON = 6i32;
pub const wlan_hosted_network_reason_service_available_on_virtual_station: WLAN_HOSTED_NETWORK_REASON = 28i32;
pub const wlan_hosted_network_reason_service_shutting_down: WLAN_HOSTED_NETWORK_REASON = 3i32;
pub const wlan_hosted_network_reason_service_unavailable: WLAN_HOSTED_NETWORK_REASON = 24i32;
pub const wlan_hosted_network_reason_stop_before_start: WLAN_HOSTED_NETWORK_REASON = 10i32;
pub const wlan_hosted_network_reason_success: WLAN_HOSTED_NETWORK_REASON = 0i32;
pub const wlan_hosted_network_reason_unspecified: WLAN_HOSTED_NETWORK_REASON = 1i32;
pub const wlan_hosted_network_reason_user_action: WLAN_HOSTED_NETWORK_REASON = 17i32;
pub const wlan_hosted_network_reason_virtual_station_blocking_use: WLAN_HOSTED_NETWORK_REASON = 27i32;
pub const wlan_hosted_network_state_change: WLAN_HOSTED_NETWORK_NOTIFICATION_CODE = 4096i32;
pub const wlan_hosted_network_unavailable: WLAN_HOSTED_NETWORK_STATE = 0i32;
pub const wlan_ihv_control_type_driver: WLAN_IHV_CONTROL_TYPE = 1i32;
pub const wlan_ihv_control_type_service: WLAN_IHV_CONTROL_TYPE = 0i32;
pub const wlan_interface_state_ad_hoc_network_formed: WLAN_INTERFACE_STATE = 2i32;
pub const wlan_interface_state_associating: WLAN_INTERFACE_STATE = 5i32;
pub const wlan_interface_state_authenticating: WLAN_INTERFACE_STATE = 7i32;
pub const wlan_interface_state_connected: WLAN_INTERFACE_STATE = 1i32;
pub const wlan_interface_state_disconnected: WLAN_INTERFACE_STATE = 4i32;
pub const wlan_interface_state_disconnecting: WLAN_INTERFACE_STATE = 3i32;
pub const wlan_interface_state_discovering: WLAN_INTERFACE_STATE = 6i32;
pub const wlan_interface_state_not_ready: WLAN_INTERFACE_STATE = 0i32;
pub const wlan_interface_type_emulated_802_11: WLAN_INTERFACE_TYPE = 0i32;
pub const wlan_interface_type_invalid: WLAN_INTERFACE_TYPE = 2i32;
pub const wlan_interface_type_native_802_11: WLAN_INTERFACE_TYPE = 1i32;
pub const wlan_intf_opcode_autoconf_enabled: WLAN_INTF_OPCODE = 1i32;
pub const wlan_intf_opcode_autoconf_end: WLAN_INTF_OPCODE = 268435455i32;
pub const wlan_intf_opcode_autoconf_start: WLAN_INTF_OPCODE = 0i32;
pub const wlan_intf_opcode_background_scan_enabled: WLAN_INTF_OPCODE = 2i32;
pub const wlan_intf_opcode_bss_type: WLAN_INTF_OPCODE = 5i32;
pub const wlan_intf_opcode_certified_safe_mode: WLAN_INTF_OPCODE = 14i32;
pub const wlan_intf_opcode_channel_number: WLAN_INTF_OPCODE = 8i32;
pub const wlan_intf_opcode_current_connection: WLAN_INTF_OPCODE = 7i32;
pub const wlan_intf_opcode_current_operation_mode: WLAN_INTF_OPCODE = 12i32;
pub const wlan_intf_opcode_hosted_network_capable: WLAN_INTF_OPCODE = 15i32;
pub const wlan_intf_opcode_ihv_end: WLAN_INTF_OPCODE = 1073741823i32;
pub const wlan_intf_opcode_ihv_start: WLAN_INTF_OPCODE = 805306368i32;
pub const wlan_intf_opcode_interface_state: WLAN_INTF_OPCODE = 6i32;
pub const wlan_intf_opcode_management_frame_protection_capable: WLAN_INTF_OPCODE = 16i32;
pub const wlan_intf_opcode_media_streaming_mode: WLAN_INTF_OPCODE = 3i32;
pub const wlan_intf_opcode_msm_end: WLAN_INTF_OPCODE = 536870911i32;
pub const wlan_intf_opcode_msm_start: WLAN_INTF_OPCODE = 268435712i32;
pub const wlan_intf_opcode_radio_state: WLAN_INTF_OPCODE = 4i32;
pub const wlan_intf_opcode_rssi: WLAN_INTF_OPCODE = 268435714i32;
pub const wlan_intf_opcode_secondary_sta_interfaces: WLAN_INTF_OPCODE = 17i32;
pub const wlan_intf_opcode_secondary_sta_synchronized_connections: WLAN_INTF_OPCODE = 18i32;
pub const wlan_intf_opcode_security_end: WLAN_INTF_OPCODE = 805306367i32;
pub const wlan_intf_opcode_security_start: WLAN_INTF_OPCODE = 536936448i32;
pub const wlan_intf_opcode_statistics: WLAN_INTF_OPCODE = 268435713i32;
pub const wlan_intf_opcode_supported_adhoc_auth_cipher_pairs: WLAN_INTF_OPCODE = 10i32;
pub const wlan_intf_opcode_supported_country_or_region_string_list: WLAN_INTF_OPCODE = 11i32;
pub const wlan_intf_opcode_supported_infrastructure_auth_cipher_pairs: WLAN_INTF_OPCODE = 9i32;
pub const wlan_intf_opcode_supported_safe_mode: WLAN_INTF_OPCODE = 13i32;
pub const wlan_notification_acm_adhoc_network_state_change: WLAN_NOTIFICATION_ACM = 22i32;
pub const wlan_notification_acm_autoconf_disabled: WLAN_NOTIFICATION_ACM = 2i32;
pub const wlan_notification_acm_autoconf_enabled: WLAN_NOTIFICATION_ACM = 1i32;
pub const wlan_notification_acm_background_scan_disabled: WLAN_NOTIFICATION_ACM = 4i32;
pub const wlan_notification_acm_background_scan_enabled: WLAN_NOTIFICATION_ACM = 3i32;
pub const wlan_notification_acm_bss_type_change: WLAN_NOTIFICATION_ACM = 5i32;
pub const wlan_notification_acm_connection_attempt_fail: WLAN_NOTIFICATION_ACM = 11i32;
pub const wlan_notification_acm_connection_complete: WLAN_NOTIFICATION_ACM = 10i32;
pub const wlan_notification_acm_connection_start: WLAN_NOTIFICATION_ACM = 9i32;
pub const wlan_notification_acm_disconnected: WLAN_NOTIFICATION_ACM = 21i32;
pub const wlan_notification_acm_disconnecting: WLAN_NOTIFICATION_ACM = 20i32;
pub const wlan_notification_acm_end: WLAN_NOTIFICATION_ACM = 28i32;
pub const wlan_notification_acm_filter_list_change: WLAN_NOTIFICATION_ACM = 12i32;
pub const wlan_notification_acm_interface_arrival: WLAN_NOTIFICATION_ACM = 13i32;
pub const wlan_notification_acm_interface_removal: WLAN_NOTIFICATION_ACM = 14i32;
pub const wlan_notification_acm_network_available: WLAN_NOTIFICATION_ACM = 19i32;
pub const wlan_notification_acm_network_not_available: WLAN_NOTIFICATION_ACM = 18i32;
pub const wlan_notification_acm_operational_state_change: WLAN_NOTIFICATION_ACM = 27i32;
pub const wlan_notification_acm_power_setting_change: WLAN_NOTIFICATION_ACM = 6i32;
pub const wlan_notification_acm_profile_blocked: WLAN_NOTIFICATION_ACM = 25i32;
pub const wlan_notification_acm_profile_change: WLAN_NOTIFICATION_ACM = 15i32;
pub const wlan_notification_acm_profile_name_change: WLAN_NOTIFICATION_ACM = 16i32;
pub const wlan_notification_acm_profile_unblocked: WLAN_NOTIFICATION_ACM = 23i32;
pub const wlan_notification_acm_profiles_exhausted: WLAN_NOTIFICATION_ACM = 17i32;
pub const wlan_notification_acm_scan_complete: WLAN_NOTIFICATION_ACM = 7i32;
pub const wlan_notification_acm_scan_fail: WLAN_NOTIFICATION_ACM = 8i32;
pub const wlan_notification_acm_scan_list_refresh: WLAN_NOTIFICATION_ACM = 26i32;
pub const wlan_notification_acm_screen_power_change: WLAN_NOTIFICATION_ACM = 24i32;
pub const wlan_notification_acm_start: WLAN_NOTIFICATION_ACM = 0i32;
pub const wlan_notification_msm_adapter_operation_mode_change: WLAN_NOTIFICATION_MSM = 14i32;
pub const wlan_notification_msm_adapter_removal: WLAN_NOTIFICATION_MSM = 13i32;
pub const wlan_notification_msm_associated: WLAN_NOTIFICATION_MSM = 2i32;
pub const wlan_notification_msm_associating: WLAN_NOTIFICATION_MSM = 1i32;
pub const wlan_notification_msm_authenticating: WLAN_NOTIFICATION_MSM = 3i32;
pub const wlan_notification_msm_connected: WLAN_NOTIFICATION_MSM = 4i32;
pub const wlan_notification_msm_disassociating: WLAN_NOTIFICATION_MSM = 9i32;
pub const wlan_notification_msm_disconnected: WLAN_NOTIFICATION_MSM = 10i32;
pub const wlan_notification_msm_end: WLAN_NOTIFICATION_MSM = 17i32;
pub const wlan_notification_msm_link_degraded: WLAN_NOTIFICATION_MSM = 15i32;
pub const wlan_notification_msm_link_improved: WLAN_NOTIFICATION_MSM = 16i32;
pub const wlan_notification_msm_peer_join: WLAN_NOTIFICATION_MSM = 11i32;
pub const wlan_notification_msm_peer_leave: WLAN_NOTIFICATION_MSM = 12i32;
pub const wlan_notification_msm_radio_state_change: WLAN_NOTIFICATION_MSM = 7i32;
pub const wlan_notification_msm_roaming_end: WLAN_NOTIFICATION_MSM = 6i32;
pub const wlan_notification_msm_roaming_start: WLAN_NOTIFICATION_MSM = 5i32;
pub const wlan_notification_msm_signal_quality_change: WLAN_NOTIFICATION_MSM = 8i32;
pub const wlan_notification_msm_start: WLAN_NOTIFICATION_MSM = 0i32;
pub const wlan_notification_security_end: WLAN_NOTIFICATION_SECURITY = 1i32;
pub const wlan_notification_security_start: WLAN_NOTIFICATION_SECURITY = 0i32;
pub const wlan_opcode_value_type_invalid: WLAN_OPCODE_VALUE_TYPE = 3i32;
pub const wlan_opcode_value_type_query_only: WLAN_OPCODE_VALUE_TYPE = 0i32;
pub const wlan_opcode_value_type_set_by_group_policy: WLAN_OPCODE_VALUE_TYPE = 1i32;
pub const wlan_opcode_value_type_set_by_user: WLAN_OPCODE_VALUE_TYPE = 2i32;
pub const wlan_operational_state_going_off: WLAN_OPERATIONAL_STATE = 3i32;
pub const wlan_operational_state_going_on: WLAN_OPERATIONAL_STATE = 4i32;
pub const wlan_operational_state_off: WLAN_OPERATIONAL_STATE = 1i32;
pub const wlan_operational_state_on: WLAN_OPERATIONAL_STATE = 2i32;
pub const wlan_operational_state_unknown: WLAN_OPERATIONAL_STATE = 0i32;
pub const wlan_power_setting_invalid: WLAN_POWER_SETTING = 4i32;
pub const wlan_power_setting_low_saving: WLAN_POWER_SETTING = 1i32;
pub const wlan_power_setting_maximum_saving: WLAN_POWER_SETTING = 3i32;
pub const wlan_power_setting_medium_saving: WLAN_POWER_SETTING = 2i32;
pub const wlan_power_setting_no_saving: WLAN_POWER_SETTING = 0i32;
pub const wlan_secure_ac_enabled: WLAN_SECURABLE_OBJECT = 2i32;
pub const wlan_secure_add_new_all_user_profiles: WLAN_SECURABLE_OBJECT = 9i32;
pub const wlan_secure_add_new_per_user_profiles: WLAN_SECURABLE_OBJECT = 10i32;
pub const wlan_secure_all_user_profiles_order: WLAN_SECURABLE_OBJECT = 8i32;
pub const wlan_secure_bc_scan_enabled: WLAN_SECURABLE_OBJECT = 3i32;
pub const wlan_secure_bss_type: WLAN_SECURABLE_OBJECT = 4i32;
pub const wlan_secure_current_operation_mode: WLAN_SECURABLE_OBJECT = 12i32;
pub const wlan_secure_deny_list: WLAN_SECURABLE_OBJECT = 1i32;
pub const wlan_secure_get_plaintext_key: WLAN_SECURABLE_OBJECT = 13i32;
pub const wlan_secure_hosted_network_elevated_access: WLAN_SECURABLE_OBJECT = 14i32;
pub const wlan_secure_ihv_control: WLAN_SECURABLE_OBJECT = 7i32;
pub const wlan_secure_interface_properties: WLAN_SECURABLE_OBJECT = 6i32;
pub const wlan_secure_media_streaming_mode_enabled: WLAN_SECURABLE_OBJECT = 11i32;
pub const wlan_secure_permit_list: WLAN_SECURABLE_OBJECT = 0i32;
pub const wlan_secure_show_denied: WLAN_SECURABLE_OBJECT = 5i32;
pub const wlan_secure_virtual_station_extensibility: WLAN_SECURABLE_OBJECT = 15i32;
pub const wlan_secure_wfd_elevated_access: WLAN_SECURABLE_OBJECT = 16i32;
pub type CH_DESCRIPTION_TYPE = i32;
pub type DEVPROP_PCIDEVICE_ACSCOMPATIBLEUPHIERARCHY = u32;
pub type DEVPROP_PCIDEVICE_ACSSUPPORT = u32;
pub type DEVPROP_PCIDEVICE_CURRENTSPEEDANDMODE = u32;
pub type DEVPROP_PCIDEVICE_DEVICEBRIDGETYPE = u32;
pub type DEVPROP_PCIDEVICE_INTERRUPTTYPE = u32;
pub type DEVPROP_PCIDEVICE_SRIOVSUPPORT = u32;
pub type DEVPROP_PCIEXPRESSDEVICE_LINKSPEED = u32;
pub type DEVPROP_PCIEXPRESSDEVICE_LINKWIDTH = u32;
pub type DEVPROP_PCIEXPRESSDEVICE_PAYLOADORREQUESTSIZE = u32;
pub type DEVPROP_PCIEXPRESSDEVICE_SPEC_VERSION = u32;
pub type DEVPROP_PCIROOTBUS_BUSWIDTH = u32;
pub type DEVPROP_PCIROOTBUS_CURRENTSPEEDANDMODE = u32;
pub type DEVPROP_PCIROOTBUS_SECONDARYINTERFACE = u32;
pub type DEVPROP_PCIROOTBUS_SUPPORTEDSPEEDSANDMODES = u32;
pub type DOT11EXT_IHV_CONNECTION_PHASE = i32;
pub type DOT11EXT_IHV_INDICATION_TYPE = i32;
pub type DOT11_AC_PARAM = i32;
pub type DOT11_ADHOC_AUTH_ALGORITHM = i32;
pub type DOT11_ADHOC_CIPHER_ALGORITHM = i32;
pub type DOT11_ADHOC_CONNECT_FAIL_REASON = i32;
pub type DOT11_ADHOC_NETWORK_CONNECTION_STATUS = i32;
pub type DOT11_ANQP_QUERY_RESULT = i32;
pub type DOT11_ASSOCIATION_STATE = i32;
pub type DOT11_AUTH_ALGORITHM = i32;
pub type DOT11_BAND = i32;
pub type DOT11_BSS_TYPE = i32;
pub type DOT11_CIPHER_ALGORITHM = i32;
pub type DOT11_DIRECTION = i32;
pub type DOT11_DIVERSITY_SUPPORT = i32;
pub type DOT11_DS_INFO = i32;
pub type DOT11_HOP_ALGO_ADOPTED = i32;
pub type DOT11_KEY_DIRECTION = i32;
pub type DOT11_MANUFACTURING_CALLBACK_TYPE = i32;
pub type DOT11_MANUFACTURING_SELF_TEST_TYPE = i32;
pub type DOT11_MANUFACTURING_TEST_TYPE = i32;
pub type DOT11_MSONEX_RESULT = i32;
pub type DOT11_OFFLOAD_TYPE = i32;
pub type DOT11_PHY_TYPE = i32;
pub type DOT11_POWER_MODE = i32;
pub type DOT11_POWER_MODE_REASON = i32;
pub type DOT11_RADIO_STATE = i32;
pub type DOT11_RESET_TYPE = i32;
pub type DOT11_SCAN_TYPE = i32;
pub type DOT11_TEMP_TYPE = i32;
pub type DOT11_UPDATE_IE_OP = i32;
pub type DOT11_WFD_DISCOVER_TYPE = i32;
pub type DOT11_WFD_SCAN_TYPE = i32;
pub type DOT11_WPS_CONFIG_METHOD = i32;
pub type DOT11_WPS_DEVICE_PASSWORD_ID = i32;
pub type ONEX_AUTH_IDENTITY = i32;
pub type ONEX_AUTH_RESTART_REASON = i32;
pub type ONEX_AUTH_STATUS = i32;
pub type ONEX_EAP_METHOD_BACKEND_SUPPORT = i32;
pub type ONEX_NOTIFICATION_TYPE = i32;
pub type ONEX_REASON_CODE = i32;
pub type WFD_ROLE_TYPE = i32;
pub type WLAN_ADHOC_NETWORK_STATE = i32;
pub type WLAN_AUTOCONF_OPCODE = i32;
pub type WLAN_CONNECTION_MODE = i32;
pub type WLAN_CONNECTION_NOTIFICATION_FLAGS = u32;
pub type WLAN_FILTER_LIST_TYPE = i32;
pub type WLAN_HOSTED_NETWORK_NOTIFICATION_CODE = i32;
pub type WLAN_HOSTED_NETWORK_OPCODE = i32;
pub type WLAN_HOSTED_NETWORK_PEER_AUTH_STATE = i32;
pub type WLAN_HOSTED_NETWORK_REASON = i32;
pub type WLAN_HOSTED_NETWORK_STATE = i32;
pub type WLAN_IHV_CONTROL_TYPE = i32;
pub type WLAN_INTERFACE_STATE = i32;
pub type WLAN_INTERFACE_TYPE = i32;
pub type WLAN_INTF_OPCODE = i32;
pub type WLAN_NOTIFICATION_ACM = i32;
pub type WLAN_NOTIFICATION_MSM = i32;
pub type WLAN_NOTIFICATION_SECURITY = i32;
pub type WLAN_NOTIFICATION_SOURCES = u32;
pub type WLAN_OPCODE_VALUE_TYPE = i32;
pub type WLAN_OPERATIONAL_STATE = i32;
pub type WLAN_POWER_SETTING = i32;
pub type WLAN_SECURABLE_OBJECT = i32;
pub type WLAN_SET_EAPHOST_FLAGS = u32;
pub type WL_DISPLAY_PAGES = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub struct DOT11EXT_APIS {
    pub Dot11ExtAllocateBuffer: DOT11EXT_ALLOCATE_BUFFER,
    pub Dot11ExtFreeBuffer: DOT11EXT_FREE_BUFFER,
    pub Dot11ExtSetProfileCustomUserData: DOT11EXT_SET_PROFILE_CUSTOM_USER_DATA,
    pub Dot11ExtGetProfileCustomUserData: DOT11EXT_GET_PROFILE_CUSTOM_USER_DATA,
    pub Dot11ExtSetCurrentProfile: DOT11EXT_SET_CURRENT_PROFILE,
    pub Dot11ExtSendUIRequest: DOT11EXT_SEND_UI_REQUEST,
    pub Dot11ExtPreAssociateCompletion: DOT11EXT_PRE_ASSOCIATE_COMPLETION,
    pub Dot11ExtPostAssociateCompletion: DOT11EXT_POST_ASSOCIATE_COMPLETION,
    pub Dot11ExtSendNotification: DOT11EXT_SEND_NOTIFICATION,
    pub Dot11ExtSendPacket: DOT11EXT_SEND_PACKET,
    pub Dot11ExtSetEtherTypeHandling: DOT11EXT_SET_ETHERTYPE_HANDLING,
    pub Dot11ExtSetAuthAlgorithm: DOT11EXT_SET_AUTH_ALGORITHM,
    pub Dot11ExtSetUnicastCipherAlgorithm: DOT11EXT_SET_UNICAST_CIPHER_ALGORITHM,
    pub Dot11ExtSetMulticastCipherAlgorithm: DOT11EXT_SET_MULTICAST_CIPHER_ALGORITHM,
    pub Dot11ExtSetDefaultKey: DOT11EXT_SET_DEFAULT_KEY,
    pub Dot11ExtSetKeyMappingKey: DOT11EXT_SET_KEY_MAPPING_KEY,
    pub Dot11ExtSetDefaultKeyId: DOT11EXT_SET_DEFAULT_KEY_ID,
    pub Dot11ExtNicSpecificExtension: DOT11EXT_NIC_SPECIFIC_EXTENSION,
    pub Dot11ExtSetExcludeUnencrypted: DOT11EXT_SET_EXCLUDE_UNENCRYPTED,
    pub Dot11ExtStartOneX: DOT11EXT_ONEX_START,
    pub Dot11ExtStopOneX: DOT11EXT_ONEX_STOP,
    pub Dot11ExtProcessSecurityPacket: DOT11EXT_PROCESS_ONEX_PACKET,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::marker::Copy for DOT11EXT_APIS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::clone::Clone for DOT11EXT_APIS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11EXT_IHV_CONNECTIVITY_PROFILE {
    pub pszXmlFragmentIhvConnectivity: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for DOT11EXT_IHV_CONNECTIVITY_PROFILE {}
impl ::core::clone::Clone for DOT11EXT_IHV_CONNECTIVITY_PROFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11EXT_IHV_DISCOVERY_PROFILE {
    pub IhvConnectivityProfile: DOT11EXT_IHV_CONNECTIVITY_PROFILE,
    pub IhvSecurityProfile: DOT11EXT_IHV_SECURITY_PROFILE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11EXT_IHV_DISCOVERY_PROFILE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11EXT_IHV_DISCOVERY_PROFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11EXT_IHV_DISCOVERY_PROFILE_LIST {
    pub dwCount: u32,
    pub pIhvDiscoveryProfiles: *mut DOT11EXT_IHV_DISCOVERY_PROFILE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11EXT_IHV_DISCOVERY_PROFILE_LIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11EXT_IHV_DISCOVERY_PROFILE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`, `\"Win32_System_RemoteDesktop\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Security_ExtensibleAuthenticationProtocol", feature = "Win32_System_RemoteDesktop"))]
pub struct DOT11EXT_IHV_HANDLERS {
    pub Dot11ExtIhvDeinitService: DOT11EXTIHV_DEINIT_SERVICE,
    pub Dot11ExtIhvInitAdapter: DOT11EXTIHV_INIT_ADAPTER,
    pub Dot11ExtIhvDeinitAdapter: DOT11EXTIHV_DEINIT_ADAPTER,
    pub Dot11ExtIhvPerformPreAssociate: DOT11EXTIHV_PERFORM_PRE_ASSOCIATE,
    pub Dot11ExtIhvAdapterReset: DOT11EXTIHV_ADAPTER_RESET,
    pub Dot11ExtIhvPerformPostAssociate: DOT11EXTIHV_PERFORM_POST_ASSOCIATE,
    pub Dot11ExtIhvStopPostAssociate: DOT11EXTIHV_STOP_POST_ASSOCIATE,
    pub Dot11ExtIhvValidateProfile: DOT11EXTIHV_VALIDATE_PROFILE,
    pub Dot11ExtIhvPerformCapabilityMatch: DOT11EXTIHV_PERFORM_CAPABILITY_MATCH,
    pub Dot11ExtIhvCreateDiscoveryProfiles: DOT11EXTIHV_CREATE_DISCOVERY_PROFILES,
    pub Dot11ExtIhvProcessSessionChange: DOT11EXTIHV_PROCESS_SESSION_CHANGE,
    pub Dot11ExtIhvReceiveIndication: DOT11EXTIHV_RECEIVE_INDICATION,
    pub Dot11ExtIhvReceivePacket: DOT11EXTIHV_RECEIVE_PACKET,
    pub Dot11ExtIhvSendPacketCompletion: DOT11EXTIHV_SEND_PACKET_COMPLETION,
    pub Dot11ExtIhvIsUIRequestPending: DOT11EXTIHV_IS_UI_REQUEST_PENDING,
    pub Dot11ExtIhvProcessUIResponse: DOT11EXTIHV_PROCESS_UI_RESPONSE,
    pub Dot11ExtIhvQueryUIRequest: DOT11EXTIHV_QUERY_UI_REQUEST,
    pub Dot11ExtIhvOnexIndicateResult: DOT11EXTIHV_ONEX_INDICATE_RESULT,
    pub Dot11ExtIhvControl: DOT11EXTIHV_CONTROL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Security_ExtensibleAuthenticationProtocol", feature = "Win32_System_RemoteDesktop"))]
impl ::core::marker::Copy for DOT11EXT_IHV_HANDLERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Security_ExtensibleAuthenticationProtocol", feature = "Win32_System_RemoteDesktop"))]
impl ::core::clone::Clone for DOT11EXT_IHV_HANDLERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub struct DOT11EXT_IHV_PARAMS {
    pub dot11ExtIhvProfileParams: DOT11EXT_IHV_PROFILE_PARAMS,
    pub wstrProfileName: [u16; 256],
    pub dwProfileTypeFlags: u32,
    pub interfaceGuid: ::windows_sys::core::GUID,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::marker::Copy for DOT11EXT_IHV_PARAMS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::clone::Clone for DOT11EXT_IHV_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub struct DOT11EXT_IHV_PROFILE_PARAMS {
    pub pSsidList: *mut DOT11EXT_IHV_SSID_LIST,
    pub BssType: DOT11_BSS_TYPE,
    pub pMSSecuritySettings: *mut DOT11_MSSECURITY_SETTINGS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::marker::Copy for DOT11EXT_IHV_PROFILE_PARAMS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::clone::Clone for DOT11EXT_IHV_PROFILE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11EXT_IHV_SECURITY_PROFILE {
    pub pszXmlFragmentIhvSecurity: ::windows_sys::core::PWSTR,
    pub bUseMSOnex: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11EXT_IHV_SECURITY_PROFILE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11EXT_IHV_SECURITY_PROFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11EXT_IHV_SSID_LIST {
    pub ulCount: u32,
    pub SSIDs: [DOT11_SSID; 1],
}
impl ::core::marker::Copy for DOT11EXT_IHV_SSID_LIST {}
impl ::core::clone::Clone for DOT11EXT_IHV_SSID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11EXT_IHV_UI_REQUEST {
    pub dwSessionId: u32,
    pub guidUIRequest: ::windows_sys::core::GUID,
    pub UIPageClsid: ::windows_sys::core::GUID,
    pub dwByteCount: u32,
    pub pvUIRequest: *mut u8,
}
impl ::core::marker::Copy for DOT11EXT_IHV_UI_REQUEST {}
impl ::core::clone::Clone for DOT11EXT_IHV_UI_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11EXT_VIRTUAL_STATION_APIS {
    pub Dot11ExtRequestVirtualStation: DOT11EXT_REQUEST_VIRTUAL_STATION,
    pub Dot11ExtReleaseVirtualStation: DOT11EXT_RELEASE_VIRTUAL_STATION,
    pub Dot11ExtQueryVirtualStationProperties: DOT11EXT_QUERY_VIRTUAL_STATION_PROPERTIES,
    pub Dot11ExtSetVirtualStationAPProperties: DOT11EXT_SET_VIRTUAL_STATION_AP_PROPERTIES,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11EXT_VIRTUAL_STATION_APIS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11EXT_VIRTUAL_STATION_APIS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11EXT_VIRTUAL_STATION_AP_PROPERTY {
    pub dot11SSID: DOT11_SSID,
    pub dot11AuthAlgo: DOT11_AUTH_ALGORITHM,
    pub dot11CipherAlgo: DOT11_CIPHER_ALGORITHM,
    pub bIsPassPhrase: super::super::Foundation::BOOL,
    pub dwKeyLength: u32,
    pub ucKeyData: [u8; 64],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11EXT_VIRTUAL_STATION_AP_PROPERTY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11EXT_VIRTUAL_STATION_AP_PROPERTY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_ACCESSNETWORKOPTIONS {
    pub AccessNetworkType: u8,
    pub Internet: u8,
    pub ASRA: u8,
    pub ESR: u8,
    pub UESA: u8,
}
impl ::core::marker::Copy for DOT11_ACCESSNETWORKOPTIONS {}
impl ::core::clone::Clone for DOT11_ACCESSNETWORKOPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_ADAPTER {
    pub gAdapterId: ::windows_sys::core::GUID,
    pub pszDescription: ::windows_sys::core::PWSTR,
    pub Dot11CurrentOpMode: DOT11_CURRENT_OPERATION_MODE,
}
impl ::core::marker::Copy for DOT11_ADAPTER {}
impl ::core::clone::Clone for DOT11_ADAPTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_ADDITIONAL_IE {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uBeaconIEsOffset: u32,
    pub uBeaconIEsLength: u32,
    pub uResponseIEsOffset: u32,
    pub uResponseIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_ADDITIONAL_IE {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_ADDITIONAL_IE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_ANQP_QUERY_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub Status: DOT11_ANQP_QUERY_RESULT,
    pub hContext: super::super::Foundation::HANDLE,
    pub uResponseLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_ANQP_QUERY_COMPLETE_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_ANQP_QUERY_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_AP_JOIN_REQUEST {
    pub uJoinFailureTimeout: u32,
    pub OperationalRateSet: DOT11_RATE_SET,
    pub uChCenterFrequency: u32,
    pub dot11BSSDescription: DOT11_BSS_DESCRIPTION,
}
impl ::core::marker::Copy for DOT11_AP_JOIN_REQUEST {}
impl ::core::clone::Clone for DOT11_AP_JOIN_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_ASSOCIATION_COMPLETION_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub MacAddr: [u8; 6],
    pub uStatus: u32,
    pub bReAssocReq: super::super::Foundation::BOOLEAN,
    pub bReAssocResp: super::super::Foundation::BOOLEAN,
    pub uAssocReqOffset: u32,
    pub uAssocReqSize: u32,
    pub uAssocRespOffset: u32,
    pub uAssocRespSize: u32,
    pub uBeaconOffset: u32,
    pub uBeaconSize: u32,
    pub uIHVDataOffset: u32,
    pub uIHVDataSize: u32,
    pub AuthAlgo: DOT11_AUTH_ALGORITHM,
    pub UnicastCipher: DOT11_CIPHER_ALGORITHM,
    pub MulticastCipher: DOT11_CIPHER_ALGORITHM,
    pub uActivePhyListOffset: u32,
    pub uActivePhyListSize: u32,
    pub bFourAddressSupported: super::super::Foundation::BOOLEAN,
    pub bPortAuthorized: super::super::Foundation::BOOLEAN,
    pub ucActiveQoSProtocol: u8,
    pub DSInfo: DOT11_DS_INFO,
    pub uEncapTableOffset: u32,
    pub uEncapTableSize: u32,
    pub MulticastMgmtCipher: DOT11_CIPHER_ALGORITHM,
    pub uAssocComebackTime: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_ASSOCIATION_COMPLETION_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_ASSOCIATION_COMPLETION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_ASSOCIATION_INFO_EX {
    pub PeerMacAddress: [u8; 6],
    pub BSSID: [u8; 6],
    pub usCapabilityInformation: u16,
    pub usListenInterval: u16,
    pub ucPeerSupportedRates: [u8; 255],
    pub usAssociationID: u16,
    pub dot11AssociationState: DOT11_ASSOCIATION_STATE,
    pub dot11PowerMode: DOT11_POWER_MODE,
    pub liAssociationUpTime: i64,
    pub ullNumOfTxPacketSuccesses: u64,
    pub ullNumOfTxPacketFailures: u64,
    pub ullNumOfRxPacketSuccesses: u64,
    pub ullNumOfRxPacketFailures: u64,
}
impl ::core::marker::Copy for DOT11_ASSOCIATION_INFO_EX {}
impl ::core::clone::Clone for DOT11_ASSOCIATION_INFO_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_ASSOCIATION_INFO_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11AssocInfo: [DOT11_ASSOCIATION_INFO_EX; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_ASSOCIATION_INFO_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_ASSOCIATION_INFO_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_ASSOCIATION_PARAMS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub BSSID: [u8; 6],
    pub uAssocRequestIEsOffset: u32,
    pub uAssocRequestIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_ASSOCIATION_PARAMS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_ASSOCIATION_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_ASSOCIATION_START_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub MacAddr: [u8; 6],
    pub SSID: DOT11_SSID,
    pub uIHVDataOffset: u32,
    pub uIHVDataSize: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_ASSOCIATION_START_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_ASSOCIATION_START_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_AUTH_ALGORITHM_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub AlgorithmIds: [DOT11_AUTH_ALGORITHM; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_AUTH_ALGORITHM_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_AUTH_ALGORITHM_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_AUTH_CIPHER_PAIR {
    pub AuthAlgoId: DOT11_AUTH_ALGORITHM,
    pub CipherAlgoId: DOT11_CIPHER_ALGORITHM,
}
impl ::core::marker::Copy for DOT11_AUTH_CIPHER_PAIR {}
impl ::core::clone::Clone for DOT11_AUTH_CIPHER_PAIR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_AUTH_CIPHER_PAIR_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub AuthCipherPairs: [DOT11_AUTH_CIPHER_PAIR; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_AUTH_CIPHER_PAIR_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_AUTH_CIPHER_PAIR_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_AVAILABLE_CHANNEL_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub uChannelNumber: [u32; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_AVAILABLE_CHANNEL_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_AVAILABLE_CHANNEL_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_AVAILABLE_FREQUENCY_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub uFrequencyValue: [u32; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_AVAILABLE_FREQUENCY_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_AVAILABLE_FREQUENCY_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_BSSID_CANDIDATE {
    pub BSSID: [u8; 6],
    pub uFlags: u32,
}
impl ::core::marker::Copy for DOT11_BSSID_CANDIDATE {}
impl ::core::clone::Clone for DOT11_BSSID_CANDIDATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_BSSID_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub BSSIDs: [u8; 6],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_BSSID_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_BSSID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_BSS_DESCRIPTION {
    pub uReserved: u32,
    pub dot11BSSID: [u8; 6],
    pub dot11BSSType: DOT11_BSS_TYPE,
    pub usBeaconPeriod: u16,
    pub ullTimestamp: u64,
    pub usCapabilityInformation: u16,
    pub uBufferLength: u32,
    pub ucBuffer: [u8; 1],
}
impl ::core::marker::Copy for DOT11_BSS_DESCRIPTION {}
impl ::core::clone::Clone for DOT11_BSS_DESCRIPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_BSS_ENTRY {
    pub uPhyId: u32,
    pub PhySpecificInfo: DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO,
    pub dot11BSSID: [u8; 6],
    pub dot11BSSType: DOT11_BSS_TYPE,
    pub lRSSI: i32,
    pub uLinkQuality: u32,
    pub bInRegDomain: super::super::Foundation::BOOLEAN,
    pub usBeaconPeriod: u16,
    pub ullTimestamp: u64,
    pub ullHostTimestamp: u64,
    pub usCapabilityInformation: u16,
    pub uBufferLength: u32,
    pub ucBuffer: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_BSS_ENTRY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_BSS_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO {
    pub uChCenterFrequency: u32,
    pub FHSS: DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO_0,
}
impl ::core::marker::Copy for DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO {}
impl ::core::clone::Clone for DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO_0 {
    pub uHopPattern: u32,
    pub uHopSet: u32,
    pub uDwellTime: u32,
}
impl ::core::marker::Copy for DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO_0 {}
impl ::core::clone::Clone for DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_BSS_LIST {
    pub uNumOfBytes: u32,
    pub pucBuffer: *mut u8,
}
impl ::core::marker::Copy for DOT11_BSS_LIST {}
impl ::core::clone::Clone for DOT11_BSS_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_BYTE_ARRAY {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfBytes: u32,
    pub uTotalNumOfBytes: u32,
    pub ucBuffer: [u8; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_BYTE_ARRAY {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_BYTE_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_CAN_SUSTAIN_AP_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ulReason: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_CAN_SUSTAIN_AP_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_CAN_SUSTAIN_AP_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_CHANNEL_HINT {
    pub Dot11PhyType: DOT11_PHY_TYPE,
    pub uChannelNumber: u32,
}
impl ::core::marker::Copy for DOT11_CHANNEL_HINT {}
impl ::core::clone::Clone for DOT11_CHANNEL_HINT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_CIPHER_ALGORITHM_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub AlgorithmIds: [DOT11_CIPHER_ALGORITHM; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_CIPHER_ALGORITHM_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_CIPHER_ALGORITHM_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_CIPHER_DEFAULT_KEY_VALUE {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uKeyIndex: u32,
    pub AlgorithmId: DOT11_CIPHER_ALGORITHM,
    pub MacAddr: [u8; 6],
    pub bDelete: super::super::Foundation::BOOLEAN,
    pub bStatic: super::super::Foundation::BOOLEAN,
    pub usKeyLength: u16,
    pub ucKey: [u8; 1],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_CIPHER_DEFAULT_KEY_VALUE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_CIPHER_DEFAULT_KEY_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_CIPHER_KEY_MAPPING_KEY_VALUE {
    pub PeerMacAddr: [u8; 6],
    pub AlgorithmId: DOT11_CIPHER_ALGORITHM,
    pub Direction: DOT11_DIRECTION,
    pub bDelete: super::super::Foundation::BOOLEAN,
    pub bStatic: super::super::Foundation::BOOLEAN,
    pub usKeyLength: u16,
    pub ucKey: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_CIPHER_KEY_MAPPING_KEY_VALUE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_CIPHER_KEY_MAPPING_KEY_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_CONNECTION_COMPLETION_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uStatus: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_CONNECTION_COMPLETION_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_CONNECTION_COMPLETION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_CONNECTION_START_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub BSSType: DOT11_BSS_TYPE,
    pub AdhocBSSID: [u8; 6],
    pub AdhocSSID: DOT11_SSID,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_CONNECTION_START_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_CONNECTION_START_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_COUNTERS_ENTRY {
    pub uTransmittedFragmentCount: u32,
    pub uMulticastTransmittedFrameCount: u32,
    pub uFailedCount: u32,
    pub uRetryCount: u32,
    pub uMultipleRetryCount: u32,
    pub uFrameDuplicateCount: u32,
    pub uRTSSuccessCount: u32,
    pub uRTSFailureCount: u32,
    pub uACKFailureCount: u32,
    pub uReceivedFragmentCount: u32,
    pub uMulticastReceivedFrameCount: u32,
    pub uFCSErrorCount: u32,
    pub uTransmittedFrameCount: u32,
}
impl ::core::marker::Copy for DOT11_COUNTERS_ENTRY {}
impl ::core::clone::Clone for DOT11_COUNTERS_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_COUNTRY_OR_REGION_STRING_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub CountryOrRegionStrings: [u8; 3],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_COUNTRY_OR_REGION_STRING_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_COUNTRY_OR_REGION_STRING_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_CURRENT_OFFLOAD_CAPABILITY {
    pub uReserved: u32,
    pub uFlags: u32,
}
impl ::core::marker::Copy for DOT11_CURRENT_OFFLOAD_CAPABILITY {}
impl ::core::clone::Clone for DOT11_CURRENT_OFFLOAD_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_CURRENT_OPERATION_MODE {
    pub uReserved: u32,
    pub uCurrentOpMode: u32,
}
impl ::core::marker::Copy for DOT11_CURRENT_OPERATION_MODE {}
impl ::core::clone::Clone for DOT11_CURRENT_OPERATION_MODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_CURRENT_OPTIONAL_CAPABILITY {
    pub uReserved: u32,
    pub bDot11CFPollable: super::super::Foundation::BOOLEAN,
    pub bDot11PCF: super::super::Foundation::BOOLEAN,
    pub bDot11PCFMPDUTransferToPC: super::super::Foundation::BOOLEAN,
    pub bStrictlyOrderedServiceClass: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_CURRENT_OPTIONAL_CAPABILITY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_CURRENT_OPTIONAL_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_DATA_RATE_MAPPING_ENTRY {
    pub ucDataRateIndex: u8,
    pub ucDataRateFlag: u8,
    pub usDataRateValue: u16,
}
impl ::core::marker::Copy for DOT11_DATA_RATE_MAPPING_ENTRY {}
impl ::core::clone::Clone for DOT11_DATA_RATE_MAPPING_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_DATA_RATE_MAPPING_TABLE {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uDataRateMappingLength: u32,
    pub DataRateMappingEntries: [DOT11_DATA_RATE_MAPPING_ENTRY; 126],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_DATA_RATE_MAPPING_TABLE {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_DATA_RATE_MAPPING_TABLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_DEFAULT_WEP_OFFLOAD {
    pub uReserved: u32,
    pub hOffloadContext: super::super::Foundation::HANDLE,
    pub hOffload: super::super::Foundation::HANDLE,
    pub dwIndex: u32,
    pub dot11OffloadType: DOT11_OFFLOAD_TYPE,
    pub dwAlgorithm: u32,
    pub uFlags: u32,
    pub dot11KeyDirection: DOT11_KEY_DIRECTION,
    pub ucMacAddress: [u8; 6],
    pub uNumOfRWsOnMe: u32,
    pub dot11IV48Counters: [DOT11_IV48_COUNTER; 16],
    pub usDot11RWBitMaps: [u16; 16],
    pub usKeyLength: u16,
    pub ucKey: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_DEFAULT_WEP_OFFLOAD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_DEFAULT_WEP_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_DEFAULT_WEP_UPLOAD {
    pub uReserved: u32,
    pub dot11OffloadType: DOT11_OFFLOAD_TYPE,
    pub hOffload: super::super::Foundation::HANDLE,
    pub uNumOfRWsUsed: u32,
    pub dot11IV48Counters: [DOT11_IV48_COUNTER; 16],
    pub usDot11RWBitMaps: [u16; 16],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_DEFAULT_WEP_UPLOAD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_DEFAULT_WEP_UPLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_DISASSOCIATE_PEER_REQUEST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerMacAddr: [u8; 6],
    pub usReason: u16,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_DISASSOCIATE_PEER_REQUEST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_DISASSOCIATE_PEER_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_DISASSOCIATION_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub MacAddr: [u8; 6],
    pub uReason: u32,
    pub uIHVDataOffset: u32,
    pub uIHVDataSize: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_DISASSOCIATION_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_DISASSOCIATION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_DIVERSITY_SELECTION_RX {
    pub uAntennaListIndex: u32,
    pub bDiversitySelectionRX: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_DIVERSITY_SELECTION_RX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_DIVERSITY_SELECTION_RX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_DIVERSITY_SELECTION_RX_LIST {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11DiversitySelectionRx: [DOT11_DIVERSITY_SELECTION_RX; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_DIVERSITY_SELECTION_RX_LIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_DIVERSITY_SELECTION_RX_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
pub struct DOT11_EAP_RESULT {
    pub dwFailureReasonCode: u32,
    pub pAttribArray: *mut super::super::Security::ExtensibleAuthenticationProtocol::EAP_ATTRIBUTES,
}
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
impl ::core::marker::Copy for DOT11_EAP_RESULT {}
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
impl ::core::clone::Clone for DOT11_EAP_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_ENCAP_ENTRY {
    pub usEtherType: u16,
    pub usEncapType: u16,
}
impl ::core::marker::Copy for DOT11_ENCAP_ENTRY {}
impl ::core::clone::Clone for DOT11_ENCAP_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_ERP_PHY_ATTRIBUTES {
    pub HRDSSSAttributes: DOT11_HRDSSS_PHY_ATTRIBUTES,
    pub bERPPBCCOptionImplemented: super::super::Foundation::BOOLEAN,
    pub bDSSSOFDMOptionImplemented: super::super::Foundation::BOOLEAN,
    pub bShortSlotTimeOptionImplemented: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_ERP_PHY_ATTRIBUTES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_ERP_PHY_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_EXTAP_ATTRIBUTES {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uScanSSIDListSize: u32,
    pub uDesiredSSIDListSize: u32,
    pub uPrivacyExemptionListSize: u32,
    pub uAssociationTableSize: u32,
    pub uDefaultKeyTableSize: u32,
    pub uWEPKeyValueMaxLength: u32,
    pub bStrictlyOrderedServiceClassImplemented: super::super::Foundation::BOOLEAN,
    pub uNumSupportedCountryOrRegionStrings: u32,
    pub pSupportedCountryOrRegionStrings: *mut u8,
    pub uInfraNumSupportedUcastAlgoPairs: u32,
    pub pInfraSupportedUcastAlgoPairs: *mut DOT11_AUTH_CIPHER_PAIR,
    pub uInfraNumSupportedMcastAlgoPairs: u32,
    pub pInfraSupportedMcastAlgoPairs: *mut DOT11_AUTH_CIPHER_PAIR,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_EXTAP_ATTRIBUTES {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_EXTAP_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_EXTSTA_ATTRIBUTES {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uScanSSIDListSize: u32,
    pub uDesiredBSSIDListSize: u32,
    pub uDesiredSSIDListSize: u32,
    pub uExcludedMacAddressListSize: u32,
    pub uPrivacyExemptionListSize: u32,
    pub uKeyMappingTableSize: u32,
    pub uDefaultKeyTableSize: u32,
    pub uWEPKeyValueMaxLength: u32,
    pub uPMKIDCacheSize: u32,
    pub uMaxNumPerSTADefaultKeyTables: u32,
    pub bStrictlyOrderedServiceClassImplemented: super::super::Foundation::BOOLEAN,
    pub ucSupportedQoSProtocolFlags: u8,
    pub bSafeModeImplemented: super::super::Foundation::BOOLEAN,
    pub uNumSupportedCountryOrRegionStrings: u32,
    pub pSupportedCountryOrRegionStrings: *mut u8,
    pub uInfraNumSupportedUcastAlgoPairs: u32,
    pub pInfraSupportedUcastAlgoPairs: *mut DOT11_AUTH_CIPHER_PAIR,
    pub uInfraNumSupportedMcastAlgoPairs: u32,
    pub pInfraSupportedMcastAlgoPairs: *mut DOT11_AUTH_CIPHER_PAIR,
    pub uAdhocNumSupportedUcastAlgoPairs: u32,
    pub pAdhocSupportedUcastAlgoPairs: *mut DOT11_AUTH_CIPHER_PAIR,
    pub uAdhocNumSupportedMcastAlgoPairs: u32,
    pub pAdhocSupportedMcastAlgoPairs: *mut DOT11_AUTH_CIPHER_PAIR,
    pub bAutoPowerSaveMode: super::super::Foundation::BOOLEAN,
    pub uMaxNetworkOffloadListSize: u32,
    pub bMFPCapable: super::super::Foundation::BOOLEAN,
    pub uInfraNumSupportedMcastMgmtAlgoPairs: u32,
    pub pInfraSupportedMcastMgmtAlgoPairs: *mut DOT11_AUTH_CIPHER_PAIR,
    pub bNeighborReportSupported: super::super::Foundation::BOOLEAN,
    pub bAPChannelReportSupported: super::super::Foundation::BOOLEAN,
    pub bActionFramesSupported: super::super::Foundation::BOOLEAN,
    pub bANQPQueryOffloadSupported: super::super::Foundation::BOOLEAN,
    pub bHESSIDConnectionSupported: super::super::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_EXTSTA_ATTRIBUTES {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_EXTSTA_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_EXTSTA_CAPABILITY {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uScanSSIDListSize: u32,
    pub uDesiredBSSIDListSize: u32,
    pub uDesiredSSIDListSize: u32,
    pub uExcludedMacAddressListSize: u32,
    pub uPrivacyExemptionListSize: u32,
    pub uKeyMappingTableSize: u32,
    pub uDefaultKeyTableSize: u32,
    pub uWEPKeyValueMaxLength: u32,
    pub uPMKIDCacheSize: u32,
    pub uMaxNumPerSTADefaultKeyTables: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_EXTSTA_CAPABILITY {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_EXTSTA_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_EXTSTA_RECV_CONTEXT {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uReceiveFlags: u32,
    pub uPhyId: u32,
    pub uChCenterFrequency: u32,
    pub usNumberOfMPDUsReceived: u16,
    pub lRSSI: i32,
    pub ucDataRate: u8,
    pub uSizeMediaSpecificInfo: u32,
    pub pvMediaSpecificInfo: *mut ::core::ffi::c_void,
    pub ullTimestamp: u64,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_EXTSTA_RECV_CONTEXT {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_EXTSTA_RECV_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_EXTSTA_SEND_CONTEXT {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub usExemptionActionType: u16,
    pub uPhyId: u32,
    pub uDelayedSleepValue: u32,
    pub pvMediaSpecificInfo: *mut ::core::ffi::c_void,
    pub uSendFlags: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_EXTSTA_SEND_CONTEXT {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_EXTSTA_SEND_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_FRAGMENT_DESCRIPTOR {
    pub uOffset: u32,
    pub uLength: u32,
}
impl ::core::marker::Copy for DOT11_FRAGMENT_DESCRIPTOR {}
impl ::core::clone::Clone for DOT11_FRAGMENT_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_GO_NEGOTIATION_CONFIRMATION_SEND_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub Status: i32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_GO_NEGOTIATION_CONFIRMATION_SEND_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_GO_NEGOTIATION_CONFIRMATION_SEND_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_GO_NEGOTIATION_REQUEST_SEND_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub Status: i32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_GO_NEGOTIATION_REQUEST_SEND_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_GO_NEGOTIATION_REQUEST_SEND_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_GO_NEGOTIATION_RESPONSE_SEND_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub Status: i32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_GO_NEGOTIATION_RESPONSE_SEND_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_GO_NEGOTIATION_RESPONSE_SEND_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_HOPPING_PATTERN_ENTRY {
    pub uHoppingPatternIndex: u32,
    pub uRandomTableFieldNumber: u32,
}
impl ::core::marker::Copy for DOT11_HOPPING_PATTERN_ENTRY {}
impl ::core::clone::Clone for DOT11_HOPPING_PATTERN_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_HOPPING_PATTERN_ENTRY_LIST {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11HoppingPatternEntry: [DOT11_HOPPING_PATTERN_ENTRY; 1],
}
impl ::core::marker::Copy for DOT11_HOPPING_PATTERN_ENTRY_LIST {}
impl ::core::clone::Clone for DOT11_HOPPING_PATTERN_ENTRY_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_HRDSSS_PHY_ATTRIBUTES {
    pub bShortPreambleOptionImplemented: super::super::Foundation::BOOLEAN,
    pub bPBCCOptionImplemented: super::super::Foundation::BOOLEAN,
    pub bChannelAgilityPresent: super::super::Foundation::BOOLEAN,
    pub uHRCCAModeSupported: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_HRDSSS_PHY_ATTRIBUTES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_HRDSSS_PHY_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_IBSS_PARAMS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub bJoinOnly: super::super::Foundation::BOOLEAN,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_IBSS_PARAMS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_IBSS_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_IHV_VERSION_INFO {
    pub dwVerMin: u32,
    pub dwVerMax: u32,
}
impl ::core::marker::Copy for DOT11_IHV_VERSION_INFO {}
impl ::core::clone::Clone for DOT11_IHV_VERSION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_INCOMING_ASSOC_COMPLETION_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerMacAddr: [u8; 6],
    pub uStatus: u32,
    pub ucErrorSource: u8,
    pub bReAssocReq: super::super::Foundation::BOOLEAN,
    pub bReAssocResp: super::super::Foundation::BOOLEAN,
    pub uAssocReqOffset: u32,
    pub uAssocReqSize: u32,
    pub uAssocRespOffset: u32,
    pub uAssocRespSize: u32,
    pub AuthAlgo: DOT11_AUTH_ALGORITHM,
    pub UnicastCipher: DOT11_CIPHER_ALGORITHM,
    pub MulticastCipher: DOT11_CIPHER_ALGORITHM,
    pub uActivePhyListOffset: u32,
    pub uActivePhyListSize: u32,
    pub uBeaconOffset: u32,
    pub uBeaconSize: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_INCOMING_ASSOC_COMPLETION_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_INCOMING_ASSOC_COMPLETION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_INCOMING_ASSOC_DECISION {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerMacAddr: [u8; 6],
    pub bAccept: super::super::Foundation::BOOLEAN,
    pub usReasonCode: u16,
    pub uAssocResponseIEsOffset: u32,
    pub uAssocResponseIEsLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_INCOMING_ASSOC_DECISION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_INCOMING_ASSOC_DECISION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_INCOMING_ASSOC_DECISION_V2 {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerMacAddr: [u8; 6],
    pub bAccept: super::super::Foundation::BOOLEAN,
    pub usReasonCode: u16,
    pub uAssocResponseIEsOffset: u32,
    pub uAssocResponseIEsLength: u32,
    pub WFDStatus: u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_INCOMING_ASSOC_DECISION_V2 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_INCOMING_ASSOC_DECISION_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_INCOMING_ASSOC_REQUEST_RECEIVED_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerMacAddr: [u8; 6],
    pub bReAssocReq: super::super::Foundation::BOOLEAN,
    pub uAssocReqOffset: u32,
    pub uAssocReqSize: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_INCOMING_ASSOC_REQUEST_RECEIVED_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_INCOMING_ASSOC_REQUEST_RECEIVED_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_INCOMING_ASSOC_STARTED_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerMacAddr: [u8; 6],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_INCOMING_ASSOC_STARTED_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_INCOMING_ASSOC_STARTED_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_INVITATION_REQUEST_SEND_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub ReceiverAddress: [u8; 6],
    pub DialogToken: u8,
    pub Status: i32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_INVITATION_REQUEST_SEND_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_INVITATION_REQUEST_SEND_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_INVITATION_RESPONSE_SEND_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ReceiverDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub Status: i32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_INVITATION_RESPONSE_SEND_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_INVITATION_RESPONSE_SEND_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_IV48_COUNTER {
    pub uIV32Counter: u32,
    pub usIV16Counter: u16,
}
impl ::core::marker::Copy for DOT11_IV48_COUNTER {}
impl ::core::clone::Clone for DOT11_IV48_COUNTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_JOIN_REQUEST {
    pub uJoinFailureTimeout: u32,
    pub OperationalRateSet: DOT11_RATE_SET,
    pub uChCenterFrequency: u32,
    pub dot11BSSDescription: DOT11_BSS_DESCRIPTION,
}
impl ::core::marker::Copy for DOT11_JOIN_REQUEST {}
impl ::core::clone::Clone for DOT11_JOIN_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_KEY_ALGO_BIP {
    pub ucIPN: [u8; 6],
    pub ulBIPKeyLength: u32,
    pub ucBIPKey: [u8; 1],
}
impl ::core::marker::Copy for DOT11_KEY_ALGO_BIP {}
impl ::core::clone::Clone for DOT11_KEY_ALGO_BIP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_KEY_ALGO_BIP_GMAC_256 {
    pub ucIPN: [u8; 6],
    pub ulBIPGmac256KeyLength: u32,
    pub ucBIPGmac256Key: [u8; 1],
}
impl ::core::marker::Copy for DOT11_KEY_ALGO_BIP_GMAC_256 {}
impl ::core::clone::Clone for DOT11_KEY_ALGO_BIP_GMAC_256 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_KEY_ALGO_CCMP {
    pub ucIV48Counter: [u8; 6],
    pub ulCCMPKeyLength: u32,
    pub ucCCMPKey: [u8; 1],
}
impl ::core::marker::Copy for DOT11_KEY_ALGO_CCMP {}
impl ::core::clone::Clone for DOT11_KEY_ALGO_CCMP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_KEY_ALGO_GCMP {
    pub ucIV48Counter: [u8; 6],
    pub ulGCMPKeyLength: u32,
    pub ucGCMPKey: [u8; 1],
}
impl ::core::marker::Copy for DOT11_KEY_ALGO_GCMP {}
impl ::core::clone::Clone for DOT11_KEY_ALGO_GCMP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_KEY_ALGO_GCMP_256 {
    pub ucIV48Counter: [u8; 6],
    pub ulGCMP256KeyLength: u32,
    pub ucGCMP256Key: [u8; 1],
}
impl ::core::marker::Copy for DOT11_KEY_ALGO_GCMP_256 {}
impl ::core::clone::Clone for DOT11_KEY_ALGO_GCMP_256 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_KEY_ALGO_TKIP_MIC {
    pub ucIV48Counter: [u8; 6],
    pub ulTKIPKeyLength: u32,
    pub ulMICKeyLength: u32,
    pub ucTKIPMICKeys: [u8; 1],
}
impl ::core::marker::Copy for DOT11_KEY_ALGO_TKIP_MIC {}
impl ::core::clone::Clone for DOT11_KEY_ALGO_TKIP_MIC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_LINK_QUALITY_ENTRY {
    pub PeerMacAddr: [u8; 6],
    pub ucLinkQuality: u8,
}
impl ::core::marker::Copy for DOT11_LINK_QUALITY_ENTRY {}
impl ::core::clone::Clone for DOT11_LINK_QUALITY_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_LINK_QUALITY_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uLinkQualityListSize: u32,
    pub uLinkQualityListOffset: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_LINK_QUALITY_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_LINK_QUALITY_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_MAC_ADDRESS_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub MacAddrs: [u8; 6],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_MAC_ADDRESS_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_MAC_ADDRESS_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MAC_FRAME_STATISTICS {
    pub ullTransmittedFrameCount: u64,
    pub ullReceivedFrameCount: u64,
    pub ullTransmittedFailureFrameCount: u64,
    pub ullReceivedFailureFrameCount: u64,
    pub ullWEPExcludedCount: u64,
    pub ullTKIPLocalMICFailures: u64,
    pub ullTKIPReplays: u64,
    pub ullTKIPICVErrorCount: u64,
    pub ullCCMPReplays: u64,
    pub ullCCMPDecryptErrors: u64,
    pub ullWEPUndecryptableCount: u64,
    pub ullWEPICVErrorCount: u64,
    pub ullDecryptSuccessCount: u64,
    pub ullDecryptFailureCount: u64,
}
impl ::core::marker::Copy for DOT11_MAC_FRAME_STATISTICS {}
impl ::core::clone::Clone for DOT11_MAC_FRAME_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MAC_INFO {
    pub uReserved: u32,
    pub uNdisPortNumber: u32,
    pub MacAddr: [u8; 6],
}
impl ::core::marker::Copy for DOT11_MAC_INFO {}
impl ::core::clone::Clone for DOT11_MAC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_MAC_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uOpmodeMask: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_MAC_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_MAC_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_MANUFACTURING_CALLBACK_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub dot11ManufacturingCallbackType: DOT11_MANUFACTURING_CALLBACK_TYPE,
    pub uStatus: u32,
    pub pvContext: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_MANUFACTURING_CALLBACK_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_MANUFACTURING_CALLBACK_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MANUFACTURING_FUNCTIONAL_TEST_QUERY_ADC {
    pub Dot11Band: DOT11_BAND,
    pub uChannel: u32,
    pub ADCPowerLevel: i32,
}
impl ::core::marker::Copy for DOT11_MANUFACTURING_FUNCTIONAL_TEST_QUERY_ADC {}
impl ::core::clone::Clone for DOT11_MANUFACTURING_FUNCTIONAL_TEST_QUERY_ADC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_MANUFACTURING_FUNCTIONAL_TEST_RX {
    pub bEnabled: super::super::Foundation::BOOLEAN,
    pub Dot11Band: DOT11_BAND,
    pub uChannel: u32,
    pub PowerLevel: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_MANUFACTURING_FUNCTIONAL_TEST_RX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_MANUFACTURING_FUNCTIONAL_TEST_RX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_MANUFACTURING_FUNCTIONAL_TEST_TX {
    pub bEnable: super::super::Foundation::BOOLEAN,
    pub bOpenLoop: super::super::Foundation::BOOLEAN,
    pub Dot11Band: DOT11_BAND,
    pub uChannel: u32,
    pub uSetPowerLevel: u32,
    pub ADCPowerLevel: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_MANUFACTURING_FUNCTIONAL_TEST_TX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_MANUFACTURING_FUNCTIONAL_TEST_TX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_MANUFACTURING_SELF_TEST_QUERY_RESULTS {
    pub SelfTestType: DOT11_MANUFACTURING_SELF_TEST_TYPE,
    pub uTestID: u32,
    pub bResult: super::super::Foundation::BOOLEAN,
    pub uPinFailedBitMask: u32,
    pub pvContext: *mut ::core::ffi::c_void,
    pub uBytesWrittenOut: u32,
    pub ucBufferOut: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_MANUFACTURING_SELF_TEST_QUERY_RESULTS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_MANUFACTURING_SELF_TEST_QUERY_RESULTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MANUFACTURING_SELF_TEST_SET_PARAMS {
    pub SelfTestType: DOT11_MANUFACTURING_SELF_TEST_TYPE,
    pub uTestID: u32,
    pub uPinBitMask: u32,
    pub pvContext: *mut ::core::ffi::c_void,
    pub uBufferLength: u32,
    pub ucBufferIn: [u8; 1],
}
impl ::core::marker::Copy for DOT11_MANUFACTURING_SELF_TEST_SET_PARAMS {}
impl ::core::clone::Clone for DOT11_MANUFACTURING_SELF_TEST_SET_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MANUFACTURING_TEST {
    pub dot11ManufacturingTestType: DOT11_MANUFACTURING_TEST_TYPE,
    pub uBufferLength: u32,
    pub ucBuffer: [u8; 1],
}
impl ::core::marker::Copy for DOT11_MANUFACTURING_TEST {}
impl ::core::clone::Clone for DOT11_MANUFACTURING_TEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MANUFACTURING_TEST_QUERY_DATA {
    pub uKey: u32,
    pub uOffset: u32,
    pub uBufferLength: u32,
    pub uBytesRead: u32,
    pub ucBufferOut: [u8; 1],
}
impl ::core::marker::Copy for DOT11_MANUFACTURING_TEST_QUERY_DATA {}
impl ::core::clone::Clone for DOT11_MANUFACTURING_TEST_QUERY_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MANUFACTURING_TEST_SET_DATA {
    pub uKey: u32,
    pub uOffset: u32,
    pub uBufferLength: u32,
    pub ucBufferIn: [u8; 1],
}
impl ::core::marker::Copy for DOT11_MANUFACTURING_TEST_SET_DATA {}
impl ::core::clone::Clone for DOT11_MANUFACTURING_TEST_SET_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MANUFACTURING_TEST_SLEEP {
    pub uSleepTime: u32,
    pub pvContext: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for DOT11_MANUFACTURING_TEST_SLEEP {}
impl ::core::clone::Clone for DOT11_MANUFACTURING_TEST_SLEEP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MD_CAPABILITY_ENTRY_LIST {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11MDCapabilityEntry: [DOT11_MULTI_DOMAIN_CAPABILITY_ENTRY; 1],
}
impl ::core::marker::Copy for DOT11_MD_CAPABILITY_ENTRY_LIST {}
impl ::core::clone::Clone for DOT11_MD_CAPABILITY_ENTRY_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_MPDU_MAX_LENGTH_INDICATION {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uPhyId: u32,
    pub uMPDUMaxLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_MPDU_MAX_LENGTH_INDICATION {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_MPDU_MAX_LENGTH_INDICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
pub struct DOT11_MSONEX_RESULT_PARAMS {
    pub Dot11OnexAuthStatus: ONEX_AUTH_STATUS,
    pub Dot11OneXReasonCode: ONEX_REASON_CODE,
    pub pbMPPESendKey: *mut u8,
    pub dwMPPESendKeyLen: u32,
    pub pbMPPERecvKey: *mut u8,
    pub dwMPPERecvKeyLen: u32,
    pub pDot11EapResult: *mut DOT11_EAP_RESULT,
}
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
impl ::core::marker::Copy for DOT11_MSONEX_RESULT_PARAMS {}
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
impl ::core::clone::Clone for DOT11_MSONEX_RESULT_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub struct DOT11_MSSECURITY_SETTINGS {
    pub dot11AuthAlgorithm: DOT11_AUTH_ALGORITHM,
    pub dot11CipherAlgorithm: DOT11_CIPHER_ALGORITHM,
    pub fOneXEnabled: super::super::Foundation::BOOL,
    pub eapMethodType: super::super::Security::ExtensibleAuthenticationProtocol::EAP_METHOD_TYPE,
    pub dwEapConnectionDataLen: u32,
    pub pEapConnectionData: *mut u8,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::marker::Copy for DOT11_MSSECURITY_SETTINGS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
impl ::core::clone::Clone for DOT11_MSSECURITY_SETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_MULTI_DOMAIN_CAPABILITY_ENTRY {
    pub uMultiDomainCapabilityIndex: u32,
    pub uFirstChannelNumber: u32,
    pub uNumberOfChannels: u32,
    pub lMaximumTransmitPowerLevel: i32,
}
impl ::core::marker::Copy for DOT11_MULTI_DOMAIN_CAPABILITY_ENTRY {}
impl ::core::clone::Clone for DOT11_MULTI_DOMAIN_CAPABILITY_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_NETWORK {
    pub dot11Ssid: DOT11_SSID,
    pub dot11BssType: DOT11_BSS_TYPE,
}
impl ::core::marker::Copy for DOT11_NETWORK {}
impl ::core::clone::Clone for DOT11_NETWORK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_NETWORK_LIST {
    pub dwNumberOfItems: u32,
    pub dwIndex: u32,
    pub Network: [DOT11_NETWORK; 1],
}
impl ::core::marker::Copy for DOT11_NETWORK_LIST {}
impl ::core::clone::Clone for DOT11_NETWORK_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_NIC_SPECIFIC_EXTENSION {
    pub uBufferLength: u32,
    pub uTotalBufferLength: u32,
    pub ucBuffer: [u8; 1],
}
impl ::core::marker::Copy for DOT11_NIC_SPECIFIC_EXTENSION {}
impl ::core::clone::Clone for DOT11_NIC_SPECIFIC_EXTENSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_OFDM_PHY_ATTRIBUTES {
    pub uFrequencyBandsSupported: u32,
}
impl ::core::marker::Copy for DOT11_OFDM_PHY_ATTRIBUTES {}
impl ::core::clone::Clone for DOT11_OFDM_PHY_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_OFFLOAD_CAPABILITY {
    pub uReserved: u32,
    pub uFlags: u32,
    pub uSupportedWEPAlgorithms: u32,
    pub uNumOfReplayWindows: u32,
    pub uMaxWEPKeyMappingLength: u32,
    pub uSupportedAuthAlgorithms: u32,
    pub uMaxAuthKeyMappingLength: u32,
}
impl ::core::marker::Copy for DOT11_OFFLOAD_CAPABILITY {}
impl ::core::clone::Clone for DOT11_OFFLOAD_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_OFFLOAD_NETWORK {
    pub Ssid: DOT11_SSID,
    pub UnicastCipher: DOT11_CIPHER_ALGORITHM,
    pub AuthAlgo: DOT11_AUTH_ALGORITHM,
    pub Dot11ChannelHints: [DOT11_CHANNEL_HINT; 4],
}
impl ::core::marker::Copy for DOT11_OFFLOAD_NETWORK {}
impl ::core::clone::Clone for DOT11_OFFLOAD_NETWORK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_OFFLOAD_NETWORK_LIST_INFO {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ulFlags: u32,
    pub FastScanPeriod: u32,
    pub FastScanIterations: u32,
    pub SlowScanPeriod: u32,
    pub uNumOfEntries: u32,
    pub offloadNetworkList: [DOT11_OFFLOAD_NETWORK; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_OFFLOAD_NETWORK_LIST_INFO {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_OFFLOAD_NETWORK_LIST_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_OFFLOAD_NETWORK_STATUS_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub Status: i32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_OFFLOAD_NETWORK_STATUS_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_OFFLOAD_NETWORK_STATUS_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_OI {
    pub OILength: u16,
    pub OI: [u8; 5],
}
impl ::core::marker::Copy for DOT11_OI {}
impl ::core::clone::Clone for DOT11_OI {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_OPERATION_MODE_CAPABILITY {
    pub uReserved: u32,
    pub uMajorVersion: u32,
    pub uMinorVersion: u32,
    pub uNumOfTXBuffers: u32,
    pub uNumOfRXBuffers: u32,
    pub uOpModeCapability: u32,
}
impl ::core::marker::Copy for DOT11_OPERATION_MODE_CAPABILITY {}
impl ::core::clone::Clone for DOT11_OPERATION_MODE_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_OPTIONAL_CAPABILITY {
    pub uReserved: u32,
    pub bDot11PCF: super::super::Foundation::BOOLEAN,
    pub bDot11PCFMPDUTransferToPC: super::super::Foundation::BOOLEAN,
    pub bStrictlyOrderedServiceClass: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_OPTIONAL_CAPABILITY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_OPTIONAL_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_PEER_INFO {
    pub MacAddress: [u8; 6],
    pub usCapabilityInformation: u16,
    pub AuthAlgo: DOT11_AUTH_ALGORITHM,
    pub UnicastCipherAlgo: DOT11_CIPHER_ALGORITHM,
    pub MulticastCipherAlgo: DOT11_CIPHER_ALGORITHM,
    pub bWpsEnabled: super::super::Foundation::BOOLEAN,
    pub usListenInterval: u16,
    pub ucSupportedRates: [u8; 255],
    pub usAssociationID: u16,
    pub AssociationState: DOT11_ASSOCIATION_STATE,
    pub PowerMode: DOT11_POWER_MODE,
    pub liAssociationUpTime: i64,
    pub Statistics: DOT11_PEER_STATISTICS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_PEER_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_PEER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_PEER_INFO_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub PeerInfo: [DOT11_PEER_INFO; 1],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_PEER_INFO_LIST {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_PEER_INFO_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_PEER_STATISTICS {
    pub ullDecryptSuccessCount: u64,
    pub ullDecryptFailureCount: u64,
    pub ullTxPacketSuccessCount: u64,
    pub ullTxPacketFailureCount: u64,
    pub ullRxPacketSuccessCount: u64,
    pub ullRxPacketFailureCount: u64,
}
impl ::core::marker::Copy for DOT11_PEER_STATISTICS {}
impl ::core::clone::Clone for DOT11_PEER_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_PER_MSDU_COUNTERS {
    pub uTransmittedFragmentCount: u32,
    pub uRetryCount: u32,
    pub uRTSSuccessCount: u32,
    pub uRTSFailureCount: u32,
    pub uACKFailureCount: u32,
}
impl ::core::marker::Copy for DOT11_PER_MSDU_COUNTERS {}
impl ::core::clone::Clone for DOT11_PER_MSDU_COUNTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_PHY_ATTRIBUTES {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PhyType: DOT11_PHY_TYPE,
    pub bHardwarePhyState: super::super::Foundation::BOOLEAN,
    pub bSoftwarePhyState: super::super::Foundation::BOOLEAN,
    pub bCFPollable: super::super::Foundation::BOOLEAN,
    pub uMPDUMaxLength: u32,
    pub TempType: DOT11_TEMP_TYPE,
    pub DiversitySupport: DOT11_DIVERSITY_SUPPORT,
    pub PhySpecificAttributes: DOT11_PHY_ATTRIBUTES_0,
    pub uNumberSupportedPowerLevels: u32,
    pub TxPowerLevels: [u32; 8],
    pub uNumDataRateMappingEntries: u32,
    pub DataRateMappingEntries: [DOT11_DATA_RATE_MAPPING_ENTRY; 126],
    pub SupportedDataRatesValue: DOT11_SUPPORTED_DATA_RATES_VALUE_V2,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_PHY_ATTRIBUTES {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_PHY_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub union DOT11_PHY_ATTRIBUTES_0 {
    pub HRDSSSAttributes: DOT11_HRDSSS_PHY_ATTRIBUTES,
    pub OFDMAttributes: DOT11_OFDM_PHY_ATTRIBUTES,
    pub ERPAttributes: DOT11_ERP_PHY_ATTRIBUTES,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_PHY_ATTRIBUTES_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_PHY_ATTRIBUTES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_PHY_FRAME_STATISTICS {
    pub ullTransmittedFrameCount: u64,
    pub ullMulticastTransmittedFrameCount: u64,
    pub ullFailedCount: u64,
    pub ullRetryCount: u64,
    pub ullMultipleRetryCount: u64,
    pub ullMaxTXLifetimeExceededCount: u64,
    pub ullTransmittedFragmentCount: u64,
    pub ullRTSSuccessCount: u64,
    pub ullRTSFailureCount: u64,
    pub ullACKFailureCount: u64,
    pub ullReceivedFrameCount: u64,
    pub ullMulticastReceivedFrameCount: u64,
    pub ullPromiscuousReceivedFrameCount: u64,
    pub ullMaxRXLifetimeExceededCount: u64,
    pub ullFrameDuplicateCount: u64,
    pub ullReceivedFragmentCount: u64,
    pub ullPromiscuousReceivedFragmentCount: u64,
    pub ullFCSErrorCount: u64,
}
impl ::core::marker::Copy for DOT11_PHY_FRAME_STATISTICS {}
impl ::core::clone::Clone for DOT11_PHY_FRAME_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ulPhyId: u32,
    pub Anonymous: DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS_0,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub union DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS_0 {
    pub ulChannel: u32,
    pub ulFrequency: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS_0 {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PHY_FREQUENCY_ADOPTED_PARAMETERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PHY_ID_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11PhyId: [u32; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PHY_ID_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PHY_ID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_PHY_STATE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uPhyId: u32,
    pub bHardwarePhyState: super::super::Foundation::BOOLEAN,
    pub bSoftwarePhyState: super::super::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_PHY_STATE_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_PHY_STATE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_PHY_TYPE_INFO {
    pub dot11PhyType: DOT11_PHY_TYPE,
    pub bUseParameters: super::super::Foundation::BOOLEAN,
    pub uProbeDelay: u32,
    pub uMinChannelTime: u32,
    pub uMaxChannelTime: u32,
    pub ChDescriptionType: CH_DESCRIPTION_TYPE,
    pub uChannelListSize: u32,
    pub ucChannelListBuffer: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_PHY_TYPE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_PHY_TYPE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PHY_TYPE_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11PhyType: [DOT11_PHY_TYPE; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PHY_TYPE_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PHY_TYPE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PMKID_CANDIDATE_LIST_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uCandidateListSize: u32,
    pub uCandidateListOffset: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PMKID_CANDIDATE_LIST_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PMKID_CANDIDATE_LIST_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_PMKID_ENTRY {
    pub BSSID: [u8; 6],
    pub PMKID: [u8; 16],
    pub uFlags: u32,
}
impl ::core::marker::Copy for DOT11_PMKID_ENTRY {}
impl ::core::clone::Clone for DOT11_PMKID_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PMKID_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub PMKIDs: [DOT11_PMKID_ENTRY; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PMKID_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PMKID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_PORT_STATE {
    pub PeerMacAddress: [u8; 6],
    pub uSessionId: u32,
    pub bPortControlled: super::super::Foundation::BOOL,
    pub bPortAuthorized: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_PORT_STATE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_PORT_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_PORT_STATE_NOTIFICATION {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerMac: [u8; 6],
    pub bOpen: super::super::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_PORT_STATE_NOTIFICATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_PORT_STATE_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_POWER_MGMT_AUTO_MODE_ENABLED_INFO {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub bEnabled: super::super::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_POWER_MGMT_AUTO_MODE_ENABLED_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_POWER_MGMT_AUTO_MODE_ENABLED_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_POWER_MGMT_MODE {
    pub dot11PowerMode: DOT11_POWER_MODE,
    pub uPowerSaveLevel: u32,
    pub usListenInterval: u16,
    pub usAID: u16,
    pub bReceiveDTIMs: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_POWER_MGMT_MODE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_POWER_MGMT_MODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_POWER_MGMT_MODE_STATUS_INFO {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PowerSaveMode: DOT11_POWER_MODE,
    pub uPowerSaveLevel: u32,
    pub Reason: DOT11_POWER_MODE_REASON,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_POWER_MGMT_MODE_STATUS_INFO {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_POWER_MGMT_MODE_STATUS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_PRIVACY_EXEMPTION {
    pub usEtherType: u16,
    pub usExemptionActionType: u16,
    pub usExemptionPacketType: u16,
}
impl ::core::marker::Copy for DOT11_PRIVACY_EXEMPTION {}
impl ::core::clone::Clone for DOT11_PRIVACY_EXEMPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PRIVACY_EXEMPTION_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub PrivacyExemptionEntries: [DOT11_PRIVACY_EXEMPTION; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PRIVACY_EXEMPTION_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PRIVACY_EXEMPTION_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PROVISION_DISCOVERY_REQUEST_SEND_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub ReceiverAddress: [u8; 6],
    pub DialogToken: u8,
    pub Status: i32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PROVISION_DISCOVERY_REQUEST_SEND_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PROVISION_DISCOVERY_REQUEST_SEND_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_PROVISION_DISCOVERY_RESPONSE_SEND_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ReceiverDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub Status: i32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_PROVISION_DISCOVERY_RESPONSE_SEND_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_PROVISION_DISCOVERY_RESPONSE_SEND_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_QOS_PARAMS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ucEnabledQoSProtocolFlags: u8,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_QOS_PARAMS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_QOS_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_QOS_TX_DURATION {
    pub uNominalMSDUSize: u32,
    pub uMinPHYRate: u32,
    pub uDuration: u32,
}
impl ::core::marker::Copy for DOT11_QOS_TX_DURATION {}
impl ::core::clone::Clone for DOT11_QOS_TX_DURATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_QOS_TX_MEDIUM_TIME {
    pub dot11PeerAddress: [u8; 6],
    pub ucQoSPriority: u8,
    pub uMediumTimeAdmited: u32,
}
impl ::core::marker::Copy for DOT11_QOS_TX_MEDIUM_TIME {}
impl ::core::clone::Clone for DOT11_QOS_TX_MEDIUM_TIME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_RATE_SET {
    pub uRateSetLength: u32,
    pub ucRateSet: [u8; 126],
}
impl ::core::marker::Copy for DOT11_RATE_SET {}
impl ::core::clone::Clone for DOT11_RATE_SET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_RECEIVED_GO_NEGOTIATION_CONFIRMATION_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_RECEIVED_GO_NEGOTIATION_CONFIRMATION_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_RECEIVED_GO_NEGOTIATION_CONFIRMATION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_RECEIVED_GO_NEGOTIATION_REQUEST_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub RequestContext: *mut ::core::ffi::c_void,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_RECEIVED_GO_NEGOTIATION_REQUEST_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_RECEIVED_GO_NEGOTIATION_REQUEST_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_RECEIVED_GO_NEGOTIATION_RESPONSE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub ResponseContext: *mut ::core::ffi::c_void,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_RECEIVED_GO_NEGOTIATION_RESPONSE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_RECEIVED_GO_NEGOTIATION_RESPONSE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_RECEIVED_INVITATION_REQUEST_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub TransmitterDeviceAddress: [u8; 6],
    pub BSSID: [u8; 6],
    pub DialogToken: u8,
    pub RequestContext: *mut ::core::ffi::c_void,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_RECEIVED_INVITATION_REQUEST_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_RECEIVED_INVITATION_REQUEST_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_RECEIVED_INVITATION_RESPONSE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub TransmitterDeviceAddress: [u8; 6],
    pub BSSID: [u8; 6],
    pub DialogToken: u8,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_RECEIVED_INVITATION_RESPONSE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_RECEIVED_INVITATION_RESPONSE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_RECEIVED_PROVISION_DISCOVERY_REQUEST_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub TransmitterDeviceAddress: [u8; 6],
    pub BSSID: [u8; 6],
    pub DialogToken: u8,
    pub RequestContext: *mut ::core::ffi::c_void,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_RECEIVED_PROVISION_DISCOVERY_REQUEST_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_RECEIVED_PROVISION_DISCOVERY_REQUEST_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_RECEIVED_PROVISION_DISCOVERY_RESPONSE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub TransmitterDeviceAddress: [u8; 6],
    pub BSSID: [u8; 6],
    pub DialogToken: u8,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_RECEIVED_PROVISION_DISCOVERY_RESPONSE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_RECEIVED_PROVISION_DISCOVERY_RESPONSE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_RECV_EXTENSION_INFO {
    pub uVersion: u32,
    pub pvReserved: *mut ::core::ffi::c_void,
    pub dot11PhyType: DOT11_PHY_TYPE,
    pub uChCenterFrequency: u32,
    pub lRSSI: i32,
    pub lRSSIMin: i32,
    pub lRSSIMax: i32,
    pub uRSSI: u32,
    pub ucPriority: u8,
    pub ucDataRate: u8,
    pub ucPeerMacAddress: [u8; 6],
    pub dwExtendedStatus: u32,
    pub hWEPOffloadContext: super::super::Foundation::HANDLE,
    pub hAuthOffloadContext: super::super::Foundation::HANDLE,
    pub usWEPAppliedMask: u16,
    pub usWPAMSDUPriority: u16,
    pub dot11LowestIV48Counter: DOT11_IV48_COUNTER,
    pub usDot11LeftRWBitMap: u16,
    pub dot11HighestIV48Counter: DOT11_IV48_COUNTER,
    pub usDot11RightRWBitMap: u16,
    pub usNumberOfMPDUsReceived: u16,
    pub usNumberOfFragments: u16,
    pub pNdisPackets: [*mut ::core::ffi::c_void; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_RECV_EXTENSION_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_RECV_EXTENSION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_RECV_EXTENSION_INFO_V2 {
    pub uVersion: u32,
    pub pvReserved: *mut ::core::ffi::c_void,
    pub dot11PhyType: DOT11_PHY_TYPE,
    pub uChCenterFrequency: u32,
    pub lRSSI: i32,
    pub uRSSI: u32,
    pub ucPriority: u8,
    pub ucDataRate: u8,
    pub ucPeerMacAddress: [u8; 6],
    pub dwExtendedStatus: u32,
    pub hWEPOffloadContext: super::super::Foundation::HANDLE,
    pub hAuthOffloadContext: super::super::Foundation::HANDLE,
    pub usWEPAppliedMask: u16,
    pub usWPAMSDUPriority: u16,
    pub dot11LowestIV48Counter: DOT11_IV48_COUNTER,
    pub usDot11LeftRWBitMap: u16,
    pub dot11HighestIV48Counter: DOT11_IV48_COUNTER,
    pub usDot11RightRWBitMap: u16,
    pub usNumberOfMPDUsReceived: u16,
    pub usNumberOfFragments: u16,
    pub pNdisPackets: [*mut ::core::ffi::c_void; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_RECV_EXTENSION_INFO_V2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_RECV_EXTENSION_INFO_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_RECV_SENSITIVITY {
    pub ucDataRate: u8,
    pub lRSSIMin: i32,
    pub lRSSIMax: i32,
}
impl ::core::marker::Copy for DOT11_RECV_SENSITIVITY {}
impl ::core::clone::Clone for DOT11_RECV_SENSITIVITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_RECV_SENSITIVITY_LIST {
    pub Anonymous: DOT11_RECV_SENSITIVITY_LIST_0,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11RecvSensitivity: [DOT11_RECV_SENSITIVITY; 1],
}
impl ::core::marker::Copy for DOT11_RECV_SENSITIVITY_LIST {}
impl ::core::clone::Clone for DOT11_RECV_SENSITIVITY_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union DOT11_RECV_SENSITIVITY_LIST_0 {
    pub dot11PhyType: DOT11_PHY_TYPE,
    pub uPhyId: u32,
}
impl ::core::marker::Copy for DOT11_RECV_SENSITIVITY_LIST_0 {}
impl ::core::clone::Clone for DOT11_RECV_SENSITIVITY_LIST_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_REG_DOMAINS_SUPPORT_VALUE {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11RegDomainValue: [DOT11_REG_DOMAIN_VALUE; 1],
}
impl ::core::marker::Copy for DOT11_REG_DOMAINS_SUPPORT_VALUE {}
impl ::core::clone::Clone for DOT11_REG_DOMAINS_SUPPORT_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_REG_DOMAIN_VALUE {
    pub uRegDomainsSupportIndex: u32,
    pub uRegDomainsSupportValue: u32,
}
impl ::core::marker::Copy for DOT11_REG_DOMAIN_VALUE {}
impl ::core::clone::Clone for DOT11_REG_DOMAIN_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_RESET_REQUEST {
    pub dot11ResetType: DOT11_RESET_TYPE,
    pub dot11MacAddress: [u8; 6],
    pub bSetDefaultMIB: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_RESET_REQUEST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_RESET_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_ROAMING_COMPLETION_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uStatus: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_ROAMING_COMPLETION_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_ROAMING_COMPLETION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_ROAMING_START_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub AdhocBSSID: [u8; 6],
    pub AdhocSSID: DOT11_SSID,
    pub uRoamingReason: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_ROAMING_START_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_ROAMING_START_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_RSSI_RANGE {
    pub dot11PhyType: DOT11_PHY_TYPE,
    pub uRSSIMin: u32,
    pub uRSSIMax: u32,
}
impl ::core::marker::Copy for DOT11_RSSI_RANGE {}
impl ::core::clone::Clone for DOT11_RSSI_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_SCAN_REQUEST {
    pub dot11BSSType: DOT11_BSS_TYPE,
    pub dot11BSSID: [u8; 6],
    pub dot11SSID: DOT11_SSID,
    pub dot11ScanType: DOT11_SCAN_TYPE,
    pub bRestrictedScan: super::super::Foundation::BOOLEAN,
    pub bUseRequestIE: super::super::Foundation::BOOLEAN,
    pub uRequestIDsOffset: u32,
    pub uNumOfRequestIDs: u32,
    pub uPhyTypesOffset: u32,
    pub uNumOfPhyTypes: u32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
    pub ucBuffer: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_SCAN_REQUEST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_SCAN_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_SCAN_REQUEST_V2 {
    pub dot11BSSType: DOT11_BSS_TYPE,
    pub dot11BSSID: [u8; 6],
    pub dot11ScanType: DOT11_SCAN_TYPE,
    pub bRestrictedScan: super::super::Foundation::BOOLEAN,
    pub udot11SSIDsOffset: u32,
    pub uNumOfdot11SSIDs: u32,
    pub bUseRequestIE: super::super::Foundation::BOOLEAN,
    pub uRequestIDsOffset: u32,
    pub uNumOfRequestIDs: u32,
    pub uPhyTypeInfosOffset: u32,
    pub uNumOfPhyTypeInfos: u32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
    pub ucBuffer: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_SCAN_REQUEST_V2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_SCAN_REQUEST_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DOT11_SECURITY_PACKET_HEADER {
    pub PeerMac: [u8; 6],
    pub usEtherType: u16,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for DOT11_SECURITY_PACKET_HEADER {}
impl ::core::clone::Clone for DOT11_SECURITY_PACKET_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_SEND_GO_NEGOTIATION_CONFIRMATION_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub ResponseContext: *mut ::core::ffi::c_void,
    pub uSendTimeout: u32,
    pub Status: u8,
    pub GroupCapability: u8,
    pub GroupID: DOT11_WFD_GROUP_ID,
    pub bUseGroupID: super::super::Foundation::BOOLEAN,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_SEND_GO_NEGOTIATION_CONFIRMATION_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_SEND_GO_NEGOTIATION_CONFIRMATION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_SEND_GO_NEGOTIATION_REQUEST_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub uSendTimeout: u32,
    pub GroupOwnerIntent: DOT11_WFD_GO_INTENT,
    pub MinimumConfigTimeout: DOT11_WFD_CONFIGURATION_TIMEOUT,
    pub IntendedInterfaceAddress: [u8; 6],
    pub GroupCapability: u8,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_SEND_GO_NEGOTIATION_REQUEST_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_SEND_GO_NEGOTIATION_REQUEST_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_SEND_GO_NEGOTIATION_RESPONSE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub PeerDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub RequestContext: *mut ::core::ffi::c_void,
    pub uSendTimeout: u32,
    pub Status: u8,
    pub GroupOwnerIntent: DOT11_WFD_GO_INTENT,
    pub MinimumConfigTimeout: DOT11_WFD_CONFIGURATION_TIMEOUT,
    pub IntendedInterfaceAddress: [u8; 6],
    pub GroupCapability: u8,
    pub GroupID: DOT11_WFD_GROUP_ID,
    pub bUseGroupID: super::super::Foundation::BOOLEAN,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_SEND_GO_NEGOTIATION_RESPONSE_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_SEND_GO_NEGOTIATION_RESPONSE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_SEND_INVITATION_REQUEST_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub DialogToken: u8,
    pub PeerDeviceAddress: [u8; 6],
    pub uSendTimeout: u32,
    pub MinimumConfigTimeout: DOT11_WFD_CONFIGURATION_TIMEOUT,
    pub InvitationFlags: DOT11_WFD_INVITATION_FLAGS,
    pub GroupBSSID: [u8; 6],
    pub bUseGroupBSSID: super::super::Foundation::BOOLEAN,
    pub OperatingChannel: DOT11_WFD_CHANNEL,
    pub bUseSpecifiedOperatingChannel: super::super::Foundation::BOOLEAN,
    pub GroupID: DOT11_WFD_GROUP_ID,
    pub bLocalGO: super::super::Foundation::BOOLEAN,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_SEND_INVITATION_REQUEST_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_SEND_INVITATION_REQUEST_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_SEND_INVITATION_RESPONSE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ReceiverDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub RequestContext: *mut ::core::ffi::c_void,
    pub uSendTimeout: u32,
    pub Status: u8,
    pub MinimumConfigTimeout: DOT11_WFD_CONFIGURATION_TIMEOUT,
    pub GroupBSSID: [u8; 6],
    pub bUseGroupBSSID: super::super::Foundation::BOOLEAN,
    pub OperatingChannel: DOT11_WFD_CHANNEL,
    pub bUseSpecifiedOperatingChannel: super::super::Foundation::BOOLEAN,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_SEND_INVITATION_RESPONSE_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_SEND_INVITATION_RESPONSE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_SEND_PROVISION_DISCOVERY_REQUEST_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub DialogToken: u8,
    pub PeerDeviceAddress: [u8; 6],
    pub uSendTimeout: u32,
    pub GroupCapability: u8,
    pub GroupID: DOT11_WFD_GROUP_ID,
    pub bUseGroupID: super::super::Foundation::BOOLEAN,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_SEND_PROVISION_DISCOVERY_REQUEST_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_SEND_PROVISION_DISCOVERY_REQUEST_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_SEND_PROVISION_DISCOVERY_RESPONSE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ReceiverDeviceAddress: [u8; 6],
    pub DialogToken: u8,
    pub RequestContext: *mut ::core::ffi::c_void,
    pub uSendTimeout: u32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_SEND_PROVISION_DISCOVERY_RESPONSE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_SEND_PROVISION_DISCOVERY_RESPONSE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SSID {
    pub uSSIDLength: u32,
    pub ucSSID: [u8; 32],
}
impl ::core::marker::Copy for DOT11_SSID {}
impl ::core::clone::Clone for DOT11_SSID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_SSID_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub SSIDs: [DOT11_SSID; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_SSID_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_SSID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_START_REQUEST {
    pub uStartFailureTimeout: u32,
    pub OperationalRateSet: DOT11_RATE_SET,
    pub uChCenterFrequency: u32,
    pub dot11BSSDescription: DOT11_BSS_DESCRIPTION,
}
impl ::core::marker::Copy for DOT11_START_REQUEST {}
impl ::core::clone::Clone for DOT11_START_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_STATISTICS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ullFourWayHandshakeFailures: u64,
    pub ullTKIPCounterMeasuresInvoked: u64,
    pub ullReserved: u64,
    pub MacUcastCounters: DOT11_MAC_FRAME_STATISTICS,
    pub MacMcastCounters: DOT11_MAC_FRAME_STATISTICS,
    pub PhyCounters: [DOT11_PHY_FRAME_STATISTICS; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_STATISTICS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_STATUS_INDICATION {
    pub uStatusType: u32,
    pub ndisStatus: i32,
}
impl ::core::marker::Copy for DOT11_STATUS_INDICATION {}
impl ::core::clone::Clone for DOT11_STATUS_INDICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_STOP_AP_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ulReason: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_STOP_AP_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_STOP_AP_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_SUPPORTED_ANTENNA {
    pub uAntennaListIndex: u32,
    pub bSupportedAntenna: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_SUPPORTED_ANTENNA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_SUPPORTED_ANTENNA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_SUPPORTED_ANTENNA_LIST {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11SupportedAntenna: [DOT11_SUPPORTED_ANTENNA; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_SUPPORTED_ANTENNA_LIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_SUPPORTED_ANTENNA_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_DATA_RATES_VALUE {
    pub ucSupportedTxDataRatesValue: [u8; 8],
    pub ucSupportedRxDataRatesValue: [u8; 8],
}
impl ::core::marker::Copy for DOT11_SUPPORTED_DATA_RATES_VALUE {}
impl ::core::clone::Clone for DOT11_SUPPORTED_DATA_RATES_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_DATA_RATES_VALUE_V2 {
    pub ucSupportedTxDataRatesValue: [u8; 255],
    pub ucSupportedRxDataRatesValue: [u8; 255],
}
impl ::core::marker::Copy for DOT11_SUPPORTED_DATA_RATES_VALUE_V2 {}
impl ::core::clone::Clone for DOT11_SUPPORTED_DATA_RATES_VALUE_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_DSSS_CHANNEL {
    pub uChannel: u32,
}
impl ::core::marker::Copy for DOT11_SUPPORTED_DSSS_CHANNEL {}
impl ::core::clone::Clone for DOT11_SUPPORTED_DSSS_CHANNEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_DSSS_CHANNEL_LIST {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11SupportedDSSSChannel: [DOT11_SUPPORTED_DSSS_CHANNEL; 1],
}
impl ::core::marker::Copy for DOT11_SUPPORTED_DSSS_CHANNEL_LIST {}
impl ::core::clone::Clone for DOT11_SUPPORTED_DSSS_CHANNEL_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_OFDM_FREQUENCY {
    pub uCenterFrequency: u32,
}
impl ::core::marker::Copy for DOT11_SUPPORTED_OFDM_FREQUENCY {}
impl ::core::clone::Clone for DOT11_SUPPORTED_OFDM_FREQUENCY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_OFDM_FREQUENCY_LIST {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11SupportedOFDMFrequency: [DOT11_SUPPORTED_OFDM_FREQUENCY; 1],
}
impl ::core::marker::Copy for DOT11_SUPPORTED_OFDM_FREQUENCY_LIST {}
impl ::core::clone::Clone for DOT11_SUPPORTED_OFDM_FREQUENCY_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_PHY_TYPES {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11PHYType: [DOT11_PHY_TYPE; 1],
}
impl ::core::marker::Copy for DOT11_SUPPORTED_PHY_TYPES {}
impl ::core::clone::Clone for DOT11_SUPPORTED_PHY_TYPES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_SUPPORTED_POWER_LEVELS {
    pub uNumOfSupportedPowerLevels: u32,
    pub uTxPowerLevelValues: [u32; 8],
}
impl ::core::marker::Copy for DOT11_SUPPORTED_POWER_LEVELS {}
impl ::core::clone::Clone for DOT11_SUPPORTED_POWER_LEVELS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_TKIPMIC_FAILURE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub bDefaultKeyFailure: super::super::Foundation::BOOLEAN,
    pub uKeyIndex: u32,
    pub PeerMac: [u8; 6],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_TKIPMIC_FAILURE_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_TKIPMIC_FAILURE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_UPDATE_IE {
    pub dot11UpdateIEOp: DOT11_UPDATE_IE_OP,
    pub uBufferLength: u32,
    pub ucBuffer: [u8; 1],
}
impl ::core::marker::Copy for DOT11_UPDATE_IE {}
impl ::core::clone::Clone for DOT11_UPDATE_IE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_VENUEINFO {
    pub VenueGroup: u8,
    pub VenueType: u8,
}
impl ::core::marker::Copy for DOT11_VENUEINFO {}
impl ::core::clone::Clone for DOT11_VENUEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_VWIFI_ATTRIBUTES {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uTotalNumOfEntries: u32,
    pub Combinations: [DOT11_VWIFI_COMBINATION; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_VWIFI_ATTRIBUTES {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_VWIFI_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_VWIFI_COMBINATION {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumInfrastructure: u32,
    pub uNumAdhoc: u32,
    pub uNumSoftAP: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_VWIFI_COMBINATION {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_VWIFI_COMBINATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_VWIFI_COMBINATION_V2 {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumInfrastructure: u32,
    pub uNumAdhoc: u32,
    pub uNumSoftAP: u32,
    pub uNumVirtualStation: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_VWIFI_COMBINATION_V2 {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_VWIFI_COMBINATION_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_VWIFI_COMBINATION_V3 {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumInfrastructure: u32,
    pub uNumAdhoc: u32,
    pub uNumSoftAP: u32,
    pub uNumVirtualStation: u32,
    pub uNumWFDGroup: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_VWIFI_COMBINATION_V3 {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_VWIFI_COMBINATION_V3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_WEP_OFFLOAD {
    pub uReserved: u32,
    pub hOffloadContext: super::super::Foundation::HANDLE,
    pub hOffload: super::super::Foundation::HANDLE,
    pub dot11OffloadType: DOT11_OFFLOAD_TYPE,
    pub dwAlgorithm: u32,
    pub bRowIsOutbound: super::super::Foundation::BOOLEAN,
    pub bUseDefault: super::super::Foundation::BOOLEAN,
    pub uFlags: u32,
    pub ucMacAddress: [u8; 6],
    pub uNumOfRWsOnPeer: u32,
    pub uNumOfRWsOnMe: u32,
    pub dot11IV48Counters: [DOT11_IV48_COUNTER; 16],
    pub usDot11RWBitMaps: [u16; 16],
    pub usKeyLength: u16,
    pub ucKey: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_WEP_OFFLOAD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_WEP_OFFLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_WEP_UPLOAD {
    pub uReserved: u32,
    pub dot11OffloadType: DOT11_OFFLOAD_TYPE,
    pub hOffload: super::super::Foundation::HANDLE,
    pub uNumOfRWsUsed: u32,
    pub dot11IV48Counters: [DOT11_IV48_COUNTER; 16],
    pub usDot11RWBitMaps: [u16; 16],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_WEP_UPLOAD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_WEP_UPLOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_WFD_ADDITIONAL_IE {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uBeaconIEsOffset: u32,
    pub uBeaconIEsLength: u32,
    pub uProbeResponseIEsOffset: u32,
    pub uProbeResponseIEsLength: u32,
    pub uDefaultRequestIEsOffset: u32,
    pub uDefaultRequestIEsLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_WFD_ADDITIONAL_IE {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_WFD_ADDITIONAL_IE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_ADVERTISED_SERVICE_DESCRIPTOR {
    pub AdvertisementID: u32,
    pub ConfigMethods: u16,
    pub ServiceNameLength: u8,
    pub ServiceName: [u8; 255],
}
impl ::core::marker::Copy for DOT11_WFD_ADVERTISED_SERVICE_DESCRIPTOR {}
impl ::core::clone::Clone for DOT11_WFD_ADVERTISED_SERVICE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_ADVERTISED_SERVICE_LIST {
    pub ServiceCount: u16,
    pub AdvertisedService: [DOT11_WFD_ADVERTISED_SERVICE_DESCRIPTOR; 1],
}
impl ::core::marker::Copy for DOT11_WFD_ADVERTISED_SERVICE_LIST {}
impl ::core::clone::Clone for DOT11_WFD_ADVERTISED_SERVICE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_ADVERTISEMENT_ID {
    pub AdvertisementID: u32,
    pub ServiceAddress: [u8; 6],
}
impl ::core::marker::Copy for DOT11_WFD_ADVERTISEMENT_ID {}
impl ::core::clone::Clone for DOT11_WFD_ADVERTISEMENT_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_WFD_ATTRIBUTES {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumConcurrentGORole: u32,
    pub uNumConcurrentClientRole: u32,
    pub WPSVersionsSupported: u32,
    pub bServiceDiscoverySupported: super::super::Foundation::BOOLEAN,
    pub bClientDiscoverabilitySupported: super::super::Foundation::BOOLEAN,
    pub bInfrastructureManagementSupported: super::super::Foundation::BOOLEAN,
    pub uMaxSecondaryDeviceTypeListSize: u32,
    pub DeviceAddress: [u8; 6],
    pub uInterfaceAddressListCount: u32,
    pub pInterfaceAddressList: *mut u8,
    pub uNumSupportedCountryOrRegionStrings: u32,
    pub pSupportedCountryOrRegionStrings: *mut u8,
    pub uDiscoveryFilterListSize: u32,
    pub uGORoleClientTableSize: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_WFD_ATTRIBUTES {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_WFD_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_CHANNEL {
    pub CountryRegionString: [u8; 3],
    pub OperatingClass: u8,
    pub ChannelNumber: u8,
}
impl ::core::marker::Copy for DOT11_WFD_CHANNEL {}
impl ::core::clone::Clone for DOT11_WFD_CHANNEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_CONFIGURATION_TIMEOUT {
    pub GOTimeout: u8,
    pub ClientTimeout: u8,
}
impl ::core::marker::Copy for DOT11_WFD_CONFIGURATION_TIMEOUT {}
impl ::core::clone::Clone for DOT11_WFD_CONFIGURATION_TIMEOUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_WFD_DEVICE_CAPABILITY_CONFIG {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub bServiceDiscoveryEnabled: super::super::Foundation::BOOLEAN,
    pub bClientDiscoverabilityEnabled: super::super::Foundation::BOOLEAN,
    pub bConcurrentOperationSupported: super::super::Foundation::BOOLEAN,
    pub bInfrastructureManagementEnabled: super::super::Foundation::BOOLEAN,
    pub bDeviceLimitReached: super::super::Foundation::BOOLEAN,
    pub bInvitationProcedureEnabled: super::super::Foundation::BOOLEAN,
    pub WPSVersionsEnabled: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_WFD_DEVICE_CAPABILITY_CONFIG {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_WFD_DEVICE_CAPABILITY_CONFIG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_DEVICE_ENTRY {
    pub uPhyId: u32,
    pub PhySpecificInfo: DOT11_BSS_ENTRY_PHY_SPECIFIC_INFO,
    pub dot11BSSID: [u8; 6],
    pub dot11BSSType: DOT11_BSS_TYPE,
    pub TransmitterAddress: [u8; 6],
    pub lRSSI: i32,
    pub uLinkQuality: u32,
    pub usBeaconPeriod: u16,
    pub ullTimestamp: u64,
    pub ullBeaconHostTimestamp: u64,
    pub ullProbeResponseHostTimestamp: u64,
    pub usCapabilityInformation: u16,
    pub uBeaconIEsOffset: u32,
    pub uBeaconIEsLength: u32,
    pub uProbeResponseIEsOffset: u32,
    pub uProbeResponseIEsLength: u32,
}
impl ::core::marker::Copy for DOT11_WFD_DEVICE_ENTRY {}
impl ::core::clone::Clone for DOT11_WFD_DEVICE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_WFD_DEVICE_INFO {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub DeviceAddress: [u8; 6],
    pub ConfigMethods: u16,
    pub PrimaryDeviceType: DOT11_WFD_DEVICE_TYPE,
    pub DeviceName: DOT11_WPS_DEVICE_NAME,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_WFD_DEVICE_INFO {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_WFD_DEVICE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_WFD_DEVICE_LISTEN_CHANNEL {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub ChannelNumber: u8,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_WFD_DEVICE_LISTEN_CHANNEL {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_WFD_DEVICE_LISTEN_CHANNEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_DEVICE_TYPE {
    pub CategoryID: u16,
    pub SubCategoryID: u16,
    pub OUI: [u8; 4],
}
impl ::core::marker::Copy for DOT11_WFD_DEVICE_TYPE {}
impl ::core::clone::Clone for DOT11_WFD_DEVICE_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_WFD_DISCOVER_COMPLETE_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub Status: i32,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub uListOffset: u32,
    pub uListLength: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_WFD_DISCOVER_COMPLETE_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_WFD_DISCOVER_COMPLETE_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_DISCOVER_DEVICE_FILTER {
    pub DeviceID: [u8; 6],
    pub ucBitmask: u8,
    pub GroupSSID: DOT11_SSID,
}
impl ::core::marker::Copy for DOT11_WFD_DISCOVER_DEVICE_FILTER {}
impl ::core::clone::Clone for DOT11_WFD_DISCOVER_DEVICE_FILTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_WFD_DISCOVER_REQUEST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub DiscoverType: DOT11_WFD_DISCOVER_TYPE,
    pub ScanType: DOT11_WFD_SCAN_TYPE,
    pub uDiscoverTimeout: u32,
    pub uDeviceFilterListOffset: u32,
    pub uNumDeviceFilters: u32,
    pub uIEsOffset: u32,
    pub uIEsLength: u32,
    pub bForceScanLegacyNetworks: super::super::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_WFD_DISCOVER_REQUEST {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_WFD_DISCOVER_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_GO_INTENT {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for DOT11_WFD_GO_INTENT {}
impl ::core::clone::Clone for DOT11_WFD_GO_INTENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_GROUP_ID {
    pub DeviceAddress: [u8; 6],
    pub SSID: DOT11_SSID,
}
impl ::core::marker::Copy for DOT11_WFD_GROUP_ID {}
impl ::core::clone::Clone for DOT11_WFD_GROUP_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_WFD_GROUP_JOIN_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub GOOperatingChannel: DOT11_WFD_CHANNEL,
    pub GOConfigTime: u32,
    pub bInGroupFormation: super::super::Foundation::BOOLEAN,
    pub bWaitForWPSReady: super::super::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_WFD_GROUP_JOIN_PARAMETERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_WFD_GROUP_JOIN_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub bPersistentGroupEnabled: super::super::Foundation::BOOLEAN,
    pub bIntraBSSDistributionSupported: super::super::Foundation::BOOLEAN,
    pub bCrossConnectionSupported: super::super::Foundation::BOOLEAN,
    pub bPersistentReconnectSupported: super::super::Foundation::BOOLEAN,
    pub bGroupFormationEnabled: super::super::Foundation::BOOLEAN,
    pub uMaximumGroupLimit: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub struct DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG_V2 {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub bPersistentGroupEnabled: super::super::Foundation::BOOLEAN,
    pub bIntraBSSDistributionSupported: super::super::Foundation::BOOLEAN,
    pub bCrossConnectionSupported: super::super::Foundation::BOOLEAN,
    pub bPersistentReconnectSupported: super::super::Foundation::BOOLEAN,
    pub bGroupFormationEnabled: super::super::Foundation::BOOLEAN,
    pub uMaximumGroupLimit: u32,
    pub bEapolKeyIpAddressAllocationSupported: super::super::Foundation::BOOLEAN,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::marker::Copy for DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG_V2 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
impl ::core::clone::Clone for DOT11_WFD_GROUP_OWNER_CAPABILITY_CONFIG_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_WFD_GROUP_START_PARAMETERS {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub AdvertisedOperatingChannel: DOT11_WFD_CHANNEL,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_WFD_GROUP_START_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_WFD_GROUP_START_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_INVITATION_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for DOT11_WFD_INVITATION_FLAGS {}
impl ::core::clone::Clone for DOT11_WFD_INVITATION_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct DOT11_WFD_SECONDARY_DEVICE_TYPE_LIST {
    pub Header: super::Ndis::NDIS_OBJECT_HEADER,
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub SecondaryDeviceTypes: [DOT11_WFD_DEVICE_TYPE; 1],
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for DOT11_WFD_SECONDARY_DEVICE_TYPE_LIST {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for DOT11_WFD_SECONDARY_DEVICE_TYPE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_SERVICE_HASH_LIST {
    pub ServiceHashCount: u16,
    pub ServiceHash: [u8; 6],
}
impl ::core::marker::Copy for DOT11_WFD_SERVICE_HASH_LIST {}
impl ::core::clone::Clone for DOT11_WFD_SERVICE_HASH_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_SESSION_ID {
    pub SessionID: u32,
    pub SessionAddress: [u8; 6],
}
impl ::core::marker::Copy for DOT11_WFD_SESSION_ID {}
impl ::core::clone::Clone for DOT11_WFD_SESSION_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WFD_SESSION_INFO {
    pub uSessionInfoLength: u16,
    pub ucSessionInfo: [u8; 144],
}
impl ::core::marker::Copy for DOT11_WFD_SESSION_INFO {}
impl ::core::clone::Clone for DOT11_WFD_SESSION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WME_AC_PARAMETERS {
    pub ucAccessCategoryIndex: u8,
    pub ucAIFSN: u8,
    pub ucECWmin: u8,
    pub ucECWmax: u8,
    pub usTXOPLimit: u16,
}
impl ::core::marker::Copy for DOT11_WME_AC_PARAMETERS {}
impl ::core::clone::Clone for DOT11_WME_AC_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WME_AC_PARAMETERS_LIST {
    pub uNumOfEntries: u32,
    pub uTotalNumOfEntries: u32,
    pub dot11WMEACParameters: [DOT11_WME_AC_PARAMETERS; 1],
}
impl ::core::marker::Copy for DOT11_WME_AC_PARAMETERS_LIST {}
impl ::core::clone::Clone for DOT11_WME_AC_PARAMETERS_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WME_UPDATE_IE {
    pub uParamElemMinBeaconIntervals: u32,
    pub uWMEInfoElemOffset: u32,
    pub uWMEInfoElemLength: u32,
    pub uWMEParamElemOffset: u32,
    pub uWMEParamElemLength: u32,
    pub ucBuffer: [u8; 1],
}
impl ::core::marker::Copy for DOT11_WME_UPDATE_IE {}
impl ::core::clone::Clone for DOT11_WME_UPDATE_IE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOT11_WPA_TSC {
    pub uReserved: u32,
    pub dot11OffloadType: DOT11_OFFLOAD_TYPE,
    pub hOffload: super::super::Foundation::HANDLE,
    pub dot11IV48Counter: DOT11_IV48_COUNTER,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOT11_WPA_TSC {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOT11_WPA_TSC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOT11_WPS_DEVICE_NAME {
    pub uDeviceNameLength: u32,
    pub ucDeviceName: [u8; 32],
}
impl ::core::marker::Copy for DOT11_WPS_DEVICE_NAME {}
impl ::core::clone::Clone for DOT11_WPS_DEVICE_NAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct L2_NOTIFICATION_DATA {
    pub NotificationSource: WLAN_NOTIFICATION_SOURCES,
    pub NotificationCode: u32,
    pub InterfaceGuid: ::windows_sys::core::GUID,
    pub dwDataSize: u32,
    pub pData: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for L2_NOTIFICATION_DATA {}
impl ::core::clone::Clone for L2_NOTIFICATION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ONEX_AUTH_PARAMS {
    pub fUpdatePending: super::super::Foundation::BOOL,
    pub oneXConnProfile: ONEX_VARIABLE_BLOB,
    pub authIdentity: ONEX_AUTH_IDENTITY,
    pub dwQuarantineState: u32,
    pub _bitfield: u32,
    pub dwSessionId: u32,
    pub hUserToken: super::super::Foundation::HANDLE,
    pub OneXUserProfile: ONEX_VARIABLE_BLOB,
    pub Identity: ONEX_VARIABLE_BLOB,
    pub UserName: ONEX_VARIABLE_BLOB,
    pub Domain: ONEX_VARIABLE_BLOB,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ONEX_AUTH_PARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ONEX_AUTH_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
pub struct ONEX_EAP_ERROR {
    pub dwWinError: u32,
    pub r#type: super::super::Security::ExtensibleAuthenticationProtocol::EAP_METHOD_TYPE,
    pub dwReasonCode: u32,
    pub rootCauseGuid: ::windows_sys::core::GUID,
    pub repairGuid: ::windows_sys::core::GUID,
    pub helpLinkGuid: ::windows_sys::core::GUID,
    pub _bitfield: u32,
    pub RootCauseString: ONEX_VARIABLE_BLOB,
    pub RepairString: ONEX_VARIABLE_BLOB,
}
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
impl ::core::marker::Copy for ONEX_EAP_ERROR {}
#[cfg(feature = "Win32_Security_ExtensibleAuthenticationProtocol")]
impl ::core::clone::Clone for ONEX_EAP_ERROR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ONEX_RESULT_UPDATE_DATA {
    pub oneXStatus: ONEX_STATUS,
    pub BackendSupport: ONEX_EAP_METHOD_BACKEND_SUPPORT,
    pub fBackendEngaged: super::super::Foundation::BOOL,
    pub _bitfield: u32,
    pub authParams: ONEX_VARIABLE_BLOB,
    pub eapError: ONEX_VARIABLE_BLOB,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ONEX_RESULT_UPDATE_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ONEX_RESULT_UPDATE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ONEX_STATUS {
    pub authStatus: ONEX_AUTH_STATUS,
    pub dwReason: u32,
    pub dwError: u32,
}
impl ::core::marker::Copy for ONEX_STATUS {}
impl ::core::clone::Clone for ONEX_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ONEX_USER_INFO {
    pub authIdentity: ONEX_AUTH_IDENTITY,
    pub _bitfield: u32,
    pub UserName: ONEX_VARIABLE_BLOB,
    pub DomainName: ONEX_VARIABLE_BLOB,
}
impl ::core::marker::Copy for ONEX_USER_INFO {}
impl ::core::clone::Clone for ONEX_USER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ONEX_VARIABLE_BLOB {
    pub dwSize: u32,
    pub dwOffset: u32,
}
impl ::core::marker::Copy for ONEX_VARIABLE_BLOB {}
impl ::core::clone::Clone for ONEX_VARIABLE_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDIAG_IHV_WLAN_ID {
    pub strProfileName: [u16; 256],
    pub Ssid: DOT11_SSID,
    pub BssType: DOT11_BSS_TYPE,
    pub dwFlags: u32,
    pub dwReasonCode: u32,
}
impl ::core::marker::Copy for WDIAG_IHV_WLAN_ID {}
impl ::core::clone::Clone for WDIAG_IHV_WLAN_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WFDSVC_CONNECTION_CAPABILITY {
    pub bNew: super::super::Foundation::BOOLEAN,
    pub bClient: super::super::Foundation::BOOLEAN,
    pub bGO: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WFDSVC_CONNECTION_CAPABILITY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WFDSVC_CONNECTION_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WFD_GROUP_ID {
    pub DeviceAddress: [u8; 6],
    pub GroupSSID: DOT11_SSID,
}
impl ::core::marker::Copy for WFD_GROUP_ID {}
impl ::core::clone::Clone for WFD_GROUP_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_ASSOCIATION_ATTRIBUTES {
    pub dot11Ssid: DOT11_SSID,
    pub dot11BssType: DOT11_BSS_TYPE,
    pub dot11Bssid: [u8; 6],
    pub dot11PhyType: DOT11_PHY_TYPE,
    pub uDot11PhyIndex: u32,
    pub wlanSignalQuality: u32,
    pub ulRxRate: u32,
    pub ulTxRate: u32,
}
impl ::core::marker::Copy for WLAN_ASSOCIATION_ATTRIBUTES {}
impl ::core::clone::Clone for WLAN_ASSOCIATION_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_AUTH_CIPHER_PAIR_LIST {
    pub dwNumberOfItems: u32,
    pub pAuthCipherPairList: [DOT11_AUTH_CIPHER_PAIR; 1],
}
impl ::core::marker::Copy for WLAN_AUTH_CIPHER_PAIR_LIST {}
impl ::core::clone::Clone for WLAN_AUTH_CIPHER_PAIR_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_AVAILABLE_NETWORK {
    pub strProfileName: [u16; 256],
    pub dot11Ssid: DOT11_SSID,
    pub dot11BssType: DOT11_BSS_TYPE,
    pub uNumberOfBssids: u32,
    pub bNetworkConnectable: super::super::Foundation::BOOL,
    pub wlanNotConnectableReason: u32,
    pub uNumberOfPhyTypes: u32,
    pub dot11PhyTypes: [DOT11_PHY_TYPE; 8],
    pub bMorePhyTypes: super::super::Foundation::BOOL,
    pub wlanSignalQuality: u32,
    pub bSecurityEnabled: super::super::Foundation::BOOL,
    pub dot11DefaultAuthAlgorithm: DOT11_AUTH_ALGORITHM,
    pub dot11DefaultCipherAlgorithm: DOT11_CIPHER_ALGORITHM,
    pub dwFlags: u32,
    pub dwReserved: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_AVAILABLE_NETWORK {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_AVAILABLE_NETWORK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_AVAILABLE_NETWORK_LIST {
    pub dwNumberOfItems: u32,
    pub dwIndex: u32,
    pub Network: [WLAN_AVAILABLE_NETWORK; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_AVAILABLE_NETWORK_LIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_AVAILABLE_NETWORK_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_AVAILABLE_NETWORK_LIST_V2 {
    pub dwNumberOfItems: u32,
    pub dwIndex: u32,
    pub Network: [WLAN_AVAILABLE_NETWORK_V2; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_AVAILABLE_NETWORK_LIST_V2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_AVAILABLE_NETWORK_LIST_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_AVAILABLE_NETWORK_V2 {
    pub strProfileName: [u16; 256],
    pub dot11Ssid: DOT11_SSID,
    pub dot11BssType: DOT11_BSS_TYPE,
    pub uNumberOfBssids: u32,
    pub bNetworkConnectable: super::super::Foundation::BOOL,
    pub wlanNotConnectableReason: u32,
    pub uNumberOfPhyTypes: u32,
    pub dot11PhyTypes: [DOT11_PHY_TYPE; 8],
    pub bMorePhyTypes: super::super::Foundation::BOOL,
    pub wlanSignalQuality: u32,
    pub bSecurityEnabled: super::super::Foundation::BOOL,
    pub dot11DefaultAuthAlgorithm: DOT11_AUTH_ALGORITHM,
    pub dot11DefaultCipherAlgorithm: DOT11_CIPHER_ALGORITHM,
    pub dwFlags: u32,
    pub AccessNetworkOptions: DOT11_ACCESSNETWORKOPTIONS,
    pub dot11HESSID: [u8; 6],
    pub VenueInfo: DOT11_VENUEINFO,
    pub dwReserved: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_AVAILABLE_NETWORK_V2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_AVAILABLE_NETWORK_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_BSS_ENTRY {
    pub dot11Ssid: DOT11_SSID,
    pub uPhyId: u32,
    pub dot11Bssid: [u8; 6],
    pub dot11BssType: DOT11_BSS_TYPE,
    pub dot11BssPhyType: DOT11_PHY_TYPE,
    pub lRssi: i32,
    pub uLinkQuality: u32,
    pub bInRegDomain: super::super::Foundation::BOOLEAN,
    pub usBeaconPeriod: u16,
    pub ullTimestamp: u64,
    pub ullHostTimestamp: u64,
    pub usCapabilityInformation: u16,
    pub ulChCenterFrequency: u32,
    pub wlanRateSet: WLAN_RATE_SET,
    pub ulIeOffset: u32,
    pub ulIeSize: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_BSS_ENTRY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_BSS_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_BSS_LIST {
    pub dwTotalSize: u32,
    pub dwNumberOfItems: u32,
    pub wlanBssEntries: [WLAN_BSS_ENTRY; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_BSS_LIST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_BSS_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_CONNECTION_ATTRIBUTES {
    pub isState: WLAN_INTERFACE_STATE,
    pub wlanConnectionMode: WLAN_CONNECTION_MODE,
    pub strProfileName: [u16; 256],
    pub wlanAssociationAttributes: WLAN_ASSOCIATION_ATTRIBUTES,
    pub wlanSecurityAttributes: WLAN_SECURITY_ATTRIBUTES,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_CONNECTION_ATTRIBUTES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_CONNECTION_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_CONNECTION_NOTIFICATION_DATA {
    pub wlanConnectionMode: WLAN_CONNECTION_MODE,
    pub strProfileName: [u16; 256],
    pub dot11Ssid: DOT11_SSID,
    pub dot11BssType: DOT11_BSS_TYPE,
    pub bSecurityEnabled: super::super::Foundation::BOOL,
    pub wlanReasonCode: u32,
    pub dwFlags: WLAN_CONNECTION_NOTIFICATION_FLAGS,
    pub strProfileXml: [u16; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_CONNECTION_NOTIFICATION_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_CONNECTION_NOTIFICATION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct WLAN_CONNECTION_PARAMETERS {
    pub wlanConnectionMode: WLAN_CONNECTION_MODE,
    pub strProfile: ::windows_sys::core::PCWSTR,
    pub pDot11Ssid: *mut DOT11_SSID,
    pub pDesiredBssidList: *mut DOT11_BSSID_LIST,
    pub dot11BssType: DOT11_BSS_TYPE,
    pub dwFlags: u32,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for WLAN_CONNECTION_PARAMETERS {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for WLAN_CONNECTION_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
pub struct WLAN_CONNECTION_PARAMETERS_V2 {
    pub wlanConnectionMode: WLAN_CONNECTION_MODE,
    pub strProfile: ::windows_sys::core::PCWSTR,
    pub pDot11Ssid: *mut DOT11_SSID,
    pub pDot11Hessid: *mut u8,
    pub pDesiredBssidList: *mut DOT11_BSSID_LIST,
    pub dot11BssType: DOT11_BSS_TYPE,
    pub dwFlags: u32,
    pub pDot11AccessNetworkOptions: *mut DOT11_ACCESSNETWORKOPTIONS,
}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::marker::Copy for WLAN_CONNECTION_PARAMETERS_V2 {}
#[cfg(feature = "Win32_NetworkManagement_Ndis")]
impl ::core::clone::Clone for WLAN_CONNECTION_PARAMETERS_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_COUNTRY_OR_REGION_STRING_LIST {
    pub dwNumberOfItems: u32,
    pub pCountryOrRegionStringList: [u8; 3],
}
impl ::core::marker::Copy for WLAN_COUNTRY_OR_REGION_STRING_LIST {}
impl ::core::clone::Clone for WLAN_COUNTRY_OR_REGION_STRING_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_DEVICE_SERVICE_GUID_LIST {
    pub dwNumberOfItems: u32,
    pub dwIndex: u32,
    pub DeviceService: [::windows_sys::core::GUID; 1],
}
impl ::core::marker::Copy for WLAN_DEVICE_SERVICE_GUID_LIST {}
impl ::core::clone::Clone for WLAN_DEVICE_SERVICE_GUID_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_DEVICE_SERVICE_NOTIFICATION_DATA {
    pub DeviceService: ::windows_sys::core::GUID,
    pub dwOpCode: u32,
    pub dwDataSize: u32,
    pub DataBlob: [u8; 1],
}
impl ::core::marker::Copy for WLAN_DEVICE_SERVICE_NOTIFICATION_DATA {}
impl ::core::clone::Clone for WLAN_DEVICE_SERVICE_NOTIFICATION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_HOSTED_NETWORK_CONNECTION_SETTINGS {
    pub hostedNetworkSSID: DOT11_SSID,
    pub dwMaxNumberOfPeers: u32,
}
impl ::core::marker::Copy for WLAN_HOSTED_NETWORK_CONNECTION_SETTINGS {}
impl ::core::clone::Clone for WLAN_HOSTED_NETWORK_CONNECTION_SETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_HOSTED_NETWORK_DATA_PEER_STATE_CHANGE {
    pub OldState: WLAN_HOSTED_NETWORK_PEER_STATE,
    pub NewState: WLAN_HOSTED_NETWORK_PEER_STATE,
    pub PeerStateChangeReason: WLAN_HOSTED_NETWORK_REASON,
}
impl ::core::marker::Copy for WLAN_HOSTED_NETWORK_DATA_PEER_STATE_CHANGE {}
impl ::core::clone::Clone for WLAN_HOSTED_NETWORK_DATA_PEER_STATE_CHANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_HOSTED_NETWORK_PEER_STATE {
    pub PeerMacAddress: [u8; 6],
    pub PeerAuthState: WLAN_HOSTED_NETWORK_PEER_AUTH_STATE,
}
impl ::core::marker::Copy for WLAN_HOSTED_NETWORK_PEER_STATE {}
impl ::core::clone::Clone for WLAN_HOSTED_NETWORK_PEER_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_HOSTED_NETWORK_RADIO_STATE {
    pub dot11SoftwareRadioState: DOT11_RADIO_STATE,
    pub dot11HardwareRadioState: DOT11_RADIO_STATE,
}
impl ::core::marker::Copy for WLAN_HOSTED_NETWORK_RADIO_STATE {}
impl ::core::clone::Clone for WLAN_HOSTED_NETWORK_RADIO_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_HOSTED_NETWORK_SECURITY_SETTINGS {
    pub dot11AuthAlgo: DOT11_AUTH_ALGORITHM,
    pub dot11CipherAlgo: DOT11_CIPHER_ALGORITHM,
}
impl ::core::marker::Copy for WLAN_HOSTED_NETWORK_SECURITY_SETTINGS {}
impl ::core::clone::Clone for WLAN_HOSTED_NETWORK_SECURITY_SETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_HOSTED_NETWORK_STATE_CHANGE {
    pub OldState: WLAN_HOSTED_NETWORK_STATE,
    pub NewState: WLAN_HOSTED_NETWORK_STATE,
    pub StateChangeReason: WLAN_HOSTED_NETWORK_REASON,
}
impl ::core::marker::Copy for WLAN_HOSTED_NETWORK_STATE_CHANGE {}
impl ::core::clone::Clone for WLAN_HOSTED_NETWORK_STATE_CHANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_HOSTED_NETWORK_STATUS {
    pub HostedNetworkState: WLAN_HOSTED_NETWORK_STATE,
    pub IPDeviceID: ::windows_sys::core::GUID,
    pub wlanHostedNetworkBSSID: [u8; 6],
    pub dot11PhyType: DOT11_PHY_TYPE,
    pub ulChannelFrequency: u32,
    pub dwNumberOfPeers: u32,
    pub PeerList: [WLAN_HOSTED_NETWORK_PEER_STATE; 1],
}
impl ::core::marker::Copy for WLAN_HOSTED_NETWORK_STATUS {}
impl ::core::clone::Clone for WLAN_HOSTED_NETWORK_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_INTERFACE_CAPABILITY {
    pub interfaceType: WLAN_INTERFACE_TYPE,
    pub bDot11DSupported: super::super::Foundation::BOOL,
    pub dwMaxDesiredSsidListSize: u32,
    pub dwMaxDesiredBssidListSize: u32,
    pub dwNumberOfSupportedPhys: u32,
    pub dot11PhyTypes: [DOT11_PHY_TYPE; 64],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_INTERFACE_CAPABILITY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_INTERFACE_CAPABILITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_INTERFACE_INFO {
    pub InterfaceGuid: ::windows_sys::core::GUID,
    pub strInterfaceDescription: [u16; 256],
    pub isState: WLAN_INTERFACE_STATE,
}
impl ::core::marker::Copy for WLAN_INTERFACE_INFO {}
impl ::core::clone::Clone for WLAN_INTERFACE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_INTERFACE_INFO_LIST {
    pub dwNumberOfItems: u32,
    pub dwIndex: u32,
    pub InterfaceInfo: [WLAN_INTERFACE_INFO; 1],
}
impl ::core::marker::Copy for WLAN_INTERFACE_INFO_LIST {}
impl ::core::clone::Clone for WLAN_INTERFACE_INFO_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_MAC_FRAME_STATISTICS {
    pub ullTransmittedFrameCount: u64,
    pub ullReceivedFrameCount: u64,
    pub ullWEPExcludedCount: u64,
    pub ullTKIPLocalMICFailures: u64,
    pub ullTKIPReplays: u64,
    pub ullTKIPICVErrorCount: u64,
    pub ullCCMPReplays: u64,
    pub ullCCMPDecryptErrors: u64,
    pub ullWEPUndecryptableCount: u64,
    pub ullWEPICVErrorCount: u64,
    pub ullDecryptSuccessCount: u64,
    pub ullDecryptFailureCount: u64,
}
impl ::core::marker::Copy for WLAN_MAC_FRAME_STATISTICS {}
impl ::core::clone::Clone for WLAN_MAC_FRAME_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_MSM_NOTIFICATION_DATA {
    pub wlanConnectionMode: WLAN_CONNECTION_MODE,
    pub strProfileName: [u16; 256],
    pub dot11Ssid: DOT11_SSID,
    pub dot11BssType: DOT11_BSS_TYPE,
    pub dot11MacAddr: [u8; 6],
    pub bSecurityEnabled: super::super::Foundation::BOOL,
    pub bFirstPeer: super::super::Foundation::BOOL,
    pub bLastPeer: super::super::Foundation::BOOL,
    pub wlanReasonCode: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_MSM_NOTIFICATION_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_MSM_NOTIFICATION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_PHY_FRAME_STATISTICS {
    pub ullTransmittedFrameCount: u64,
    pub ullMulticastTransmittedFrameCount: u64,
    pub ullFailedCount: u64,
    pub ullRetryCount: u64,
    pub ullMultipleRetryCount: u64,
    pub ullMaxTXLifetimeExceededCount: u64,
    pub ullTransmittedFragmentCount: u64,
    pub ullRTSSuccessCount: u64,
    pub ullRTSFailureCount: u64,
    pub ullACKFailureCount: u64,
    pub ullReceivedFrameCount: u64,
    pub ullMulticastReceivedFrameCount: u64,
    pub ullPromiscuousReceivedFrameCount: u64,
    pub ullMaxRXLifetimeExceededCount: u64,
    pub ullFrameDuplicateCount: u64,
    pub ullReceivedFragmentCount: u64,
    pub ullPromiscuousReceivedFragmentCount: u64,
    pub ullFCSErrorCount: u64,
}
impl ::core::marker::Copy for WLAN_PHY_FRAME_STATISTICS {}
impl ::core::clone::Clone for WLAN_PHY_FRAME_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_PHY_RADIO_STATE {
    pub dwPhyIndex: u32,
    pub dot11SoftwareRadioState: DOT11_RADIO_STATE,
    pub dot11HardwareRadioState: DOT11_RADIO_STATE,
}
impl ::core::marker::Copy for WLAN_PHY_RADIO_STATE {}
impl ::core::clone::Clone for WLAN_PHY_RADIO_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_PROFILE_INFO {
    pub strProfileName: [u16; 256],
    pub dwFlags: u32,
}
impl ::core::marker::Copy for WLAN_PROFILE_INFO {}
impl ::core::clone::Clone for WLAN_PROFILE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_PROFILE_INFO_LIST {
    pub dwNumberOfItems: u32,
    pub dwIndex: u32,
    pub ProfileInfo: [WLAN_PROFILE_INFO; 1],
}
impl ::core::marker::Copy for WLAN_PROFILE_INFO_LIST {}
impl ::core::clone::Clone for WLAN_PROFILE_INFO_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_RADIO_STATE {
    pub dwNumberOfPhys: u32,
    pub PhyRadioState: [WLAN_PHY_RADIO_STATE; 64],
}
impl ::core::marker::Copy for WLAN_RADIO_STATE {}
impl ::core::clone::Clone for WLAN_RADIO_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_RATE_SET {
    pub uRateSetLength: u32,
    pub usRateSet: [u16; 126],
}
impl ::core::marker::Copy for WLAN_RATE_SET {}
impl ::core::clone::Clone for WLAN_RATE_SET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_RAW_DATA {
    pub dwDataSize: u32,
    pub DataBlob: [u8; 1],
}
impl ::core::marker::Copy for WLAN_RAW_DATA {}
impl ::core::clone::Clone for WLAN_RAW_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_RAW_DATA_LIST {
    pub dwTotalSize: u32,
    pub dwNumberOfItems: u32,
    pub DataList: [WLAN_RAW_DATA_LIST_0; 1],
}
impl ::core::marker::Copy for WLAN_RAW_DATA_LIST {}
impl ::core::clone::Clone for WLAN_RAW_DATA_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_RAW_DATA_LIST_0 {
    pub dwDataOffset: u32,
    pub dwDataSize: u32,
}
impl ::core::marker::Copy for WLAN_RAW_DATA_LIST_0 {}
impl ::core::clone::Clone for WLAN_RAW_DATA_LIST_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WLAN_SECURITY_ATTRIBUTES {
    pub bSecurityEnabled: super::super::Foundation::BOOL,
    pub bOneXEnabled: super::super::Foundation::BOOL,
    pub dot11AuthAlgorithm: DOT11_AUTH_ALGORITHM,
    pub dot11CipherAlgorithm: DOT11_CIPHER_ALGORITHM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WLAN_SECURITY_ATTRIBUTES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WLAN_SECURITY_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WLAN_STATISTICS {
    pub ullFourWayHandshakeFailures: u64,
    pub ullTKIPCounterMeasuresInvoked: u64,
    pub ullReserved: u64,
    pub MacUcastCounters: WLAN_MAC_FRAME_STATISTICS,
    pub MacMcastCounters: WLAN_MAC_FRAME_STATISTICS,
    pub dwNumberOfPhys: u32,
    pub PhyCounters: [WLAN_PHY_FRAME_STATISTICS; 1],
}
impl ::core::marker::Copy for WLAN_STATISTICS {}
impl ::core::clone::Clone for WLAN_STATISTICS {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_ADAPTER_RESET = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_CONTROL = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, dwinbuffersize: u32, pinbuffer: *const u8, dwoutbuffersize: u32, poutbuffer: *mut u8, pdwbytesreturned: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub type DOT11EXTIHV_CREATE_DISCOVERY_PROFILES = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, binsecure: super::super::Foundation::BOOL, pihvprofileparams: *const DOT11EXT_IHV_PROFILE_PARAMS, pconnectablebssid: *const DOT11_BSS_LIST, pihvdiscoveryprofilelist: *mut DOT11EXT_IHV_DISCOVERY_PROFILE_LIST, pdwreasoncode: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_DEINIT_ADAPTER = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE) -> ()>;
pub type DOT11EXTIHV_DEINIT_SERVICE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
pub type DOT11EXTIHV_GET_VERSION_INFO = ::core::option::Option<unsafe extern "system" fn(pdot11ihvversioninfo: *mut DOT11_IHV_VERSION_INFO) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_INIT_ADAPTER = ::core::option::Option<unsafe extern "system" fn(pdot11adapter: *const DOT11_ADAPTER, hdot11svchandle: super::super::Foundation::HANDLE, phihvextadapter: *mut super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`, `\"Win32_System_RemoteDesktop\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis", feature = "Win32_Security_ExtensibleAuthenticationProtocol", feature = "Win32_System_RemoteDesktop"))]
pub type DOT11EXTIHV_INIT_SERVICE = ::core::option::Option<unsafe extern "system" fn(dwvernumused: u32, pdot11extapi: *const DOT11EXT_APIS, pvreserved: *const ::core::ffi::c_void, pdot11ihvhandlers: *mut DOT11EXT_IHV_HANDLERS) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_INIT_VIRTUAL_STATION = ::core::option::Option<unsafe extern "system" fn(pdot11extvsapi: *const DOT11EXT_VIRTUAL_STATION_APIS, pvreserved: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_IS_UI_REQUEST_PENDING = ::core::option::Option<unsafe extern "system" fn(guiduirequest: ::windows_sys::core::GUID, pbisrequestpending: *mut super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub type DOT11EXTIHV_ONEX_INDICATE_RESULT = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, msonexresult: DOT11_MSONEX_RESULT, pdot11msonexresultparams: *const DOT11_MSONEX_RESULT_PARAMS) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub type DOT11EXTIHV_PERFORM_CAPABILITY_MATCH = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, pihvprofileparams: *const DOT11EXT_IHV_PROFILE_PARAMS, pihvconnprofile: *const DOT11EXT_IHV_CONNECTIVITY_PROFILE, pihvsecprofile: *const DOT11EXT_IHV_SECURITY_PROFILE, pconnectablebssid: *const DOT11_BSS_LIST, pdwreasoncode: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub type DOT11EXTIHV_PERFORM_POST_ASSOCIATE = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, hsecuritysessionid: super::super::Foundation::HANDLE, pportstate: *const DOT11_PORT_STATE, udot11assocparamsbytes: u32, pdot11assocparams: *const DOT11_ASSOCIATION_COMPLETION_PARAMETERS) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub type DOT11EXTIHV_PERFORM_PRE_ASSOCIATE = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, hconnectsession: super::super::Foundation::HANDLE, pihvprofileparams: *const DOT11EXT_IHV_PROFILE_PARAMS, pihvconnprofile: *const DOT11EXT_IHV_CONNECTIVITY_PROFILE, pihvsecprofile: *const DOT11EXT_IHV_SECURITY_PROFILE, pconnectablebssid: *const DOT11_BSS_LIST, pdwreasoncode: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_RemoteDesktop\"`"]
#[cfg(feature = "Win32_System_RemoteDesktop")]
pub type DOT11EXTIHV_PROCESS_SESSION_CHANGE = ::core::option::Option<unsafe extern "system" fn(ueventtype: u32, psessionnotification: *const super::super::System::RemoteDesktop::WTSSESSION_NOTIFICATION) -> u32>;
pub type DOT11EXTIHV_PROCESS_UI_RESPONSE = ::core::option::Option<unsafe extern "system" fn(guiduirequest: ::windows_sys::core::GUID, dwbytecount: u32, pvresponsebuffer: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_QUERY_UI_REQUEST = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, connectionphase: DOT11EXT_IHV_CONNECTION_PHASE, ppihvuirequest: *mut *mut DOT11EXT_IHV_UI_REQUEST) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_RECEIVE_INDICATION = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, indicationtype: DOT11EXT_IHV_INDICATION_TYPE, ubufferlength: u32, pvbuffer: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_RECEIVE_PACKET = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, dwinbuffersize: u32, pvinbuffer: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_SEND_PACKET_COMPLETION = ::core::option::Option<unsafe extern "system" fn(hsendcompletion: super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXTIHV_STOP_POST_ASSOCIATE = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, ppeer: *const *const u8, dot11assocstatus: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub type DOT11EXTIHV_VALIDATE_PROFILE = ::core::option::Option<unsafe extern "system" fn(hihvextadapter: super::super::Foundation::HANDLE, pihvprofileparams: *const DOT11EXT_IHV_PROFILE_PARAMS, pihvconnprofile: *const DOT11EXT_IHV_CONNECTIVITY_PROFILE, pihvsecprofile: *const DOT11EXT_IHV_SECURITY_PROFILE, pdwreasoncode: *mut u32) -> u32>;
pub type DOT11EXT_ALLOCATE_BUFFER = ::core::option::Option<unsafe extern "system" fn(dwbytecount: u32, ppvbuffer: *mut *mut ::core::ffi::c_void) -> u32>;
pub type DOT11EXT_FREE_BUFFER = ::core::option::Option<unsafe extern "system" fn(pvmemory: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_GET_PROFILE_CUSTOM_USER_DATA = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, hconnectsession: super::super::Foundation::HANDLE, dwsessionid: u32, pdwdatasize: *mut u32, ppvdata: *mut *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_NIC_SPECIFIC_EXTENSION = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, dwinbuffersize: u32, pvinbuffer: *const ::core::ffi::c_void, pdwoutbuffersize: *mut u32, pvoutbuffer: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_ExtensibleAuthenticationProtocol\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_ExtensibleAuthenticationProtocol"))]
pub type DOT11EXT_ONEX_START = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, peapattributes: *const super::super::Security::ExtensibleAuthenticationProtocol::EAP_ATTRIBUTES) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_ONEX_STOP = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_POST_ASSOCIATE_COMPLETION = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, hsecuritysessionid: super::super::Foundation::HANDLE, ppeer: *const *const u8, dwreasoncode: u32, dwwin32error: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_PRE_ASSOCIATE_COMPLETION = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, hconnectsession: super::super::Foundation::HANDLE, dwreasoncode: u32, dwwin32error: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_PROCESS_ONEX_PACKET = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, dwinpacketsize: u32, pvinpacket: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_QUERY_VIRTUAL_STATION_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, pbisvirtualstation: *mut super::super::Foundation::BOOL, pgprimary: *mut ::windows_sys::core::GUID, pvreserved: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_RELEASE_VIRTUAL_STATION = ::core::option::Option<unsafe extern "system" fn(hdot11primaryhandle: super::super::Foundation::HANDLE, pvreserved: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_REQUEST_VIRTUAL_STATION = ::core::option::Option<unsafe extern "system" fn(hdot11primaryhandle: super::super::Foundation::HANDLE, pvreserved: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SEND_NOTIFICATION = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, pnotificationdata: *const L2_NOTIFICATION_DATA) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SEND_PACKET = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, upacketlen: u32, pvpacket: *const ::core::ffi::c_void, hsendcompletion: super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SEND_UI_REQUEST = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, pihvuirequest: *const DOT11EXT_IHV_UI_REQUEST) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_AUTH_ALGORITHM = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, dwauthalgo: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_CURRENT_PROFILE = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, hconnectsession: super::super::Foundation::HANDLE, pihvconnprofile: *const DOT11EXT_IHV_CONNECTIVITY_PROFILE, pihvsecprofile: *const DOT11EXT_IHV_SECURITY_PROFILE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_NetworkManagement_Ndis\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_NetworkManagement_Ndis"))]
pub type DOT11EXT_SET_DEFAULT_KEY = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, pkey: *const DOT11_CIPHER_DEFAULT_KEY_VALUE, dot11direction: DOT11_DIRECTION) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_DEFAULT_KEY_ID = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, udefaultkeyid: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_ETHERTYPE_HANDLING = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, umaxbacklog: u32, unumofexemption: u32, pexemption: *const DOT11_PRIVACY_EXEMPTION, unumofregistration: u32, pusregistration: *const u16) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_EXCLUDE_UNENCRYPTED = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, bexcludeunencrypted: super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_KEY_MAPPING_KEY = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, pkey: *const DOT11_CIPHER_KEY_MAPPING_KEY_VALUE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_MULTICAST_CIPHER_ALGORITHM = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, dwmulticastcipheralgo: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_PROFILE_CUSTOM_USER_DATA = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, hconnectsession: super::super::Foundation::HANDLE, dwsessionid: u32, dwdatasize: u32, pvdata: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_UNICAST_CIPHER_ALGORITHM = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, dwunicastcipheralgo: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DOT11EXT_SET_VIRTUAL_STATION_AP_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hdot11svchandle: super::super::Foundation::HANDLE, hconnectsession: super::super::Foundation::HANDLE, dwnumproperties: u32, pproperties: *const DOT11EXT_VIRTUAL_STATION_AP_PROPERTY, pvreserved: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WFD_OPEN_SESSION_COMPLETE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(hsessionhandle: super::super::Foundation::HANDLE, pvcontext: *const ::core::ffi::c_void, guidsessioninterface: ::windows_sys::core::GUID, dwerror: u32, dwreasoncode: u32) -> ()>;
pub type WLAN_NOTIFICATION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(param0: *mut L2_NOTIFICATION_DATA, param1: *mut ::core::ffi::c_void) -> ()>;
