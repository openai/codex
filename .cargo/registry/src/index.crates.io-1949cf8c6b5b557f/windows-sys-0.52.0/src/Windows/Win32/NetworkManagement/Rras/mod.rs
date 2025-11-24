#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmAddGroupMembershipEntry(hprotocol : super::super::Foundation:: HANDLE, dwsourceaddr : u32, dwsourcemask : u32, dwgroupaddr : u32, dwgroupmask : u32, dwifindex : u32, dwifnexthopipaddr : u32, dwflags : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmDeRegisterMProtocol(hprotocol : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmDeleteGroupMembershipEntry(hprotocol : super::super::Foundation:: HANDLE, dwsourceaddr : u32, dwsourcemask : u32, dwgroupaddr : u32, dwgroupmask : u32, dwifindex : u32, dwifnexthopipaddr : u32, dwflags : u32) -> u32);
::windows_targets::link!("rtm.dll" "system" fn MgmGetFirstMfe(pdwbuffersize : *mut u32, pbbuffer : *mut u8, pdwnumentries : *mut u32) -> u32);
::windows_targets::link!("rtm.dll" "system" fn MgmGetFirstMfeStats(pdwbuffersize : *mut u32, pbbuffer : *mut u8, pdwnumentries : *mut u32, dwflags : u32) -> u32);
#[cfg(feature = "Win32_NetworkManagement_IpHelper")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_NetworkManagement_IpHelper\"`"] fn MgmGetMfe(pimm : *mut super::IpHelper:: MIB_IPMCAST_MFE, pdwbuffersize : *mut u32, pbbuffer : *mut u8) -> u32);
#[cfg(feature = "Win32_NetworkManagement_IpHelper")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_NetworkManagement_IpHelper\"`"] fn MgmGetMfeStats(pimm : *mut super::IpHelper:: MIB_IPMCAST_MFE, pdwbuffersize : *mut u32, pbbuffer : *mut u8, dwflags : u32) -> u32);
#[cfg(feature = "Win32_NetworkManagement_IpHelper")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_NetworkManagement_IpHelper\"`"] fn MgmGetNextMfe(pimmstart : *mut super::IpHelper:: MIB_IPMCAST_MFE, pdwbuffersize : *mut u32, pbbuffer : *mut u8, pdwnumentries : *mut u32) -> u32);
#[cfg(feature = "Win32_NetworkManagement_IpHelper")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_NetworkManagement_IpHelper\"`"] fn MgmGetNextMfeStats(pimmstart : *mut super::IpHelper:: MIB_IPMCAST_MFE, pdwbuffersize : *mut u32, pbbuffer : *mut u8, pdwnumentries : *mut u32, dwflags : u32) -> u32);
::windows_targets::link!("rtm.dll" "system" fn MgmGetProtocolOnInterface(dwifindex : u32, dwifnexthopaddr : u32, pdwifprotocolid : *mut u32, pdwifcomponentid : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmGroupEnumerationEnd(henum : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmGroupEnumerationGetNext(henum : super::super::Foundation:: HANDLE, pdwbuffersize : *mut u32, pbbuffer : *mut u8, pdwnumentries : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmGroupEnumerationStart(hprotocol : super::super::Foundation:: HANDLE, metenumtype : MGM_ENUM_TYPES, phenumhandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmRegisterMProtocol(prpiinfo : *mut ROUTING_PROTOCOL_CONFIG, dwprotocolid : u32, dwcomponentid : u32, phprotocol : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmReleaseInterfaceOwnership(hprotocol : super::super::Foundation:: HANDLE, dwifindex : u32, dwifnexthopaddr : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MgmTakeInterfaceOwnership(hprotocol : super::super::Foundation:: HANDLE, dwifindex : u32, dwifnexthopaddr : u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminBufferFree(pbuffer : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminConnectionClearStats(hrasserver : isize, hrasconnection : super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminConnectionEnum(hrasserver : isize, dwlevel : u32, lplpbbuffer : *mut *mut u8, dwprefmaxlen : u32, lpdwentriesread : *mut u32, lpdwtotalentries : *mut u32, lpdwresumehandle : *const u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminConnectionEnumEx(hrasserver : isize, pobjectheader : *const MPRAPI_OBJECT_HEADER, dwpreferedmaxlen : u32, lpdwentriesread : *mut u32, lpdwtotalentries : *mut u32, pprasconn : *mut *mut RAS_CONNECTION_EX, lpdwresumehandle : *const u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminConnectionGetInfo(hrasserver : isize, dwlevel : u32, hrasconnection : super::super::Foundation:: HANDLE, lplpbbuffer : *mut *mut u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminConnectionGetInfoEx(hrasserver : isize, hrasconnection : super::super::Foundation:: HANDLE, prasconnection : *mut RAS_CONNECTION_EX) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminConnectionRemoveQuarantine(hrasserver : super::super::Foundation:: HANDLE, hrasconnection : super::super::Foundation:: HANDLE, fisipaddress : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminDeregisterConnectionNotification(hmprserver : isize, heventnotification : super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminDeviceEnum(hmprserver : isize, dwlevel : u32, lplpbbuffer : *mut *mut u8, lpdwtotalentries : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminEstablishDomainRasServer(pszdomain : ::windows_sys::core::PCWSTR, pszmachine : ::windows_sys::core::PCWSTR, benable : super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminGetErrorString(dwerror : u32, lplpwserrorstring : *mut ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminGetPDCServer(lpszdomain : ::windows_sys::core::PCWSTR, lpszserver : ::windows_sys::core::PCWSTR, lpszpdcserver : ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceConnect(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, hevent : super::super::Foundation:: HANDLE, fsynchronous : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceCreate(hmprserver : isize, dwlevel : u32, lpbbuffer : *const u8, phinterface : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceDelete(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceDeviceGetInfo(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwindex : u32, dwlevel : u32, lplpbuffer : *mut *mut u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceDeviceSetInfo(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwindex : u32, dwlevel : u32, lpbbuffer : *const u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceDisconnect(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminInterfaceEnum(hmprserver : isize, dwlevel : u32, lplpbbuffer : *mut *mut u8, dwprefmaxlen : u32, lpdwentriesread : *mut u32, lpdwtotalentries : *mut u32, lpdwresumehandle : *const u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminInterfaceGetCredentials(lpwsserver : ::windows_sys::core::PCWSTR, lpwsinterfacename : ::windows_sys::core::PCWSTR, lpwsusername : ::windows_sys::core::PWSTR, lpwspassword : ::windows_sys::core::PWSTR, lpwsdomainname : ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceGetCredentialsEx(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwlevel : u32, lplpbbuffer : *mut *mut u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`, `\"Win32_Security_Cryptography\"`"] fn MprAdminInterfaceGetCustomInfoEx(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, pcustominfo : *mut MPR_IF_CUSTOMINFOEX2) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceGetHandle(hmprserver : isize, lpwsinterfacename : ::windows_sys::core::PCWSTR, phinterface : *mut super::super::Foundation:: HANDLE, fincludeclientinterfaces : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceGetInfo(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwlevel : u32, lplpbbuffer : *const *const u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceQueryUpdateResult(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwprotocolid : u32, lpdwupdateresult : *mut u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminInterfaceSetCredentials(lpwsserver : ::windows_sys::core::PCWSTR, lpwsinterfacename : ::windows_sys::core::PCWSTR, lpwsusername : ::windows_sys::core::PCWSTR, lpwsdomainname : ::windows_sys::core::PCWSTR, lpwspassword : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceSetCredentialsEx(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwlevel : u32, lpbbuffer : *const u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`, `\"Win32_Security_Cryptography\"`"] fn MprAdminInterfaceSetCustomInfoEx(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, pcustominfo : *const MPR_IF_CUSTOMINFOEX2) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceSetInfo(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwlevel : u32, lpbbuffer : *const u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceTransportAdd(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwtransportid : u32, pinterfaceinfo : *const u8, dwinterfaceinfosize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceTransportGetInfo(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwtransportid : u32, ppinterfaceinfo : *mut *mut u8, lpdwinterfaceinfosize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceTransportRemove(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwtransportid : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceTransportSetInfo(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwtransportid : u32, pinterfaceinfo : *const u8, dwinterfaceinfosize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceUpdatePhonebookInfo(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminInterfaceUpdateRoutes(hmprserver : isize, hinterface : super::super::Foundation:: HANDLE, dwprotocolid : u32, hevent : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminIsDomainRasServer(pszdomain : ::windows_sys::core::PCWSTR, pszmachine : ::windows_sys::core::PCWSTR, pbisrasserver : *mut super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminIsServiceInitialized(lpwsservername : ::windows_sys::core::PCWSTR, fisserviceinitialized : *const super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminIsServiceRunning(lpwsservername : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBBufferFree(pbuffer : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBEntryCreate(hmibserver : isize, dwpid : u32, dwroutingpid : u32, lpentry : *const ::core::ffi::c_void, dwentrysize : u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBEntryDelete(hmibserver : isize, dwprotocolid : u32, dwroutingpid : u32, lpentry : *const ::core::ffi::c_void, dwentrysize : u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBEntryGet(hmibserver : isize, dwprotocolid : u32, dwroutingpid : u32, lpinentry : *const ::core::ffi::c_void, dwinentrysize : u32, lplpoutentry : *mut *mut ::core::ffi::c_void, lpoutentrysize : *mut u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBEntryGetFirst(hmibserver : isize, dwprotocolid : u32, dwroutingpid : u32, lpinentry : *const ::core::ffi::c_void, dwinentrysize : u32, lplpoutentry : *mut *mut ::core::ffi::c_void, lpoutentrysize : *mut u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBEntryGetNext(hmibserver : isize, dwprotocolid : u32, dwroutingpid : u32, lpinentry : *const ::core::ffi::c_void, dwinentrysize : u32, lplpoutentry : *mut *mut ::core::ffi::c_void, lpoutentrysize : *mut u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBEntrySet(hmibserver : isize, dwprotocolid : u32, dwroutingpid : u32, lpentry : *const ::core::ffi::c_void, dwentrysize : u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBServerConnect(lpwsservername : ::windows_sys::core::PCWSTR, phmibserver : *mut isize) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminMIBServerDisconnect(hmibserver : isize) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminPortClearStats(hrasserver : isize, hport : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminPortDisconnect(hrasserver : isize, hport : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminPortEnum(hrasserver : isize, dwlevel : u32, hrasconnection : super::super::Foundation:: HANDLE, lplpbbuffer : *mut *mut u8, dwprefmaxlen : u32, lpdwentriesread : *mut u32, lpdwtotalentries : *mut u32, lpdwresumehandle : *const u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminPortGetInfo(hrasserver : isize, dwlevel : u32, hport : super::super::Foundation:: HANDLE, lplpbbuffer : *mut *mut u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminPortReset(hrasserver : isize, hport : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminRegisterConnectionNotification(hmprserver : isize, heventnotification : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminSendUserMessage(hmprserver : isize, hconnection : super::super::Foundation:: HANDLE, lpwszmessage : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminServerConnect(lpwsservername : ::windows_sys::core::PCWSTR, phmprserver : *mut isize) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminServerDisconnect(hmprserver : isize) -> ());
::windows_targets::link!("mprapi.dll" "system" fn MprAdminServerGetCredentials(hmprserver : isize, dwlevel : u32, lplpbbuffer : *const *const u8) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminServerGetInfo(hmprserver : isize, dwlevel : u32, lplpbbuffer : *mut *mut u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn MprAdminServerGetInfoEx(hmprserver : isize, pserverinfo : *mut MPR_SERVER_EX1) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminServerSetCredentials(hmprserver : isize, dwlevel : u32, lpbbuffer : *const u8) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminServerSetInfo(hmprserver : isize, dwlevel : u32, lpbbuffer : *const u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn MprAdminServerSetInfoEx(hmprserver : isize, pserverinfo : *const MPR_SERVER_SET_CONFIG_EX1) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminTransportCreate(hmprserver : isize, dwtransportid : u32, lpwstransportname : ::windows_sys::core::PCWSTR, pglobalinfo : *const u8, dwglobalinfosize : u32, pclientinterfaceinfo : *const u8, dwclientinterfaceinfosize : u32, lpwsdllpath : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminTransportGetInfo(hmprserver : isize, dwtransportid : u32, ppglobalinfo : *mut *mut u8, lpdwglobalinfosize : *mut u32, ppclientinterfaceinfo : *mut *mut u8, lpdwclientinterfaceinfosize : *mut u32) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminTransportSetInfo(hmprserver : isize, dwtransportid : u32, pglobalinfo : *const u8, dwglobalinfosize : u32, pclientinterfaceinfo : *const u8, dwclientinterfaceinfosize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprAdminUpdateConnection(hrasserver : isize, hrasconnection : super::super::Foundation:: HANDLE, prasupdateconnection : *const RAS_UPDATE_CONNECTION) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminUserGetInfo(lpszserver : ::windows_sys::core::PCWSTR, lpszuser : ::windows_sys::core::PCWSTR, dwlevel : u32, lpbbuffer : *mut u8) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprAdminUserSetInfo(lpszserver : ::windows_sys::core::PCWSTR, lpszuser : ::windows_sys::core::PCWSTR, dwlevel : u32, lpbbuffer : *const u8) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprConfigBufferFree(pbuffer : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigFilterGetInfo(hmprconfig : super::super::Foundation:: HANDLE, dwlevel : u32, dwtransportid : u32, lpbuffer : *mut u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigFilterSetInfo(hmprconfig : super::super::Foundation:: HANDLE, dwlevel : u32, dwtransportid : u32, lpbuffer : *const u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigGetFriendlyName(hmprconfig : super::super::Foundation:: HANDLE, pszguidname : ::windows_sys::core::PCWSTR, pszbuffer : ::windows_sys::core::PWSTR, dwbuffersize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigGetGuidName(hmprconfig : super::super::Foundation:: HANDLE, pszfriendlyname : ::windows_sys::core::PCWSTR, pszbuffer : ::windows_sys::core::PWSTR, dwbuffersize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceCreate(hmprconfig : super::super::Foundation:: HANDLE, dwlevel : u32, lpbbuffer : *const u8, phrouterinterface : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceDelete(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceEnum(hmprconfig : super::super::Foundation:: HANDLE, dwlevel : u32, lplpbuffer : *mut *mut u8, dwprefmaxlen : u32, lpdwentriesread : *mut u32, lpdwtotalentries : *mut u32, lpdwresumehandle : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`, `\"Win32_Security_Cryptography\"`"] fn MprConfigInterfaceGetCustomInfoEx(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, pcustominfo : *mut MPR_IF_CUSTOMINFOEX2) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceGetHandle(hmprconfig : super::super::Foundation:: HANDLE, lpwsinterfacename : ::windows_sys::core::PCWSTR, phrouterinterface : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceGetInfo(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, dwlevel : u32, lplpbuffer : *mut *mut u8, lpdwbuffersize : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`, `\"Win32_Security_Cryptography\"`"] fn MprConfigInterfaceSetCustomInfoEx(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, pcustominfo : *const MPR_IF_CUSTOMINFOEX2) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceSetInfo(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, dwlevel : u32, lpbbuffer : *const u8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceTransportAdd(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, dwtransportid : u32, lpwstransportname : ::windows_sys::core::PCWSTR, pinterfaceinfo : *const u8, dwinterfaceinfosize : u32, phrouteriftransport : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceTransportEnum(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, dwlevel : u32, lplpbuffer : *mut *mut u8, dwprefmaxlen : u32, lpdwentriesread : *mut u32, lpdwtotalentries : *mut u32, lpdwresumehandle : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceTransportGetHandle(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, dwtransportid : u32, phrouteriftransport : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceTransportGetInfo(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, hrouteriftransport : super::super::Foundation:: HANDLE, ppinterfaceinfo : *mut *mut u8, lpdwinterfaceinfosize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceTransportRemove(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, hrouteriftransport : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigInterfaceTransportSetInfo(hmprconfig : super::super::Foundation:: HANDLE, hrouterinterface : super::super::Foundation:: HANDLE, hrouteriftransport : super::super::Foundation:: HANDLE, pinterfaceinfo : *const u8, dwinterfaceinfosize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigServerBackup(hmprconfig : super::super::Foundation:: HANDLE, lpwspath : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigServerConnect(lpwsservername : ::windows_sys::core::PCWSTR, phmprconfig : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigServerDisconnect(hmprconfig : super::super::Foundation:: HANDLE) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigServerGetInfo(hmprconfig : super::super::Foundation:: HANDLE, dwlevel : u32, lplpbbuffer : *mut *mut u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn MprConfigServerGetInfoEx(hmprconfig : super::super::Foundation:: HANDLE, pserverinfo : *mut MPR_SERVER_EX1) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprConfigServerInstall(dwlevel : u32, pbuffer : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigServerRefresh(hmprconfig : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigServerRestore(hmprconfig : super::super::Foundation:: HANDLE, lpwspath : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprConfigServerSetInfo(hmprserver : isize, dwlevel : u32, lpbbuffer : *const u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn MprConfigServerSetInfoEx(hmprconfig : super::super::Foundation:: HANDLE, psetserverconfig : *const MPR_SERVER_SET_CONFIG_EX1) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigTransportCreate(hmprconfig : super::super::Foundation:: HANDLE, dwtransportid : u32, lpwstransportname : ::windows_sys::core::PCWSTR, pglobalinfo : *const u8, dwglobalinfosize : u32, pclientinterfaceinfo : *const u8, dwclientinterfaceinfosize : u32, lpwsdllpath : ::windows_sys::core::PCWSTR, phroutertransport : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigTransportDelete(hmprconfig : super::super::Foundation:: HANDLE, hroutertransport : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigTransportEnum(hmprconfig : super::super::Foundation:: HANDLE, dwlevel : u32, lplpbuffer : *mut *mut u8, dwprefmaxlen : u32, lpdwentriesread : *mut u32, lpdwtotalentries : *mut u32, lpdwresumehandle : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigTransportGetHandle(hmprconfig : super::super::Foundation:: HANDLE, dwtransportid : u32, phroutertransport : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigTransportGetInfo(hmprconfig : super::super::Foundation:: HANDLE, hroutertransport : super::super::Foundation:: HANDLE, ppglobalinfo : *mut *mut u8, lpdwglobalinfosize : *mut u32, ppclientinterfaceinfo : *mut *mut u8, lpdwclientinterfaceinfosize : *mut u32, lplpwsdllpath : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mprapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MprConfigTransportSetInfo(hmprconfig : super::super::Foundation:: HANDLE, hroutertransport : super::super::Foundation:: HANDLE, pglobalinfo : *const u8, dwglobalinfosize : u32, pclientinterfaceinfo : *const u8, dwclientinterfaceinfosize : u32, lpwsdllpath : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoBlockAdd(lpheader : *const ::core::ffi::c_void, dwinfotype : u32, dwitemsize : u32, dwitemcount : u32, lpitemdata : *const u8, lplpnewheader : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoBlockFind(lpheader : *const ::core::ffi::c_void, dwinfotype : u32, lpdwitemsize : *mut u32, lpdwitemcount : *mut u32, lplpitemdata : *mut *mut u8) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoBlockQuerySize(lpheader : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoBlockRemove(lpheader : *const ::core::ffi::c_void, dwinfotype : u32, lplpnewheader : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoBlockSet(lpheader : *const ::core::ffi::c_void, dwinfotype : u32, dwitemsize : u32, dwitemcount : u32, lpitemdata : *const u8, lplpnewheader : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoCreate(dwversion : u32, lplpnewheader : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoDelete(lpheader : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoDuplicate(lpheader : *const ::core::ffi::c_void, lplpnewheader : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("mprapi.dll" "system" fn MprInfoRemoveAll(lpheader : *const ::core::ffi::c_void, lplpnewheader : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasClearConnectionStatistics(hrasconn : HRASCONN) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasClearLinkStatistics(hrasconn : HRASCONN, dwsubentry : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasConnectionNotificationA(param0 : HRASCONN, param1 : super::super::Foundation:: HANDLE, param2 : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasConnectionNotificationW(param0 : HRASCONN, param1 : super::super::Foundation:: HANDLE, param2 : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasCreatePhonebookEntryA(param0 : super::super::Foundation:: HWND, param1 : ::windows_sys::core::PCSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasCreatePhonebookEntryW(param0 : super::super::Foundation:: HWND, param1 : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasDeleteEntryA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasDeleteEntryW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasDeleteSubEntryA(pszphonebook : ::windows_sys::core::PCSTR, pszentry : ::windows_sys::core::PCSTR, dwsubentryid : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasDeleteSubEntryW(pszphonebook : ::windows_sys::core::PCWSTR, pszentry : ::windows_sys::core::PCWSTR, dwsubentryid : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasDialA(param0 : *const RASDIALEXTENSIONS, param1 : ::windows_sys::core::PCSTR, param2 : *const RASDIALPARAMSA, param3 : u32, param4 : *const ::core::ffi::c_void, param5 : *mut HRASCONN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasdlg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasDialDlgA(lpszphonebook : ::windows_sys::core::PCSTR, lpszentry : ::windows_sys::core::PCSTR, lpszphonenumber : ::windows_sys::core::PCSTR, lpinfo : *mut RASDIALDLG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasdlg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasDialDlgW(lpszphonebook : ::windows_sys::core::PCWSTR, lpszentry : ::windows_sys::core::PCWSTR, lpszphonenumber : ::windows_sys::core::PCWSTR, lpinfo : *mut RASDIALDLG) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasDialW(param0 : *const RASDIALEXTENSIONS, param1 : ::windows_sys::core::PCWSTR, param2 : *const RASDIALPARAMSW, param3 : u32, param4 : *const ::core::ffi::c_void, param5 : *mut HRASCONN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasEditPhonebookEntryA(param0 : super::super::Foundation:: HWND, param1 : ::windows_sys::core::PCSTR, param2 : ::windows_sys::core::PCSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasEditPhonebookEntryW(param0 : super::super::Foundation:: HWND, param1 : ::windows_sys::core::PCWSTR, param2 : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasdlg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasEntryDlgA(lpszphonebook : ::windows_sys::core::PCSTR, lpszentry : ::windows_sys::core::PCSTR, lpinfo : *mut RASENTRYDLGA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasdlg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasEntryDlgW(lpszphonebook : ::windows_sys::core::PCWSTR, lpszentry : ::windows_sys::core::PCWSTR, lpinfo : *mut RASENTRYDLGW) -> super::super::Foundation:: BOOL);
::windows_targets::link!("rasapi32.dll" "system" fn RasEnumAutodialAddressesA(lpprasautodialaddresses : *mut ::windows_sys::core::PSTR, lpdwcbrasautodialaddresses : *mut u32, lpdwcrasautodialaddresses : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasEnumAutodialAddressesW(lpprasautodialaddresses : *mut ::windows_sys::core::PWSTR, lpdwcbrasautodialaddresses : *mut u32, lpdwcrasautodialaddresses : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasEnumConnectionsA(param0 : *mut RASCONNA, param1 : *mut u32, param2 : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasEnumConnectionsW(param0 : *mut RASCONNW, param1 : *mut u32, param2 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasEnumDevicesA(param0 : *mut RASDEVINFOA, param1 : *mut u32, param2 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasEnumDevicesW(param0 : *mut RASDEVINFOW, param1 : *mut u32, param2 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasEnumEntriesA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : *mut RASENTRYNAMEA, param3 : *mut u32, param4 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasEnumEntriesW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : *mut RASENTRYNAMEW, param3 : *mut u32, param4 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasFreeEapUserIdentityA(praseapuseridentity : *const RASEAPUSERIDENTITYA) -> ());
::windows_targets::link!("rasapi32.dll" "system" fn RasFreeEapUserIdentityW(praseapuseridentity : *const RASEAPUSERIDENTITYW) -> ());
::windows_targets::link!("rasapi32.dll" "system" fn RasGetAutodialAddressA(param0 : ::windows_sys::core::PCSTR, param1 : *const u32, param2 : *mut RASAUTODIALENTRYA, param3 : *mut u32, param4 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetAutodialAddressW(param0 : ::windows_sys::core::PCWSTR, param1 : *const u32, param2 : *mut RASAUTODIALENTRYW, param3 : *mut u32, param4 : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetAutodialEnableA(param0 : u32, param1 : *mut super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetAutodialEnableW(param0 : u32, param1 : *mut super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetAutodialParamA(param0 : u32, param1 : *mut ::core::ffi::c_void, param2 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetAutodialParamW(param0 : u32, param1 : *mut ::core::ffi::c_void, param2 : *mut u32) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn RasGetConnectStatusA(param0 : HRASCONN, param1 : *mut RASCONNSTATUSA) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn RasGetConnectStatusW(param0 : HRASCONN, param1 : *mut RASCONNSTATUSW) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetConnectionStatistics(hrasconn : HRASCONN, lpstatistics : *mut RAS_STATS) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetCountryInfoA(param0 : *mut RASCTRYINFO, param1 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetCountryInfoW(param0 : *mut RASCTRYINFO, param1 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetCredentialsA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : *mut RASCREDENTIALSA) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetCredentialsW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : *mut RASCREDENTIALSW) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetCustomAuthDataA(pszphonebook : ::windows_sys::core::PCSTR, pszentry : ::windows_sys::core::PCSTR, pbcustomauthdata : *mut u8, pdwsizeofcustomauthdata : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetCustomAuthDataW(pszphonebook : ::windows_sys::core::PCWSTR, pszentry : ::windows_sys::core::PCWSTR, pbcustomauthdata : *mut u8, pdwsizeofcustomauthdata : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetEapUserDataA(htoken : super::super::Foundation:: HANDLE, pszphonebook : ::windows_sys::core::PCSTR, pszentry : ::windows_sys::core::PCSTR, pbeapdata : *mut u8, pdwsizeofeapdata : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetEapUserDataW(htoken : super::super::Foundation:: HANDLE, pszphonebook : ::windows_sys::core::PCWSTR, pszentry : ::windows_sys::core::PCWSTR, pbeapdata : *mut u8, pdwsizeofeapdata : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetEapUserIdentityA(pszphonebook : ::windows_sys::core::PCSTR, pszentry : ::windows_sys::core::PCSTR, dwflags : u32, hwnd : super::super::Foundation:: HWND, ppraseapuseridentity : *mut *mut RASEAPUSERIDENTITYA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetEapUserIdentityW(pszphonebook : ::windows_sys::core::PCWSTR, pszentry : ::windows_sys::core::PCWSTR, dwflags : u32, hwnd : super::super::Foundation:: HWND, ppraseapuseridentity : *mut *mut RASEAPUSERIDENTITYW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetEntryDialParamsA(param0 : ::windows_sys::core::PCSTR, param1 : *mut RASDIALPARAMSA, param2 : *mut super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasGetEntryDialParamsW(param0 : ::windows_sys::core::PCWSTR, param1 : *mut RASDIALPARAMSW, param2 : *mut super::super::Foundation:: BOOL) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn RasGetEntryPropertiesA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : *mut RASENTRYA, param3 : *mut u32, param4 : *mut u8, param5 : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn RasGetEntryPropertiesW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : *mut RASENTRYW, param3 : *mut u32, param4 : *mut u8, param5 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetErrorStringA(resourceid : u32, lpszstring : ::windows_sys::core::PSTR, inbufsize : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetErrorStringW(resourceid : u32, lpszstring : ::windows_sys::core::PWSTR, inbufsize : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetLinkStatistics(hrasconn : HRASCONN, dwsubentry : u32, lpstatistics : *mut RAS_STATS) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetPCscf(lpszpcscf : ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetProjectionInfoA(param0 : HRASCONN, param1 : RASPROJECTION, param2 : *mut ::core::ffi::c_void, param3 : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn RasGetProjectionInfoEx(hrasconn : HRASCONN, prasprojection : *mut RAS_PROJECTION_INFO, lpdwsize : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetProjectionInfoW(param0 : HRASCONN, param1 : RASPROJECTION, param2 : *mut ::core::ffi::c_void, param3 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetSubEntryHandleA(param0 : HRASCONN, param1 : u32, param2 : *mut HRASCONN) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetSubEntryHandleW(param0 : HRASCONN, param1 : u32, param2 : *mut HRASCONN) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetSubEntryPropertiesA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : u32, param3 : *mut RASSUBENTRYA, param4 : *mut u32, param5 : *mut u8, param6 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasGetSubEntryPropertiesW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : u32, param3 : *mut RASSUBENTRYW, param4 : *mut u32, param5 : *mut u8, param6 : *mut u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasHangUpA(param0 : HRASCONN) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasHangUpW(param0 : HRASCONN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasInvokeEapUI(param0 : HRASCONN, param1 : u32, param2 : *const RASDIALEXTENSIONS, param3 : super::super::Foundation:: HWND) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasdlg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasPhonebookDlgA(lpszphonebook : ::windows_sys::core::PCSTR, lpszentry : ::windows_sys::core::PCSTR, lpinfo : *mut RASPBDLGA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasdlg.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasPhonebookDlgW(lpszphonebook : ::windows_sys::core::PCWSTR, lpszentry : ::windows_sys::core::PCWSTR, lpinfo : *mut RASPBDLGW) -> super::super::Foundation:: BOOL);
::windows_targets::link!("rasapi32.dll" "system" fn RasRenameEntryA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasRenameEntryW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetAutodialAddressA(param0 : ::windows_sys::core::PCSTR, param1 : u32, param2 : *const RASAUTODIALENTRYA, param3 : u32, param4 : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetAutodialAddressW(param0 : ::windows_sys::core::PCWSTR, param1 : u32, param2 : *const RASAUTODIALENTRYW, param3 : u32, param4 : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetAutodialEnableA(param0 : u32, param1 : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetAutodialEnableW(param0 : u32, param1 : super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetAutodialParamA(param0 : u32, param1 : *const ::core::ffi::c_void, param2 : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetAutodialParamW(param0 : u32, param1 : *const ::core::ffi::c_void, param2 : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetCredentialsA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : *const RASCREDENTIALSA, param3 : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetCredentialsW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : *const RASCREDENTIALSW, param3 : super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetCustomAuthDataA(pszphonebook : ::windows_sys::core::PCSTR, pszentry : ::windows_sys::core::PCSTR, pbcustomauthdata : *const u8, dwsizeofcustomauthdata : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetCustomAuthDataW(pszphonebook : ::windows_sys::core::PCWSTR, pszentry : ::windows_sys::core::PCWSTR, pbcustomauthdata : *const u8, dwsizeofcustomauthdata : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetEapUserDataA(htoken : super::super::Foundation:: HANDLE, pszphonebook : ::windows_sys::core::PCSTR, pszentry : ::windows_sys::core::PCSTR, pbeapdata : *const u8, dwsizeofeapdata : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetEapUserDataW(htoken : super::super::Foundation:: HANDLE, pszphonebook : ::windows_sys::core::PCWSTR, pszentry : ::windows_sys::core::PCWSTR, pbeapdata : *const u8, dwsizeofeapdata : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetEntryDialParamsA(param0 : ::windows_sys::core::PCSTR, param1 : *const RASDIALPARAMSA, param2 : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RasSetEntryDialParamsW(param0 : ::windows_sys::core::PCWSTR, param1 : *const RASDIALPARAMSW, param2 : super::super::Foundation:: BOOL) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn RasSetEntryPropertiesA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : *const RASENTRYA, param3 : u32, param4 : *const u8, param5 : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn RasSetEntryPropertiesW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : *const RASENTRYW, param3 : u32, param4 : *const u8, param5 : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetSubEntryPropertiesA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR, param2 : u32, param3 : *const RASSUBENTRYA, param4 : u32, param5 : *const u8, param6 : u32) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasSetSubEntryPropertiesW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR, param2 : u32, param3 : *const RASSUBENTRYW, param4 : u32, param5 : *const u8, param6 : u32) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("rasapi32.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn RasUpdateConnection(hrasconn : HRASCONN, lprasupdateconn : *const RASUPDATECONN) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasValidateEntryNameA(param0 : ::windows_sys::core::PCSTR, param1 : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("rasapi32.dll" "system" fn RasValidateEntryNameW(param0 : ::windows_sys::core::PCWSTR, param1 : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmAddNextHop(rtmreghandle : isize, nexthopinfo : *mut RTM_NEXTHOP_INFO, nexthophandle : *mut isize, changeflags : *mut u32) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmAddRouteToDest(rtmreghandle : isize, routehandle : *mut isize, destaddress : *mut RTM_NET_ADDRESS, routeinfo : *mut RTM_ROUTE_INFO, timetolive : u32, routelisthandle : isize, notifytype : u32, notifyhandle : isize, changeflags : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmBlockMethods(rtmreghandle : isize, targethandle : super::super::Foundation:: HANDLE, targettype : u8, blockingflag : u32) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn RtmConvertIpv6AddressAndLengthToNetAddress(pnetaddress : *mut RTM_NET_ADDRESS, address : super::super::Networking::WinSock:: IN6_ADDR, dwlength : u32, dwaddresssize : u32) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn RtmConvertNetAddressToIpv6AddressAndLength(pnetaddress : *mut RTM_NET_ADDRESS, paddress : *mut super::super::Networking::WinSock:: IN6_ADDR, plength : *mut u32, dwaddresssize : u32) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmCreateDestEnum(rtmreghandle : isize, targetviews : u32, enumflags : u32, netaddress : *mut RTM_NET_ADDRESS, protocolid : u32, rtmenumhandle : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmCreateNextHopEnum(rtmreghandle : isize, enumflags : u32, netaddress : *mut RTM_NET_ADDRESS, rtmenumhandle : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmCreateRouteEnum(rtmreghandle : isize, desthandle : isize, targetviews : u32, enumflags : u32, startdest : *mut RTM_NET_ADDRESS, matchingflags : u32, criteriaroute : *mut RTM_ROUTE_INFO, criteriainterface : u32, rtmenumhandle : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmCreateRouteList(rtmreghandle : isize, routelisthandle : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmCreateRouteListEnum(rtmreghandle : isize, routelisthandle : isize, rtmenumhandle : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmDeleteEnumHandle(rtmreghandle : isize, enumhandle : isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmDeleteNextHop(rtmreghandle : isize, nexthophandle : isize, nexthopinfo : *mut RTM_NEXTHOP_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmDeleteRouteList(rtmreghandle : isize, routelisthandle : isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmDeleteRouteToDest(rtmreghandle : isize, routehandle : isize, changeflags : *mut u32) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmDeregisterEntity(rtmreghandle : isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmDeregisterFromChangeNotification(rtmreghandle : isize, notifyhandle : isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmFindNextHop(rtmreghandle : isize, nexthopinfo : *mut RTM_NEXTHOP_INFO, nexthophandle : *mut isize, nexthoppointer : *mut *mut RTM_NEXTHOP_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmGetChangeStatus(rtmreghandle : isize, notifyhandle : isize, desthandle : isize, changestatus : *mut super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmGetChangedDests(rtmreghandle : isize, notifyhandle : isize, numdests : *mut u32, changeddests : *mut RTM_DEST_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmGetDestInfo(rtmreghandle : isize, desthandle : isize, protocolid : u32, targetviews : u32, destinfo : *mut RTM_DEST_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetEntityInfo(rtmreghandle : isize, entityhandle : isize, entityinfo : *mut RTM_ENTITY_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetEntityMethods(rtmreghandle : isize, entityhandle : isize, nummethods : *mut u32, exptmethods : *mut RTM_ENTITY_EXPORT_METHOD) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmGetEnumDests(rtmreghandle : isize, enumhandle : isize, numdests : *mut u32, destinfos : *mut RTM_DEST_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetEnumNextHops(rtmreghandle : isize, enumhandle : isize, numnexthops : *mut u32, nexthophandles : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetEnumRoutes(rtmreghandle : isize, enumhandle : isize, numroutes : *mut u32, routehandles : *mut isize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmGetExactMatchDestination(rtmreghandle : isize, destaddress : *mut RTM_NET_ADDRESS, protocolid : u32, targetviews : u32, destinfo : *mut RTM_DEST_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetExactMatchRoute(rtmreghandle : isize, destaddress : *mut RTM_NET_ADDRESS, matchingflags : u32, routeinfo : *mut RTM_ROUTE_INFO, interfaceindex : u32, targetviews : u32, routehandle : *mut isize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmGetLessSpecificDestination(rtmreghandle : isize, desthandle : isize, protocolid : u32, targetviews : u32, destinfo : *mut RTM_DEST_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetListEnumRoutes(rtmreghandle : isize, enumhandle : isize, numroutes : *mut u32, routehandles : *mut isize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmGetMostSpecificDestination(rtmreghandle : isize, destaddress : *mut RTM_NET_ADDRESS, protocolid : u32, targetviews : u32, destinfo : *mut RTM_DEST_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetNextHopInfo(rtmreghandle : isize, nexthophandle : isize, nexthopinfo : *mut RTM_NEXTHOP_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetNextHopPointer(rtmreghandle : isize, nexthophandle : isize, nexthoppointer : *mut *mut RTM_NEXTHOP_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetOpaqueInformationPointer(rtmreghandle : isize, desthandle : isize, opaqueinfopointer : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetRegisteredEntities(rtmreghandle : isize, numentities : *mut u32, entityhandles : *mut isize, entityinfos : *mut RTM_ENTITY_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetRouteInfo(rtmreghandle : isize, routehandle : isize, routeinfo : *mut RTM_ROUTE_INFO, destaddress : *mut RTM_NET_ADDRESS) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmGetRoutePointer(rtmreghandle : isize, routehandle : isize, routepointer : *mut *mut RTM_ROUTE_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmHoldDestination(rtmreghandle : isize, desthandle : isize, targetviews : u32, holdtime : u32) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmIgnoreChangedDests(rtmreghandle : isize, notifyhandle : isize, numdests : u32, changeddests : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmInsertInRouteList(rtmreghandle : isize, routelisthandle : isize, numroutes : u32, routehandles : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmInvokeMethod(rtmreghandle : isize, entityhandle : isize, input : *mut RTM_ENTITY_METHOD_INPUT, outputsize : *mut u32, output : *mut RTM_ENTITY_METHOD_OUTPUT) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmIsBestRoute(rtmreghandle : isize, routehandle : isize, bestinviews : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmIsMarkedForChangeNotification(rtmreghandle : isize, notifyhandle : isize, desthandle : isize, destmarked : *mut super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmLockDestination(rtmreghandle : isize, desthandle : isize, exclusive : super::super::Foundation:: BOOL, lockdest : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmLockNextHop(rtmreghandle : isize, nexthophandle : isize, exclusive : super::super::Foundation:: BOOL, locknexthop : super::super::Foundation:: BOOL, nexthoppointer : *mut *mut RTM_NEXTHOP_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmLockRoute(rtmreghandle : isize, routehandle : isize, exclusive : super::super::Foundation:: BOOL, lockroute : super::super::Foundation:: BOOL, routepointer : *mut *mut RTM_ROUTE_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmMarkDestForChangeNotification(rtmreghandle : isize, notifyhandle : isize, desthandle : isize, markdest : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmReferenceHandles(rtmreghandle : isize, numhandles : u32, rtmhandles : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmRegisterEntity(rtmentityinfo : *mut RTM_ENTITY_INFO, exportmethods : *mut RTM_ENTITY_EXPORT_METHODS, eventcallback : RTM_EVENT_CALLBACK, reserveopaquepointer : super::super::Foundation:: BOOL, rtmregprofile : *mut RTM_REGN_PROFILE, rtmreghandle : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmRegisterForChangeNotification(rtmreghandle : isize, targetviews : u32, notifyflags : u32, notifycontext : *mut ::core::ffi::c_void, notifyhandle : *mut isize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmReleaseChangedDests(rtmreghandle : isize, notifyhandle : isize, numdests : u32, changeddests : *mut RTM_DEST_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmReleaseDestInfo(rtmreghandle : isize, destinfo : *mut RTM_DEST_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rtm.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtmReleaseDests(rtmreghandle : isize, numdests : u32, destinfos : *mut RTM_DEST_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmReleaseEntities(rtmreghandle : isize, numentities : u32, entityhandles : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmReleaseEntityInfo(rtmreghandle : isize, entityinfo : *mut RTM_ENTITY_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmReleaseNextHopInfo(rtmreghandle : isize, nexthopinfo : *mut RTM_NEXTHOP_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmReleaseNextHops(rtmreghandle : isize, numnexthops : u32, nexthophandles : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmReleaseRouteInfo(rtmreghandle : isize, routeinfo : *mut RTM_ROUTE_INFO) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmReleaseRoutes(rtmreghandle : isize, numroutes : u32, routehandles : *mut isize) -> u32);
::windows_targets::link!("rtm.dll" "system" fn RtmUpdateAndUnlockRoute(rtmreghandle : isize, routehandle : isize, timetolive : u32, routelisthandle : isize, notifytype : u32, notifyhandle : isize, changeflags : *mut u32) -> u32);
pub const ALLOW_NO_AUTH: u32 = 1u32;
pub const ALL_SOURCES: MGM_ENUM_TYPES = 1i32;
pub const ANY_SOURCE: MGM_ENUM_TYPES = 0i32;
pub const ATADDRESSLEN: u32 = 32u32;
pub const DO_NOT_ALLOW_NO_AUTH: u32 = 0u32;
pub const ERROR_ACCESSING_TCPCFGDLL: u32 = 727u32;
pub const ERROR_ACCT_DISABLED: u32 = 647u32;
pub const ERROR_ACCT_EXPIRED: u32 = 708u32;
pub const ERROR_ACTION_REQUIRED: u32 = 877u32;
pub const ERROR_ALLOCATING_MEMORY: u32 = 664u32;
pub const ERROR_ALREADY_DISCONNECTING: u32 = 617u32;
pub const ERROR_ASYNC_REQUEST_PENDING: u32 = 616u32;
pub const ERROR_AUTHENTICATION_FAILURE: u32 = 691u32;
pub const ERROR_AUTH_INTERNAL: u32 = 645u32;
pub const ERROR_AUTOMATIC_VPN_FAILED: u32 = 800u32;
pub const ERROR_BAD_ADDRESS_SPECIFIED: u32 = 769u32;
pub const ERROR_BAD_CALLBACK_NUMBER: u32 = 704u32;
pub const ERROR_BAD_PHONE_NUMBER: u32 = 749u32;
pub const ERROR_BAD_STRING: u32 = 637u32;
pub const ERROR_BAD_USAGE_IN_INI_FILE: u32 = 669u32;
pub const ERROR_BIPLEX_PORT_NOT_AVAILABLE: u32 = 712u32;
pub const ERROR_BLOCKED: u32 = 775u32;
pub const ERROR_BROADBAND_ACTIVE: u32 = 813u32;
pub const ERROR_BROADBAND_NO_NIC: u32 = 814u32;
pub const ERROR_BROADBAND_TIMEOUT: u32 = 815u32;
pub const ERROR_BUFFER_INVALID: u32 = 610u32;
pub const ERROR_BUFFER_TOO_SMALL: u32 = 603u32;
pub const ERROR_BUNDLE_NOT_FOUND: u32 = 754u32;
pub const ERROR_CANNOT_DELETE: u32 = 817u32;
pub const ERROR_CANNOT_DO_CUSTOMDIAL: u32 = 755u32;
pub const ERROR_CANNOT_FIND_PHONEBOOK_ENTRY: u32 = 623u32;
pub const ERROR_CANNOT_GET_LANA: u32 = 639u32;
pub const ERROR_CANNOT_INITIATE_MOBIKE_UPDATE: u32 = 844u32;
pub const ERROR_CANNOT_LOAD_PHONEBOOK: u32 = 622u32;
pub const ERROR_CANNOT_LOAD_STRING: u32 = 626u32;
pub const ERROR_CANNOT_OPEN_PHONEBOOK: u32 = 621u32;
pub const ERROR_CANNOT_PROJECT_CLIENT: u32 = 634u32;
pub const ERROR_CANNOT_SET_PORT_INFO: u32 = 605u32;
pub const ERROR_CANNOT_SHARE_CONNECTION: u32 = 763u32;
pub const ERROR_CANNOT_USE_LOGON_CREDENTIALS: u32 = 739u32;
pub const ERROR_CANNOT_WRITE_PHONEBOOK: u32 = 624u32;
pub const ERROR_CERT_FOR_ENCRYPTION_NOT_FOUND: u32 = 781u32;
pub const ERROR_CHANGING_PASSWORD: u32 = 709u32;
pub const ERROR_CMD_TOO_LONG: u32 = 700u32;
pub const ERROR_CONGESTION: u32 = 771u32;
pub const ERROR_CONNECTING_DEVICE_NOT_FOUND: u32 = 797u32;
pub const ERROR_CONNECTION_ALREADY_SHARED: u32 = 758u32;
pub const ERROR_CONNECTION_REJECT: u32 = 770u32;
pub const ERROR_CORRUPT_PHONEBOOK: u32 = 625u32;
pub const ERROR_DCB_NOT_FOUND: u32 = 694u32;
pub const ERROR_DEFAULTOFF_MACRO_NOT_FOUND: u32 = 656u32;
pub const ERROR_DEVICENAME_NOT_FOUND: u32 = 659u32;
pub const ERROR_DEVICENAME_TOO_LONG: u32 = 658u32;
pub const ERROR_DEVICETYPE_DOES_NOT_EXIST: u32 = 609u32;
pub const ERROR_DEVICE_COMPLIANCE: u32 = 875u32;
pub const ERROR_DEVICE_DOES_NOT_EXIST: u32 = 608u32;
pub const ERROR_DEVICE_NOT_READY: u32 = 666u32;
pub const ERROR_DIAL_ALREADY_IN_PROGRESS: u32 = 756u32;
pub const ERROR_DISCONNECTION: u32 = 628u32;
pub const ERROR_DNSNAME_NOT_RESOLVABLE: u32 = 868u32;
pub const ERROR_DONOTDISTURB: u32 = 776u32;
pub const ERROR_EAPTLS_CACHE_CREDENTIALS_INVALID: u32 = 826u32;
pub const ERROR_EAPTLS_PASSWD_INVALID: u32 = 869u32;
pub const ERROR_EAPTLS_SCARD_CACHE_CREDENTIALS_INVALID: u32 = 847u32;
pub const ERROR_EAP_METHOD_DOES_NOT_SUPPORT_SSO: u32 = 851u32;
pub const ERROR_EAP_METHOD_NOT_INSTALLED: u32 = 850u32;
pub const ERROR_EAP_METHOD_OPERATION_NOT_SUPPORTED: u32 = 852u32;
pub const ERROR_EAP_SERVER_CERT_EXPIRED: u32 = 858u32;
pub const ERROR_EAP_SERVER_CERT_INVALID: u32 = 857u32;
pub const ERROR_EAP_SERVER_CERT_OTHER_ERROR: u32 = 860u32;
pub const ERROR_EAP_SERVER_CERT_REVOKED: u32 = 859u32;
pub const ERROR_EAP_SERVER_ROOT_CERT_INVALID: u32 = 865u32;
pub const ERROR_EAP_SERVER_ROOT_CERT_NAME_REQUIRED: u32 = 866u32;
pub const ERROR_EAP_SERVER_ROOT_CERT_NOT_FOUND: u32 = 864u32;
pub const ERROR_EAP_USER_CERT_EXPIRED: u32 = 854u32;
pub const ERROR_EAP_USER_CERT_INVALID: u32 = 853u32;
pub const ERROR_EAP_USER_CERT_OTHER_ERROR: u32 = 856u32;
pub const ERROR_EAP_USER_CERT_REVOKED: u32 = 855u32;
pub const ERROR_EAP_USER_ROOT_CERT_EXPIRED: u32 = 863u32;
pub const ERROR_EAP_USER_ROOT_CERT_INVALID: u32 = 862u32;
pub const ERROR_EAP_USER_ROOT_CERT_NOT_FOUND: u32 = 861u32;
pub const ERROR_EMPTY_INI_FILE: u32 = 690u32;
pub const ERROR_EVENT_INVALID: u32 = 607u32;
pub const ERROR_FAILED_CP_REQUIRED: u32 = 841u32;
pub const ERROR_FAILED_TO_ENCRYPT: u32 = 768u32;
pub const ERROR_FAST_USER_SWITCH: u32 = 831u32;
pub const ERROR_FEATURE_DEPRECATED: u32 = 816u32;
pub const ERROR_FILE_COULD_NOT_BE_OPENED: u32 = 657u32;
pub const ERROR_FROM_DEVICE: u32 = 651u32;
pub const ERROR_HANGUP_FAILED: u32 = 753u32;
pub const ERROR_HARDWARE_FAILURE: u32 = 630u32;
pub const ERROR_HIBERNATION: u32 = 832u32;
pub const ERROR_IDLE_TIMEOUT: u32 = 828u32;
pub const ERROR_IKEV2_PSK_INTERFACE_ALREADY_EXISTS: u32 = 870u32;
pub const ERROR_INCOMPATIBLE: u32 = 772u32;
pub const ERROR_INTERACTIVE_MODE: u32 = 703u32;
pub const ERROR_INTERNAL_ADDRESS_FAILURE: u32 = 840u32;
pub const ERROR_INVALID_AUTH_STATE: u32 = 705u32;
pub const ERROR_INVALID_CALLBACK_NUMBER: u32 = 751u32;
pub const ERROR_INVALID_COMPRESSION_SPECIFIED: u32 = 613u32;
pub const ERROR_INVALID_DESTINATION_IP: u32 = 871u32;
pub const ERROR_INVALID_FUNCTION_FOR_ENTRY: u32 = 780u32;
pub const ERROR_INVALID_INTERFACE_CONFIG: u32 = 872u32;
pub const ERROR_INVALID_MSCHAPV2_CONFIG: u32 = 805u32;
pub const ERROR_INVALID_PEAP_COOKIE_ATTRIBUTES: u32 = 849u32;
pub const ERROR_INVALID_PEAP_COOKIE_CONFIG: u32 = 803u32;
pub const ERROR_INVALID_PEAP_COOKIE_USER: u32 = 804u32;
pub const ERROR_INVALID_PORT_HANDLE: u32 = 601u32;
pub const ERROR_INVALID_PREFERENCES: u32 = 846u32;
pub const ERROR_INVALID_SERVER_CERT: u32 = 835u32;
pub const ERROR_INVALID_SIZE: u32 = 632u32;
pub const ERROR_INVALID_SMM: u32 = 745u32;
pub const ERROR_INVALID_TUNNELID: u32 = 837u32;
pub const ERROR_INVALID_VPNSTRATEGY: u32 = 825u32;
pub const ERROR_IN_COMMAND: u32 = 681u32;
pub const ERROR_IPSEC_SERVICE_STOPPED: u32 = 827u32;
pub const ERROR_IPXCP_DIALOUT_ALREADY_ACTIVE: u32 = 726u32;
pub const ERROR_IPXCP_NET_NUMBER_CONFLICT: u32 = 744u32;
pub const ERROR_IPXCP_NO_DIALIN_CONFIGURED: u32 = 725u32;
pub const ERROR_IPXCP_NO_DIALOUT_CONFIGURED: u32 = 724u32;
pub const ERROR_IP_CONFIGURATION: u32 = 716u32;
pub const ERROR_KEY_NOT_FOUND: u32 = 627u32;
pub const ERROR_LINE_BUSY: u32 = 676u32;
pub const ERROR_LINK_FAILURE: u32 = 829u32;
pub const ERROR_MACRO_NOT_DEFINED: u32 = 654u32;
pub const ERROR_MACRO_NOT_FOUND: u32 = 653u32;
pub const ERROR_MESSAGE_MACRO_NOT_FOUND: u32 = 655u32;
pub const ERROR_MOBIKE_DISABLED: u32 = 843u32;
pub const ERROR_NAME_EXISTS_ON_NET: u32 = 642u32;
pub const ERROR_NETBIOS_ERROR: u32 = 640u32;
pub const ERROR_NOT_BINARY_MACRO: u32 = 693u32;
pub const ERROR_NOT_NAP_CAPABLE: u32 = 836u32;
pub const ERROR_NO_ACTIVE_ISDN_LINES: u32 = 713u32;
pub const ERROR_NO_ANSWER: u32 = 678u32;
pub const ERROR_NO_CARRIER: u32 = 679u32;
pub const ERROR_NO_CERTIFICATE: u32 = 766u32;
pub const ERROR_NO_COMMAND_FOUND: u32 = 661u32;
pub const ERROR_NO_CONNECTION: u32 = 668u32;
pub const ERROR_NO_DIALIN_PERMISSION: u32 = 649u32;
pub const ERROR_NO_DIALTONE: u32 = 680u32;
pub const ERROR_NO_DIFF_USER_AT_LOGON: u32 = 784u32;
pub const ERROR_NO_EAPTLS_CERTIFICATE: u32 = 798u32;
pub const ERROR_NO_ENDPOINTS: u32 = 620u32;
pub const ERROR_NO_IP_ADDRESSES: u32 = 717u32;
pub const ERROR_NO_IP_RAS_ADAPTER: u32 = 728u32;
pub const ERROR_NO_ISDN_CHANNELS_AVAILABLE: u32 = 714u32;
pub const ERROR_NO_LOCAL_ENCRYPTION: u32 = 741u32;
pub const ERROR_NO_MAC_FOR_PORT: u32 = 747u32;
pub const ERROR_NO_REG_CERT_AT_LOGON: u32 = 785u32;
pub const ERROR_NO_REMOTE_ENCRYPTION: u32 = 742u32;
pub const ERROR_NO_RESPONSES: u32 = 660u32;
pub const ERROR_NO_SMART_CARD_READER: u32 = 764u32;
pub const ERROR_NUMBERCHANGED: u32 = 773u32;
pub const ERROR_OAKLEY_ATTRIB_FAIL: u32 = 788u32;
pub const ERROR_OAKLEY_AUTH_FAIL: u32 = 787u32;
pub const ERROR_OAKLEY_ERROR: u32 = 793u32;
pub const ERROR_OAKLEY_GENERAL_PROCESSING: u32 = 789u32;
pub const ERROR_OAKLEY_NO_CERT: u32 = 786u32;
pub const ERROR_OAKLEY_NO_PEER_CERT: u32 = 790u32;
pub const ERROR_OAKLEY_NO_POLICY: u32 = 791u32;
pub const ERROR_OAKLEY_TIMED_OUT: u32 = 792u32;
pub const ERROR_OUTOFORDER: u32 = 777u32;
pub const ERROR_OUT_OF_BUFFERS: u32 = 614u32;
pub const ERROR_OVERRUN: u32 = 710u32;
pub const ERROR_PARTIAL_RESPONSE_LOOPING: u32 = 697u32;
pub const ERROR_PASSWD_EXPIRED: u32 = 648u32;
pub const ERROR_PEAP_CRYPTOBINDING_INVALID: u32 = 823u32;
pub const ERROR_PEAP_CRYPTOBINDING_NOTRECEIVED: u32 = 824u32;
pub const ERROR_PEAP_IDENTITY_MISMATCH: u32 = 867u32;
pub const ERROR_PEAP_SERVER_REJECTED_CLIENT_TLV: u32 = 845u32;
pub const ERROR_PHONE_NUMBER_TOO_LONG: u32 = 723u32;
pub const ERROR_PLUGIN_NOT_INSTALLED: u32 = 876u32;
pub const ERROR_PORT_ALREADY_OPEN: u32 = 602u32;
pub const ERROR_PORT_DISCONNECTED: u32 = 619u32;
pub const ERROR_PORT_NOT_AVAILABLE: u32 = 633u32;
pub const ERROR_PORT_NOT_CONFIGURED: u32 = 665u32;
pub const ERROR_PORT_NOT_CONNECTED: u32 = 606u32;
pub const ERROR_PORT_NOT_FOUND: u32 = 615u32;
pub const ERROR_PORT_NOT_OPEN: u32 = 618u32;
pub const ERROR_PORT_OR_DEVICE: u32 = 692u32;
pub const ERROR_PPP_CP_REJECTED: u32 = 733u32;
pub const ERROR_PPP_INVALID_PACKET: u32 = 722u32;
pub const ERROR_PPP_LCP_TERMINATED: u32 = 734u32;
pub const ERROR_PPP_LOOPBACK_DETECTED: u32 = 737u32;
pub const ERROR_PPP_NCP_TERMINATED: u32 = 736u32;
pub const ERROR_PPP_NOT_CONVERGING: u32 = 732u32;
pub const ERROR_PPP_NO_ADDRESS_ASSIGNED: u32 = 738u32;
pub const ERROR_PPP_NO_PROTOCOLS_CONFIGURED: u32 = 720u32;
pub const ERROR_PPP_NO_RESPONSE: u32 = 721u32;
pub const ERROR_PPP_REMOTE_TERMINATED: u32 = 719u32;
pub const ERROR_PPP_REQUIRED_ADDRESS_REJECTED: u32 = 735u32;
pub const ERROR_PPP_TIMEOUT: u32 = 718u32;
pub const ERROR_PROJECTION_NOT_COMPLETE: u32 = 730u32;
pub const ERROR_PROTOCOL_ENGINE_DISABLED: u32 = 839u32;
pub const ERROR_PROTOCOL_NOT_CONFIGURED: u32 = 731u32;
pub const ERROR_RASAUTO_CANNOT_INITIALIZE: u32 = 757u32;
pub const ERROR_RASMAN_CANNOT_INITIALIZE: u32 = 711u32;
pub const ERROR_RASMAN_SERVICE_STOPPED: u32 = 834u32;
pub const ERROR_RASQEC_CONN_DOESNOTEXIST: u32 = 821u32;
pub const ERROR_RASQEC_NAPAGENT_NOT_CONNECTED: u32 = 820u32;
pub const ERROR_RASQEC_NAPAGENT_NOT_ENABLED: u32 = 819u32;
pub const ERROR_RASQEC_RESOURCE_CREATION_FAILED: u32 = 818u32;
pub const ERROR_RASQEC_TIMEOUT: u32 = 822u32;
pub const ERROR_READING_DEFAULTOFF: u32 = 689u32;
pub const ERROR_READING_DEVICENAME: u32 = 672u32;
pub const ERROR_READING_DEVICETYPE: u32 = 671u32;
pub const ERROR_READING_INI_FILE: u32 = 667u32;
pub const ERROR_READING_MAXCARRIERBPS: u32 = 675u32;
pub const ERROR_READING_MAXCONNECTBPS: u32 = 674u32;
pub const ERROR_READING_SCARD: u32 = 802u32;
pub const ERROR_READING_SECTIONNAME: u32 = 670u32;
pub const ERROR_READING_USAGE: u32 = 673u32;
pub const ERROR_RECV_BUF_FULL: u32 = 699u32;
pub const ERROR_REMOTE_DISCONNECTION: u32 = 629u32;
pub const ERROR_REMOTE_REQUIRES_ENCRYPTION: u32 = 743u32;
pub const ERROR_REQUEST_TIMEOUT: u32 = 638u32;
pub const ERROR_RESTRICTED_LOGON_HOURS: u32 = 646u32;
pub const ERROR_ROUTE_NOT_ALLOCATED: u32 = 612u32;
pub const ERROR_ROUTE_NOT_AVAILABLE: u32 = 611u32;
pub const ERROR_SCRIPT_SYNTAX: u32 = 752u32;
pub const ERROR_SERVER_GENERAL_NET_FAILURE: u32 = 643u32;
pub const ERROR_SERVER_NOT_RESPONDING: u32 = 650u32;
pub const ERROR_SERVER_OUT_OF_RESOURCES: u32 = 641u32;
pub const ERROR_SERVER_POLICY: u32 = 812u32;
pub const ERROR_SHARE_CONNECTION_FAILED: u32 = 761u32;
pub const ERROR_SHARING_ADDRESS_EXISTS: u32 = 765u32;
pub const ERROR_SHARING_CHANGE_FAILED: u32 = 759u32;
pub const ERROR_SHARING_HOST_ADDRESS_CONFLICT: u32 = 799u32;
pub const ERROR_SHARING_MULTIPLE_ADDRESSES: u32 = 767u32;
pub const ERROR_SHARING_NO_PRIVATE_LAN: u32 = 783u32;
pub const ERROR_SHARING_PRIVATE_INSTALL: u32 = 762u32;
pub const ERROR_SHARING_ROUTER_INSTALL: u32 = 760u32;
pub const ERROR_SHARING_RRAS_CONFLICT: u32 = 782u32;
pub const ERROR_SLIP_REQUIRES_IP: u32 = 729u32;
pub const ERROR_SMART_CARD_REQUIRED: u32 = 779u32;
pub const ERROR_SMM_TIMEOUT: u32 = 748u32;
pub const ERROR_SMM_UNINITIALIZED: u32 = 746u32;
pub const ERROR_SSO_CERT_MISSING: u32 = 874u32;
pub const ERROR_SSTP_COOKIE_SET_FAILURE: u32 = 848u32;
pub const ERROR_STATE_MACHINES_ALREADY_STARTED: u32 = 696u32;
pub const ERROR_STATE_MACHINES_NOT_STARTED: u32 = 695u32;
pub const ERROR_SYSTEM_SUSPENDED: u32 = 833u32;
pub const ERROR_TAPI_CONFIGURATION: u32 = 740u32;
pub const ERROR_TEMPFAILURE: u32 = 774u32;
pub const ERROR_TOO_MANY_LINE_ERRORS: u32 = 715u32;
pub const ERROR_TS_UNACCEPTABLE: u32 = 842u32;
pub const ERROR_UNABLE_TO_AUTHENTICATE_SERVER: u32 = 778u32;
pub const ERROR_UNEXPECTED_RESPONSE: u32 = 702u32;
pub const ERROR_UNKNOWN: u32 = 635u32;
pub const ERROR_UNKNOWN_DEVICE_TYPE: u32 = 663u32;
pub const ERROR_UNKNOWN_FRAMED_PROTOCOL: u32 = 794u32;
pub const ERROR_UNKNOWN_RESPONSE_KEY: u32 = 698u32;
pub const ERROR_UNKNOWN_SERVICE_TYPE: u32 = 796u32;
pub const ERROR_UNRECOGNIZED_RESPONSE: u32 = 652u32;
pub const ERROR_UNSUPPORTED_BPS: u32 = 701u32;
pub const ERROR_UPDATECONNECTION_REQUEST_IN_PROCESS: u32 = 838u32;
pub const ERROR_USER_DISCONNECTION: u32 = 631u32;
pub const ERROR_USER_LOGOFF: u32 = 830u32;
pub const ERROR_VALIDATING_SERVER_CERT: u32 = 801u32;
pub const ERROR_VOICE_ANSWER: u32 = 677u32;
pub const ERROR_VPN_BAD_CERT: u32 = 810u32;
pub const ERROR_VPN_BAD_PSK: u32 = 811u32;
pub const ERROR_VPN_DISCONNECT: u32 = 807u32;
pub const ERROR_VPN_GRE_BLOCKED: u32 = 806u32;
pub const ERROR_VPN_PLUGIN_GENERIC: u32 = 873u32;
pub const ERROR_VPN_REFUSED: u32 = 808u32;
pub const ERROR_VPN_TIMEOUT: u32 = 809u32;
pub const ERROR_WRITING_DEFAULTOFF: u32 = 688u32;
pub const ERROR_WRITING_DEVICENAME: u32 = 684u32;
pub const ERROR_WRITING_DEVICETYPE: u32 = 683u32;
pub const ERROR_WRITING_INITBPS: u32 = 706u32;
pub const ERROR_WRITING_MAXCARRIERBPS: u32 = 686u32;
pub const ERROR_WRITING_MAXCONNECTBPS: u32 = 685u32;
pub const ERROR_WRITING_SECTIONNAME: u32 = 682u32;
pub const ERROR_WRITING_USAGE: u32 = 687u32;
pub const ERROR_WRONG_DEVICE_ATTACHED: u32 = 636u32;
pub const ERROR_WRONG_INFO_SPECIFIED: u32 = 604u32;
pub const ERROR_WRONG_KEY_SPECIFIED: u32 = 662u32;
pub const ERROR_WRONG_MODULE: u32 = 750u32;
pub const ERROR_WRONG_TUNNEL_TYPE: u32 = 795u32;
pub const ERROR_X25_DIAGNOSTIC: u32 = 707u32;
pub const ET_None: u32 = 0u32;
pub const ET_Optional: u32 = 3u32;
pub const ET_Require: u32 = 1u32;
pub const ET_RequireMax: u32 = 2u32;
pub const IKEV2_ID_PAYLOAD_TYPE_DER_ASN1_DN: IKEV2_ID_PAYLOAD_TYPE = 9i32;
pub const IKEV2_ID_PAYLOAD_TYPE_DER_ASN1_GN: IKEV2_ID_PAYLOAD_TYPE = 10i32;
pub const IKEV2_ID_PAYLOAD_TYPE_FQDN: IKEV2_ID_PAYLOAD_TYPE = 2i32;
pub const IKEV2_ID_PAYLOAD_TYPE_ID_IPV6_ADDR: IKEV2_ID_PAYLOAD_TYPE = 5i32;
pub const IKEV2_ID_PAYLOAD_TYPE_INVALID: IKEV2_ID_PAYLOAD_TYPE = 0i32;
pub const IKEV2_ID_PAYLOAD_TYPE_IPV4_ADDR: IKEV2_ID_PAYLOAD_TYPE = 1i32;
pub const IKEV2_ID_PAYLOAD_TYPE_KEY_ID: IKEV2_ID_PAYLOAD_TYPE = 11i32;
pub const IKEV2_ID_PAYLOAD_TYPE_MAX: IKEV2_ID_PAYLOAD_TYPE = 12i32;
pub const IKEV2_ID_PAYLOAD_TYPE_RESERVED1: IKEV2_ID_PAYLOAD_TYPE = 4i32;
pub const IKEV2_ID_PAYLOAD_TYPE_RESERVED2: IKEV2_ID_PAYLOAD_TYPE = 6i32;
pub const IKEV2_ID_PAYLOAD_TYPE_RESERVED3: IKEV2_ID_PAYLOAD_TYPE = 7i32;
pub const IKEV2_ID_PAYLOAD_TYPE_RESERVED4: IKEV2_ID_PAYLOAD_TYPE = 8i32;
pub const IKEV2_ID_PAYLOAD_TYPE_RFC822_ADDR: IKEV2_ID_PAYLOAD_TYPE = 3i32;
pub const IPADDRESSLEN: u32 = 15u32;
pub const IPV6_ADDRESS_LEN_IN_BYTES: u32 = 16u32;
pub const IPXADDRESSLEN: u32 = 22u32;
pub const MAXIPADRESSLEN: u32 = 64u32;
pub const MAX_SSTP_HASH_SIZE: u32 = 32u32;
pub const METHOD_BGP4_AS_PATH: u32 = 1u32;
pub const METHOD_BGP4_NEXTHOP_ATTR: u32 = 8u32;
pub const METHOD_BGP4_PA_ORIGIN: u32 = 4u32;
pub const METHOD_BGP4_PEER_ID: u32 = 2u32;
pub const METHOD_RIP2_NEIGHBOUR_ADDR: u32 = 1u32;
pub const METHOD_RIP2_OUTBOUND_INTF: u32 = 2u32;
pub const METHOD_RIP2_ROUTE_TAG: u32 = 4u32;
pub const METHOD_RIP2_ROUTE_TIMESTAMP: u32 = 8u32;
pub const METHOD_TYPE_ALL_METHODS: u32 = 4294967295u32;
pub const MGM_FORWARD_STATE_FLAG: u32 = 2u32;
pub const MGM_JOIN_STATE_FLAG: u32 = 1u32;
pub const MGM_MFE_STATS_0: u32 = 1u32;
pub const MGM_MFE_STATS_1: u32 = 2u32;
pub const MPRAPI_ADMIN_DLL_VERSION_1: u32 = 1u32;
pub const MPRAPI_ADMIN_DLL_VERSION_2: u32 = 2u32;
pub const MPRAPI_IF_CUSTOM_CONFIG_FOR_IKEV2: u32 = 1u32;
pub const MPRAPI_IKEV2_AUTH_USING_CERT: u32 = 1u32;
pub const MPRAPI_IKEV2_AUTH_USING_EAP: u32 = 2u32;
pub const MPRAPI_IKEV2_PROJECTION_INFO_TYPE: u32 = 2u32;
pub const MPRAPI_IKEV2_SET_TUNNEL_CONFIG_PARAMS: u32 = 1u32;
pub const MPRAPI_L2TP_SET_TUNNEL_CONFIG_PARAMS: u32 = 1u32;
pub const MPRAPI_MPR_IF_CUSTOM_CONFIG_OBJECT_REVISION_1: u32 = 1u32;
pub const MPRAPI_MPR_IF_CUSTOM_CONFIG_OBJECT_REVISION_2: u32 = 2u32;
pub const MPRAPI_MPR_IF_CUSTOM_CONFIG_OBJECT_REVISION_3: u32 = 3u32;
pub const MPRAPI_MPR_SERVER_OBJECT_REVISION_1: u32 = 1u32;
pub const MPRAPI_MPR_SERVER_OBJECT_REVISION_2: u32 = 2u32;
pub const MPRAPI_MPR_SERVER_OBJECT_REVISION_3: u32 = 3u32;
pub const MPRAPI_MPR_SERVER_OBJECT_REVISION_4: u32 = 4u32;
pub const MPRAPI_MPR_SERVER_OBJECT_REVISION_5: u32 = 5u32;
pub const MPRAPI_MPR_SERVER_SET_CONFIG_OBJECT_REVISION_1: u32 = 1u32;
pub const MPRAPI_MPR_SERVER_SET_CONFIG_OBJECT_REVISION_2: u32 = 2u32;
pub const MPRAPI_MPR_SERVER_SET_CONFIG_OBJECT_REVISION_3: u32 = 3u32;
pub const MPRAPI_MPR_SERVER_SET_CONFIG_OBJECT_REVISION_4: u32 = 4u32;
pub const MPRAPI_MPR_SERVER_SET_CONFIG_OBJECT_REVISION_5: u32 = 5u32;
pub const MPRAPI_OBJECT_TYPE_AUTH_VALIDATION_OBJECT: MPRAPI_OBJECT_TYPE = 4i32;
pub const MPRAPI_OBJECT_TYPE_IF_CUSTOM_CONFIG_OBJECT: MPRAPI_OBJECT_TYPE = 6i32;
pub const MPRAPI_OBJECT_TYPE_MPR_SERVER_OBJECT: MPRAPI_OBJECT_TYPE = 2i32;
pub const MPRAPI_OBJECT_TYPE_MPR_SERVER_SET_CONFIG_OBJECT: MPRAPI_OBJECT_TYPE = 3i32;
pub const MPRAPI_OBJECT_TYPE_RAS_CONNECTION_OBJECT: MPRAPI_OBJECT_TYPE = 1i32;
pub const MPRAPI_OBJECT_TYPE_UPDATE_CONNECTION_OBJECT: MPRAPI_OBJECT_TYPE = 5i32;
pub const MPRAPI_PPP_PROJECTION_INFO_TYPE: u32 = 1u32;
pub const MPRAPI_RAS_CONNECTION_OBJECT_REVISION_1: u32 = 1u32;
pub const MPRAPI_RAS_UPDATE_CONNECTION_OBJECT_REVISION_1: u32 = 1u32;
pub const MPRAPI_SET_CONFIG_PROTOCOL_FOR_GRE: u32 = 16u32;
pub const MPRAPI_SET_CONFIG_PROTOCOL_FOR_IKEV2: u32 = 8u32;
pub const MPRAPI_SET_CONFIG_PROTOCOL_FOR_L2TP: u32 = 2u32;
pub const MPRAPI_SET_CONFIG_PROTOCOL_FOR_PPTP: u32 = 1u32;
pub const MPRAPI_SET_CONFIG_PROTOCOL_FOR_SSTP: u32 = 4u32;
pub const MPRDM_DialAll: MPR_INTERFACE_DIAL_MODE = 1u32;
pub const MPRDM_DialAsNeeded: MPR_INTERFACE_DIAL_MODE = 2u32;
pub const MPRDM_DialFirst: MPR_INTERFACE_DIAL_MODE = 0u32;
pub const MPRDT_Atm: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ATM");
pub const MPRDT_FrameRelay: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FRAMERELAY");
pub const MPRDT_Generic: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GENERIC");
pub const MPRDT_Irda: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IRDA");
pub const MPRDT_Isdn: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("isdn");
pub const MPRDT_Modem: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("modem");
pub const MPRDT_Pad: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("pad");
pub const MPRDT_Parallel: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PARALLEL");
pub const MPRDT_SW56: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SW56");
pub const MPRDT_Serial: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SERIAL");
pub const MPRDT_Sonet: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SONET");
pub const MPRDT_Vpn: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("vpn");
pub const MPRDT_X25: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("x25");
pub const MPRET_Direct: u32 = 3u32;
pub const MPRET_Phone: u32 = 1u32;
pub const MPRET_Vpn: u32 = 2u32;
pub const MPRIDS_Disabled: u32 = 4294967295u32;
pub const MPRIDS_UseGlobalValue: u32 = 0u32;
pub const MPRIO_DisableLcpExtensions: u32 = 32u32;
pub const MPRIO_IpHeaderCompression: u32 = 8u32;
pub const MPRIO_IpSecPreSharedKey: u32 = 2147483648u32;
pub const MPRIO_NetworkLogon: u32 = 8192u32;
pub const MPRIO_PromoteAlternates: u32 = 32768u32;
pub const MPRIO_RemoteDefaultGateway: u32 = 16u32;
pub const MPRIO_RequireCHAP: u32 = 134217728u32;
pub const MPRIO_RequireDataEncryption: u32 = 4096u32;
pub const MPRIO_RequireEAP: u32 = 131072u32;
pub const MPRIO_RequireEncryptedPw: u32 = 1024u32;
pub const MPRIO_RequireMachineCertificates: u32 = 16777216u32;
pub const MPRIO_RequireMsCHAP: u32 = 268435456u32;
pub const MPRIO_RequireMsCHAP2: u32 = 536870912u32;
pub const MPRIO_RequireMsEncryptedPw: u32 = 2048u32;
pub const MPRIO_RequirePAP: u32 = 262144u32;
pub const MPRIO_RequireSPAP: u32 = 524288u32;
pub const MPRIO_SecureLocalFiles: u32 = 65536u32;
pub const MPRIO_SharedPhoneNumbers: u32 = 8388608u32;
pub const MPRIO_SpecificIpAddr: u32 = 2u32;
pub const MPRIO_SpecificNameServers: u32 = 4u32;
pub const MPRIO_SwCompression: u32 = 512u32;
pub const MPRIO_UsePreSharedKeyForIkev2Initiator: u32 = 33554432u32;
pub const MPRIO_UsePreSharedKeyForIkev2Responder: u32 = 67108864u32;
pub const MPRNP_Ip: u32 = 4u32;
pub const MPRNP_Ipv6: u32 = 8u32;
pub const MPRNP_Ipx: u32 = 2u32;
pub const MPR_ENABLE_RAS_ON_DEVICE: u32 = 1u32;
pub const MPR_ENABLE_ROUTING_ON_DEVICE: u32 = 2u32;
pub const MPR_ET_None: MPR_ET = 0u32;
pub const MPR_ET_Optional: MPR_ET = 3u32;
pub const MPR_ET_Require: MPR_ET = 1u32;
pub const MPR_ET_RequireMax: MPR_ET = 2u32;
pub const MPR_INTERFACE_ADMIN_DISABLED: u32 = 2u32;
pub const MPR_INTERFACE_CONNECTION_FAILURE: u32 = 4u32;
pub const MPR_INTERFACE_DIALOUT_HOURS_RESTRICTION: u32 = 16u32;
pub const MPR_INTERFACE_NO_DEVICE: u32 = 64u32;
pub const MPR_INTERFACE_NO_MEDIA_SENSE: u32 = 32u32;
pub const MPR_INTERFACE_OUT_OF_RESOURCES: u32 = 1u32;
pub const MPR_INTERFACE_SERVICE_PAUSED: u32 = 8u32;
pub const MPR_MaxAreaCode: u32 = 10u32;
pub const MPR_MaxCallbackNumber: u32 = 128u32;
pub const MPR_MaxDeviceName: u32 = 128u32;
pub const MPR_MaxDeviceType: u32 = 16u32;
pub const MPR_MaxEntryName: u32 = 256u32;
pub const MPR_MaxFacilities: u32 = 200u32;
pub const MPR_MaxIpAddress: u32 = 15u32;
pub const MPR_MaxIpxAddress: u32 = 21u32;
pub const MPR_MaxPadType: u32 = 32u32;
pub const MPR_MaxPhoneNumber: u32 = 128u32;
pub const MPR_MaxUserData: u32 = 200u32;
pub const MPR_MaxX25Address: u32 = 200u32;
pub const MPR_VPN_TS_IPv4_ADDR_RANGE: MPR_VPN_TS_TYPE = 7i32;
pub const MPR_VPN_TS_IPv6_ADDR_RANGE: MPR_VPN_TS_TYPE = 8i32;
pub const MPR_VS_Default: MPR_VS = 0u32;
pub const MPR_VS_Ikev2First: u32 = 8u32;
pub const MPR_VS_Ikev2Only: u32 = 7u32;
pub const MPR_VS_L2tpFirst: MPR_VS = 4u32;
pub const MPR_VS_L2tpOnly: MPR_VS = 3u32;
pub const MPR_VS_PptpFirst: MPR_VS = 2u32;
pub const MPR_VS_PptpOnly: MPR_VS = 1u32;
pub const PENDING: u32 = 600u32;
pub const PID_ATALK: u32 = 41u32;
pub const PID_IP: u32 = 33u32;
pub const PID_IPV6: u32 = 87u32;
pub const PID_IPX: u32 = 43u32;
pub const PID_NBF: u32 = 63u32;
pub const PPP_CCP_COMPRESSION: u32 = 1u32;
pub const PPP_CCP_ENCRYPTION128BIT: u32 = 64u32;
pub const PPP_CCP_ENCRYPTION40BIT: u32 = 32u32;
pub const PPP_CCP_ENCRYPTION40BITOLD: u32 = 16u32;
pub const PPP_CCP_ENCRYPTION56BIT: u32 = 128u32;
pub const PPP_CCP_HISTORYLESS: u32 = 16777216u32;
pub const PPP_IPCP_VJ: u32 = 1u32;
pub const PPP_LCP_3_DES: u32 = 32u32;
pub const PPP_LCP_ACFC: u32 = 4u32;
pub const PPP_LCP_AES_128: u32 = 64u32;
pub const PPP_LCP_AES_192: u32 = 256u32;
pub const PPP_LCP_AES_256: u32 = 128u32;
pub const PPP_LCP_CHAP: PPP_LCP = 49699u32;
pub const PPP_LCP_CHAP_MD5: PPP_LCP_INFO_AUTH_DATA = 5u32;
pub const PPP_LCP_CHAP_MS: PPP_LCP_INFO_AUTH_DATA = 128u32;
pub const PPP_LCP_CHAP_MSV2: PPP_LCP_INFO_AUTH_DATA = 129u32;
pub const PPP_LCP_DES_56: u32 = 16u32;
pub const PPP_LCP_EAP: PPP_LCP = 49703u32;
pub const PPP_LCP_GCM_AES_128: u32 = 512u32;
pub const PPP_LCP_GCM_AES_192: u32 = 1024u32;
pub const PPP_LCP_GCM_AES_256: u32 = 2048u32;
pub const PPP_LCP_MULTILINK_FRAMING: u32 = 1u32;
pub const PPP_LCP_PAP: PPP_LCP = 49187u32;
pub const PPP_LCP_PFC: u32 = 2u32;
pub const PPP_LCP_SPAP: PPP_LCP = 49191u32;
pub const PPP_LCP_SSHF: u32 = 8u32;
pub const PROJECTION_INFO_TYPE_IKEv2: RASPROJECTION_INFO_TYPE = 2i32;
pub const PROJECTION_INFO_TYPE_PPP: RASPROJECTION_INFO_TYPE = 1i32;
pub const RASADFLG_PositionDlg: u32 = 1u32;
pub const RASADP_ConnectionQueryTimeout: u32 = 4u32;
pub const RASADP_DisableConnectionQuery: u32 = 0u32;
pub const RASADP_FailedConnectionTimeout: u32 = 3u32;
pub const RASADP_LoginSessionDisable: u32 = 1u32;
pub const RASADP_SavedAddressesLimit: u32 = 2u32;
pub const RASAPIVERSION_500: RASAPIVERSION = 1i32;
pub const RASAPIVERSION_501: RASAPIVERSION = 2i32;
pub const RASAPIVERSION_600: RASAPIVERSION = 3i32;
pub const RASAPIVERSION_601: RASAPIVERSION = 4i32;
pub const RASBASE: u32 = 600u32;
pub const RASBASEEND: u32 = 877u32;
pub const RASCCPCA_MPPC: u32 = 6u32;
pub const RASCCPCA_STAC: u32 = 5u32;
pub const RASCCPO_Compression: u32 = 1u32;
pub const RASCCPO_Encryption128bit: u32 = 64u32;
pub const RASCCPO_Encryption40bit: u32 = 32u32;
pub const RASCCPO_Encryption56bit: u32 = 16u32;
pub const RASCCPO_HistoryLess: u32 = 2u32;
pub const RASCF_AllUsers: u32 = 1u32;
pub const RASCF_GlobalCreds: u32 = 2u32;
pub const RASCF_OwnerKnown: u32 = 4u32;
pub const RASCF_OwnerMatch: u32 = 8u32;
pub const RASCM_DDMPreSharedKey: u32 = 64u32;
pub const RASCM_DefaultCreds: u32 = 8u32;
pub const RASCM_Domain: u32 = 4u32;
pub const RASCM_Password: u32 = 2u32;
pub const RASCM_PreSharedKey: u32 = 16u32;
pub const RASCM_ServerPreSharedKey: u32 = 32u32;
pub const RASCM_UserName: u32 = 1u32;
pub const RASCN_BandwidthAdded: u32 = 4u32;
pub const RASCN_BandwidthRemoved: u32 = 8u32;
pub const RASCN_Connection: u32 = 1u32;
pub const RASCN_Disconnection: u32 = 2u32;
pub const RASCN_Dormant: u32 = 16u32;
pub const RASCN_EPDGPacketArrival: u32 = 64u32;
pub const RASCN_ReConnection: u32 = 32u32;
pub const RASCSS_DONE: u32 = 8192u32;
pub const RASCSS_Dormant: RASCONNSUBSTATE = 1i32;
pub const RASCSS_None: RASCONNSUBSTATE = 0i32;
pub const RASCSS_Reconnected: RASCONNSUBSTATE = 8192i32;
pub const RASCSS_Reconnecting: RASCONNSUBSTATE = 2i32;
pub const RASCS_AllDevicesConnected: RASCONNSTATE = 4i32;
pub const RASCS_ApplySettings: RASCONNSTATE = 24i32;
pub const RASCS_AuthAck: RASCONNSTATE = 12i32;
pub const RASCS_AuthCallback: RASCONNSTATE = 8i32;
pub const RASCS_AuthChangePassword: RASCONNSTATE = 9i32;
pub const RASCS_AuthLinkSpeed: RASCONNSTATE = 11i32;
pub const RASCS_AuthNotify: RASCONNSTATE = 6i32;
pub const RASCS_AuthProject: RASCONNSTATE = 10i32;
pub const RASCS_AuthRetry: RASCONNSTATE = 7i32;
pub const RASCS_Authenticate: RASCONNSTATE = 5i32;
pub const RASCS_Authenticated: RASCONNSTATE = 14i32;
pub const RASCS_CallbackComplete: RASCONNSTATE = 20i32;
pub const RASCS_CallbackSetByCaller: RASCONNSTATE = 4098i32;
pub const RASCS_ConnectDevice: RASCONNSTATE = 2i32;
pub const RASCS_Connected: RASCONNSTATE = 8192i32;
pub const RASCS_DONE: u32 = 8192u32;
pub const RASCS_DeviceConnected: RASCONNSTATE = 3i32;
pub const RASCS_Disconnected: RASCONNSTATE = 8193i32;
pub const RASCS_Interactive: RASCONNSTATE = 4096i32;
pub const RASCS_InvokeEapUI: RASCONNSTATE = 4100i32;
pub const RASCS_LogonNetwork: RASCONNSTATE = 21i32;
pub const RASCS_OpenPort: RASCONNSTATE = 0i32;
pub const RASCS_PAUSED: u32 = 4096u32;
pub const RASCS_PasswordExpired: RASCONNSTATE = 4099i32;
pub const RASCS_PortOpened: RASCONNSTATE = 1i32;
pub const RASCS_PrepareForCallback: RASCONNSTATE = 15i32;
pub const RASCS_Projected: RASCONNSTATE = 18i32;
pub const RASCS_ReAuthenticate: RASCONNSTATE = 13i32;
pub const RASCS_RetryAuthentication: RASCONNSTATE = 4097i32;
pub const RASCS_StartAuthentication: RASCONNSTATE = 19i32;
pub const RASCS_SubEntryConnected: RASCONNSTATE = 22i32;
pub const RASCS_SubEntryDisconnected: RASCONNSTATE = 23i32;
pub const RASCS_WaitForCallback: RASCONNSTATE = 17i32;
pub const RASCS_WaitForModemReset: RASCONNSTATE = 16i32;
pub const RASDDFLAG_AoacRedial: u32 = 4u32;
pub const RASDDFLAG_LinkFailure: u32 = 2147483648u32;
pub const RASDDFLAG_NoPrompt: u32 = 2u32;
pub const RASDDFLAG_PositionDlg: u32 = 1u32;
pub const RASDIALEVENT: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("RasDialEvent");
pub const RASDT_Atm: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ATM");
pub const RASDT_FrameRelay: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FRAMERELAY");
pub const RASDT_Generic: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GENERIC");
pub const RASDT_Irda: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IRDA");
pub const RASDT_Isdn: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("isdn");
pub const RASDT_Modem: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("modem");
pub const RASDT_PPPoE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PPPoE");
pub const RASDT_Pad: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("pad");
pub const RASDT_Parallel: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PARALLEL");
pub const RASDT_SW56: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SW56");
pub const RASDT_Serial: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SERIAL");
pub const RASDT_Sonet: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SONET");
pub const RASDT_Vpn: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("vpn");
pub const RASDT_X25: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("x25");
pub const RASEAPF_Logon: u32 = 4u32;
pub const RASEAPF_NonInteractive: u32 = 2u32;
pub const RASEAPF_Preview: u32 = 8u32;
pub const RASEDFLAG_CloneEntry: u32 = 4u32;
pub const RASEDFLAG_IncomingConnection: u32 = 1024u32;
pub const RASEDFLAG_InternetEntry: u32 = 256u32;
pub const RASEDFLAG_NAT: u32 = 512u32;
pub const RASEDFLAG_NewBroadbandEntry: u32 = 128u32;
pub const RASEDFLAG_NewDirectEntry: u32 = 64u32;
pub const RASEDFLAG_NewEntry: u32 = 2u32;
pub const RASEDFLAG_NewPhoneEntry: u32 = 16u32;
pub const RASEDFLAG_NewTunnelEntry: u32 = 32u32;
pub const RASEDFLAG_NoRename: u32 = 8u32;
pub const RASEDFLAG_PositionDlg: u32 = 1u32;
pub const RASEDFLAG_ShellOwned: u32 = 1073741824u32;
pub const RASEDM_DialAll: RASENTRY_DIAL_MODE = 1u32;
pub const RASEDM_DialAsNeeded: RASENTRY_DIAL_MODE = 2u32;
pub const RASEO2_AuthTypeIsOtp: u32 = 268435456u32;
pub const RASEO2_AutoTriggerCapable: u32 = 67108864u32;
pub const RASEO2_CacheCredentials: u32 = 33554432u32;
pub const RASEO2_DisableClassBasedStaticRoute: u32 = 524288u32;
pub const RASEO2_DisableIKENameEkuCheck: u32 = 262144u32;
pub const RASEO2_DisableMobility: u32 = 2097152u32;
pub const RASEO2_DisableNbtOverIP: u32 = 64u32;
pub const RASEO2_DontNegotiateMultilink: u32 = 4u32;
pub const RASEO2_DontUseRasCredentials: u32 = 8u32;
pub const RASEO2_IPv4ExplicitMetric: u32 = 65536u32;
pub const RASEO2_IPv6ExplicitMetric: u32 = 131072u32;
pub const RASEO2_IPv6RemoteDefaultGateway: u32 = 8192u32;
pub const RASEO2_IPv6SpecificNameServers: u32 = 4096u32;
pub const RASEO2_Internet: u32 = 32u32;
pub const RASEO2_IsAlwaysOn: u32 = 536870912u32;
pub const RASEO2_IsPrivateNetwork: u32 = 1073741824u32;
pub const RASEO2_IsThirdPartyProfile: u32 = 134217728u32;
pub const RASEO2_PlumbIKEv2TSAsRoutes: u32 = 2147483648u32;
pub const RASEO2_ReconnectIfDropped: u32 = 256u32;
pub const RASEO2_RegisterIpWithDNS: u32 = 16384u32;
pub const RASEO2_RequireMachineCertificates: u32 = 4194304u32;
pub const RASEO2_SecureClientForMSNet: u32 = 2u32;
pub const RASEO2_SecureFileAndPrint: u32 = 1u32;
pub const RASEO2_SecureRoutingCompartment: u32 = 1024u32;
pub const RASEO2_SharePhoneNumbers: u32 = 512u32;
pub const RASEO2_SpecificIPv6Addr: u32 = 1048576u32;
pub const RASEO2_UseDNSSuffixForRegistration: u32 = 32768u32;
pub const RASEO2_UseGlobalDeviceSettings: u32 = 128u32;
pub const RASEO2_UsePreSharedKey: u32 = 16u32;
pub const RASEO2_UsePreSharedKeyForIkev2Initiator: u32 = 8388608u32;
pub const RASEO2_UsePreSharedKeyForIkev2Responder: u32 = 16777216u32;
pub const RASEO2_UseTypicalSettings: u32 = 2048u32;
pub const RASEO_Custom: u32 = 1048576u32;
pub const RASEO_CustomScript: u32 = 2147483648u32;
pub const RASEO_DisableLcpExtensions: u32 = 32u32;
pub const RASEO_IpHeaderCompression: u32 = 8u32;
pub const RASEO_ModemLights: u32 = 256u32;
pub const RASEO_NetworkLogon: u32 = 8192u32;
pub const RASEO_PreviewDomain: u32 = 33554432u32;
pub const RASEO_PreviewPhoneNumber: u32 = 2097152u32;
pub const RASEO_PreviewUserPw: u32 = 16777216u32;
pub const RASEO_PromoteAlternates: u32 = 32768u32;
pub const RASEO_RemoteDefaultGateway: u32 = 16u32;
pub const RASEO_RequireCHAP: u32 = 134217728u32;
pub const RASEO_RequireDataEncryption: u32 = 4096u32;
pub const RASEO_RequireEAP: u32 = 131072u32;
pub const RASEO_RequireEncryptedPw: u32 = 1024u32;
pub const RASEO_RequireMsCHAP: u32 = 268435456u32;
pub const RASEO_RequireMsCHAP2: u32 = 536870912u32;
pub const RASEO_RequireMsEncryptedPw: u32 = 2048u32;
pub const RASEO_RequirePAP: u32 = 262144u32;
pub const RASEO_RequireSPAP: u32 = 524288u32;
pub const RASEO_RequireW95MSCHAP: u32 = 1073741824u32;
pub const RASEO_SecureLocalFiles: u32 = 65536u32;
pub const RASEO_SharedPhoneNumbers: u32 = 8388608u32;
pub const RASEO_ShowDialingProgress: u32 = 67108864u32;
pub const RASEO_SpecificIpAddr: u32 = 2u32;
pub const RASEO_SpecificNameServers: u32 = 4u32;
pub const RASEO_SwCompression: u32 = 512u32;
pub const RASEO_TerminalAfterDial: u32 = 128u32;
pub const RASEO_TerminalBeforeDial: u32 = 64u32;
pub const RASEO_UseCountryAndAreaCodes: u32 = 1u32;
pub const RASEO_UseLogonCredentials: u32 = 16384u32;
pub const RASET_Broadband: u32 = 5u32;
pub const RASET_Direct: u32 = 3u32;
pub const RASET_Internet: u32 = 4u32;
pub const RASET_Phone: u32 = 1u32;
pub const RASET_Vpn: u32 = 2u32;
pub const RASFP_Ppp: u32 = 1u32;
pub const RASFP_Ras: u32 = 4u32;
pub const RASFP_Slip: u32 = 2u32;
pub const RASIDS_Disabled: u32 = 4294967295u32;
pub const RASIDS_UseGlobalValue: u32 = 0u32;
pub const RASIKEv2_AUTH_EAP: u32 = 2u32;
pub const RASIKEv2_AUTH_MACHINECERTIFICATES: u32 = 1u32;
pub const RASIKEv2_AUTH_PSK: u32 = 3u32;
pub const RASIKEv2_FLAGS_BEHIND_NAT: RASIKEV_PROJECTION_INFO_FLAGS = 2u32;
pub const RASIKEv2_FLAGS_MOBIKESUPPORTED: RASIKEV_PROJECTION_INFO_FLAGS = 1u32;
pub const RASIKEv2_FLAGS_SERVERBEHIND_NAT: RASIKEV_PROJECTION_INFO_FLAGS = 4u32;
pub const RASIPO_VJ: u32 = 1u32;
pub const RASLCPAD_CHAP_MD5: RASPPP_PROJECTION_INFO_SERVER_AUTH_DATA = 5u32;
pub const RASLCPAD_CHAP_MS: RASPPP_PROJECTION_INFO_SERVER_AUTH_DATA = 128u32;
pub const RASLCPAD_CHAP_MSV2: RASPPP_PROJECTION_INFO_SERVER_AUTH_DATA = 129u32;
pub const RASLCPAP_CHAP: RASPPP_PROJECTION_INFO_SERVER_AUTH_PROTOCOL = 49699u32;
pub const RASLCPAP_EAP: RASPPP_PROJECTION_INFO_SERVER_AUTH_PROTOCOL = 49703u32;
pub const RASLCPAP_PAP: RASPPP_PROJECTION_INFO_SERVER_AUTH_PROTOCOL = 49187u32;
pub const RASLCPAP_SPAP: RASPPP_PROJECTION_INFO_SERVER_AUTH_PROTOCOL = 49191u32;
pub const RASLCPO_3_DES: u32 = 16u32;
pub const RASLCPO_ACFC: u32 = 2u32;
pub const RASLCPO_AES_128: u32 = 32u32;
pub const RASLCPO_AES_192: u32 = 128u32;
pub const RASLCPO_AES_256: u32 = 64u32;
pub const RASLCPO_DES_56: u32 = 8u32;
pub const RASLCPO_GCM_AES_128: u32 = 256u32;
pub const RASLCPO_GCM_AES_192: u32 = 512u32;
pub const RASLCPO_GCM_AES_256: u32 = 1024u32;
pub const RASLCPO_PFC: u32 = 1u32;
pub const RASLCPO_SSHF: u32 = 4u32;
pub const RASNAP_ProbationTime: u32 = 1u32;
pub const RASNOUSER_SmartCard: u32 = 1u32;
pub const RASNP_Ip: u32 = 4u32;
pub const RASNP_Ipv6: u32 = 8u32;
pub const RASNP_Ipx: u32 = 2u32;
pub const RASNP_NetBEUI: u32 = 1u32;
pub const RASPBDEVENT_AddEntry: u32 = 1u32;
pub const RASPBDEVENT_DialEntry: u32 = 4u32;
pub const RASPBDEVENT_EditEntry: u32 = 2u32;
pub const RASPBDEVENT_EditGlobals: u32 = 5u32;
pub const RASPBDEVENT_NoUser: u32 = 6u32;
pub const RASPBDEVENT_NoUserEdit: u32 = 7u32;
pub const RASPBDEVENT_RemoveEntry: u32 = 3u32;
pub const RASPBDFLAG_ForceCloseOnDial: u32 = 2u32;
pub const RASPBDFLAG_NoUser: u32 = 16u32;
pub const RASPBDFLAG_PositionDlg: u32 = 1u32;
pub const RASPBDFLAG_UpdateDefaults: u32 = 2147483648u32;
pub const RASPRIV2_DialinPolicy: u32 = 1u32;
pub const RASPRIV_AdminSetCallback: u32 = 2u32;
pub const RASPRIV_CallerSetCallback: u32 = 4u32;
pub const RASPRIV_DialinPrivilege: u32 = 8u32;
pub const RASPRIV_NoCallback: u32 = 1u32;
pub const RASP_Amb: RASPROJECTION = 65536i32;
pub const RASP_PppCcp: RASPROJECTION = 33021i32;
pub const RASP_PppIp: RASPROJECTION = 32801i32;
pub const RASP_PppIpv6: RASPROJECTION = 32855i32;
pub const RASP_PppIpx: RASPROJECTION = 32811i32;
pub const RASP_PppLcp: RASPROJECTION = 49185i32;
pub const RASP_PppNbf: RASPROJECTION = 32831i32;
pub const RASTUNNELENDPOINT_IPv4: u32 = 1u32;
pub const RASTUNNELENDPOINT_IPv6: u32 = 2u32;
pub const RASTUNNELENDPOINT_UNKNOWN: u32 = 0u32;
pub const RAS_FLAGS_ARAP_CONNECTION: RAS_FLAGS = 16u32;
pub const RAS_FLAGS_DORMANT: RAS_FLAGS = 32u32;
pub const RAS_FLAGS_IKEV2_CONNECTION: RAS_FLAGS = 16u32;
pub const RAS_FLAGS_MESSENGER_PRESENT: RAS_FLAGS = 2u32;
pub const RAS_FLAGS_PPP_CONNECTION: RAS_FLAGS = 1u32;
pub const RAS_FLAGS_QUARANTINE_PRESENT: RAS_FLAGS = 8u32;
pub const RAS_FLAGS_RAS_CONNECTION: u32 = 4u32;
pub const RAS_HARDWARE_FAILURE: RAS_HARDWARE_CONDITION = 1i32;
pub const RAS_HARDWARE_OPERATIONAL: RAS_HARDWARE_CONDITION = 0i32;
pub const RAS_MaxAreaCode: u32 = 10u32;
pub const RAS_MaxCallbackNumber: u32 = 128u32;
pub const RAS_MaxDeviceName: u32 = 128u32;
pub const RAS_MaxDeviceType: u32 = 16u32;
pub const RAS_MaxDnsSuffix: u32 = 256u32;
pub const RAS_MaxEntryName: u32 = 256u32;
pub const RAS_MaxFacilities: u32 = 200u32;
pub const RAS_MaxIDSize: u32 = 256u32;
pub const RAS_MaxIpAddress: u32 = 15u32;
pub const RAS_MaxIpxAddress: u32 = 21u32;
pub const RAS_MaxPadType: u32 = 32u32;
pub const RAS_MaxPhoneNumber: u32 = 128u32;
pub const RAS_MaxReplyMessage: u32 = 1024u32;
pub const RAS_MaxUserData: u32 = 200u32;
pub const RAS_MaxX25Address: u32 = 200u32;
pub const RAS_PORT_AUTHENTICATED: RAS_PORT_CONDITION = 5i32;
pub const RAS_PORT_AUTHENTICATING: RAS_PORT_CONDITION = 4i32;
pub const RAS_PORT_CALLING_BACK: RAS_PORT_CONDITION = 2i32;
pub const RAS_PORT_DISCONNECTED: RAS_PORT_CONDITION = 1i32;
pub const RAS_PORT_INITIALIZING: RAS_PORT_CONDITION = 6i32;
pub const RAS_PORT_LISTENING: RAS_PORT_CONDITION = 3i32;
pub const RAS_PORT_NON_OPERATIONAL: RAS_PORT_CONDITION = 0i32;
pub const RAS_QUAR_STATE_NORMAL: RAS_QUARANTINE_STATE = 0i32;
pub const RAS_QUAR_STATE_NOT_CAPABLE: RAS_QUARANTINE_STATE = 3i32;
pub const RAS_QUAR_STATE_PROBATION: RAS_QUARANTINE_STATE = 2i32;
pub const RAS_QUAR_STATE_QUARANTINE: RAS_QUARANTINE_STATE = 1i32;
pub const RCD_AllUsers: u32 = 1u32;
pub const RCD_Eap: u32 = 2u32;
pub const RCD_Logon: u32 = 4u32;
pub const RCD_SingleUser: u32 = 0u32;
pub const RDEOPT_CustomDial: u32 = 4096u32;
pub const RDEOPT_DisableConnectedUI: u32 = 64u32;
pub const RDEOPT_DisableReconnect: u32 = 256u32;
pub const RDEOPT_DisableReconnectUI: u32 = 128u32;
pub const RDEOPT_EapInfoCryptInCapable: u32 = 32768u32;
pub const RDEOPT_IgnoreModemSpeaker: u32 = 4u32;
pub const RDEOPT_IgnoreSoftwareCompression: u32 = 16u32;
pub const RDEOPT_InvokeAutoTriggerCredentialUI: u32 = 16384u32;
pub const RDEOPT_NoUser: u32 = 512u32;
pub const RDEOPT_PauseOnScript: u32 = 1024u32;
pub const RDEOPT_PausedStates: u32 = 2u32;
pub const RDEOPT_Router: u32 = 2048u32;
pub const RDEOPT_SetModemSpeaker: u32 = 8u32;
pub const RDEOPT_SetSoftwareCompression: u32 = 32u32;
pub const RDEOPT_UseCustomScripting: u32 = 8192u32;
pub const RDEOPT_UsePrefixSuffix: u32 = 1u32;
pub const REN_AllUsers: u32 = 1u32;
pub const REN_User: u32 = 0u32;
pub const ROUTER_IF_STATE_CONNECTED: ROUTER_CONNECTION_STATE = 3i32;
pub const ROUTER_IF_STATE_CONNECTING: ROUTER_CONNECTION_STATE = 2i32;
pub const ROUTER_IF_STATE_DISCONNECTED: ROUTER_CONNECTION_STATE = 1i32;
pub const ROUTER_IF_STATE_UNREACHABLE: ROUTER_CONNECTION_STATE = 0i32;
pub const ROUTER_IF_TYPE_CLIENT: ROUTER_INTERFACE_TYPE = 0i32;
pub const ROUTER_IF_TYPE_DEDICATED: ROUTER_INTERFACE_TYPE = 3i32;
pub const ROUTER_IF_TYPE_DIALOUT: ROUTER_INTERFACE_TYPE = 7i32;
pub const ROUTER_IF_TYPE_FULL_ROUTER: ROUTER_INTERFACE_TYPE = 2i32;
pub const ROUTER_IF_TYPE_HOME_ROUTER: ROUTER_INTERFACE_TYPE = 1i32;
pub const ROUTER_IF_TYPE_INTERNAL: ROUTER_INTERFACE_TYPE = 4i32;
pub const ROUTER_IF_TYPE_LOOPBACK: ROUTER_INTERFACE_TYPE = 5i32;
pub const ROUTER_IF_TYPE_MAX: ROUTER_INTERFACE_TYPE = 8i32;
pub const ROUTER_IF_TYPE_TUNNEL1: ROUTER_INTERFACE_TYPE = 6i32;
pub const RRAS_SERVICE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RemoteAccess");
pub const RTM_BLOCK_METHODS: u32 = 1u32;
pub const RTM_CHANGE_NOTIFICATION: RTM_EVENT_TYPE = 3i32;
pub const RTM_CHANGE_TYPE_ALL: u32 = 1u32;
pub const RTM_CHANGE_TYPE_BEST: u32 = 2u32;
pub const RTM_CHANGE_TYPE_FORWARDING: u32 = 4u32;
pub const RTM_DEST_FLAG_DONT_FORWARD: u32 = 4u32;
pub const RTM_DEST_FLAG_FWD_ENGIN_ADD: u32 = 2u32;
pub const RTM_DEST_FLAG_NATURAL_NET: u32 = 1u32;
pub const RTM_ENTITY_DEREGISTERED: RTM_EVENT_TYPE = 1i32;
pub const RTM_ENTITY_REGISTERED: RTM_EVENT_TYPE = 0i32;
pub const RTM_ENUM_ALL_DESTS: u32 = 0u32;
pub const RTM_ENUM_ALL_ROUTES: u32 = 0u32;
pub const RTM_ENUM_NEXT: u32 = 1u32;
pub const RTM_ENUM_OWN_DESTS: u32 = 16777216u32;
pub const RTM_ENUM_OWN_ROUTES: u32 = 65536u32;
pub const RTM_ENUM_RANGE: u32 = 2u32;
pub const RTM_ENUM_START: u32 = 0u32;
pub const RTM_MATCH_FULL: u32 = 65535u32;
pub const RTM_MATCH_INTERFACE: u32 = 16u32;
pub const RTM_MATCH_NEIGHBOUR: u32 = 2u32;
pub const RTM_MATCH_NEXTHOP: u32 = 8u32;
pub const RTM_MATCH_NONE: u32 = 0u32;
pub const RTM_MATCH_OWNER: u32 = 1u32;
pub const RTM_MATCH_PREF: u32 = 4u32;
pub const RTM_MAX_ADDRESS_SIZE: u32 = 16u32;
pub const RTM_MAX_VIEWS: u32 = 32u32;
pub const RTM_NEXTHOP_CHANGE_NEW: u32 = 1u32;
pub const RTM_NEXTHOP_FLAGS_DOWN: u32 = 2u32;
pub const RTM_NEXTHOP_FLAGS_REMOTE: u32 = 1u32;
pub const RTM_NEXTHOP_STATE_CREATED: u32 = 0u32;
pub const RTM_NEXTHOP_STATE_DELETED: u32 = 1u32;
pub const RTM_NOTIFY_ONLY_MARKED_DESTS: u32 = 65536u32;
pub const RTM_NUM_CHANGE_TYPES: u32 = 3u32;
pub const RTM_RESUME_METHODS: u32 = 0u32;
pub const RTM_ROUTE_CHANGE_BEST: u32 = 65536u32;
pub const RTM_ROUTE_CHANGE_FIRST: u32 = 1u32;
pub const RTM_ROUTE_CHANGE_NEW: u32 = 2u32;
pub const RTM_ROUTE_EXPIRED: RTM_EVENT_TYPE = 2i32;
pub const RTM_ROUTE_FLAGS_BLACKHOLE: u32 = 2u32;
pub const RTM_ROUTE_FLAGS_DISCARD: u32 = 4u32;
pub const RTM_ROUTE_FLAGS_INACTIVE: u32 = 8u32;
pub const RTM_ROUTE_FLAGS_LIMITED_BC: u32 = 1024u32;
pub const RTM_ROUTE_FLAGS_LOCAL: u32 = 16u32;
pub const RTM_ROUTE_FLAGS_LOCAL_MCAST: u32 = 512u32;
pub const RTM_ROUTE_FLAGS_LOOPBACK: u32 = 128u32;
pub const RTM_ROUTE_FLAGS_MARTIAN: u32 = 1u32;
pub const RTM_ROUTE_FLAGS_MCAST: u32 = 256u32;
pub const RTM_ROUTE_FLAGS_MYSELF: u32 = 64u32;
pub const RTM_ROUTE_FLAGS_ONES_NETBC: u32 = 16384u32;
pub const RTM_ROUTE_FLAGS_ONES_SUBNETBC: u32 = 32768u32;
pub const RTM_ROUTE_FLAGS_REMOTE: u32 = 32u32;
pub const RTM_ROUTE_FLAGS_ZEROS_NETBC: u32 = 4096u32;
pub const RTM_ROUTE_FLAGS_ZEROS_SUBNETBC: u32 = 8192u32;
pub const RTM_ROUTE_STATE_CREATED: u32 = 0u32;
pub const RTM_ROUTE_STATE_DELETED: u32 = 2u32;
pub const RTM_ROUTE_STATE_DELETING: u32 = 1u32;
pub const RTM_VIEW_ID_MCAST: u32 = 1u32;
pub const RTM_VIEW_ID_UCAST: u32 = 0u32;
pub const RTM_VIEW_MASK_ALL: u32 = 4294967295u32;
pub const RTM_VIEW_MASK_ANY: u32 = 0u32;
pub const RTM_VIEW_MASK_MCAST: u32 = 2u32;
pub const RTM_VIEW_MASK_NONE: u32 = 0u32;
pub const RTM_VIEW_MASK_SIZE: u32 = 32u32;
pub const RTM_VIEW_MASK_UCAST: u32 = 1u32;
pub const SECURITYMSG_ERROR: SECURITY_MESSAGE_MSG_ID = 3u32;
pub const SECURITYMSG_FAILURE: SECURITY_MESSAGE_MSG_ID = 2u32;
pub const SECURITYMSG_SUCCESS: SECURITY_MESSAGE_MSG_ID = 1u32;
pub const VS_Default: u32 = 0u32;
pub const VS_GREOnly: u32 = 9u32;
pub const VS_Ikev2First: u32 = 8u32;
pub const VS_Ikev2Only: u32 = 7u32;
pub const VS_Ikev2Sstp: u32 = 14u32;
pub const VS_L2tpFirst: u32 = 4u32;
pub const VS_L2tpOnly: u32 = 3u32;
pub const VS_L2tpSstp: u32 = 13u32;
pub const VS_PptpFirst: u32 = 2u32;
pub const VS_PptpOnly: u32 = 1u32;
pub const VS_PptpSstp: u32 = 12u32;
pub const VS_ProtocolList: u32 = 15u32;
pub const VS_SstpFirst: u32 = 6u32;
pub const VS_SstpOnly: u32 = 5u32;
pub const WARNING_MSG_ALIAS_NOT_ADDED: u32 = 644u32;
pub const WM_RASDIALEVENT: u32 = 52429u32;
pub type IKEV2_ID_PAYLOAD_TYPE = i32;
pub type MGM_ENUM_TYPES = i32;
pub type MPRAPI_OBJECT_TYPE = i32;
pub type MPR_ET = u32;
pub type MPR_INTERFACE_DIAL_MODE = u32;
pub type MPR_VPN_TS_TYPE = i32;
pub type MPR_VS = u32;
pub type PPP_LCP = u32;
pub type PPP_LCP_INFO_AUTH_DATA = u32;
pub type RASAPIVERSION = i32;
pub type RASCONNSTATE = i32;
pub type RASCONNSUBSTATE = i32;
pub type RASENTRY_DIAL_MODE = u32;
pub type RASIKEV_PROJECTION_INFO_FLAGS = u32;
pub type RASPPP_PROJECTION_INFO_SERVER_AUTH_DATA = u32;
pub type RASPPP_PROJECTION_INFO_SERVER_AUTH_PROTOCOL = u32;
pub type RASPROJECTION = i32;
pub type RASPROJECTION_INFO_TYPE = i32;
pub type RAS_FLAGS = u32;
pub type RAS_HARDWARE_CONDITION = i32;
pub type RAS_PORT_CONDITION = i32;
pub type RAS_QUARANTINE_STATE = i32;
pub type ROUTER_CONNECTION_STATE = i32;
pub type ROUTER_INTERFACE_TYPE = i32;
pub type RTM_EVENT_TYPE = i32;
pub type SECURITY_MESSAGE_MSG_ID = u32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct AUTH_VALIDATION_EX {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub hRasConnection: super::super::Foundation::HANDLE,
    pub wszUserName: [u16; 257],
    pub wszLogonDomain: [u16; 16],
    pub AuthInfoSize: u32,
    pub AuthInfo: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for AUTH_VALIDATION_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for AUTH_VALIDATION_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GRE_CONFIG_PARAMS0 {
    pub dwNumPorts: u32,
    pub dwPortFlags: u32,
}
impl ::core::marker::Copy for GRE_CONFIG_PARAMS0 {}
impl ::core::clone::Clone for GRE_CONFIG_PARAMS0 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HRASCONN = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct IKEV2_CONFIG_PARAMS {
    pub dwNumPorts: u32,
    pub dwPortFlags: u32,
    pub dwTunnelConfigParamFlags: u32,
    pub TunnelConfigParams: IKEV2_TUNNEL_CONFIG_PARAMS4,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for IKEV2_CONFIG_PARAMS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for IKEV2_CONFIG_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IKEV2_PROJECTION_INFO {
    pub dwIPv4NegotiationError: u32,
    pub wszAddress: [u16; 16],
    pub wszRemoteAddress: [u16; 16],
    pub IPv4SubInterfaceIndex: u64,
    pub dwIPv6NegotiationError: u32,
    pub bInterfaceIdentifier: [u8; 8],
    pub bRemoteInterfaceIdentifier: [u8; 8],
    pub bPrefix: [u8; 8],
    pub dwPrefixLength: u32,
    pub IPv6SubInterfaceIndex: u64,
    pub dwOptions: u32,
    pub dwAuthenticationProtocol: u32,
    pub dwEapTypeId: u32,
    pub dwCompressionAlgorithm: u32,
    pub dwEncryptionMethod: u32,
}
impl ::core::marker::Copy for IKEV2_PROJECTION_INFO {}
impl ::core::clone::Clone for IKEV2_PROJECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IKEV2_PROJECTION_INFO2 {
    pub dwIPv4NegotiationError: u32,
    pub wszAddress: [u16; 16],
    pub wszRemoteAddress: [u16; 16],
    pub IPv4SubInterfaceIndex: u64,
    pub dwIPv6NegotiationError: u32,
    pub bInterfaceIdentifier: [u8; 8],
    pub bRemoteInterfaceIdentifier: [u8; 8],
    pub bPrefix: [u8; 8],
    pub dwPrefixLength: u32,
    pub IPv6SubInterfaceIndex: u64,
    pub dwOptions: u32,
    pub dwAuthenticationProtocol: u32,
    pub dwEapTypeId: u32,
    pub dwEmbeddedEAPTypeId: u32,
    pub dwCompressionAlgorithm: u32,
    pub dwEncryptionMethod: u32,
}
impl ::core::marker::Copy for IKEV2_PROJECTION_INFO2 {}
impl ::core::clone::Clone for IKEV2_PROJECTION_INFO2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct IKEV2_TUNNEL_CONFIG_PARAMS2 {
    pub dwIdleTimeout: u32,
    pub dwNetworkBlackoutTime: u32,
    pub dwSaLifeTime: u32,
    pub dwSaDataSizeForRenegotiation: u32,
    pub dwConfigOptions: u32,
    pub dwTotalCertificates: u32,
    pub certificateNames: *mut super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub machineCertificateName: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub dwEncryptionType: u32,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for IKEV2_TUNNEL_CONFIG_PARAMS2 {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for IKEV2_TUNNEL_CONFIG_PARAMS2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct IKEV2_TUNNEL_CONFIG_PARAMS3 {
    pub dwIdleTimeout: u32,
    pub dwNetworkBlackoutTime: u32,
    pub dwSaLifeTime: u32,
    pub dwSaDataSizeForRenegotiation: u32,
    pub dwConfigOptions: u32,
    pub dwTotalCertificates: u32,
    pub certificateNames: *mut super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub machineCertificateName: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub dwEncryptionType: u32,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
    pub dwTotalEkus: u32,
    pub certificateEKUs: *mut MPR_CERT_EKU,
    pub machineCertificateHash: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for IKEV2_TUNNEL_CONFIG_PARAMS3 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for IKEV2_TUNNEL_CONFIG_PARAMS3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct IKEV2_TUNNEL_CONFIG_PARAMS4 {
    pub dwIdleTimeout: u32,
    pub dwNetworkBlackoutTime: u32,
    pub dwSaLifeTime: u32,
    pub dwSaDataSizeForRenegotiation: u32,
    pub dwConfigOptions: u32,
    pub dwTotalCertificates: u32,
    pub certificateNames: *mut super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub machineCertificateName: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub dwEncryptionType: u32,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
    pub dwTotalEkus: u32,
    pub certificateEKUs: *mut MPR_CERT_EKU,
    pub machineCertificateHash: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub dwMmSaLifeTime: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for IKEV2_TUNNEL_CONFIG_PARAMS4 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for IKEV2_TUNNEL_CONFIG_PARAMS4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct L2TP_CONFIG_PARAMS0 {
    pub dwNumPorts: u32,
    pub dwPortFlags: u32,
}
impl ::core::marker::Copy for L2TP_CONFIG_PARAMS0 {}
impl ::core::clone::Clone for L2TP_CONFIG_PARAMS0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct L2TP_CONFIG_PARAMS1 {
    pub dwNumPorts: u32,
    pub dwPortFlags: u32,
    pub dwTunnelConfigParamFlags: u32,
    pub TunnelConfigParams: L2TP_TUNNEL_CONFIG_PARAMS2,
}
impl ::core::marker::Copy for L2TP_CONFIG_PARAMS1 {}
impl ::core::clone::Clone for L2TP_CONFIG_PARAMS1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct L2TP_TUNNEL_CONFIG_PARAMS1 {
    pub dwIdleTimeout: u32,
    pub dwEncryptionType: u32,
    pub dwSaLifeTime: u32,
    pub dwSaDataSizeForRenegotiation: u32,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
}
impl ::core::marker::Copy for L2TP_TUNNEL_CONFIG_PARAMS1 {}
impl ::core::clone::Clone for L2TP_TUNNEL_CONFIG_PARAMS1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct L2TP_TUNNEL_CONFIG_PARAMS2 {
    pub dwIdleTimeout: u32,
    pub dwEncryptionType: u32,
    pub dwSaLifeTime: u32,
    pub dwSaDataSizeForRenegotiation: u32,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
    pub dwMmSaLifeTime: u32,
}
impl ::core::marker::Copy for L2TP_TUNNEL_CONFIG_PARAMS2 {}
impl ::core::clone::Clone for L2TP_TUNNEL_CONFIG_PARAMS2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MGM_IF_ENTRY {
    pub dwIfIndex: u32,
    pub dwIfNextHopAddr: u32,
    pub bIGMP: super::super::Foundation::BOOL,
    pub bIsEnabled: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MGM_IF_ENTRY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MGM_IF_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct MPRAPI_ADMIN_DLL_CALLBACKS {
    pub revision: u8,
    pub lpfnMprAdminGetIpAddressForUser: PMPRADMINGETIPADDRESSFORUSER,
    pub lpfnMprAdminReleaseIpAddress: PMPRADMINRELEASEIPADRESS,
    pub lpfnMprAdminGetIpv6AddressForUser: PMPRADMINGETIPV6ADDRESSFORUSER,
    pub lpfnMprAdminReleaseIpV6AddressForUser: PMPRADMINRELEASEIPV6ADDRESSFORUSER,
    pub lpfnRasAdminAcceptNewLink: PMPRADMINACCEPTNEWLINK,
    pub lpfnRasAdminLinkHangupNotification: PMPRADMINLINKHANGUPNOTIFICATION,
    pub lpfnRasAdminTerminateDll: PMPRADMINTERMINATEDLL,
    pub lpfnRasAdminAcceptNewConnectionEx: PMPRADMINACCEPTNEWCONNECTIONEX,
    pub lpfnRasAdminAcceptEndpointChangeEx: PMPRADMINACCEPTTUNNELENDPOINTCHANGEEX,
    pub lpfnRasAdminAcceptReauthenticationEx: PMPRADMINACCEPTREAUTHENTICATIONEX,
    pub lpfnRasAdminConnectionHangupNotificationEx: PMPRADMINCONNECTIONHANGUPNOTIFICATIONEX,
    pub lpfnRASValidatePreAuthenticatedConnectionEx: PMPRADMINRASVALIDATEPREAUTHENTICATEDCONNECTIONEX,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for MPRAPI_ADMIN_DLL_CALLBACKS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for MPRAPI_ADMIN_DLL_CALLBACKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPRAPI_OBJECT_HEADER {
    pub revision: u8,
    pub r#type: u8,
    pub size: u16,
}
impl ::core::marker::Copy for MPRAPI_OBJECT_HEADER {}
impl ::core::clone::Clone for MPRAPI_OBJECT_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct MPRAPI_TUNNEL_CONFIG_PARAMS0 {
    pub IkeConfigParams: IKEV2_CONFIG_PARAMS,
    pub PptpConfigParams: PPTP_CONFIG_PARAMS,
    pub L2tpConfigParams: L2TP_CONFIG_PARAMS1,
    pub SstpConfigParams: SSTP_CONFIG_PARAMS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for MPRAPI_TUNNEL_CONFIG_PARAMS0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for MPRAPI_TUNNEL_CONFIG_PARAMS0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct MPRAPI_TUNNEL_CONFIG_PARAMS1 {
    pub IkeConfigParams: IKEV2_CONFIG_PARAMS,
    pub PptpConfigParams: PPTP_CONFIG_PARAMS,
    pub L2tpConfigParams: L2TP_CONFIG_PARAMS1,
    pub SstpConfigParams: SSTP_CONFIG_PARAMS,
    pub GREConfigParams: GRE_CONFIG_PARAMS0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for MPRAPI_TUNNEL_CONFIG_PARAMS1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for MPRAPI_TUNNEL_CONFIG_PARAMS1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_CERT_EKU {
    pub dwSize: u32,
    pub IsEKUOID: super::super::Foundation::BOOL,
    pub pwszEKU: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_CERT_EKU {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_CERT_EKU {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPR_CREDENTIALSEX_0 {
    pub dwSize: u32,
    pub lpbCredentialsInfo: *mut u8,
}
impl ::core::marker::Copy for MPR_CREDENTIALSEX_0 {}
impl ::core::clone::Clone for MPR_CREDENTIALSEX_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPR_CREDENTIALSEX_1 {
    pub dwSize: u32,
    pub lpbCredentialsInfo: *mut u8,
}
impl ::core::marker::Copy for MPR_CREDENTIALSEX_1 {}
impl ::core::clone::Clone for MPR_CREDENTIALSEX_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPR_DEVICE_0 {
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
}
impl ::core::marker::Copy for MPR_DEVICE_0 {}
impl ::core::clone::Clone for MPR_DEVICE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPR_DEVICE_1 {
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szLocalPhoneNumber: [u16; 129],
    pub szAlternates: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for MPR_DEVICE_1 {}
impl ::core::clone::Clone for MPR_DEVICE_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_FILTER_0 {
    pub fEnable: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_FILTER_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_FILTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_IFTRANSPORT_0 {
    pub dwTransportId: u32,
    pub hIfTransport: super::super::Foundation::HANDLE,
    pub wszIfTransportName: [u16; 41],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_IFTRANSPORT_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_IFTRANSPORT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct MPR_IF_CUSTOMINFOEX0 {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub dwFlags: u32,
    pub customIkev2Config: ROUTER_IKEv2_IF_CUSTOM_CONFIG0,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for MPR_IF_CUSTOMINFOEX0 {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for MPR_IF_CUSTOMINFOEX0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct MPR_IF_CUSTOMINFOEX1 {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub dwFlags: u32,
    pub customIkev2Config: ROUTER_IKEv2_IF_CUSTOM_CONFIG1,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for MPR_IF_CUSTOMINFOEX1 {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for MPR_IF_CUSTOMINFOEX1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
pub struct MPR_IF_CUSTOMINFOEX2 {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub dwFlags: u32,
    pub customIkev2Config: ROUTER_IKEv2_IF_CUSTOM_CONFIG2,
}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for MPR_IF_CUSTOMINFOEX2 {}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for MPR_IF_CUSTOMINFOEX2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_INTERFACE_0 {
    pub wszInterfaceName: [u16; 257],
    pub hInterface: super::super::Foundation::HANDLE,
    pub fEnabled: super::super::Foundation::BOOL,
    pub dwIfType: ROUTER_INTERFACE_TYPE,
    pub dwConnectionState: ROUTER_CONNECTION_STATE,
    pub fUnReachabilityReasons: u32,
    pub dwLastError: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_INTERFACE_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_INTERFACE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_INTERFACE_1 {
    pub wszInterfaceName: [u16; 257],
    pub hInterface: super::super::Foundation::HANDLE,
    pub fEnabled: super::super::Foundation::BOOL,
    pub dwIfType: ROUTER_INTERFACE_TYPE,
    pub dwConnectionState: ROUTER_CONNECTION_STATE,
    pub fUnReachabilityReasons: u32,
    pub dwLastError: u32,
    pub lpwsDialoutHoursRestriction: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_INTERFACE_1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_INTERFACE_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_INTERFACE_2 {
    pub wszInterfaceName: [u16; 257],
    pub hInterface: super::super::Foundation::HANDLE,
    pub fEnabled: super::super::Foundation::BOOL,
    pub dwIfType: ROUTER_INTERFACE_TYPE,
    pub dwConnectionState: ROUTER_CONNECTION_STATE,
    pub fUnReachabilityReasons: u32,
    pub dwLastError: u32,
    pub dwfOptions: u32,
    pub szLocalPhoneNumber: [u16; 129],
    pub szAlternates: ::windows_sys::core::PWSTR,
    pub ipaddr: u32,
    pub ipaddrDns: u32,
    pub ipaddrDnsAlt: u32,
    pub ipaddrWins: u32,
    pub ipaddrWinsAlt: u32,
    pub dwfNetProtocols: u32,
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szX25PadType: [u16; 33],
    pub szX25Address: [u16; 201],
    pub szX25Facilities: [u16; 201],
    pub szX25UserData: [u16; 201],
    pub dwChannels: u32,
    pub dwSubEntries: u32,
    pub dwDialMode: MPR_INTERFACE_DIAL_MODE,
    pub dwDialExtraPercent: u32,
    pub dwDialExtraSampleSeconds: u32,
    pub dwHangUpExtraPercent: u32,
    pub dwHangUpExtraSampleSeconds: u32,
    pub dwIdleDisconnectSeconds: u32,
    pub dwType: u32,
    pub dwEncryptionType: MPR_ET,
    pub dwCustomAuthKey: u32,
    pub dwCustomAuthDataSize: u32,
    pub lpbCustomAuthData: *mut u8,
    pub guidId: ::windows_sys::core::GUID,
    pub dwVpnStrategy: MPR_VS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_INTERFACE_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_INTERFACE_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct MPR_INTERFACE_3 {
    pub wszInterfaceName: [u16; 257],
    pub hInterface: super::super::Foundation::HANDLE,
    pub fEnabled: super::super::Foundation::BOOL,
    pub dwIfType: ROUTER_INTERFACE_TYPE,
    pub dwConnectionState: ROUTER_CONNECTION_STATE,
    pub fUnReachabilityReasons: u32,
    pub dwLastError: u32,
    pub dwfOptions: u32,
    pub szLocalPhoneNumber: [u16; 129],
    pub szAlternates: ::windows_sys::core::PWSTR,
    pub ipaddr: u32,
    pub ipaddrDns: u32,
    pub ipaddrDnsAlt: u32,
    pub ipaddrWins: u32,
    pub ipaddrWinsAlt: u32,
    pub dwfNetProtocols: u32,
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szX25PadType: [u16; 33],
    pub szX25Address: [u16; 201],
    pub szX25Facilities: [u16; 201],
    pub szX25UserData: [u16; 201],
    pub dwChannels: u32,
    pub dwSubEntries: u32,
    pub dwDialMode: MPR_INTERFACE_DIAL_MODE,
    pub dwDialExtraPercent: u32,
    pub dwDialExtraSampleSeconds: u32,
    pub dwHangUpExtraPercent: u32,
    pub dwHangUpExtraSampleSeconds: u32,
    pub dwIdleDisconnectSeconds: u32,
    pub dwType: u32,
    pub dwEncryptionType: MPR_ET,
    pub dwCustomAuthKey: u32,
    pub dwCustomAuthDataSize: u32,
    pub lpbCustomAuthData: *mut u8,
    pub guidId: ::windows_sys::core::GUID,
    pub dwVpnStrategy: MPR_VS,
    pub AddressCount: u32,
    pub ipv6addrDns: super::super::Networking::WinSock::IN6_ADDR,
    pub ipv6addrDnsAlt: super::super::Networking::WinSock::IN6_ADDR,
    pub ipv6addr: *mut super::super::Networking::WinSock::IN6_ADDR,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for MPR_INTERFACE_3 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for MPR_INTERFACE_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPR_IPINIP_INTERFACE_0 {
    pub wszFriendlyName: [u16; 257],
    pub Guid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for MPR_IPINIP_INTERFACE_0 {}
impl ::core::clone::Clone for MPR_IPINIP_INTERFACE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_SERVER_0 {
    pub fLanOnlyMode: super::super::Foundation::BOOL,
    pub dwUpTime: u32,
    pub dwTotalPorts: u32,
    pub dwPortsInUse: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_SERVER_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_SERVER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPR_SERVER_1 {
    pub dwNumPptpPorts: u32,
    pub dwPptpPortFlags: u32,
    pub dwNumL2tpPorts: u32,
    pub dwL2tpPortFlags: u32,
}
impl ::core::marker::Copy for MPR_SERVER_1 {}
impl ::core::clone::Clone for MPR_SERVER_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MPR_SERVER_2 {
    pub dwNumPptpPorts: u32,
    pub dwPptpPortFlags: u32,
    pub dwNumL2tpPorts: u32,
    pub dwL2tpPortFlags: u32,
    pub dwNumSstpPorts: u32,
    pub dwSstpPortFlags: u32,
}
impl ::core::marker::Copy for MPR_SERVER_2 {}
impl ::core::clone::Clone for MPR_SERVER_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct MPR_SERVER_EX0 {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub fLanOnlyMode: u32,
    pub dwUpTime: u32,
    pub dwTotalPorts: u32,
    pub dwPortsInUse: u32,
    pub Reserved: u32,
    pub ConfigParams: MPRAPI_TUNNEL_CONFIG_PARAMS0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for MPR_SERVER_EX0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for MPR_SERVER_EX0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct MPR_SERVER_EX1 {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub fLanOnlyMode: u32,
    pub dwUpTime: u32,
    pub dwTotalPorts: u32,
    pub dwPortsInUse: u32,
    pub Reserved: u32,
    pub ConfigParams: MPRAPI_TUNNEL_CONFIG_PARAMS1,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for MPR_SERVER_EX1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for MPR_SERVER_EX1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct MPR_SERVER_SET_CONFIG_EX0 {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub setConfigForProtocols: u32,
    pub ConfigParams: MPRAPI_TUNNEL_CONFIG_PARAMS0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for MPR_SERVER_SET_CONFIG_EX0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for MPR_SERVER_SET_CONFIG_EX0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct MPR_SERVER_SET_CONFIG_EX1 {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub setConfigForProtocols: u32,
    pub ConfigParams: MPRAPI_TUNNEL_CONFIG_PARAMS1,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for MPR_SERVER_SET_CONFIG_EX1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for MPR_SERVER_SET_CONFIG_EX1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MPR_TRANSPORT_0 {
    pub dwTransportId: u32,
    pub hTransport: super::super::Foundation::HANDLE,
    pub wszTransportName: [u16; 41],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MPR_TRANSPORT_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MPR_TRANSPORT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct MPR_VPN_TRAFFIC_SELECTOR {
    pub r#type: MPR_VPN_TS_TYPE,
    pub protocolId: u8,
    pub portStart: u16,
    pub portEnd: u16,
    pub tsPayloadId: u16,
    pub addrStart: VPN_TS_IP_ADDRESS,
    pub addrEnd: VPN_TS_IP_ADDRESS,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for MPR_VPN_TRAFFIC_SELECTOR {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for MPR_VPN_TRAFFIC_SELECTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct MPR_VPN_TRAFFIC_SELECTORS {
    pub numTsi: u32,
    pub numTsr: u32,
    pub tsI: *mut MPR_VPN_TRAFFIC_SELECTOR,
    pub tsR: *mut MPR_VPN_TRAFFIC_SELECTOR,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for MPR_VPN_TRAFFIC_SELECTORS {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for MPR_VPN_TRAFFIC_SELECTORS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_ATCP_INFO {
    pub dwError: u32,
    pub wszAddress: [u16; 33],
}
impl ::core::marker::Copy for PPP_ATCP_INFO {}
impl ::core::clone::Clone for PPP_ATCP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_CCP_INFO {
    pub dwError: u32,
    pub dwCompressionAlgorithm: u32,
    pub dwOptions: u32,
    pub dwRemoteCompressionAlgorithm: u32,
    pub dwRemoteOptions: u32,
}
impl ::core::marker::Copy for PPP_CCP_INFO {}
impl ::core::clone::Clone for PPP_CCP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_INFO {
    pub nbf: PPP_NBFCP_INFO,
    pub ip: PPP_IPCP_INFO,
    pub ipx: PPP_IPXCP_INFO,
    pub at: PPP_ATCP_INFO,
}
impl ::core::marker::Copy for PPP_INFO {}
impl ::core::clone::Clone for PPP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_INFO_2 {
    pub nbf: PPP_NBFCP_INFO,
    pub ip: PPP_IPCP_INFO2,
    pub ipx: PPP_IPXCP_INFO,
    pub at: PPP_ATCP_INFO,
    pub ccp: PPP_CCP_INFO,
    pub lcp: PPP_LCP_INFO,
}
impl ::core::marker::Copy for PPP_INFO_2 {}
impl ::core::clone::Clone for PPP_INFO_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_INFO_3 {
    pub nbf: PPP_NBFCP_INFO,
    pub ip: PPP_IPCP_INFO2,
    pub ipv6: PPP_IPV6_CP_INFO,
    pub ccp: PPP_CCP_INFO,
    pub lcp: PPP_LCP_INFO,
}
impl ::core::marker::Copy for PPP_INFO_3 {}
impl ::core::clone::Clone for PPP_INFO_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_IPCP_INFO {
    pub dwError: u32,
    pub wszAddress: [u16; 16],
    pub wszRemoteAddress: [u16; 16],
}
impl ::core::marker::Copy for PPP_IPCP_INFO {}
impl ::core::clone::Clone for PPP_IPCP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_IPCP_INFO2 {
    pub dwError: u32,
    pub wszAddress: [u16; 16],
    pub wszRemoteAddress: [u16; 16],
    pub dwOptions: u32,
    pub dwRemoteOptions: u32,
}
impl ::core::marker::Copy for PPP_IPCP_INFO2 {}
impl ::core::clone::Clone for PPP_IPCP_INFO2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_IPV6_CP_INFO {
    pub dwVersion: u32,
    pub dwSize: u32,
    pub dwError: u32,
    pub bInterfaceIdentifier: [u8; 8],
    pub bRemoteInterfaceIdentifier: [u8; 8],
    pub dwOptions: u32,
    pub dwRemoteOptions: u32,
    pub bPrefix: [u8; 8],
    pub dwPrefixLength: u32,
}
impl ::core::marker::Copy for PPP_IPV6_CP_INFO {}
impl ::core::clone::Clone for PPP_IPV6_CP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_IPXCP_INFO {
    pub dwError: u32,
    pub wszAddress: [u16; 23],
}
impl ::core::marker::Copy for PPP_IPXCP_INFO {}
impl ::core::clone::Clone for PPP_IPXCP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_LCP_INFO {
    pub dwError: u32,
    pub dwAuthenticationProtocol: PPP_LCP,
    pub dwAuthenticationData: PPP_LCP_INFO_AUTH_DATA,
    pub dwRemoteAuthenticationProtocol: u32,
    pub dwRemoteAuthenticationData: u32,
    pub dwTerminateReason: u32,
    pub dwRemoteTerminateReason: u32,
    pub dwOptions: u32,
    pub dwRemoteOptions: u32,
    pub dwEapTypeId: u32,
    pub dwRemoteEapTypeId: u32,
}
impl ::core::marker::Copy for PPP_LCP_INFO {}
impl ::core::clone::Clone for PPP_LCP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_NBFCP_INFO {
    pub dwError: u32,
    pub wszWksta: [u16; 17],
}
impl ::core::marker::Copy for PPP_NBFCP_INFO {}
impl ::core::clone::Clone for PPP_NBFCP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_PROJECTION_INFO {
    pub dwIPv4NegotiationError: u32,
    pub wszAddress: [u16; 16],
    pub wszRemoteAddress: [u16; 16],
    pub dwIPv4Options: u32,
    pub dwIPv4RemoteOptions: u32,
    pub IPv4SubInterfaceIndex: u64,
    pub dwIPv6NegotiationError: u32,
    pub bInterfaceIdentifier: [u8; 8],
    pub bRemoteInterfaceIdentifier: [u8; 8],
    pub bPrefix: [u8; 8],
    pub dwPrefixLength: u32,
    pub IPv6SubInterfaceIndex: u64,
    pub dwLcpError: u32,
    pub dwAuthenticationProtocol: PPP_LCP,
    pub dwAuthenticationData: PPP_LCP_INFO_AUTH_DATA,
    pub dwRemoteAuthenticationProtocol: PPP_LCP,
    pub dwRemoteAuthenticationData: PPP_LCP_INFO_AUTH_DATA,
    pub dwLcpTerminateReason: u32,
    pub dwLcpRemoteTerminateReason: u32,
    pub dwLcpOptions: u32,
    pub dwLcpRemoteOptions: u32,
    pub dwEapTypeId: u32,
    pub dwRemoteEapTypeId: u32,
    pub dwCcpError: u32,
    pub dwCompressionAlgorithm: u32,
    pub dwCcpOptions: u32,
    pub dwRemoteCompressionAlgorithm: u32,
    pub dwCcpRemoteOptions: u32,
}
impl ::core::marker::Copy for PPP_PROJECTION_INFO {}
impl ::core::clone::Clone for PPP_PROJECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPP_PROJECTION_INFO2 {
    pub dwIPv4NegotiationError: u32,
    pub wszAddress: [u16; 16],
    pub wszRemoteAddress: [u16; 16],
    pub dwIPv4Options: u32,
    pub dwIPv4RemoteOptions: u32,
    pub IPv4SubInterfaceIndex: u64,
    pub dwIPv6NegotiationError: u32,
    pub bInterfaceIdentifier: [u8; 8],
    pub bRemoteInterfaceIdentifier: [u8; 8],
    pub bPrefix: [u8; 8],
    pub dwPrefixLength: u32,
    pub IPv6SubInterfaceIndex: u64,
    pub dwLcpError: u32,
    pub dwAuthenticationProtocol: PPP_LCP,
    pub dwAuthenticationData: PPP_LCP_INFO_AUTH_DATA,
    pub dwRemoteAuthenticationProtocol: PPP_LCP,
    pub dwRemoteAuthenticationData: PPP_LCP_INFO_AUTH_DATA,
    pub dwLcpTerminateReason: u32,
    pub dwLcpRemoteTerminateReason: u32,
    pub dwLcpOptions: u32,
    pub dwLcpRemoteOptions: u32,
    pub dwEapTypeId: u32,
    pub dwEmbeddedEAPTypeId: u32,
    pub dwRemoteEapTypeId: u32,
    pub dwCcpError: u32,
    pub dwCompressionAlgorithm: u32,
    pub dwCcpOptions: u32,
    pub dwRemoteCompressionAlgorithm: u32,
    pub dwCcpRemoteOptions: u32,
}
impl ::core::marker::Copy for PPP_PROJECTION_INFO2 {}
impl ::core::clone::Clone for PPP_PROJECTION_INFO2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PPTP_CONFIG_PARAMS {
    pub dwNumPorts: u32,
    pub dwPortFlags: u32,
}
impl ::core::marker::Copy for PPTP_CONFIG_PARAMS {}
impl ::core::clone::Clone for PPTP_CONFIG_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROJECTION_INFO {
    pub projectionInfoType: u8,
    pub Anonymous: PROJECTION_INFO_0,
}
impl ::core::marker::Copy for PROJECTION_INFO {}
impl ::core::clone::Clone for PROJECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union PROJECTION_INFO_0 {
    pub PppProjectionInfo: PPP_PROJECTION_INFO,
    pub Ikev2ProjectionInfo: IKEV2_PROJECTION_INFO,
}
impl ::core::marker::Copy for PROJECTION_INFO_0 {}
impl ::core::clone::Clone for PROJECTION_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROJECTION_INFO2 {
    pub projectionInfoType: u8,
    pub Anonymous: PROJECTION_INFO2_0,
}
impl ::core::marker::Copy for PROJECTION_INFO2 {}
impl ::core::clone::Clone for PROJECTION_INFO2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union PROJECTION_INFO2_0 {
    pub PppProjectionInfo: PPP_PROJECTION_INFO2,
    pub Ikev2ProjectionInfo: IKEV2_PROJECTION_INFO2,
}
impl ::core::marker::Copy for PROJECTION_INFO2_0 {}
impl ::core::clone::Clone for PROJECTION_INFO2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RASADPARAMS {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASADPARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASADPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASAMBA {
    pub dwSize: u32,
    pub dwError: u32,
    pub szNetBiosError: [u8; 17],
    pub bLana: u8,
}
impl ::core::marker::Copy for RASAMBA {}
impl ::core::clone::Clone for RASAMBA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASAMBW {
    pub dwSize: u32,
    pub dwError: u32,
    pub szNetBiosError: [u16; 17],
    pub bLana: u8,
}
impl ::core::marker::Copy for RASAMBW {}
impl ::core::clone::Clone for RASAMBW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASAUTODIALENTRYA {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwDialingLocation: u32,
    pub szEntry: [u8; 257],
}
impl ::core::marker::Copy for RASAUTODIALENTRYA {}
impl ::core::clone::Clone for RASAUTODIALENTRYA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASAUTODIALENTRYW {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwDialingLocation: u32,
    pub szEntry: [u16; 257],
}
impl ::core::marker::Copy for RASAUTODIALENTRYW {}
impl ::core::clone::Clone for RASAUTODIALENTRYW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASCOMMSETTINGS {
    pub dwSize: u32,
    pub bParity: u8,
    pub bStop: u8,
    pub bByteSize: u8,
    pub bAlign: u8,
}
impl ::core::marker::Copy for RASCOMMSETTINGS {}
impl ::core::clone::Clone for RASCOMMSETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct RASCONNA {
    pub dwSize: u32,
    pub hrasconn: HRASCONN,
    pub szEntryName: [u8; 257],
    pub szDeviceType: [u8; 17],
    pub szDeviceName: [u8; 129],
    pub szPhonebook: [u8; 260],
    pub dwSubEntry: u32,
    pub guidEntry: ::windows_sys::core::GUID,
    pub dwFlags: u32,
    pub luid: super::super::Foundation::LUID,
    pub guidCorrelationId: ::windows_sys::core::GUID,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASCONNA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASCONNA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct RASCONNA {
    pub dwSize: u32,
    pub hrasconn: HRASCONN,
    pub szEntryName: [u8; 257],
    pub szDeviceType: [u8; 17],
    pub szDeviceName: [u8; 129],
    pub szPhonebook: [u8; 260],
    pub dwSubEntry: u32,
    pub guidEntry: ::windows_sys::core::GUID,
    pub dwFlags: u32,
    pub luid: super::super::Foundation::LUID,
    pub guidCorrelationId: ::windows_sys::core::GUID,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASCONNA {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASCONNA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RASCONNSTATUSA {
    pub dwSize: u32,
    pub rasconnstate: RASCONNSTATE,
    pub dwError: u32,
    pub szDeviceType: [u8; 17],
    pub szDeviceName: [u8; 129],
    pub szPhoneNumber: [u8; 129],
    pub localEndPoint: RASTUNNELENDPOINT,
    pub remoteEndPoint: RASTUNNELENDPOINT,
    pub rasconnsubstate: RASCONNSUBSTATE,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RASCONNSTATUSA {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RASCONNSTATUSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RASCONNSTATUSW {
    pub dwSize: u32,
    pub rasconnstate: RASCONNSTATE,
    pub dwError: u32,
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szPhoneNumber: [u16; 129],
    pub localEndPoint: RASTUNNELENDPOINT,
    pub remoteEndPoint: RASTUNNELENDPOINT,
    pub rasconnsubstate: RASCONNSUBSTATE,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RASCONNSTATUSW {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RASCONNSTATUSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct RASCONNW {
    pub dwSize: u32,
    pub hrasconn: HRASCONN,
    pub szEntryName: [u16; 257],
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szPhonebook: [u16; 260],
    pub dwSubEntry: u32,
    pub guidEntry: ::windows_sys::core::GUID,
    pub dwFlags: u32,
    pub luid: super::super::Foundation::LUID,
    pub guidCorrelationId: ::windows_sys::core::GUID,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASCONNW {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASCONNW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct RASCONNW {
    pub dwSize: u32,
    pub hrasconn: HRASCONN,
    pub szEntryName: [u16; 257],
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szPhonebook: [u16; 260],
    pub dwSubEntry: u32,
    pub guidEntry: ::windows_sys::core::GUID,
    pub dwFlags: u32,
    pub luid: super::super::Foundation::LUID,
    pub guidCorrelationId: ::windows_sys::core::GUID,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASCONNW {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASCONNW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASCREDENTIALSA {
    pub dwSize: u32,
    pub dwMask: u32,
    pub szUserName: [u8; 257],
    pub szPassword: [u8; 257],
    pub szDomain: [u8; 16],
}
impl ::core::marker::Copy for RASCREDENTIALSA {}
impl ::core::clone::Clone for RASCREDENTIALSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASCREDENTIALSW {
    pub dwSize: u32,
    pub dwMask: u32,
    pub szUserName: [u16; 257],
    pub szPassword: [u16; 257],
    pub szDomain: [u16; 16],
}
impl ::core::marker::Copy for RASCREDENTIALSW {}
impl ::core::clone::Clone for RASCREDENTIALSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASCTRYINFO {
    pub dwSize: u32,
    pub dwCountryID: u32,
    pub dwNextCountryID: u32,
    pub dwCountryCode: u32,
    pub dwCountryNameOffset: u32,
}
impl ::core::marker::Copy for RASCTRYINFO {}
impl ::core::clone::Clone for RASCTRYINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RASCUSTOMSCRIPTEXTENSIONS {
    pub dwSize: u32,
    pub pfnRasSetCommSettings: PFNRASSETCOMMSETTINGS,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASCUSTOMSCRIPTEXTENSIONS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASCUSTOMSCRIPTEXTENSIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASDEVINFOA {
    pub dwSize: u32,
    pub szDeviceType: [u8; 17],
    pub szDeviceName: [u8; 129],
}
impl ::core::marker::Copy for RASDEVINFOA {}
impl ::core::clone::Clone for RASDEVINFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASDEVINFOW {
    pub dwSize: u32,
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
}
impl ::core::marker::Copy for RASDEVINFOW {}
impl ::core::clone::Clone for RASDEVINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct RASDEVSPECIFICINFO {
    pub dwSize: u32,
    pub pbDevSpecificInfo: *mut u8,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for RASDEVSPECIFICINFO {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for RASDEVSPECIFICINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct RASDEVSPECIFICINFO {
    pub dwSize: u32,
    pub pbDevSpecificInfo: *mut u8,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for RASDEVSPECIFICINFO {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for RASDEVSPECIFICINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RASDIALDLG {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub dwSubEntry: u32,
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASDIALDLG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASDIALDLG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RASDIALEXTENSIONS {
    pub dwSize: u32,
    pub dwfOptions: u32,
    pub hwndParent: super::super::Foundation::HWND,
    pub reserved: usize,
    pub reserved1: usize,
    pub RasEapInfo: RASEAPINFO,
    pub fSkipPppAuth: super::super::Foundation::BOOL,
    pub RasDevSpecificInfo: RASDEVSPECIFICINFO,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASDIALEXTENSIONS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASDIALEXTENSIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct RASDIALPARAMSA {
    pub dwSize: u32,
    pub szEntryName: [u8; 257],
    pub szPhoneNumber: [u8; 129],
    pub szCallbackNumber: [u8; 129],
    pub szUserName: [u8; 257],
    pub szPassword: [u8; 257],
    pub szDomain: [u8; 16],
    pub dwSubEntry: u32,
    pub dwCallbackId: usize,
    pub dwIfIndex: u32,
    pub szEncPassword: ::windows_sys::core::PSTR,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for RASDIALPARAMSA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for RASDIALPARAMSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct RASDIALPARAMSA {
    pub dwSize: u32,
    pub szEntryName: [u8; 257],
    pub szPhoneNumber: [u8; 129],
    pub szCallbackNumber: [u8; 129],
    pub szUserName: [u8; 257],
    pub szPassword: [u8; 257],
    pub szDomain: [u8; 16],
    pub dwSubEntry: u32,
    pub dwCallbackId: usize,
    pub dwIfIndex: u32,
    pub szEncPassword: ::windows_sys::core::PSTR,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for RASDIALPARAMSA {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for RASDIALPARAMSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct RASDIALPARAMSW {
    pub dwSize: u32,
    pub szEntryName: [u16; 257],
    pub szPhoneNumber: [u16; 129],
    pub szCallbackNumber: [u16; 129],
    pub szUserName: [u16; 257],
    pub szPassword: [u16; 257],
    pub szDomain: [u16; 16],
    pub dwSubEntry: u32,
    pub dwCallbackId: usize,
    pub dwIfIndex: u32,
    pub szEncPassword: ::windows_sys::core::PWSTR,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for RASDIALPARAMSW {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for RASDIALPARAMSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct RASDIALPARAMSW {
    pub dwSize: u32,
    pub szEntryName: [u16; 257],
    pub szPhoneNumber: [u16; 129],
    pub szCallbackNumber: [u16; 129],
    pub szUserName: [u16; 257],
    pub szPassword: [u16; 257],
    pub szDomain: [u16; 16],
    pub dwSubEntry: u32,
    pub dwCallbackId: usize,
    pub dwIfIndex: u32,
    pub szEncPassword: ::windows_sys::core::PWSTR,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for RASDIALPARAMSW {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for RASDIALPARAMSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct RASEAPINFO {
    pub dwSizeofEapInfo: u32,
    pub pbEapInfo: *mut u8,
}
impl ::core::marker::Copy for RASEAPINFO {}
impl ::core::clone::Clone for RASEAPINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASEAPUSERIDENTITYA {
    pub szUserName: [u8; 257],
    pub dwSizeofEapInfo: u32,
    pub pbEapInfo: [u8; 1],
}
impl ::core::marker::Copy for RASEAPUSERIDENTITYA {}
impl ::core::clone::Clone for RASEAPUSERIDENTITYA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASEAPUSERIDENTITYW {
    pub szUserName: [u16; 257],
    pub dwSizeofEapInfo: u32,
    pub pbEapInfo: [u8; 1],
}
impl ::core::marker::Copy for RASEAPUSERIDENTITYW {}
impl ::core::clone::Clone for RASEAPUSERIDENTITYW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct RASENTRYA {
    pub dwSize: u32,
    pub dwfOptions: u32,
    pub dwCountryID: u32,
    pub dwCountryCode: u32,
    pub szAreaCode: [u8; 11],
    pub szLocalPhoneNumber: [u8; 129],
    pub dwAlternateOffset: u32,
    pub ipaddr: RASIPADDR,
    pub ipaddrDns: RASIPADDR,
    pub ipaddrDnsAlt: RASIPADDR,
    pub ipaddrWins: RASIPADDR,
    pub ipaddrWinsAlt: RASIPADDR,
    pub dwFrameSize: u32,
    pub dwfNetProtocols: u32,
    pub dwFramingProtocol: u32,
    pub szScript: [u8; 260],
    pub szAutodialDll: [u8; 260],
    pub szAutodialFunc: [u8; 260],
    pub szDeviceType: [u8; 17],
    pub szDeviceName: [u8; 129],
    pub szX25PadType: [u8; 33],
    pub szX25Address: [u8; 201],
    pub szX25Facilities: [u8; 201],
    pub szX25UserData: [u8; 201],
    pub dwChannels: u32,
    pub dwReserved1: u32,
    pub dwReserved2: u32,
    pub dwSubEntries: u32,
    pub dwDialMode: RASENTRY_DIAL_MODE,
    pub dwDialExtraPercent: u32,
    pub dwDialExtraSampleSeconds: u32,
    pub dwHangUpExtraPercent: u32,
    pub dwHangUpExtraSampleSeconds: u32,
    pub dwIdleDisconnectSeconds: u32,
    pub dwType: u32,
    pub dwEncryptionType: u32,
    pub dwCustomAuthKey: u32,
    pub guidId: ::windows_sys::core::GUID,
    pub szCustomDialDll: [u8; 260],
    pub dwVpnStrategy: u32,
    pub dwfOptions2: u32,
    pub dwfOptions3: u32,
    pub szDnsSuffix: [u8; 256],
    pub dwTcpWindowSize: u32,
    pub szPrerequisitePbk: [u8; 260],
    pub szPrerequisiteEntry: [u8; 257],
    pub dwRedialCount: u32,
    pub dwRedialPause: u32,
    pub ipv6addrDns: super::super::Networking::WinSock::IN6_ADDR,
    pub ipv6addrDnsAlt: super::super::Networking::WinSock::IN6_ADDR,
    pub dwIPv4InterfaceMetric: u32,
    pub dwIPv6InterfaceMetric: u32,
    pub ipv6addr: super::super::Networking::WinSock::IN6_ADDR,
    pub dwIPv6PrefixLength: u32,
    pub dwNetworkOutageTime: u32,
    pub szIDi: [u8; 257],
    pub szIDr: [u8; 257],
    pub fIsImsConfig: super::super::Foundation::BOOL,
    pub IdiType: IKEV2_ID_PAYLOAD_TYPE,
    pub IdrType: IKEV2_ID_PAYLOAD_TYPE,
    pub fDisableIKEv2Fragmentation: super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for RASENTRYA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for RASENTRYA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct RASENTRYDLGA {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub szEntry: [u8; 257],
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASENTRYDLGA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASENTRYDLGA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct RASENTRYDLGA {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub szEntry: [u8; 257],
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASENTRYDLGA {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASENTRYDLGA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct RASENTRYDLGW {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub szEntry: [u16; 257],
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASENTRYDLGW {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASENTRYDLGW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct RASENTRYDLGW {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub szEntry: [u16; 257],
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASENTRYDLGW {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASENTRYDLGW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASENTRYNAMEA {
    pub dwSize: u32,
    pub szEntryName: [u8; 257],
    pub dwFlags: u32,
    pub szPhonebookPath: [u8; 261],
}
impl ::core::marker::Copy for RASENTRYNAMEA {}
impl ::core::clone::Clone for RASENTRYNAMEA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASENTRYNAMEW {
    pub dwSize: u32,
    pub szEntryName: [u16; 257],
    pub dwFlags: u32,
    pub szPhonebookPath: [u16; 261],
}
impl ::core::marker::Copy for RASENTRYNAMEW {}
impl ::core::clone::Clone for RASENTRYNAMEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct RASENTRYW {
    pub dwSize: u32,
    pub dwfOptions: u32,
    pub dwCountryID: u32,
    pub dwCountryCode: u32,
    pub szAreaCode: [u16; 11],
    pub szLocalPhoneNumber: [u16; 129],
    pub dwAlternateOffset: u32,
    pub ipaddr: RASIPADDR,
    pub ipaddrDns: RASIPADDR,
    pub ipaddrDnsAlt: RASIPADDR,
    pub ipaddrWins: RASIPADDR,
    pub ipaddrWinsAlt: RASIPADDR,
    pub dwFrameSize: u32,
    pub dwfNetProtocols: u32,
    pub dwFramingProtocol: u32,
    pub szScript: [u16; 260],
    pub szAutodialDll: [u16; 260],
    pub szAutodialFunc: [u16; 260],
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szX25PadType: [u16; 33],
    pub szX25Address: [u16; 201],
    pub szX25Facilities: [u16; 201],
    pub szX25UserData: [u16; 201],
    pub dwChannels: u32,
    pub dwReserved1: u32,
    pub dwReserved2: u32,
    pub dwSubEntries: u32,
    pub dwDialMode: RASENTRY_DIAL_MODE,
    pub dwDialExtraPercent: u32,
    pub dwDialExtraSampleSeconds: u32,
    pub dwHangUpExtraPercent: u32,
    pub dwHangUpExtraSampleSeconds: u32,
    pub dwIdleDisconnectSeconds: u32,
    pub dwType: u32,
    pub dwEncryptionType: u32,
    pub dwCustomAuthKey: u32,
    pub guidId: ::windows_sys::core::GUID,
    pub szCustomDialDll: [u16; 260],
    pub dwVpnStrategy: u32,
    pub dwfOptions2: u32,
    pub dwfOptions3: u32,
    pub szDnsSuffix: [u16; 256],
    pub dwTcpWindowSize: u32,
    pub szPrerequisitePbk: [u16; 260],
    pub szPrerequisiteEntry: [u16; 257],
    pub dwRedialCount: u32,
    pub dwRedialPause: u32,
    pub ipv6addrDns: super::super::Networking::WinSock::IN6_ADDR,
    pub ipv6addrDnsAlt: super::super::Networking::WinSock::IN6_ADDR,
    pub dwIPv4InterfaceMetric: u32,
    pub dwIPv6InterfaceMetric: u32,
    pub ipv6addr: super::super::Networking::WinSock::IN6_ADDR,
    pub dwIPv6PrefixLength: u32,
    pub dwNetworkOutageTime: u32,
    pub szIDi: [u16; 257],
    pub szIDr: [u16; 257],
    pub fIsImsConfig: super::super::Foundation::BOOL,
    pub IdiType: IKEV2_ID_PAYLOAD_TYPE,
    pub IdrType: IKEV2_ID_PAYLOAD_TYPE,
    pub fDisableIKEv2Fragmentation: super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for RASENTRYW {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for RASENTRYW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RASIKEV2_PROJECTION_INFO {
    pub dwIPv4NegotiationError: u32,
    pub ipv4Address: super::super::Networking::WinSock::IN_ADDR,
    pub ipv4ServerAddress: super::super::Networking::WinSock::IN_ADDR,
    pub dwIPv6NegotiationError: u32,
    pub ipv6Address: super::super::Networking::WinSock::IN6_ADDR,
    pub ipv6ServerAddress: super::super::Networking::WinSock::IN6_ADDR,
    pub dwPrefixLength: u32,
    pub dwAuthenticationProtocol: u32,
    pub dwEapTypeId: u32,
    pub dwFlags: RASIKEV_PROJECTION_INFO_FLAGS,
    pub dwEncryptionMethod: u32,
    pub numIPv4ServerAddresses: u32,
    pub ipv4ServerAddresses: *mut super::super::Networking::WinSock::IN_ADDR,
    pub numIPv6ServerAddresses: u32,
    pub ipv6ServerAddresses: *mut super::super::Networking::WinSock::IN6_ADDR,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RASIKEV2_PROJECTION_INFO {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RASIKEV2_PROJECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RASIKEV2_PROJECTION_INFO {
    pub dwIPv4NegotiationError: u32,
    pub ipv4Address: super::super::Networking::WinSock::IN_ADDR,
    pub ipv4ServerAddress: super::super::Networking::WinSock::IN_ADDR,
    pub dwIPv6NegotiationError: u32,
    pub ipv6Address: super::super::Networking::WinSock::IN6_ADDR,
    pub ipv6ServerAddress: super::super::Networking::WinSock::IN6_ADDR,
    pub dwPrefixLength: u32,
    pub dwAuthenticationProtocol: u32,
    pub dwEapTypeId: u32,
    pub dwFlags: RASIKEV_PROJECTION_INFO_FLAGS,
    pub dwEncryptionMethod: u32,
    pub numIPv4ServerAddresses: u32,
    pub ipv4ServerAddresses: *mut super::super::Networking::WinSock::IN_ADDR,
    pub numIPv6ServerAddresses: u32,
    pub ipv6ServerAddresses: *mut super::super::Networking::WinSock::IN6_ADDR,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RASIKEV2_PROJECTION_INFO {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RASIKEV2_PROJECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASIPADDR {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
}
impl ::core::marker::Copy for RASIPADDR {}
impl ::core::clone::Clone for RASIPADDR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASIPXW {
    pub dwSize: u32,
    pub dwError: u32,
    pub szIpxAddress: [u16; 22],
}
impl ::core::marker::Copy for RASIPXW {}
impl ::core::clone::Clone for RASIPXW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASNOUSERA {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwTimeoutMs: u32,
    pub szUserName: [u8; 257],
    pub szPassword: [u8; 257],
    pub szDomain: [u8; 16],
}
impl ::core::marker::Copy for RASNOUSERA {}
impl ::core::clone::Clone for RASNOUSERA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASNOUSERW {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwTimeoutMs: u32,
    pub szUserName: [u16; 257],
    pub szPassword: [u16; 257],
    pub szDomain: [u16; 16],
}
impl ::core::marker::Copy for RASNOUSERW {}
impl ::core::clone::Clone for RASNOUSERW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct RASPBDLGA {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub dwCallbackId: usize,
    pub pCallback: RASPBDLGFUNCA,
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASPBDLGA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASPBDLGA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct RASPBDLGA {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub dwCallbackId: usize,
    pub pCallback: RASPBDLGFUNCA,
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASPBDLGA {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASPBDLGA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct RASPBDLGW {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub dwCallbackId: usize,
    pub pCallback: RASPBDLGFUNCW,
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASPBDLGW {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASPBDLGW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct RASPBDLGW {
    pub dwSize: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub dwFlags: u32,
    pub xDlg: i32,
    pub yDlg: i32,
    pub dwCallbackId: usize,
    pub pCallback: RASPBDLGFUNCW,
    pub dwError: u32,
    pub reserved: usize,
    pub reserved2: usize,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASPBDLGW {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASPBDLGW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASPPPCCP {
    pub dwSize: u32,
    pub dwError: u32,
    pub dwCompressionAlgorithm: u32,
    pub dwOptions: u32,
    pub dwServerCompressionAlgorithm: u32,
    pub dwServerOptions: u32,
}
impl ::core::marker::Copy for RASPPPCCP {}
impl ::core::clone::Clone for RASPPPCCP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASPPPIPA {
    pub dwSize: u32,
    pub dwError: u32,
    pub szIpAddress: [u8; 16],
    pub szServerIpAddress: [u8; 16],
    pub dwOptions: u32,
    pub dwServerOptions: u32,
}
impl ::core::marker::Copy for RASPPPIPA {}
impl ::core::clone::Clone for RASPPPIPA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASPPPIPV6 {
    pub dwSize: u32,
    pub dwError: u32,
    pub bLocalInterfaceIdentifier: [u8; 8],
    pub bPeerInterfaceIdentifier: [u8; 8],
    pub bLocalCompressionProtocol: [u8; 2],
    pub bPeerCompressionProtocol: [u8; 2],
}
impl ::core::marker::Copy for RASPPPIPV6 {}
impl ::core::clone::Clone for RASPPPIPV6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASPPPIPW {
    pub dwSize: u32,
    pub dwError: u32,
    pub szIpAddress: [u16; 16],
    pub szServerIpAddress: [u16; 16],
    pub dwOptions: u32,
    pub dwServerOptions: u32,
}
impl ::core::marker::Copy for RASPPPIPW {}
impl ::core::clone::Clone for RASPPPIPW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASPPPIPXA {
    pub dwSize: u32,
    pub dwError: u32,
    pub szIpxAddress: [u8; 22],
}
impl ::core::marker::Copy for RASPPPIPXA {}
impl ::core::clone::Clone for RASPPPIPXA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RASPPPLCPA {
    pub dwSize: u32,
    pub fBundled: super::super::Foundation::BOOL,
    pub dwError: u32,
    pub dwAuthenticationProtocol: u32,
    pub dwAuthenticationData: u32,
    pub dwEapTypeId: u32,
    pub dwServerAuthenticationProtocol: u32,
    pub dwServerAuthenticationData: u32,
    pub dwServerEapTypeId: u32,
    pub fMultilink: super::super::Foundation::BOOL,
    pub dwTerminateReason: u32,
    pub dwServerTerminateReason: u32,
    pub szReplyMessage: [u8; 1024],
    pub dwOptions: u32,
    pub dwServerOptions: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASPPPLCPA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASPPPLCPA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RASPPPLCPW {
    pub dwSize: u32,
    pub fBundled: super::super::Foundation::BOOL,
    pub dwError: u32,
    pub dwAuthenticationProtocol: u32,
    pub dwAuthenticationData: u32,
    pub dwEapTypeId: u32,
    pub dwServerAuthenticationProtocol: u32,
    pub dwServerAuthenticationData: u32,
    pub dwServerEapTypeId: u32,
    pub fMultilink: super::super::Foundation::BOOL,
    pub dwTerminateReason: u32,
    pub dwServerTerminateReason: u32,
    pub szReplyMessage: [u16; 1024],
    pub dwOptions: u32,
    pub dwServerOptions: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RASPPPLCPW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RASPPPLCPW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASPPPNBFA {
    pub dwSize: u32,
    pub dwError: u32,
    pub dwNetBiosError: u32,
    pub szNetBiosError: [u8; 17],
    pub szWorkstationName: [u8; 17],
    pub bLana: u8,
}
impl ::core::marker::Copy for RASPPPNBFA {}
impl ::core::clone::Clone for RASPPPNBFA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASPPPNBFW {
    pub dwSize: u32,
    pub dwError: u32,
    pub dwNetBiosError: u32,
    pub szNetBiosError: [u16; 17],
    pub szWorkstationName: [u16; 17],
    pub bLana: u8,
}
impl ::core::marker::Copy for RASPPPNBFW {}
impl ::core::clone::Clone for RASPPPNBFW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct RASPPP_PROJECTION_INFO {
    pub dwIPv4NegotiationError: u32,
    pub ipv4Address: super::super::Networking::WinSock::IN_ADDR,
    pub ipv4ServerAddress: super::super::Networking::WinSock::IN_ADDR,
    pub dwIPv4Options: u32,
    pub dwIPv4ServerOptions: u32,
    pub dwIPv6NegotiationError: u32,
    pub bInterfaceIdentifier: [u8; 8],
    pub bServerInterfaceIdentifier: [u8; 8],
    pub fBundled: super::super::Foundation::BOOL,
    pub fMultilink: super::super::Foundation::BOOL,
    pub dwAuthenticationProtocol: RASPPP_PROJECTION_INFO_SERVER_AUTH_PROTOCOL,
    pub dwAuthenticationData: RASPPP_PROJECTION_INFO_SERVER_AUTH_DATA,
    pub dwServerAuthenticationProtocol: RASPPP_PROJECTION_INFO_SERVER_AUTH_PROTOCOL,
    pub dwServerAuthenticationData: RASPPP_PROJECTION_INFO_SERVER_AUTH_DATA,
    pub dwEapTypeId: u32,
    pub dwServerEapTypeId: u32,
    pub dwLcpOptions: u32,
    pub dwLcpServerOptions: u32,
    pub dwCcpError: u32,
    pub dwCcpCompressionAlgorithm: u32,
    pub dwCcpServerCompressionAlgorithm: u32,
    pub dwCcpOptions: u32,
    pub dwCcpServerOptions: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for RASPPP_PROJECTION_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for RASPPP_PROJECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASSUBENTRYA {
    pub dwSize: u32,
    pub dwfFlags: u32,
    pub szDeviceType: [u8; 17],
    pub szDeviceName: [u8; 129],
    pub szLocalPhoneNumber: [u8; 129],
    pub dwAlternateOffset: u32,
}
impl ::core::marker::Copy for RASSUBENTRYA {}
impl ::core::clone::Clone for RASSUBENTRYA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RASSUBENTRYW {
    pub dwSize: u32,
    pub dwfFlags: u32,
    pub szDeviceType: [u16; 17],
    pub szDeviceName: [u16; 129],
    pub szLocalPhoneNumber: [u16; 129],
    pub dwAlternateOffset: u32,
}
impl ::core::marker::Copy for RASSUBENTRYW {}
impl ::core::clone::Clone for RASSUBENTRYW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RASTUNNELENDPOINT {
    pub dwType: u32,
    pub Anonymous: RASTUNNELENDPOINT_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RASTUNNELENDPOINT {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RASTUNNELENDPOINT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union RASTUNNELENDPOINT_0 {
    pub ipv4: super::super::Networking::WinSock::IN_ADDR,
    pub ipv6: super::super::Networking::WinSock::IN6_ADDR,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RASTUNNELENDPOINT_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RASTUNNELENDPOINT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct RASUPDATECONN {
    pub version: RASAPIVERSION,
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwIfIndex: u32,
    pub localEndPoint: RASTUNNELENDPOINT,
    pub remoteEndPoint: RASTUNNELENDPOINT,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for RASUPDATECONN {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for RASUPDATECONN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_CONNECTION_0 {
    pub hConnection: super::super::Foundation::HANDLE,
    pub hInterface: super::super::Foundation::HANDLE,
    pub dwConnectDuration: u32,
    pub dwInterfaceType: ROUTER_INTERFACE_TYPE,
    pub dwConnectionFlags: RAS_FLAGS,
    pub wszInterfaceName: [u16; 257],
    pub wszUserName: [u16; 257],
    pub wszLogonDomain: [u16; 16],
    pub wszRemoteComputer: [u16; 17],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_CONNECTION_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_CONNECTION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_CONNECTION_1 {
    pub hConnection: super::super::Foundation::HANDLE,
    pub hInterface: super::super::Foundation::HANDLE,
    pub PppInfo: PPP_INFO,
    pub dwBytesXmited: u32,
    pub dwBytesRcved: u32,
    pub dwFramesXmited: u32,
    pub dwFramesRcved: u32,
    pub dwCrcErr: u32,
    pub dwTimeoutErr: u32,
    pub dwAlignmentErr: u32,
    pub dwHardwareOverrunErr: u32,
    pub dwFramingErr: u32,
    pub dwBufferOverrunErr: u32,
    pub dwCompressionRatioIn: u32,
    pub dwCompressionRatioOut: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_CONNECTION_1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_CONNECTION_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_CONNECTION_2 {
    pub hConnection: super::super::Foundation::HANDLE,
    pub wszUserName: [u16; 257],
    pub dwInterfaceType: ROUTER_INTERFACE_TYPE,
    pub guid: ::windows_sys::core::GUID,
    pub PppInfo2: PPP_INFO_2,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_CONNECTION_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_CONNECTION_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_CONNECTION_3 {
    pub dwVersion: u32,
    pub dwSize: u32,
    pub hConnection: super::super::Foundation::HANDLE,
    pub wszUserName: [u16; 257],
    pub dwInterfaceType: ROUTER_INTERFACE_TYPE,
    pub guid: ::windows_sys::core::GUID,
    pub PppInfo3: PPP_INFO_3,
    pub rasQuarState: RAS_QUARANTINE_STATE,
    pub timer: super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_CONNECTION_3 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_CONNECTION_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_CONNECTION_4 {
    pub dwConnectDuration: u32,
    pub dwInterfaceType: ROUTER_INTERFACE_TYPE,
    pub dwConnectionFlags: RAS_FLAGS,
    pub wszInterfaceName: [u16; 257],
    pub wszUserName: [u16; 257],
    pub wszLogonDomain: [u16; 16],
    pub wszRemoteComputer: [u16; 17],
    pub guid: ::windows_sys::core::GUID,
    pub rasQuarState: RAS_QUARANTINE_STATE,
    pub probationTime: super::super::Foundation::FILETIME,
    pub connectionStartTime: super::super::Foundation::FILETIME,
    pub ullBytesXmited: u64,
    pub ullBytesRcved: u64,
    pub dwFramesXmited: u32,
    pub dwFramesRcved: u32,
    pub dwCrcErr: u32,
    pub dwTimeoutErr: u32,
    pub dwAlignmentErr: u32,
    pub dwHardwareOverrunErr: u32,
    pub dwFramingErr: u32,
    pub dwBufferOverrunErr: u32,
    pub dwCompressionRatioIn: u32,
    pub dwCompressionRatioOut: u32,
    pub dwNumSwitchOvers: u32,
    pub wszRemoteEndpointAddress: [u16; 65],
    pub wszLocalEndpointAddress: [u16; 65],
    pub ProjectionInfo: PROJECTION_INFO2,
    pub hConnection: super::super::Foundation::HANDLE,
    pub hInterface: super::super::Foundation::HANDLE,
    pub dwDeviceType: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_CONNECTION_4 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_CONNECTION_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_CONNECTION_EX {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub dwConnectDuration: u32,
    pub dwInterfaceType: ROUTER_INTERFACE_TYPE,
    pub dwConnectionFlags: RAS_FLAGS,
    pub wszInterfaceName: [u16; 257],
    pub wszUserName: [u16; 257],
    pub wszLogonDomain: [u16; 16],
    pub wszRemoteComputer: [u16; 17],
    pub guid: ::windows_sys::core::GUID,
    pub rasQuarState: RAS_QUARANTINE_STATE,
    pub probationTime: super::super::Foundation::FILETIME,
    pub dwBytesXmited: u32,
    pub dwBytesRcved: u32,
    pub dwFramesXmited: u32,
    pub dwFramesRcved: u32,
    pub dwCrcErr: u32,
    pub dwTimeoutErr: u32,
    pub dwAlignmentErr: u32,
    pub dwHardwareOverrunErr: u32,
    pub dwFramingErr: u32,
    pub dwBufferOverrunErr: u32,
    pub dwCompressionRatioIn: u32,
    pub dwCompressionRatioOut: u32,
    pub dwNumSwitchOvers: u32,
    pub wszRemoteEndpointAddress: [u16; 65],
    pub wszLocalEndpointAddress: [u16; 65],
    pub ProjectionInfo: PROJECTION_INFO,
    pub hConnection: super::super::Foundation::HANDLE,
    pub hInterface: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_CONNECTION_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_CONNECTION_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_PORT_0 {
    pub hPort: super::super::Foundation::HANDLE,
    pub hConnection: super::super::Foundation::HANDLE,
    pub dwPortCondition: RAS_PORT_CONDITION,
    pub dwTotalNumberOfCalls: u32,
    pub dwConnectDuration: u32,
    pub wszPortName: [u16; 17],
    pub wszMediaName: [u16; 17],
    pub wszDeviceName: [u16; 129],
    pub wszDeviceType: [u16; 17],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_PORT_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_PORT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_PORT_1 {
    pub hPort: super::super::Foundation::HANDLE,
    pub hConnection: super::super::Foundation::HANDLE,
    pub dwHardwareCondition: RAS_HARDWARE_CONDITION,
    pub dwLineSpeed: u32,
    pub dwBytesXmited: u32,
    pub dwBytesRcved: u32,
    pub dwFramesXmited: u32,
    pub dwFramesRcved: u32,
    pub dwCrcErr: u32,
    pub dwTimeoutErr: u32,
    pub dwAlignmentErr: u32,
    pub dwHardwareOverrunErr: u32,
    pub dwFramingErr: u32,
    pub dwBufferOverrunErr: u32,
    pub dwCompressionRatioIn: u32,
    pub dwCompressionRatioOut: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_PORT_1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_PORT_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RAS_PORT_2 {
    pub hPort: super::super::Foundation::HANDLE,
    pub hConnection: super::super::Foundation::HANDLE,
    pub dwConn_State: u32,
    pub wszPortName: [u16; 17],
    pub wszMediaName: [u16; 17],
    pub wszDeviceName: [u16; 129],
    pub wszDeviceType: [u16; 17],
    pub dwHardwareCondition: RAS_HARDWARE_CONDITION,
    pub dwLineSpeed: u32,
    pub dwCrcErr: u32,
    pub dwSerialOverRunErrs: u32,
    pub dwTimeoutErr: u32,
    pub dwAlignmentErr: u32,
    pub dwHardwareOverrunErr: u32,
    pub dwFramingErr: u32,
    pub dwBufferOverrunErr: u32,
    pub dwCompressionRatioIn: u32,
    pub dwCompressionRatioOut: u32,
    pub dwTotalErrors: u32,
    pub ullBytesXmited: u64,
    pub ullBytesRcved: u64,
    pub ullFramesXmited: u64,
    pub ullFramesRcved: u64,
    pub ullBytesTxUncompressed: u64,
    pub ullBytesTxCompressed: u64,
    pub ullBytesRcvUncompressed: u64,
    pub ullBytesRcvCompressed: u64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RAS_PORT_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RAS_PORT_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct RAS_PROJECTION_INFO {
    pub version: RASAPIVERSION,
    pub r#type: RASPROJECTION_INFO_TYPE,
    pub Anonymous: RAS_PROJECTION_INFO_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for RAS_PROJECTION_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for RAS_PROJECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub union RAS_PROJECTION_INFO_0 {
    pub ppp: RASPPP_PROJECTION_INFO,
    pub ikev2: RASIKEV2_PROJECTION_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for RAS_PROJECTION_INFO_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for RAS_PROJECTION_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAS_SECURITY_INFO {
    pub LastError: u32,
    pub BytesReceived: u32,
    pub DeviceName: [u8; 129],
}
impl ::core::marker::Copy for RAS_SECURITY_INFO {}
impl ::core::clone::Clone for RAS_SECURITY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAS_STATS {
    pub dwSize: u32,
    pub dwBytesXmited: u32,
    pub dwBytesRcved: u32,
    pub dwFramesXmited: u32,
    pub dwFramesRcved: u32,
    pub dwCrcErr: u32,
    pub dwTimeoutErr: u32,
    pub dwAlignmentErr: u32,
    pub dwHardwareOverrunErr: u32,
    pub dwFramingErr: u32,
    pub dwBufferOverrunErr: u32,
    pub dwCompressionRatioIn: u32,
    pub dwCompressionRatioOut: u32,
    pub dwBps: u32,
    pub dwConnectDuration: u32,
}
impl ::core::marker::Copy for RAS_STATS {}
impl ::core::clone::Clone for RAS_STATS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAS_UPDATE_CONNECTION {
    pub Header: MPRAPI_OBJECT_HEADER,
    pub dwIfIndex: u32,
    pub wszLocalEndpointAddress: [u16; 65],
    pub wszRemoteEndpointAddress: [u16; 65],
}
impl ::core::marker::Copy for RAS_UPDATE_CONNECTION {}
impl ::core::clone::Clone for RAS_UPDATE_CONNECTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAS_USER_0 {
    pub bfPrivilege: u8,
    pub wszPhoneNumber: [u16; 129],
}
impl ::core::marker::Copy for RAS_USER_0 {}
impl ::core::clone::Clone for RAS_USER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RAS_USER_1 {
    pub bfPrivilege: u8,
    pub wszPhoneNumber: [u16; 129],
    pub bfPrivilege2: u8,
}
impl ::core::marker::Copy for RAS_USER_1 {}
impl ::core::clone::Clone for RAS_USER_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ROUTER_CUSTOM_IKEv2_POLICY0 {
    pub dwIntegrityMethod: u32,
    pub dwEncryptionMethod: u32,
    pub dwCipherTransformConstant: u32,
    pub dwAuthTransformConstant: u32,
    pub dwPfsGroup: u32,
    pub dwDhGroup: u32,
}
impl ::core::marker::Copy for ROUTER_CUSTOM_IKEv2_POLICY0 {}
impl ::core::clone::Clone for ROUTER_CUSTOM_IKEv2_POLICY0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct ROUTER_IKEv2_IF_CUSTOM_CONFIG0 {
    pub dwSaLifeTime: u32,
    pub dwSaDataSize: u32,
    pub certificateName: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for ROUTER_IKEv2_IF_CUSTOM_CONFIG0 {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for ROUTER_IKEv2_IF_CUSTOM_CONFIG0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security_Cryptography\"`"]
#[cfg(feature = "Win32_Security_Cryptography")]
pub struct ROUTER_IKEv2_IF_CUSTOM_CONFIG1 {
    pub dwSaLifeTime: u32,
    pub dwSaDataSize: u32,
    pub certificateName: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
    pub certificateHash: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::marker::Copy for ROUTER_IKEv2_IF_CUSTOM_CONFIG1 {}
#[cfg(feature = "Win32_Security_Cryptography")]
impl ::core::clone::Clone for ROUTER_IKEv2_IF_CUSTOM_CONFIG1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
pub struct ROUTER_IKEv2_IF_CUSTOM_CONFIG2 {
    pub dwSaLifeTime: u32,
    pub dwSaDataSize: u32,
    pub certificateName: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub customPolicy: *mut ROUTER_CUSTOM_IKEv2_POLICY0,
    pub certificateHash: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
    pub dwMmSaLifeTime: u32,
    pub vpnTrafficSelectors: MPR_VPN_TRAFFIC_SELECTORS,
}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for ROUTER_IKEv2_IF_CUSTOM_CONFIG2 {}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for ROUTER_IKEv2_IF_CUSTOM_CONFIG2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ROUTING_PROTOCOL_CONFIG {
    pub dwCallbackFlags: u32,
    pub pfnRpfCallback: PMGM_RPF_CALLBACK,
    pub pfnCreationAlertCallback: PMGM_CREATION_ALERT_CALLBACK,
    pub pfnPruneAlertCallback: PMGM_PRUNE_ALERT_CALLBACK,
    pub pfnJoinAlertCallback: PMGM_JOIN_ALERT_CALLBACK,
    pub pfnWrongIfCallback: PMGM_WRONG_IF_CALLBACK,
    pub pfnLocalJoinCallback: PMGM_LOCAL_JOIN_CALLBACK,
    pub pfnLocalLeaveCallback: PMGM_LOCAL_LEAVE_CALLBACK,
    pub pfnDisableIgmpCallback: PMGM_DISABLE_IGMP_CALLBACK,
    pub pfnEnableIgmpCallback: PMGM_ENABLE_IGMP_CALLBACK,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ROUTING_PROTOCOL_CONFIG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ROUTING_PROTOCOL_CONFIG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RTM_DEST_INFO {
    pub DestHandle: isize,
    pub DestAddress: RTM_NET_ADDRESS,
    pub LastChanged: super::super::Foundation::FILETIME,
    pub BelongsToViews: u32,
    pub NumberOfViews: u32,
    pub ViewInfo: [RTM_DEST_INFO_0; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RTM_DEST_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RTM_DEST_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RTM_DEST_INFO_0 {
    pub ViewId: i32,
    pub NumRoutes: u32,
    pub Route: isize,
    pub Owner: isize,
    pub DestFlags: u32,
    pub HoldRoute: isize,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RTM_DEST_INFO_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RTM_DEST_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_ENTITY_EXPORT_METHODS {
    pub NumMethods: u32,
    pub Methods: [RTM_ENTITY_EXPORT_METHOD; 1],
}
impl ::core::marker::Copy for RTM_ENTITY_EXPORT_METHODS {}
impl ::core::clone::Clone for RTM_ENTITY_EXPORT_METHODS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_ENTITY_ID {
    pub Anonymous: RTM_ENTITY_ID_0,
}
impl ::core::marker::Copy for RTM_ENTITY_ID {}
impl ::core::clone::Clone for RTM_ENTITY_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union RTM_ENTITY_ID_0 {
    pub Anonymous: RTM_ENTITY_ID_0_0,
    pub EntityId: u64,
}
impl ::core::marker::Copy for RTM_ENTITY_ID_0 {}
impl ::core::clone::Clone for RTM_ENTITY_ID_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_ENTITY_ID_0_0 {
    pub EntityProtocolId: u32,
    pub EntityInstanceId: u32,
}
impl ::core::marker::Copy for RTM_ENTITY_ID_0_0 {}
impl ::core::clone::Clone for RTM_ENTITY_ID_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_ENTITY_INFO {
    pub RtmInstanceId: u16,
    pub AddressFamily: u16,
    pub EntityId: RTM_ENTITY_ID,
}
impl ::core::marker::Copy for RTM_ENTITY_INFO {}
impl ::core::clone::Clone for RTM_ENTITY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_ENTITY_METHOD_INPUT {
    pub MethodType: u32,
    pub InputSize: u32,
    pub InputData: [u8; 1],
}
impl ::core::marker::Copy for RTM_ENTITY_METHOD_INPUT {}
impl ::core::clone::Clone for RTM_ENTITY_METHOD_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_ENTITY_METHOD_OUTPUT {
    pub MethodType: u32,
    pub MethodStatus: u32,
    pub OutputSize: u32,
    pub OutputData: [u8; 1],
}
impl ::core::marker::Copy for RTM_ENTITY_METHOD_OUTPUT {}
impl ::core::clone::Clone for RTM_ENTITY_METHOD_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_NET_ADDRESS {
    pub AddressFamily: u16,
    pub NumBits: u16,
    pub AddrBits: [u8; 16],
}
impl ::core::marker::Copy for RTM_NET_ADDRESS {}
impl ::core::clone::Clone for RTM_NET_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_NEXTHOP_INFO {
    pub NextHopAddress: RTM_NET_ADDRESS,
    pub NextHopOwner: isize,
    pub InterfaceIndex: u32,
    pub State: u16,
    pub Flags: u16,
    pub EntitySpecificInfo: *mut ::core::ffi::c_void,
    pub RemoteNextHop: isize,
}
impl ::core::marker::Copy for RTM_NEXTHOP_INFO {}
impl ::core::clone::Clone for RTM_NEXTHOP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_NEXTHOP_LIST {
    pub NumNextHops: u16,
    pub NextHops: [isize; 1],
}
impl ::core::marker::Copy for RTM_NEXTHOP_LIST {}
impl ::core::clone::Clone for RTM_NEXTHOP_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_PREF_INFO {
    pub Metric: u32,
    pub Preference: u32,
}
impl ::core::marker::Copy for RTM_PREF_INFO {}
impl ::core::clone::Clone for RTM_PREF_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_REGN_PROFILE {
    pub MaxNextHopsInRoute: u32,
    pub MaxHandlesInEnum: u32,
    pub ViewsSupported: u32,
    pub NumberOfViews: u32,
}
impl ::core::marker::Copy for RTM_REGN_PROFILE {}
impl ::core::clone::Clone for RTM_REGN_PROFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RTM_ROUTE_INFO {
    pub DestHandle: isize,
    pub RouteOwner: isize,
    pub Neighbour: isize,
    pub State: u8,
    pub Flags1: u8,
    pub Flags: u16,
    pub PrefInfo: RTM_PREF_INFO,
    pub BelongsToViews: u32,
    pub EntitySpecificInfo: *mut ::core::ffi::c_void,
    pub NextHopsList: RTM_NEXTHOP_LIST,
}
impl ::core::marker::Copy for RTM_ROUTE_INFO {}
impl ::core::clone::Clone for RTM_ROUTE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SECURITY_MESSAGE {
    pub dwMsgId: SECURITY_MESSAGE_MSG_ID,
    pub hPort: isize,
    pub dwError: u32,
    pub UserName: [u8; 257],
    pub Domain: [u8; 16],
}
impl ::core::marker::Copy for SECURITY_MESSAGE {}
impl ::core::clone::Clone for SECURITY_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SOURCE_GROUP_ENTRY {
    pub dwSourceAddr: u32,
    pub dwSourceMask: u32,
    pub dwGroupAddr: u32,
    pub dwGroupMask: u32,
}
impl ::core::marker::Copy for SOURCE_GROUP_ENTRY {}
impl ::core::clone::Clone for SOURCE_GROUP_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct SSTP_CERT_INFO {
    pub isDefault: super::super::Foundation::BOOL,
    pub certBlob: super::super::Security::Cryptography::CRYPT_INTEGER_BLOB,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for SSTP_CERT_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for SSTP_CERT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct SSTP_CONFIG_PARAMS {
    pub dwNumPorts: u32,
    pub dwPortFlags: u32,
    pub isUseHttps: super::super::Foundation::BOOL,
    pub certAlgorithm: u32,
    pub sstpCertDetails: SSTP_CERT_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for SSTP_CONFIG_PARAMS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for SSTP_CONFIG_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct VPN_TS_IP_ADDRESS {
    pub Type: u16,
    pub Anonymous: VPN_TS_IP_ADDRESS_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for VPN_TS_IP_ADDRESS {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for VPN_TS_IP_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union VPN_TS_IP_ADDRESS_0 {
    pub v4: super::super::Networking::WinSock::IN_ADDR,
    pub v6: super::super::Networking::WinSock::IN6_ADDR,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for VPN_TS_IP_ADDRESS_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for VPN_TS_IP_ADDRESS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type ORASADFUNC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: ::windows_sys::core::PCSTR, param2: u32, param3: *mut u32) -> super::super::Foundation::BOOL>;
pub type PFNRASFREEBUFFER = ::core::option::Option<unsafe extern "system" fn(pbufer: *mut u8) -> u32>;
pub type PFNRASGETBUFFER = ::core::option::Option<unsafe extern "system" fn(ppbuffer: *mut *mut u8, pdwsize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNRASRECEIVEBUFFER = ::core::option::Option<unsafe extern "system" fn(hport: super::super::Foundation::HANDLE, pbuffer: *mut u8, pdwsize: *mut u32, dwtimeout: u32, hevent: super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNRASRETRIEVEBUFFER = ::core::option::Option<unsafe extern "system" fn(hport: super::super::Foundation::HANDLE, pbuffer: *mut u8, pdwsize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNRASSENDBUFFER = ::core::option::Option<unsafe extern "system" fn(hport: super::super::Foundation::HANDLE, pbuffer: *mut u8, dwsize: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNRASSETCOMMSETTINGS = ::core::option::Option<unsafe extern "system" fn(hport: super::super::Foundation::HANDLE, prascommsettings: *mut RASCOMMSETTINGS, pvreserved: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMGM_CREATION_ALERT_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwsourceaddr: u32, dwsourcemask: u32, dwgroupaddr: u32, dwgroupmask: u32, dwinifindex: u32, dwinifnexthopaddr: u32, dwifcount: u32, pmieoutiflist: *mut MGM_IF_ENTRY) -> u32>;
pub type PMGM_DISABLE_IGMP_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwifindex: u32, dwifnexthopaddr: u32) -> u32>;
pub type PMGM_ENABLE_IGMP_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwifindex: u32, dwifnexthopaddr: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMGM_JOIN_ALERT_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwsourceaddr: u32, dwsourcemask: u32, dwgroupaddr: u32, dwgroupmask: u32, bmemberupdate: super::super::Foundation::BOOL) -> u32>;
pub type PMGM_LOCAL_JOIN_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwsourceaddr: u32, dwsourcemask: u32, dwgroupaddr: u32, dwgroupmask: u32, dwifindex: u32, dwifnexthopaddr: u32) -> u32>;
pub type PMGM_LOCAL_LEAVE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwsourceaddr: u32, dwsourcemask: u32, dwgroupaddr: u32, dwgroupmask: u32, dwifindex: u32, dwifnexthopaddr: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMGM_PRUNE_ALERT_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwsourceaddr: u32, dwsourcemask: u32, dwgroupaddr: u32, dwgroupmask: u32, dwifindex: u32, dwifnexthopaddr: u32, bmemberdelete: super::super::Foundation::BOOL, pdwtimeout: *mut u32) -> u32>;
pub type PMGM_RPF_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwsourceaddr: u32, dwsourcemask: u32, dwgroupaddr: u32, dwgroupmask: u32, pdwinifindex: *mut u32, pdwinifnexthopaddr: *mut u32, pdwupstreamnbr: *mut u32, dwhdrsize: u32, pbpackethdr: *mut u8, pbroute: *mut u8) -> u32>;
pub type PMGM_WRONG_IF_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwsourceaddr: u32, dwgroupaddr: u32, dwifindex: u32, dwifnexthopaddr: u32, dwhdrsize: u32, pbpackethdr: *mut u8) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTNEWCONNECTION = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_0, param1: *mut RAS_CONNECTION_1) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTNEWCONNECTION2 = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_0, param1: *mut RAS_CONNECTION_1, param2: *mut RAS_CONNECTION_2) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTNEWCONNECTION3 = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_0, param1: *mut RAS_CONNECTION_1, param2: *mut RAS_CONNECTION_2, param3: *mut RAS_CONNECTION_3) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTNEWCONNECTIONEX = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_EX) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTNEWLINK = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_PORT_0, param1: *mut RAS_PORT_1) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTREAUTHENTICATION = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_0, param1: *mut RAS_CONNECTION_1, param2: *mut RAS_CONNECTION_2, param3: *mut RAS_CONNECTION_3) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTREAUTHENTICATIONEX = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_EX) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINACCEPTTUNNELENDPOINTCHANGEEX = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_EX) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINCONNECTIONHANGUPNOTIFICATION = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_0, param1: *mut RAS_CONNECTION_1) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINCONNECTIONHANGUPNOTIFICATION2 = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_0, param1: *mut RAS_CONNECTION_1, param2: *mut RAS_CONNECTION_2) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINCONNECTIONHANGUPNOTIFICATION3 = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_0, param1: *mut RAS_CONNECTION_1, param2: *mut RAS_CONNECTION_2, param3: RAS_CONNECTION_3) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINCONNECTIONHANGUPNOTIFICATIONEX = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_CONNECTION_EX) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINGETIPADDRESSFORUSER = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut u32, param3: *mut super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub type PMPRADMINGETIPV6ADDRESSFORUSER = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut super::super::Networking::WinSock::IN6_ADDR, param3: *mut super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINLINKHANGUPNOTIFICATION = ::core::option::Option<unsafe extern "system" fn(param0: *mut RAS_PORT_0, param1: *mut RAS_PORT_1) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PMPRADMINRASVALIDATEPREAUTHENTICATEDCONNECTIONEX = ::core::option::Option<unsafe extern "system" fn(param0: *mut AUTH_VALIDATION_EX) -> u32>;
pub type PMPRADMINRELEASEIPADRESS = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut u32) -> ()>;
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub type PMPRADMINRELEASEIPV6ADDRESSFORUSER = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut super::super::Networking::WinSock::IN6_ADDR) -> ()>;
pub type PMPRADMINTERMINATEDLL = ::core::option::Option<unsafe extern "system" fn() -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type RASADFUNCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: *mut RASADPARAMS, param3: *mut u32) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type RASADFUNCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut RASADPARAMS, param3: *mut u32) -> super::super::Foundation::BOOL>;
pub type RASDIALFUNC = ::core::option::Option<unsafe extern "system" fn(param0: u32, param1: RASCONNSTATE, param2: u32) -> ()>;
pub type RASDIALFUNC1 = ::core::option::Option<unsafe extern "system" fn(param0: HRASCONN, param1: u32, param2: RASCONNSTATE, param3: u32, param4: u32) -> ()>;
pub type RASDIALFUNC2 = ::core::option::Option<unsafe extern "system" fn(param0: usize, param1: u32, param2: HRASCONN, param3: u32, param4: RASCONNSTATE, param5: u32, param6: u32) -> u32>;
pub type RASPBDLGFUNCA = ::core::option::Option<unsafe extern "system" fn(param0: usize, param1: u32, param2: ::windows_sys::core::PCSTR, param3: *mut ::core::ffi::c_void) -> ()>;
pub type RASPBDLGFUNCW = ::core::option::Option<unsafe extern "system" fn(param0: usize, param1: u32, param2: ::windows_sys::core::PCWSTR, param3: *mut ::core::ffi::c_void) -> ()>;
pub type RASSECURITYPROC = ::core::option::Option<unsafe extern "system" fn() -> u32>;
pub type RTM_ENTITY_EXPORT_METHOD = ::core::option::Option<unsafe extern "system" fn(callerhandle: isize, calleehandle: isize, input: *mut RTM_ENTITY_METHOD_INPUT, output: *mut RTM_ENTITY_METHOD_OUTPUT) -> ()>;
pub type RTM_EVENT_CALLBACK = ::core::option::Option<unsafe extern "system" fn(rtmreghandle: isize, eventtype: RTM_EVENT_TYPE, context1: *mut ::core::ffi::c_void, context2: *mut ::core::ffi::c_void) -> u32>;
pub type RasCustomDeleteEntryNotifyFn = ::core::option::Option<unsafe extern "system" fn(lpszphonebook: ::windows_sys::core::PCWSTR, lpszentry: ::windows_sys::core::PCWSTR, dwflags: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type RasCustomDialDlgFn = ::core::option::Option<unsafe extern "system" fn(hinstdll: super::super::Foundation::HINSTANCE, dwflags: u32, lpszphonebook: ::windows_sys::core::PCWSTR, lpszentry: ::windows_sys::core::PCWSTR, lpszphonenumber: ::windows_sys::core::PCWSTR, lpinfo: *mut RASDIALDLG, pvinfo: *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type RasCustomDialFn = ::core::option::Option<unsafe extern "system" fn(hinstdll: super::super::Foundation::HINSTANCE, lprasdialextensions: *mut RASDIALEXTENSIONS, lpszphonebook: ::windows_sys::core::PCWSTR, lprasdialparams: *mut RASDIALPARAMSA, dwnotifiertype: u32, lpvnotifier: *mut ::core::ffi::c_void, lphrasconn: *mut HRASCONN, dwflags: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type RasCustomEntryDlgFn = ::core::option::Option<unsafe extern "system" fn(hinstdll: super::super::Foundation::HINSTANCE, lpszphonebook: ::windows_sys::core::PCWSTR, lpszentry: ::windows_sys::core::PCWSTR, lpinfo: *mut RASENTRYDLGA, dwflags: u32) -> super::super::Foundation::BOOL>;
pub type RasCustomHangUpFn = ::core::option::Option<unsafe extern "system" fn(hrasconn: HRASCONN) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type RasCustomScriptExecuteFn = ::core::option::Option<unsafe extern "system" fn(hport: super::super::Foundation::HANDLE, lpszphonebook: ::windows_sys::core::PCWSTR, lpszentryname: ::windows_sys::core::PCWSTR, pfnrasgetbuffer: PFNRASGETBUFFER, pfnrasfreebuffer: PFNRASFREEBUFFER, pfnrassendbuffer: PFNRASSENDBUFFER, pfnrasreceivebuffer: PFNRASRECEIVEBUFFER, pfnrasretrievebuffer: PFNRASRETRIEVEBUFFER, hwnd: super::super::Foundation::HWND, prasdialparams: *mut RASDIALPARAMSA, pvreserved: *mut ::core::ffi::c_void) -> u32>;
