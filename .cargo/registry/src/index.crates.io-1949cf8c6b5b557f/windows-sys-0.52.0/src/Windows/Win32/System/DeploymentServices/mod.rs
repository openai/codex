#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeAsyncRecvDone(hclientrequest : super::super::Foundation:: HANDLE, action : u32) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpAppendOption(preplypacket : *mut ::core::ffi::c_void, umaxreplypacketlen : u32, pureplypacketlen : *mut u32, boption : u8, boptionlen : u8, pvalue : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpAppendOptionRaw(preplypacket : *mut ::core::ffi::c_void, umaxreplypacketlen : u32, pureplypacketlen : *mut u32, ubufferlen : u16, pbuffer : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpGetOptionValue(ppacket : *const ::core::ffi::c_void, upacketlen : u32, uinstance : u32, boption : u8, pboptionlen : *mut u8, ppoptionvalue : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpGetVendorOptionValue(ppacket : *const ::core::ffi::c_void, upacketlen : u32, boption : u8, uinstance : u32, pboptionlen : *mut u8, ppoptionvalue : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpInitialize(precvpacket : *const ::core::ffi::c_void, urecvpacketlen : u32, preplypacket : *mut ::core::ffi::c_void, umaxreplypacketlen : u32, pureplypacketlen : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeDhcpIsValid(ppacket : *const ::core::ffi::c_void, upacketlen : u32, brequestpacket : super::super::Foundation:: BOOL, pbpxeoptionpresent : *mut super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpv6AppendOption(preply : *mut ::core::ffi::c_void, cbreply : u32, pcbreplyused : *mut u32, woptiontype : u16, cboption : u16, poption : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpv6AppendOptionRaw(preply : *mut ::core::ffi::c_void, cbreply : u32, pcbreplyused : *mut u32, cbbuffer : u16, pbuffer : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpv6CreateRelayRepl(prelaymessages : *const PXE_DHCPV6_NESTED_RELAY_MESSAGE, nrelaymessages : u32, pinnerpacket : *const u8, cbinnerpacket : u32, preplybuffer : *mut ::core::ffi::c_void, cbreplybuffer : u32, pcbreplybuffer : *mut u32) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpv6GetOptionValue(ppacket : *const ::core::ffi::c_void, upacketlen : u32, uinstance : u32, woption : u16, pwoptionlen : *mut u16, ppoptionvalue : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpv6GetVendorOptionValue(ppacket : *const ::core::ffi::c_void, upacketlen : u32, dwenterprisenumber : u32, woption : u16, uinstance : u32, pwoptionlen : *mut u16, ppoptionvalue : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpv6Initialize(prequest : *const ::core::ffi::c_void, cbrequest : u32, preply : *mut ::core::ffi::c_void, cbreply : u32, pcbreplyused : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeDhcpv6IsValid(ppacket : *const ::core::ffi::c_void, upacketlen : u32, brequestpacket : super::super::Foundation:: BOOL, pbpxeoptionpresent : *mut super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeDhcpv6ParseRelayForw(prelayforwpacket : *const ::core::ffi::c_void, urelayforwpacketlen : u32, prelaymessages : *mut PXE_DHCPV6_NESTED_RELAY_MESSAGE, nrelaymessages : u32, pnrelaymessages : *mut u32, ppinnerpacket : *mut *mut u8, pcbinnerpacket : *mut u32) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeGetServerInfo(uinfotype : u32, pbuffer : *mut ::core::ffi::c_void, ubufferlen : u32) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeGetServerInfoEx(uinfotype : u32, pbuffer : *mut ::core::ffi::c_void, ubufferlen : u32, pubufferused : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxePacketAllocate(hprovider : super::super::Foundation:: HANDLE, hclientrequest : super::super::Foundation:: HANDLE, usize : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxePacketFree(hprovider : super::super::Foundation:: HANDLE, hclientrequest : super::super::Foundation:: HANDLE, ppacket : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeProviderEnumClose(henum : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeProviderEnumFirst(phenum : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeProviderEnumNext(henum : super::super::Foundation:: HANDLE, ppprovider : *mut *mut PXE_PROVIDER) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeProviderFreeInfo(pprovider : *const PXE_PROVIDER) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeProviderQueryIndex(pszprovidername : ::windows_sys::core::PCWSTR, puindex : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn PxeProviderRegister(pszprovidername : ::windows_sys::core::PCWSTR, pszmodulepath : ::windows_sys::core::PCWSTR, index : u32, biscritical : super::super::Foundation:: BOOL, phproviderkey : *mut super::Registry:: HKEY) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeProviderSetAttribute(hprovider : super::super::Foundation:: HANDLE, attribute : u32, pparameterbuffer : *const ::core::ffi::c_void, uparamlen : u32) -> u32);
::windows_targets::link!("wdspxe.dll" "system" fn PxeProviderUnRegister(pszprovidername : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeRegisterCallback(hprovider : super::super::Foundation:: HANDLE, callbacktype : u32, pcallbackfunction : *const ::core::ffi::c_void, pcontext : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeSendReply(hclientrequest : super::super::Foundation:: HANDLE, ppacket : *const ::core::ffi::c_void, upacketlen : u32, paddress : *const PXE_ADDRESS) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeTrace(hprovider : super::super::Foundation:: HANDLE, severity : u32, pszformat : ::windows_sys::core::PCWSTR, ...) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdspxe.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PxeTraceV(hprovider : super::super::Foundation:: HANDLE, severity : u32, pszformat : ::windows_sys::core::PCWSTR, params : *const i8) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsbp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsBpAddOption(hhandle : super::super::Foundation:: HANDLE, uoption : u32, uvaluelen : u32, pvalue : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsbp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsBpCloseHandle(hhandle : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsbp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsBpGetOptionBuffer(hhandle : super::super::Foundation:: HANDLE, ubufferlen : u32, pbuffer : *mut ::core::ffi::c_void, pubytes : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsbp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsBpInitialize(bpackettype : u8, phhandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsbp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsBpParseInitialize(ppacket : *const ::core::ffi::c_void, upacketlen : u32, pbpackettype : *mut u8, phhandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsbp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsBpParseInitializev6(ppacket : *const ::core::ffi::c_void, upacketlen : u32, pbpackettype : *mut u8, phhandle : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsbp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsBpQueryOption(hhandle : super::super::Foundation:: HANDLE, uoption : u32, uvaluelen : u32, pvalue : *mut ::core::ffi::c_void, pubytes : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliAuthorizeSession(hsession : super::super::Foundation:: HANDLE, pcred : *const WDS_CLI_CRED) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliCancelTransfer(htransfer : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliClose(handle : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliCreateSession(pwszserver : ::windows_sys::core::PCWSTR, pcred : *const WDS_CLI_CRED, phsession : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliFindFirstImage(hsession : super::super::Foundation:: HANDLE, phfindhandle : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliFindNextImage(handle : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wdsclientapi.dll" "system" fn WdsCliFreeStringArray(ppwszarray : *mut ::windows_sys::core::PWSTR, ulcount : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wdsclientapi.dll" "system" fn WdsCliGetDriverQueryXml(pwszwindirpath : ::windows_sys::core::PCWSTR, ppwszdriverquery : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetEnumerationFlags(handle : super::super::Foundation:: HANDLE, pdwflags : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageArchitecture(hifh : super::super::Foundation:: HANDLE, pdwvalue : *mut CPU_ARCHITECTURE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageDescription(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageFiles(hifh : super::super::Foundation:: HANDLE, pppwszfiles : *mut *mut ::windows_sys::core::PWSTR, pdwcount : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageGroup(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageHalName(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageHandleFromFindHandle(findhandle : super::super::Foundation:: HANDLE, phimagehandle : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageHandleFromTransferHandle(htransfer : super::super::Foundation:: HANDLE, phimagehandle : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageIndex(hifh : super::super::Foundation:: HANDLE, pdwvalue : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageLanguage(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageLanguages(hifh : super::super::Foundation:: HANDLE, pppszvalues : *mut *mut *mut i8, pdwnumvalues : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageLastModifiedTime(hifh : super::super::Foundation:: HANDLE, ppsystimevalue : *mut *mut super::super::Foundation:: SYSTEMTIME) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageName(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageNamespace(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageParameter(hifh : super::super::Foundation:: HANDLE, paramtype : WDS_CLI_IMAGE_PARAM_TYPE, presponse : *mut ::core::ffi::c_void, uresponselen : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImagePath(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageSize(hifh : super::super::Foundation:: HANDLE, pullvalue : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageType(hifh : super::super::Foundation:: HANDLE, pimagetype : *mut WDS_CLI_IMAGE_TYPE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetImageVersion(hifh : super::super::Foundation:: HANDLE, ppwszvalue : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliGetTransferSize(hifh : super::super::Foundation:: HANDLE, pullvalue : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliInitializeLog(hsession : super::super::Foundation:: HANDLE, ulclientarchitecture : CPU_ARCHITECTURE, pwszclientid : ::windows_sys::core::PCWSTR, pwszclientaddress : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliLog(hsession : super::super::Foundation:: HANDLE, ulloglevel : u32, ulmessagecode : u32, ...) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliObtainDriverPackages(himage : super::super::Foundation:: HANDLE, ppwszservername : *mut ::windows_sys::core::PWSTR, pppwszdriverpackages : *mut *mut ::windows_sys::core::PWSTR, pulcount : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliObtainDriverPackagesEx(hsession : super::super::Foundation:: HANDLE, pwszmachineinfo : ::windows_sys::core::PCWSTR, ppwszservername : *mut ::windows_sys::core::PWSTR, pppwszdriverpackages : *mut *mut ::windows_sys::core::PWSTR, pulcount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wdsclientapi.dll" "system" fn WdsCliRegisterTrace(pfn : PFN_WdsCliTraceFunction) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wdsclientapi.dll" "system" fn WdsCliSetTransferBufferSize(ulsizeinbytes : u32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliTransferFile(pwszserver : ::windows_sys::core::PCWSTR, pwsznamespace : ::windows_sys::core::PCWSTR, pwszremotefilepath : ::windows_sys::core::PCWSTR, pwszlocalfilepath : ::windows_sys::core::PCWSTR, dwflags : u32, dwreserved : u32, pfnwdsclicallback : PFN_WdsCliCallback, pvuserdata : *const ::core::ffi::c_void, phtransfer : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliTransferImage(himage : super::super::Foundation:: HANDLE, pwszlocalpath : ::windows_sys::core::PCWSTR, dwflags : u32, dwreserved : u32, pfnwdsclicallback : PFN_WdsCliCallback, pvuserdata : *const ::core::ffi::c_void, phtransfer : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsclientapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsCliWaitForTransfer(htransfer : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("wdstptc.dll" "system" fn WdsTransportClientAddRefBuffer(pvbuffer : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientCancelSession(hsessionkey : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientCancelSessionEx(hsessionkey : super::super::Foundation:: HANDLE, dwerrorcode : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientCloseSession(hsessionkey : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientCompleteReceive(hsessionkey : super::super::Foundation:: HANDLE, ulsize : u32, pulloffset : *const u64) -> u32);
::windows_targets::link!("wdstptc.dll" "system" fn WdsTransportClientInitialize() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientInitializeSession(psessionrequest : *const WDS_TRANSPORTCLIENT_REQUEST, pcallerdata : *const ::core::ffi::c_void, hsessionkey : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientQueryStatus(hsessionkey : super::super::Foundation:: HANDLE, pustatus : *mut u32, puerrorcode : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientRegisterCallback(hsessionkey : super::super::Foundation:: HANDLE, callbackid : TRANSPORTCLIENT_CALLBACK_ID, pfncallback : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdstptc.dll" "system" fn WdsTransportClientReleaseBuffer(pvbuffer : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wdstptc.dll" "system" fn WdsTransportClientShutdown() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientStartSession(hsessionkey : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdstptc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportClientWaitForCompletion(hsessionkey : super::super::Foundation:: HANDLE, utimeout : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsmc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportServerAllocateBuffer(hprovider : super::super::Foundation:: HANDLE, ulbuffersize : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsmc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportServerCompleteRead(hprovider : super::super::Foundation:: HANDLE, ulbytesread : u32, pvuserdata : *const ::core::ffi::c_void, hreadresult : ::windows_sys::core::HRESULT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsmc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportServerFreeBuffer(hprovider : super::super::Foundation:: HANDLE, pvbuffer : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsmc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportServerRegisterCallback(hprovider : super::super::Foundation:: HANDLE, callbackid : TRANSPORTPROVIDER_CALLBACK_ID, pfncallback : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsmc.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportServerTrace(hprovider : super::super::Foundation:: HANDLE, severity : u32, pwszformat : ::windows_sys::core::PCWSTR, ...) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wdsmc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WdsTransportServerTraceV(hprovider : super::super::Foundation:: HANDLE, severity : u32, pwszformat : ::windows_sys::core::PCWSTR, params : *const i8) -> ::windows_sys::core::HRESULT);
pub type IWdsTransportCacheable = *mut ::core::ffi::c_void;
pub type IWdsTransportClient = *mut ::core::ffi::c_void;
pub type IWdsTransportCollection = *mut ::core::ffi::c_void;
pub type IWdsTransportConfigurationManager = *mut ::core::ffi::c_void;
pub type IWdsTransportConfigurationManager2 = *mut ::core::ffi::c_void;
pub type IWdsTransportContent = *mut ::core::ffi::c_void;
pub type IWdsTransportContentProvider = *mut ::core::ffi::c_void;
pub type IWdsTransportDiagnosticsPolicy = *mut ::core::ffi::c_void;
pub type IWdsTransportManager = *mut ::core::ffi::c_void;
pub type IWdsTransportMulticastSessionPolicy = *mut ::core::ffi::c_void;
pub type IWdsTransportNamespace = *mut ::core::ffi::c_void;
pub type IWdsTransportNamespaceAutoCast = *mut ::core::ffi::c_void;
pub type IWdsTransportNamespaceManager = *mut ::core::ffi::c_void;
pub type IWdsTransportNamespaceScheduledCast = *mut ::core::ffi::c_void;
pub type IWdsTransportNamespaceScheduledCastAutoStart = *mut ::core::ffi::c_void;
pub type IWdsTransportNamespaceScheduledCastManualStart = *mut ::core::ffi::c_void;
pub type IWdsTransportServer = *mut ::core::ffi::c_void;
pub type IWdsTransportServer2 = *mut ::core::ffi::c_void;
pub type IWdsTransportServicePolicy = *mut ::core::ffi::c_void;
pub type IWdsTransportServicePolicy2 = *mut ::core::ffi::c_void;
pub type IWdsTransportSession = *mut ::core::ffi::c_void;
pub type IWdsTransportSetupManager = *mut ::core::ffi::c_void;
pub type IWdsTransportSetupManager2 = *mut ::core::ffi::c_void;
pub type IWdsTransportTftpClient = *mut ::core::ffi::c_void;
pub type IWdsTransportTftpManager = *mut ::core::ffi::c_void;
pub const CPU_ARCHITECTURE_AMD64: CPU_ARCHITECTURE = 9u32;
pub const CPU_ARCHITECTURE_IA64: CPU_ARCHITECTURE = 6u32;
pub const CPU_ARCHITECTURE_INTEL: CPU_ARCHITECTURE = 0u32;
pub const EVT_WDSMCS_E_CP_CALLBACKS_NOT_REG: ::windows_sys::core::HRESULT = -1054801324i32;
pub const EVT_WDSMCS_E_CP_CLOSE_INSTANCE_FAILED: ::windows_sys::core::HRESULT = -1054801320i32;
pub const EVT_WDSMCS_E_CP_DLL_LOAD_FAILED: ::windows_sys::core::HRESULT = -1054801328i32;
pub const EVT_WDSMCS_E_CP_DLL_LOAD_FAILED_CRITICAL: ::windows_sys::core::HRESULT = -1054801317i32;
pub const EVT_WDSMCS_E_CP_INCOMPATIBLE_SERVER_VERSION: ::windows_sys::core::HRESULT = -1054801325i32;
pub const EVT_WDSMCS_E_CP_INIT_FUNC_FAILED: ::windows_sys::core::HRESULT = -1054801326i32;
pub const EVT_WDSMCS_E_CP_INIT_FUNC_MISSING: ::windows_sys::core::HRESULT = -1054801327i32;
pub const EVT_WDSMCS_E_CP_MEMORY_LEAK: ::windows_sys::core::HRESULT = -1054801322i32;
pub const EVT_WDSMCS_E_CP_OPEN_CONTENT_FAILED: ::windows_sys::core::HRESULT = -1054801319i32;
pub const EVT_WDSMCS_E_CP_OPEN_INSTANCE_FAILED: ::windows_sys::core::HRESULT = -1054801321i32;
pub const EVT_WDSMCS_E_CP_SHUTDOWN_FUNC_FAILED: ::windows_sys::core::HRESULT = -1054801323i32;
pub const EVT_WDSMCS_E_DUPLICATE_MULTICAST_ADDR: ::windows_sys::core::HRESULT = -1054801406i32;
pub const EVT_WDSMCS_E_NON_WDS_DUPLICATE_MULTICAST_ADDR: ::windows_sys::core::HRESULT = -1054801405i32;
pub const EVT_WDSMCS_E_NSREG_CONTENT_PROVIDER_NOT_REG: ::windows_sys::core::HRESULT = -1054801151i32;
pub const EVT_WDSMCS_E_NSREG_FAILURE: ::windows_sys::core::HRESULT = -1054801149i32;
pub const EVT_WDSMCS_E_NSREG_NAMESPACE_EXISTS: ::windows_sys::core::HRESULT = -1054801150i32;
pub const EVT_WDSMCS_E_NSREG_START_TIME_IN_PAST: ::windows_sys::core::HRESULT = -1054801152i32;
pub const EVT_WDSMCS_E_PARAMETERS_READ_FAILED: ::windows_sys::core::HRESULT = -1054801407i32;
pub const EVT_WDSMCS_S_PARAMETERS_READ: ::windows_sys::core::HRESULT = 1092682240i32;
pub const EVT_WDSMCS_W_CP_DLL_LOAD_FAILED_NOT_CRITICAL: ::windows_sys::core::HRESULT = -2128543142i32;
pub const FACILITY_WDSMCCLIENT: u32 = 290u32;
pub const FACILITY_WDSMCSERVER: u32 = 289u32;
pub const FACILITY_WDSTPTMGMT: u32 = 272u32;
pub const MC_SERVER_CURRENT_VERSION: u32 = 1u32;
pub const PXE_ADDR_BROADCAST: u32 = 1u32;
pub const PXE_ADDR_USE_ADDR: u32 = 4u32;
pub const PXE_ADDR_USE_DHCP_RULES: u32 = 8u32;
pub const PXE_ADDR_USE_PORT: u32 = 2u32;
pub const PXE_BA_CUSTOM: u32 = 2u32;
pub const PXE_BA_IGNORE: u32 = 3u32;
pub const PXE_BA_NBP: u32 = 1u32;
pub const PXE_BA_REJECTED: u32 = 4u32;
pub const PXE_CALLBACK_MAX: u32 = 3u32;
pub const PXE_CALLBACK_RECV_REQUEST: u32 = 0u32;
pub const PXE_CALLBACK_SERVICE_CONTROL: u32 = 2u32;
pub const PXE_CALLBACK_SHUTDOWN: u32 = 1u32;
pub const PXE_DHCPV6_CLIENT_PORT: u32 = 546u32;
pub const PXE_DHCPV6_RELAY_HOP_COUNT_LIMIT: u32 = 32u32;
pub const PXE_DHCPV6_SERVER_PORT: u32 = 547u32;
pub const PXE_DHCP_CLIENT_PORT: u32 = 68u32;
pub const PXE_DHCP_FILE_SIZE: u32 = 128u32;
pub const PXE_DHCP_HWAADR_SIZE: u32 = 16u32;
pub const PXE_DHCP_MAGIC_COOKIE_SIZE: u32 = 4u32;
pub const PXE_DHCP_SERVER_PORT: u32 = 67u32;
pub const PXE_DHCP_SERVER_SIZE: u32 = 64u32;
pub const PXE_GSI_SERVER_DUID: u32 = 2u32;
pub const PXE_GSI_TRACE_ENABLED: u32 = 1u32;
pub const PXE_MAX_ADDRESS: u32 = 16u32;
pub const PXE_PROV_ATTR_FILTER: u32 = 0u32;
pub const PXE_PROV_ATTR_FILTER_IPV6: u32 = 1u32;
pub const PXE_PROV_ATTR_IPV6_CAPABLE: u32 = 2u32;
pub const PXE_PROV_FILTER_ALL: u32 = 0u32;
pub const PXE_PROV_FILTER_DHCP_ONLY: u32 = 1u32;
pub const PXE_PROV_FILTER_PXE_ONLY: u32 = 2u32;
pub const PXE_REG_INDEX_BOTTOM: u32 = 4294967295u32;
pub const PXE_REG_INDEX_TOP: u32 = 0u32;
pub const PXE_SERVER_PORT: u32 = 4011u32;
pub const PXE_TRACE_ERROR: u32 = 524288u32;
pub const PXE_TRACE_FATAL: u32 = 1048576u32;
pub const PXE_TRACE_INFO: u32 = 131072u32;
pub const PXE_TRACE_VERBOSE: u32 = 65536u32;
pub const PXE_TRACE_WARNING: u32 = 262144u32;
pub const TRANSPORTPROVIDER_CURRENT_VERSION: u32 = 1u32;
pub const WDSBP_OPTVAL_ACTION_ABORT: u32 = 5u32;
pub const WDSBP_OPTVAL_ACTION_APPROVAL: u32 = 1u32;
pub const WDSBP_OPTVAL_ACTION_REFERRAL: u32 = 3u32;
pub const WDSBP_OPTVAL_NBP_VER_7: u32 = 1792u32;
pub const WDSBP_OPTVAL_NBP_VER_8: u32 = 2048u32;
pub const WDSBP_OPTVAL_PXE_PROMPT_NOPROMPT: u32 = 2u32;
pub const WDSBP_OPTVAL_PXE_PROMPT_OPTIN: u32 = 1u32;
pub const WDSBP_OPTVAL_PXE_PROMPT_OPTOUT: u32 = 3u32;
pub const WDSBP_OPT_TYPE_BYTE: u32 = 1u32;
pub const WDSBP_OPT_TYPE_IP4: u32 = 6u32;
pub const WDSBP_OPT_TYPE_IP6: u32 = 7u32;
pub const WDSBP_OPT_TYPE_NONE: u32 = 0u32;
pub const WDSBP_OPT_TYPE_STR: u32 = 5u32;
pub const WDSBP_OPT_TYPE_ULONG: u32 = 3u32;
pub const WDSBP_OPT_TYPE_USHORT: u32 = 2u32;
pub const WDSBP_OPT_TYPE_WSTR: u32 = 4u32;
pub const WDSBP_PK_TYPE_BCD: u32 = 4u32;
pub const WDSBP_PK_TYPE_DHCP: u32 = 1u32;
pub const WDSBP_PK_TYPE_DHCPV6: u32 = 8u32;
pub const WDSBP_PK_TYPE_WDSNBP: u32 = 2u32;
pub const WDSMCCLIENT_CATEGORY: ::windows_sys::core::HRESULT = 2i32;
pub const WDSMCSERVER_CATEGORY: ::windows_sys::core::HRESULT = 1i32;
pub const WDSMCS_E_CLIENT_DOESNOT_SUPPORT_SECURITY_MODE: ::windows_sys::core::HRESULT = -1054801648i32;
pub const WDSMCS_E_CLIENT_NOT_FOUND: ::windows_sys::core::HRESULT = -1054801660i32;
pub const WDSMCS_E_CONTENT_NOT_FOUND: ::windows_sys::core::HRESULT = -1054801661i32;
pub const WDSMCS_E_CONTENT_PROVIDER_NOT_FOUND: ::windows_sys::core::HRESULT = -1054801658i32;
pub const WDSMCS_E_INCOMPATIBLE_VERSION: ::windows_sys::core::HRESULT = -1054801662i32;
pub const WDSMCS_E_NAMESPACE_ALREADY_EXISTS: ::windows_sys::core::HRESULT = -1054801657i32;
pub const WDSMCS_E_NAMESPACE_ALREADY_STARTED: ::windows_sys::core::HRESULT = -1054801655i32;
pub const WDSMCS_E_NAMESPACE_NOT_FOUND: ::windows_sys::core::HRESULT = -1054801659i32;
pub const WDSMCS_E_NAMESPACE_SHUTDOWN_IN_PROGRESS: ::windows_sys::core::HRESULT = -1054801656i32;
pub const WDSMCS_E_NS_START_FAILED_NO_CLIENTS: ::windows_sys::core::HRESULT = -1054801654i32;
pub const WDSMCS_E_PACKET_HAS_SECURITY: ::windows_sys::core::HRESULT = -1054801650i32;
pub const WDSMCS_E_PACKET_NOT_CHECKSUMED: ::windows_sys::core::HRESULT = -1054801649i32;
pub const WDSMCS_E_PACKET_NOT_HASHED: ::windows_sys::core::HRESULT = -1054801652i32;
pub const WDSMCS_E_PACKET_NOT_SIGNED: ::windows_sys::core::HRESULT = -1054801651i32;
pub const WDSMCS_E_REQCALLBACKS_NOT_REG: ::windows_sys::core::HRESULT = -1054801663i32;
pub const WDSMCS_E_SESSION_SHUTDOWN_IN_PROGRESS: ::windows_sys::core::HRESULT = -1054801664i32;
pub const WDSMCS_E_START_TIME_IN_PAST: ::windows_sys::core::HRESULT = -1054801653i32;
pub const WDSTPC_E_ALREADY_COMPLETED: ::windows_sys::core::HRESULT = -1054735615i32;
pub const WDSTPC_E_ALREADY_IN_LOWEST_SESSION: ::windows_sys::core::HRESULT = -1054735606i32;
pub const WDSTPC_E_ALREADY_IN_PROGRESS: ::windows_sys::core::HRESULT = -1054735614i32;
pub const WDSTPC_E_CALLBACKS_NOT_REG: ::windows_sys::core::HRESULT = -1054735616i32;
pub const WDSTPC_E_CLIENT_DEMOTE_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1054735605i32;
pub const WDSTPC_E_KICKED_FAIL: ::windows_sys::core::HRESULT = -1054735609i32;
pub const WDSTPC_E_KICKED_FALLBACK: ::windows_sys::core::HRESULT = -1054735610i32;
pub const WDSTPC_E_KICKED_POLICY_NOT_MET: ::windows_sys::core::HRESULT = -1054735611i32;
pub const WDSTPC_E_KICKED_UNKNOWN: ::windows_sys::core::HRESULT = -1054735608i32;
pub const WDSTPC_E_MULTISTREAM_NOT_ENABLED: ::windows_sys::core::HRESULT = -1054735607i32;
pub const WDSTPC_E_NOT_INITIALIZED: ::windows_sys::core::HRESULT = -1054735612i32;
pub const WDSTPC_E_NO_IP4_INTERFACE: ::windows_sys::core::HRESULT = -1054735604i32;
pub const WDSTPC_E_UNKNOWN_ERROR: ::windows_sys::core::HRESULT = -1054735613i32;
pub const WDSTPTC_E_WIM_APPLY_REQUIRES_REFERENCE_IMAGE: ::windows_sys::core::HRESULT = -1054735603i32;
pub const WDSTPTMGMT_CATEGORY: ::windows_sys::core::HRESULT = 1i32;
pub const WDSTPTMGMT_E_CANNOT_REFRESH_DIRTY_OBJECT: ::windows_sys::core::HRESULT = -1055915761i32;
pub const WDSTPTMGMT_E_CANNOT_REINITIALIZE_OBJECT: ::windows_sys::core::HRESULT = -1055915767i32;
pub const WDSTPTMGMT_E_CONTENT_PROVIDER_ALREADY_REGISTERED: ::windows_sys::core::HRESULT = -1055915773i32;
pub const WDSTPTMGMT_E_CONTENT_PROVIDER_NOT_REGISTERED: ::windows_sys::core::HRESULT = -1055915772i32;
pub const WDSTPTMGMT_E_INVALID_AUTO_DISCONNECT_THRESHOLD: ::windows_sys::core::HRESULT = -1055915748i32;
pub const WDSTPTMGMT_E_INVALID_CLASS: ::windows_sys::core::HRESULT = -1055915774i32;
pub const WDSTPTMGMT_E_INVALID_CONTENT_PROVIDER_NAME: ::windows_sys::core::HRESULT = -1055915771i32;
pub const WDSTPTMGMT_E_INVALID_DIAGNOSTICS_COMPONENTS: ::windows_sys::core::HRESULT = -1055915762i32;
pub const WDSTPTMGMT_E_INVALID_IPV4_MULTICAST_ADDRESS: ::windows_sys::core::HRESULT = -1055915753i32;
pub const WDSTPTMGMT_E_INVALID_IPV6_MULTICAST_ADDRESS: ::windows_sys::core::HRESULT = -1055915752i32;
pub const WDSTPTMGMT_E_INVALID_IPV6_MULTICAST_ADDRESS_SOURCE: ::windows_sys::core::HRESULT = -1055915750i32;
pub const WDSTPTMGMT_E_INVALID_IP_ADDRESS: ::windows_sys::core::HRESULT = -1055915754i32;
pub const WDSTPTMGMT_E_INVALID_MULTISTREAM_STREAM_COUNT: ::windows_sys::core::HRESULT = -1055915749i32;
pub const WDSTPTMGMT_E_INVALID_NAMESPACE_DATA: ::windows_sys::core::HRESULT = -1055915765i32;
pub const WDSTPTMGMT_E_INVALID_NAMESPACE_NAME: ::windows_sys::core::HRESULT = -1055915766i32;
pub const WDSTPTMGMT_E_INVALID_NAMESPACE_START_PARAMETERS: ::windows_sys::core::HRESULT = -1055915758i32;
pub const WDSTPTMGMT_E_INVALID_NAMESPACE_START_TIME: ::windows_sys::core::HRESULT = -1055915763i32;
pub const WDSTPTMGMT_E_INVALID_OPERATION: ::windows_sys::core::HRESULT = -1055915775i32;
pub const WDSTPTMGMT_E_INVALID_PROPERTY: ::windows_sys::core::HRESULT = -1055915776i32;
pub const WDSTPTMGMT_E_INVALID_SERVICE_IP_ADDRESS_RANGE: ::windows_sys::core::HRESULT = -1055915760i32;
pub const WDSTPTMGMT_E_INVALID_SERVICE_PORT_RANGE: ::windows_sys::core::HRESULT = -1055915759i32;
pub const WDSTPTMGMT_E_INVALID_SLOW_CLIENT_HANDLING_TYPE: ::windows_sys::core::HRESULT = -1055915746i32;
pub const WDSTPTMGMT_E_INVALID_TFTP_MAX_BLOCKSIZE: ::windows_sys::core::HRESULT = -1055915741i32;
pub const WDSTPTMGMT_E_IPV6_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1055915751i32;
pub const WDSTPTMGMT_E_MULTICAST_SESSION_POLICY_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1055915747i32;
pub const WDSTPTMGMT_E_NAMESPACE_ALREADY_REGISTERED: ::windows_sys::core::HRESULT = -1055915769i32;
pub const WDSTPTMGMT_E_NAMESPACE_NOT_ON_SERVER: ::windows_sys::core::HRESULT = -1055915756i32;
pub const WDSTPTMGMT_E_NAMESPACE_NOT_REGISTERED: ::windows_sys::core::HRESULT = -1055915768i32;
pub const WDSTPTMGMT_E_NAMESPACE_READ_ONLY: ::windows_sys::core::HRESULT = -1055915764i32;
pub const WDSTPTMGMT_E_NAMESPACE_REMOVED_FROM_SERVER: ::windows_sys::core::HRESULT = -1055915755i32;
pub const WDSTPTMGMT_E_NETWORK_PROFILES_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1055915745i32;
pub const WDSTPTMGMT_E_TFTP_MAX_BLOCKSIZE_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1055915743i32;
pub const WDSTPTMGMT_E_TFTP_VAR_WINDOW_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1055915742i32;
pub const WDSTPTMGMT_E_TRANSPORT_SERVER_ROLE_NOT_CONFIGURED: ::windows_sys::core::HRESULT = -1055915770i32;
pub const WDSTPTMGMT_E_TRANSPORT_SERVER_UNAVAILABLE: ::windows_sys::core::HRESULT = -1055915757i32;
pub const WDSTPTMGMT_E_UDP_PORT_POLICY_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1055915744i32;
pub const WDSTRANSPORT_RESOURCE_UTILIZATION_UNKNOWN: u32 = 255u32;
pub const WDS_CLI_FIRMWARE_BIOS: WDS_CLI_FIRMWARE_TYPE = 1i32;
pub const WDS_CLI_FIRMWARE_EFI: WDS_CLI_FIRMWARE_TYPE = 2i32;
pub const WDS_CLI_FIRMWARE_UNKNOWN: WDS_CLI_FIRMWARE_TYPE = 0i32;
pub const WDS_CLI_IMAGE_PARAM_SPARSE_FILE: WDS_CLI_IMAGE_PARAM_TYPE = 1i32;
pub const WDS_CLI_IMAGE_PARAM_SUPPORTED_FIRMWARES: WDS_CLI_IMAGE_PARAM_TYPE = 2i32;
pub const WDS_CLI_IMAGE_PARAM_UNKNOWN: WDS_CLI_IMAGE_PARAM_TYPE = 0i32;
pub const WDS_CLI_IMAGE_TYPE_UNKNOWN: WDS_CLI_IMAGE_TYPE = 0i32;
pub const WDS_CLI_IMAGE_TYPE_VHD: WDS_CLI_IMAGE_TYPE = 2i32;
pub const WDS_CLI_IMAGE_TYPE_VHDX: WDS_CLI_IMAGE_TYPE = 3i32;
pub const WDS_CLI_IMAGE_TYPE_WIM: WDS_CLI_IMAGE_TYPE = 1i32;
pub const WDS_CLI_MSG_COMPLETE: PFN_WDS_CLI_CALLBACK_MESSAGE_ID = 1u32;
pub const WDS_CLI_MSG_PROGRESS: PFN_WDS_CLI_CALLBACK_MESSAGE_ID = 2u32;
pub const WDS_CLI_MSG_START: PFN_WDS_CLI_CALLBACK_MESSAGE_ID = 0u32;
pub const WDS_CLI_MSG_TEXT: PFN_WDS_CLI_CALLBACK_MESSAGE_ID = 3u32;
pub const WDS_CLI_NO_SPARSE_FILE: u32 = 2u32;
pub const WDS_CLI_TRANSFER_ASYNCHRONOUS: u32 = 1u32;
pub const WDS_LOG_LEVEL_DISABLED: i32 = 0i32;
pub const WDS_LOG_LEVEL_ERROR: i32 = 1i32;
pub const WDS_LOG_LEVEL_INFO: i32 = 3i32;
pub const WDS_LOG_LEVEL_WARNING: i32 = 2i32;
pub const WDS_LOG_TYPE_CLIENT_APPLY_FINISHED: i32 = 6i32;
pub const WDS_LOG_TYPE_CLIENT_APPLY_FINISHED_2: i32 = 16i32;
pub const WDS_LOG_TYPE_CLIENT_APPLY_STARTED: i32 = 5i32;
pub const WDS_LOG_TYPE_CLIENT_APPLY_STARTED_2: i32 = 15i32;
pub const WDS_LOG_TYPE_CLIENT_DOMAINJOINERROR: i32 = 12i32;
pub const WDS_LOG_TYPE_CLIENT_DOMAINJOINERROR_2: i32 = 17i32;
pub const WDS_LOG_TYPE_CLIENT_DRIVER_PACKAGE_NOT_ACCESSIBLE: i32 = 18i32;
pub const WDS_LOG_TYPE_CLIENT_ERROR: i32 = 1i32;
pub const WDS_LOG_TYPE_CLIENT_FINISHED: i32 = 3i32;
pub const WDS_LOG_TYPE_CLIENT_GENERIC_MESSAGE: i32 = 7i32;
pub const WDS_LOG_TYPE_CLIENT_IMAGE_SELECTED: i32 = 4i32;
pub const WDS_LOG_TYPE_CLIENT_IMAGE_SELECTED2: i32 = 22i32;
pub const WDS_LOG_TYPE_CLIENT_IMAGE_SELECTED3: i32 = 23i32;
pub const WDS_LOG_TYPE_CLIENT_MAX_CODE: i32 = 24i32;
pub const WDS_LOG_TYPE_CLIENT_OFFLINE_DRIVER_INJECTION_END: i32 = 20i32;
pub const WDS_LOG_TYPE_CLIENT_OFFLINE_DRIVER_INJECTION_FAILURE: i32 = 21i32;
pub const WDS_LOG_TYPE_CLIENT_OFFLINE_DRIVER_INJECTION_START: i32 = 19i32;
pub const WDS_LOG_TYPE_CLIENT_POST_ACTIONS_END: i32 = 14i32;
pub const WDS_LOG_TYPE_CLIENT_POST_ACTIONS_START: i32 = 13i32;
pub const WDS_LOG_TYPE_CLIENT_STARTED: i32 = 2i32;
pub const WDS_LOG_TYPE_CLIENT_TRANSFER_DOWNGRADE: i32 = 11i32;
pub const WDS_LOG_TYPE_CLIENT_TRANSFER_END: i32 = 10i32;
pub const WDS_LOG_TYPE_CLIENT_TRANSFER_START: i32 = 9i32;
pub const WDS_LOG_TYPE_CLIENT_UNATTEND_MODE: i32 = 8i32;
pub const WDS_MC_TRACE_ERROR: u32 = 524288u32;
pub const WDS_MC_TRACE_FATAL: u32 = 1048576u32;
pub const WDS_MC_TRACE_INFO: u32 = 131072u32;
pub const WDS_MC_TRACE_VERBOSE: u32 = 65536u32;
pub const WDS_MC_TRACE_WARNING: u32 = 262144u32;
pub const WDS_TRANSPORTCLIENT_AUTH: WDS_TRANSPORTCLIENT_REQUEST_AUTH_LEVEL = 1u32;
pub const WDS_TRANSPORTCLIENT_CURRENT_API_VERSION: u32 = 1u32;
pub const WDS_TRANSPORTCLIENT_MAX_CALLBACKS: TRANSPORTCLIENT_CALLBACK_ID = 6i32;
pub const WDS_TRANSPORTCLIENT_NO_AUTH: WDS_TRANSPORTCLIENT_REQUEST_AUTH_LEVEL = 2u32;
pub const WDS_TRANSPORTCLIENT_NO_CACHE: u32 = 0u32;
pub const WDS_TRANSPORTCLIENT_PROTOCOL_MULTICAST: u32 = 1u32;
pub const WDS_TRANSPORTCLIENT_RECEIVE_CONTENTS: TRANSPORTCLIENT_CALLBACK_ID = 1i32;
pub const WDS_TRANSPORTCLIENT_RECEIVE_METADATA: TRANSPORTCLIENT_CALLBACK_ID = 3i32;
pub const WDS_TRANSPORTCLIENT_SESSION_COMPLETE: TRANSPORTCLIENT_CALLBACK_ID = 2i32;
pub const WDS_TRANSPORTCLIENT_SESSION_NEGOTIATE: TRANSPORTCLIENT_CALLBACK_ID = 5i32;
pub const WDS_TRANSPORTCLIENT_SESSION_START: TRANSPORTCLIENT_CALLBACK_ID = 0i32;
pub const WDS_TRANSPORTCLIENT_SESSION_STARTEX: TRANSPORTCLIENT_CALLBACK_ID = 4i32;
pub const WDS_TRANSPORTCLIENT_STATUS_FAILURE: u32 = 3u32;
pub const WDS_TRANSPORTCLIENT_STATUS_IN_PROGRESS: u32 = 1u32;
pub const WDS_TRANSPORTCLIENT_STATUS_SUCCESS: u32 = 2u32;
pub const WDS_TRANSPORTPROVIDER_CLOSE_CONTENT: TRANSPORTPROVIDER_CALLBACK_ID = 6i32;
pub const WDS_TRANSPORTPROVIDER_CLOSE_INSTANCE: TRANSPORTPROVIDER_CALLBACK_ID = 7i32;
pub const WDS_TRANSPORTPROVIDER_COMPARE_CONTENT: TRANSPORTPROVIDER_CALLBACK_ID = 1i32;
pub const WDS_TRANSPORTPROVIDER_CREATE_INSTANCE: TRANSPORTPROVIDER_CALLBACK_ID = 0i32;
pub const WDS_TRANSPORTPROVIDER_DUMP_STATE: TRANSPORTPROVIDER_CALLBACK_ID = 9i32;
pub const WDS_TRANSPORTPROVIDER_GET_CONTENT_METADATA: TRANSPORTPROVIDER_CALLBACK_ID = 11i32;
pub const WDS_TRANSPORTPROVIDER_GET_CONTENT_SIZE: TRANSPORTPROVIDER_CALLBACK_ID = 4i32;
pub const WDS_TRANSPORTPROVIDER_MAX_CALLBACKS: TRANSPORTPROVIDER_CALLBACK_ID = 12i32;
pub const WDS_TRANSPORTPROVIDER_OPEN_CONTENT: TRANSPORTPROVIDER_CALLBACK_ID = 2i32;
pub const WDS_TRANSPORTPROVIDER_READ_CONTENT: TRANSPORTPROVIDER_CALLBACK_ID = 5i32;
pub const WDS_TRANSPORTPROVIDER_REFRESH_SETTINGS: TRANSPORTPROVIDER_CALLBACK_ID = 10i32;
pub const WDS_TRANSPORTPROVIDER_SHUTDOWN: TRANSPORTPROVIDER_CALLBACK_ID = 8i32;
pub const WDS_TRANSPORTPROVIDER_USER_ACCESS_CHECK: TRANSPORTPROVIDER_CALLBACK_ID = 3i32;
pub const WdsCliFlagEnumFilterFirmware: i32 = 2i32;
pub const WdsCliFlagEnumFilterVersion: i32 = 1i32;
pub const WdsTptDiagnosticsComponentImageServer: WDSTRANSPORT_DIAGNOSTICS_COMPONENT_FLAGS = 4i32;
pub const WdsTptDiagnosticsComponentMulticast: WDSTRANSPORT_DIAGNOSTICS_COMPONENT_FLAGS = 8i32;
pub const WdsTptDiagnosticsComponentPxe: WDSTRANSPORT_DIAGNOSTICS_COMPONENT_FLAGS = 1i32;
pub const WdsTptDiagnosticsComponentTftp: WDSTRANSPORT_DIAGNOSTICS_COMPONENT_FLAGS = 2i32;
pub const WdsTptDisconnectAbort: WDSTRANSPORT_DISCONNECT_TYPE = 2i32;
pub const WdsTptDisconnectFallback: WDSTRANSPORT_DISCONNECT_TYPE = 1i32;
pub const WdsTptDisconnectUnknown: WDSTRANSPORT_DISCONNECT_TYPE = 0i32;
pub const WdsTptFeatureAdminPack: WDSTRANSPORT_FEATURE_FLAGS = 1i32;
pub const WdsTptFeatureDeploymentServer: WDSTRANSPORT_FEATURE_FLAGS = 4i32;
pub const WdsTptFeatureTransportServer: WDSTRANSPORT_FEATURE_FLAGS = 2i32;
pub const WdsTptIpAddressIpv4: WDSTRANSPORT_IP_ADDRESS_TYPE = 1i32;
pub const WdsTptIpAddressIpv6: WDSTRANSPORT_IP_ADDRESS_TYPE = 2i32;
pub const WdsTptIpAddressSourceDhcp: WDSTRANSPORT_IP_ADDRESS_SOURCE_TYPE = 1i32;
pub const WdsTptIpAddressSourceRange: WDSTRANSPORT_IP_ADDRESS_SOURCE_TYPE = 2i32;
pub const WdsTptIpAddressSourceUnknown: WDSTRANSPORT_IP_ADDRESS_SOURCE_TYPE = 0i32;
pub const WdsTptIpAddressUnknown: WDSTRANSPORT_IP_ADDRESS_TYPE = 0i32;
pub const WdsTptNamespaceTypeAutoCast: WDSTRANSPORT_NAMESPACE_TYPE = 1i32;
pub const WdsTptNamespaceTypeScheduledCastAutoStart: WDSTRANSPORT_NAMESPACE_TYPE = 3i32;
pub const WdsTptNamespaceTypeScheduledCastManualStart: WDSTRANSPORT_NAMESPACE_TYPE = 2i32;
pub const WdsTptNamespaceTypeUnknown: WDSTRANSPORT_NAMESPACE_TYPE = 0i32;
pub const WdsTptNetworkProfile100Mbps: WDSTRANSPORT_NETWORK_PROFILE_TYPE = 3i32;
pub const WdsTptNetworkProfile10Mbps: WDSTRANSPORT_NETWORK_PROFILE_TYPE = 2i32;
pub const WdsTptNetworkProfile1Gbps: WDSTRANSPORT_NETWORK_PROFILE_TYPE = 4i32;
pub const WdsTptNetworkProfileCustom: WDSTRANSPORT_NETWORK_PROFILE_TYPE = 1i32;
pub const WdsTptNetworkProfileUnknown: WDSTRANSPORT_NETWORK_PROFILE_TYPE = 0i32;
pub const WdsTptProtocolMulticast: WDSTRANSPORT_PROTOCOL_FLAGS = 2i32;
pub const WdsTptProtocolUnicast: WDSTRANSPORT_PROTOCOL_FLAGS = 1i32;
pub const WdsTptServiceNotifyReadSettings: WDSTRANSPORT_SERVICE_NOTIFICATION = 1i32;
pub const WdsTptServiceNotifyUnknown: WDSTRANSPORT_SERVICE_NOTIFICATION = 0i32;
pub const WdsTptSlowClientHandlingAutoDisconnect: WDSTRANSPORT_SLOW_CLIENT_HANDLING_TYPE = 2i32;
pub const WdsTptSlowClientHandlingMultistream: WDSTRANSPORT_SLOW_CLIENT_HANDLING_TYPE = 3i32;
pub const WdsTptSlowClientHandlingNone: WDSTRANSPORT_SLOW_CLIENT_HANDLING_TYPE = 1i32;
pub const WdsTptSlowClientHandlingUnknown: WDSTRANSPORT_SLOW_CLIENT_HANDLING_TYPE = 0i32;
pub const WdsTptTftpCapMaximumBlockSize: WDSTRANSPORT_TFTP_CAPABILITY = 1i32;
pub const WdsTptTftpCapVariableWindow: WDSTRANSPORT_TFTP_CAPABILITY = 2i32;
pub const WdsTptUdpPortPolicyDynamic: WDSTRANSPORT_UDP_PORT_POLICY = 0i32;
pub const WdsTptUdpPortPolicyFixed: WDSTRANSPORT_UDP_PORT_POLICY = 1i32;
pub const WdsTransportCacheable: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x70590b16_f146_46bd_bd9d_4aaa90084bf5);
pub const WdsTransportClient: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x66d2c5e9_0ff6_49ec_9733_dafb1e01df1c);
pub const WdsTransportCollection: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc7f18b09_391e_436e_b10b_c3ef46f2c34f);
pub const WdsTransportConfigurationManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8743f674_904c_47ca_8512_35fe98f6b0ac);
pub const WdsTransportContent: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a891fe7_4a3f_4c65_b6f2_1467619679ea);
pub const WdsTransportContentProvider: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe0be741f_5a75_4eb9_8a2d_5e189b45f327);
pub const WdsTransportDiagnosticsPolicy: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeb3333e1_a7ad_46f5_80d6_6b740204e509);
pub const WdsTransportManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf21523f6_837c_4a58_af99_8a7e27f8ff59);
pub const WdsTransportMulticastSessionPolicy: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3c6bc3f4_6418_472a_b6f1_52d457195437);
pub const WdsTransportNamespace: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd8385768_0732_4ec1_95ea_16da581908a1);
pub const WdsTransportNamespaceAutoCast: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb091f5a8_6a99_478d_b23b_09e8fee04574);
pub const WdsTransportNamespaceManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf08cdb63_85de_4a28_a1a9_5ca3e7efda73);
pub const WdsTransportNamespaceScheduledCast: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xbadc1897_7025_44eb_9108_fb61c4055792);
pub const WdsTransportNamespaceScheduledCastAutoStart: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa1107052_122c_4b81_9b7c_386e6855383f);
pub const WdsTransportNamespaceScheduledCastManualStart: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd3e1a2aa_caac_460e_b98a_47f9f318a1fa);
pub const WdsTransportServer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xea19b643_4adf_4413_942c_14f379118760);
pub const WdsTransportServicePolicy: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x65aceadc_2f0b_4f43_9f4d_811865d8cead);
pub const WdsTransportSession: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x749ac4e0_67bc_4743_bfe5_cacb1f26f57f);
pub const WdsTransportSetupManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc7beeaad_9f04_4923_9f0c_fbf52bc7590f);
pub const WdsTransportTftpClient: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50343925_7c5c_4c8c_96c4_ad9fa5005fba);
pub const WdsTransportTftpManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc8e9dca2_3241_4e4d_b806_bc74019dfeda);
pub type CPU_ARCHITECTURE = u32;
pub type PFN_WDS_CLI_CALLBACK_MESSAGE_ID = u32;
pub type TRANSPORTCLIENT_CALLBACK_ID = i32;
pub type TRANSPORTPROVIDER_CALLBACK_ID = i32;
pub type WDSTRANSPORT_DIAGNOSTICS_COMPONENT_FLAGS = i32;
pub type WDSTRANSPORT_DISCONNECT_TYPE = i32;
pub type WDSTRANSPORT_FEATURE_FLAGS = i32;
pub type WDSTRANSPORT_IP_ADDRESS_SOURCE_TYPE = i32;
pub type WDSTRANSPORT_IP_ADDRESS_TYPE = i32;
pub type WDSTRANSPORT_NAMESPACE_TYPE = i32;
pub type WDSTRANSPORT_NETWORK_PROFILE_TYPE = i32;
pub type WDSTRANSPORT_PROTOCOL_FLAGS = i32;
pub type WDSTRANSPORT_SERVICE_NOTIFICATION = i32;
pub type WDSTRANSPORT_SLOW_CLIENT_HANDLING_TYPE = i32;
pub type WDSTRANSPORT_TFTP_CAPABILITY = i32;
pub type WDSTRANSPORT_UDP_PORT_POLICY = i32;
pub type WDS_CLI_FIRMWARE_TYPE = i32;
pub type WDS_CLI_IMAGE_PARAM_TYPE = i32;
pub type WDS_CLI_IMAGE_TYPE = i32;
pub type WDS_TRANSPORTCLIENT_REQUEST_AUTH_LEVEL = u32;
#[repr(C)]
pub struct PXE_ADDRESS {
    pub uFlags: u32,
    pub Anonymous: PXE_ADDRESS_0,
    pub uAddrLen: u32,
    pub uPort: u16,
}
impl ::core::marker::Copy for PXE_ADDRESS {}
impl ::core::clone::Clone for PXE_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union PXE_ADDRESS_0 {
    pub bAddress: [u8; 16],
    pub uIpAddress: u32,
}
impl ::core::marker::Copy for PXE_ADDRESS_0 {}
impl ::core::clone::Clone for PXE_ADDRESS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PXE_DHCPV6_MESSAGE {
    pub MessageType: u8,
    pub TransactionIDByte1: u8,
    pub TransactionIDByte2: u8,
    pub TransactionIDByte3: u8,
    pub Options: [PXE_DHCPV6_OPTION; 1],
}
impl ::core::marker::Copy for PXE_DHCPV6_MESSAGE {}
impl ::core::clone::Clone for PXE_DHCPV6_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PXE_DHCPV6_MESSAGE_HEADER {
    pub MessageType: u8,
    pub Message: [u8; 1],
}
impl ::core::marker::Copy for PXE_DHCPV6_MESSAGE_HEADER {}
impl ::core::clone::Clone for PXE_DHCPV6_MESSAGE_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PXE_DHCPV6_NESTED_RELAY_MESSAGE {
    pub pRelayMessage: *mut PXE_DHCPV6_RELAY_MESSAGE,
    pub cbRelayMessage: u32,
    pub pInterfaceIdOption: *mut ::core::ffi::c_void,
    pub cbInterfaceIdOption: u16,
}
impl ::core::marker::Copy for PXE_DHCPV6_NESTED_RELAY_MESSAGE {}
impl ::core::clone::Clone for PXE_DHCPV6_NESTED_RELAY_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PXE_DHCPV6_OPTION {
    pub OptionCode: u16,
    pub DataLength: u16,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for PXE_DHCPV6_OPTION {}
impl ::core::clone::Clone for PXE_DHCPV6_OPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PXE_DHCPV6_RELAY_MESSAGE {
    pub MessageType: u8,
    pub HopCount: u8,
    pub LinkAddress: [u8; 16],
    pub PeerAddress: [u8; 16],
    pub Options: [PXE_DHCPV6_OPTION; 1],
}
impl ::core::marker::Copy for PXE_DHCPV6_RELAY_MESSAGE {}
impl ::core::clone::Clone for PXE_DHCPV6_RELAY_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PXE_DHCP_MESSAGE {
    pub Operation: u8,
    pub HardwareAddressType: u8,
    pub HardwareAddressLength: u8,
    pub HopCount: u8,
    pub TransactionID: u32,
    pub SecondsSinceBoot: u16,
    pub Reserved: u16,
    pub ClientIpAddress: u32,
    pub YourIpAddress: u32,
    pub BootstrapServerAddress: u32,
    pub RelayAgentIpAddress: u32,
    pub HardwareAddress: [u8; 16],
    pub HostName: [u8; 64],
    pub BootFileName: [u8; 128],
    pub Anonymous: PXE_DHCP_MESSAGE_0,
    pub Option: PXE_DHCP_OPTION,
}
impl ::core::marker::Copy for PXE_DHCP_MESSAGE {}
impl ::core::clone::Clone for PXE_DHCP_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union PXE_DHCP_MESSAGE_0 {
    pub bMagicCookie: [u8; 4],
    pub uMagicCookie: u32,
}
impl ::core::marker::Copy for PXE_DHCP_MESSAGE_0 {}
impl ::core::clone::Clone for PXE_DHCP_MESSAGE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PXE_DHCP_OPTION {
    pub OptionType: u8,
    pub OptionLength: u8,
    pub OptionValue: [u8; 1],
}
impl ::core::marker::Copy for PXE_DHCP_OPTION {}
impl ::core::clone::Clone for PXE_DHCP_OPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PXE_PROVIDER {
    pub uSizeOfStruct: u32,
    pub pwszName: ::windows_sys::core::PWSTR,
    pub pwszFilePath: ::windows_sys::core::PWSTR,
    pub bIsCritical: super::super::Foundation::BOOL,
    pub uIndex: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PXE_PROVIDER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PXE_PROVIDER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRANSPORTCLIENT_SESSION_INFO {
    pub ulStructureLength: u32,
    pub ullFileSize: u64,
    pub ulBlockSize: u32,
}
impl ::core::marker::Copy for TRANSPORTCLIENT_SESSION_INFO {}
impl ::core::clone::Clone for TRANSPORTCLIENT_SESSION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDS_CLI_CRED {
    pub pwszUserName: ::windows_sys::core::PCWSTR,
    pub pwszDomain: ::windows_sys::core::PCWSTR,
    pub pwszPassword: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for WDS_CLI_CRED {}
impl ::core::clone::Clone for WDS_CLI_CRED {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WDS_TRANSPORTCLIENT_CALLBACKS {
    pub SessionStart: PFN_WdsTransportClientSessionStart,
    pub SessionStartEx: PFN_WdsTransportClientSessionStartEx,
    pub ReceiveContents: PFN_WdsTransportClientReceiveContents,
    pub ReceiveMetadata: PFN_WdsTransportClientReceiveMetadata,
    pub SessionComplete: PFN_WdsTransportClientSessionComplete,
    pub SessionNegotiate: PFN_WdsTransportClientSessionNegotiate,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WDS_TRANSPORTCLIENT_CALLBACKS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WDS_TRANSPORTCLIENT_CALLBACKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDS_TRANSPORTCLIENT_REQUEST {
    pub ulLength: u32,
    pub ulApiVersion: u32,
    pub ulAuthLevel: WDS_TRANSPORTCLIENT_REQUEST_AUTH_LEVEL,
    pub pwszServer: ::windows_sys::core::PCWSTR,
    pub pwszNamespace: ::windows_sys::core::PCWSTR,
    pub pwszObjectName: ::windows_sys::core::PCWSTR,
    pub ulCacheSize: u32,
    pub ulProtocol: u32,
    pub pvProtocolData: *mut ::core::ffi::c_void,
    pub ulProtocolDataLength: u32,
}
impl ::core::marker::Copy for WDS_TRANSPORTCLIENT_REQUEST {}
impl ::core::clone::Clone for WDS_TRANSPORTCLIENT_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub struct WDS_TRANSPORTPROVIDER_INIT_PARAMS {
    pub ulLength: u32,
    pub ulMcServerVersion: u32,
    pub hRegistryKey: super::Registry::HKEY,
    pub hProvider: super::super::Foundation::HANDLE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::marker::Copy for WDS_TRANSPORTPROVIDER_INIT_PARAMS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::clone::Clone for WDS_TRANSPORTPROVIDER_INIT_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WDS_TRANSPORTPROVIDER_SETTINGS {
    pub ulLength: u32,
    pub ulProviderVersion: u32,
}
impl ::core::marker::Copy for WDS_TRANSPORTPROVIDER_SETTINGS {}
impl ::core::clone::Clone for WDS_TRANSPORTPROVIDER_SETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_WdsCliCallback = ::core::option::Option<unsafe extern "system" fn(dwmessageid: PFN_WDS_CLI_CALLBACK_MESSAGE_ID, wparam: super::super::Foundation::WPARAM, lparam: super::super::Foundation::LPARAM, pvuserdata: *const ::core::ffi::c_void) -> ()>;
pub type PFN_WdsCliTraceFunction = ::core::option::Option<unsafe extern "system" fn(pwszformat: ::windows_sys::core::PCWSTR, params: *const i8) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_WdsTransportClientReceiveContents = ::core::option::Option<unsafe extern "system" fn(hsessionkey: super::super::Foundation::HANDLE, pcallerdata: *const ::core::ffi::c_void, pcontents: *const ::core::ffi::c_void, ulsize: u32, pullcontentoffset: *const u64) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_WdsTransportClientReceiveMetadata = ::core::option::Option<unsafe extern "system" fn(hsessionkey: super::super::Foundation::HANDLE, pcallerdata: *const ::core::ffi::c_void, pmetadata: *const ::core::ffi::c_void, ulsize: u32) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_WdsTransportClientSessionComplete = ::core::option::Option<unsafe extern "system" fn(hsessionkey: super::super::Foundation::HANDLE, pcallerdata: *const ::core::ffi::c_void, dwerror: u32) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_WdsTransportClientSessionNegotiate = ::core::option::Option<unsafe extern "system" fn(hsessionkey: super::super::Foundation::HANDLE, pcallerdata: *const ::core::ffi::c_void, pinfo: *const TRANSPORTCLIENT_SESSION_INFO, hnegotiatekey: super::super::Foundation::HANDLE) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_WdsTransportClientSessionStart = ::core::option::Option<unsafe extern "system" fn(hsessionkey: super::super::Foundation::HANDLE, pcallerdata: *const ::core::ffi::c_void, ullfilesize: *const u64) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_WdsTransportClientSessionStartEx = ::core::option::Option<unsafe extern "system" fn(hsessionkey: super::super::Foundation::HANDLE, pcallerdata: *const ::core::ffi::c_void, info: *const TRANSPORTCLIENT_SESSION_INFO) -> ()>;
