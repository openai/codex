#[cfg(all(feature = "Win32_System_Com", feature = "Win32_System_Ole"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`"] fn ADsBuildEnumerator(padscontainer : IADsContainer, ppenumvariant : *mut super::super::System::Ole:: IEnumVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn ADsBuildVarArrayInt(lpdwobjecttypes : *mut u32, dwobjecttypes : u32, pvar : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn ADsBuildVarArrayStr(lpppathnames : *const ::windows_sys::core::PCWSTR, dwpathnames : u32, pvar : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("activeds.dll" "system" fn ADsDecodeBinaryData(szsrcdata : ::windows_sys::core::PCWSTR, ppbdestdata : *mut *mut u8, pdwdestlen : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("activeds.dll" "system" fn ADsEncodeBinaryData(pbsrcdata : *mut u8, dwsrclen : u32, ppszdestdata : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn ADsEnumerateNext(penumvariant : super::super::System::Ole:: IEnumVARIANT, celements : u32, pvar : *mut super::super::System::Variant:: VARIANT, pcelementsfetched : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Ole")]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_System_Ole\"`"] fn ADsFreeEnumerator(penumvariant : super::super::System::Ole:: IEnumVARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("activeds.dll" "system" fn ADsGetLastError(lperror : *mut u32, lperrorbuf : ::windows_sys::core::PWSTR, dwerrorbuflen : u32, lpnamebuf : ::windows_sys::core::PWSTR, dwnamebuflen : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("activeds.dll" "system" fn ADsGetObject(lpszpathname : ::windows_sys::core::PCWSTR, riid : *const ::windows_sys::core::GUID, ppobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("activeds.dll" "system" fn ADsOpenObject(lpszpathname : ::windows_sys::core::PCWSTR, lpszusername : ::windows_sys::core::PCWSTR, lpszpassword : ::windows_sys::core::PCWSTR, dwreserved : ADS_AUTHENTICATION_ENUM, riid : *const ::windows_sys::core::GUID, ppobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsprop.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ADsPropCheckIfWritable(pwzattr : ::windows_sys::core::PCWSTR, pwritableattrs : *const ADS_ATTR_INFO) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("dsprop.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ADsPropCreateNotifyObj(pappthddataobj : super::super::System::Com:: IDataObject, pwzadsobjname : ::windows_sys::core::PCWSTR, phnotifyobj : *mut super::super::Foundation:: HWND) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsprop.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ADsPropGetInitInfo(hnotifyobj : super::super::Foundation:: HWND, pinitparams : *mut ADSPROPINITPARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsprop.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ADsPropSendErrorMessage(hnotifyobj : super::super::Foundation:: HWND, perror : *mut ADSPROPERROR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsprop.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ADsPropSetHwnd(hnotifyobj : super::super::Foundation:: HWND, hpage : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsprop.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ADsPropSetHwndWithTitle(hnotifyobj : super::super::Foundation:: HWND, hpage : super::super::Foundation:: HWND, ptztitle : *const i8) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsprop.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ADsPropShowErrorDialog(hnotifyobj : super::super::Foundation:: HWND, hpage : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
::windows_targets::link!("activeds.dll" "system" fn ADsSetLastError(dwerr : u32, pszerror : ::windows_sys::core::PCWSTR, pszprovider : ::windows_sys::core::PCWSTR) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AdsFreeAdsValues(padsvalues : *mut ADSVALUE, dwnumvalues : u32) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn AdsTypeToPropVariant(padsvalues : *mut ADSVALUE, dwnumvalues : u32, pvariant : *mut super::super::System::Variant:: VARIANT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("activeds.dll" "system" fn AllocADsMem(cb : u32) -> *mut ::core::ffi::c_void);
::windows_targets::link!("activeds.dll" "system" fn AllocADsStr(pstr : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn BinarySDToSecurityDescriptor(psecuritydescriptor : super::super::Security:: PSECURITY_DESCRIPTOR, pvarsec : *mut super::super::System::Variant:: VARIANT, pszservername : ::windows_sys::core::PCWSTR, username : ::windows_sys::core::PCWSTR, password : ::windows_sys::core::PCWSTR, dwflags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsAddSidHistoryA(hds : super::super::Foundation:: HANDLE, flags : u32, srcdomain : ::windows_sys::core::PCSTR, srcprincipal : ::windows_sys::core::PCSTR, srcdomaincontroller : ::windows_sys::core::PCSTR, srcdomaincreds : *const ::core::ffi::c_void, dstdomain : ::windows_sys::core::PCSTR, dstprincipal : ::windows_sys::core::PCSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsAddSidHistoryW(hds : super::super::Foundation:: HANDLE, flags : u32, srcdomain : ::windows_sys::core::PCWSTR, srcprincipal : ::windows_sys::core::PCWSTR, srcdomaincontroller : ::windows_sys::core::PCWSTR, srcdomaincreds : *const ::core::ffi::c_void, dstdomain : ::windows_sys::core::PCWSTR, dstprincipal : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn DsAddressToSiteNamesA(computername : ::windows_sys::core::PCSTR, entrycount : u32, socketaddresses : *const super::WinSock:: SOCKET_ADDRESS, sitenames : *mut *mut ::windows_sys::core::PSTR) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn DsAddressToSiteNamesExA(computername : ::windows_sys::core::PCSTR, entrycount : u32, socketaddresses : *const super::WinSock:: SOCKET_ADDRESS, sitenames : *mut *mut ::windows_sys::core::PSTR, subnetnames : *mut *mut ::windows_sys::core::PSTR) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn DsAddressToSiteNamesExW(computername : ::windows_sys::core::PCWSTR, entrycount : u32, socketaddresses : *const super::WinSock:: SOCKET_ADDRESS, sitenames : *mut *mut ::windows_sys::core::PWSTR, subnetnames : *mut *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn DsAddressToSiteNamesW(computername : ::windows_sys::core::PCWSTR, entrycount : u32, socketaddresses : *const super::WinSock:: SOCKET_ADDRESS, sitenames : *mut *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindA(domaincontrollername : ::windows_sys::core::PCSTR, dnsdomainname : ::windows_sys::core::PCSTR, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindByInstanceA(servername : ::windows_sys::core::PCSTR, annotation : ::windows_sys::core::PCSTR, instanceguid : *const ::windows_sys::core::GUID, dnsdomainname : ::windows_sys::core::PCSTR, authidentity : *const ::core::ffi::c_void, serviceprincipalname : ::windows_sys::core::PCSTR, bindflags : u32, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindByInstanceW(servername : ::windows_sys::core::PCWSTR, annotation : ::windows_sys::core::PCWSTR, instanceguid : *const ::windows_sys::core::GUID, dnsdomainname : ::windows_sys::core::PCWSTR, authidentity : *const ::core::ffi::c_void, serviceprincipalname : ::windows_sys::core::PCWSTR, bindflags : u32, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindToISTGA(sitename : ::windows_sys::core::PCSTR, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindToISTGW(sitename : ::windows_sys::core::PCWSTR, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindW(domaincontrollername : ::windows_sys::core::PCWSTR, dnsdomainname : ::windows_sys::core::PCWSTR, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindWithCredA(domaincontrollername : ::windows_sys::core::PCSTR, dnsdomainname : ::windows_sys::core::PCSTR, authidentity : *const ::core::ffi::c_void, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindWithCredW(domaincontrollername : ::windows_sys::core::PCWSTR, dnsdomainname : ::windows_sys::core::PCWSTR, authidentity : *const ::core::ffi::c_void, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindWithSpnA(domaincontrollername : ::windows_sys::core::PCSTR, dnsdomainname : ::windows_sys::core::PCSTR, authidentity : *const ::core::ffi::c_void, serviceprincipalname : ::windows_sys::core::PCSTR, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindWithSpnExA(domaincontrollername : ::windows_sys::core::PCSTR, dnsdomainname : ::windows_sys::core::PCSTR, authidentity : *const ::core::ffi::c_void, serviceprincipalname : ::windows_sys::core::PCSTR, bindflags : u32, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindWithSpnExW(domaincontrollername : ::windows_sys::core::PCWSTR, dnsdomainname : ::windows_sys::core::PCWSTR, authidentity : *const ::core::ffi::c_void, serviceprincipalname : ::windows_sys::core::PCWSTR, bindflags : u32, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindWithSpnW(domaincontrollername : ::windows_sys::core::PCWSTR, dnsdomainname : ::windows_sys::core::PCWSTR, authidentity : *const ::core::ffi::c_void, serviceprincipalname : ::windows_sys::core::PCWSTR, phds : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsBindingSetTimeout(hds : super::super::Foundation:: HANDLE, ctimeoutsecs : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
::windows_targets::link!("dsuiext.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"] fn DsBrowseForContainerA(pinfo : *mut DSBROWSEINFOA) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
::windows_targets::link!("dsuiext.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"] fn DsBrowseForContainerW(pinfo : *mut DSBROWSEINFOW) -> i32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsClientMakeSpnForTargetServerA(serviceclass : ::windows_sys::core::PCSTR, servicename : ::windows_sys::core::PCSTR, pcspnlength : *mut u32, pszspn : ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsClientMakeSpnForTargetServerW(serviceclass : ::windows_sys::core::PCWSTR, servicename : ::windows_sys::core::PCWSTR, pcspnlength : *mut u32, pszspn : ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsCrackNamesA(hds : super::super::Foundation:: HANDLE, flags : DS_NAME_FLAGS, formatoffered : DS_NAME_FORMAT, formatdesired : DS_NAME_FORMAT, cnames : u32, rpnames : *const ::windows_sys::core::PCSTR, ppresult : *mut *mut DS_NAME_RESULTA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsCrackNamesW(hds : super::super::Foundation:: HANDLE, flags : DS_NAME_FLAGS, formatoffered : DS_NAME_FORMAT, formatdesired : DS_NAME_FORMAT, cnames : u32, rpnames : *const ::windows_sys::core::PCWSTR, ppresult : *mut *mut DS_NAME_RESULTW) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsCrackSpn2A(pszspn : ::windows_sys::core::PCSTR, cspn : u32, pcserviceclass : *mut u32, serviceclass : ::windows_sys::core::PSTR, pcservicename : *mut u32, servicename : ::windows_sys::core::PSTR, pcinstancename : *mut u32, instancename : ::windows_sys::core::PSTR, pinstanceport : *mut u16) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsCrackSpn2W(pszspn : ::windows_sys::core::PCWSTR, cspn : u32, pcserviceclass : *mut u32, serviceclass : ::windows_sys::core::PWSTR, pcservicename : *mut u32, servicename : ::windows_sys::core::PWSTR, pcinstancename : *mut u32, instancename : ::windows_sys::core::PWSTR, pinstanceport : *mut u16) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsCrackSpn3W(pszspn : ::windows_sys::core::PCWSTR, cspn : u32, pchostname : *mut u32, hostname : ::windows_sys::core::PWSTR, pcinstancename : *mut u32, instancename : ::windows_sys::core::PWSTR, pportnumber : *mut u16, pcdomainname : *mut u32, domainname : ::windows_sys::core::PWSTR, pcrealmname : *mut u32, realmname : ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsCrackSpn4W(pszspn : ::windows_sys::core::PCWSTR, cspn : u32, pchostname : *mut u32, hostname : ::windows_sys::core::PWSTR, pcinstancename : *mut u32, instancename : ::windows_sys::core::PWSTR, pcportname : *mut u32, portname : ::windows_sys::core::PWSTR, pcdomainname : *mut u32, domainname : ::windows_sys::core::PWSTR, pcrealmname : *mut u32, realmname : ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsCrackSpnA(pszspn : ::windows_sys::core::PCSTR, pcserviceclass : *mut u32, serviceclass : ::windows_sys::core::PSTR, pcservicename : *mut u32, servicename : ::windows_sys::core::PSTR, pcinstancename : *mut u32, instancename : ::windows_sys::core::PSTR, pinstanceport : *mut u16) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsCrackSpnW(pszspn : ::windows_sys::core::PCWSTR, pcserviceclass : *mut u32, serviceclass : ::windows_sys::core::PWSTR, pcservicename : *mut u32, servicename : ::windows_sys::core::PWSTR, pcinstancename : *mut u32, instancename : ::windows_sys::core::PWSTR, pinstanceport : *mut u16) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsparse.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsCrackUnquotedMangledRdnA(pszrdn : ::windows_sys::core::PCSTR, cchrdn : u32, pguid : *mut ::windows_sys::core::GUID, pedsmanglefor : *mut DS_MANGLE_FOR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsparse.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsCrackUnquotedMangledRdnW(pszrdn : ::windows_sys::core::PCWSTR, cchrdn : u32, pguid : *mut ::windows_sys::core::GUID, pedsmanglefor : *mut DS_MANGLE_FOR) -> super::super::Foundation:: BOOL);
::windows_targets::link!("netapi32.dll" "system" fn DsDeregisterDnsHostRecordsA(servername : ::windows_sys::core::PCSTR, dnsdomainname : ::windows_sys::core::PCSTR, domainguid : *const ::windows_sys::core::GUID, dsaguid : *const ::windows_sys::core::GUID, dnshostname : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsDeregisterDnsHostRecordsW(servername : ::windows_sys::core::PCWSTR, dnsdomainname : ::windows_sys::core::PCWSTR, domainguid : *const ::windows_sys::core::GUID, dsaguid : *const ::windows_sys::core::GUID, dnshostname : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsEnumerateDomainTrustsA(servername : ::windows_sys::core::PCSTR, flags : u32, domains : *mut *mut DS_DOMAIN_TRUSTSA, domaincount : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsEnumerateDomainTrustsW(servername : ::windows_sys::core::PCWSTR, flags : u32, domains : *mut *mut DS_DOMAIN_TRUSTSW, domaincount : *mut u32) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeDomainControllerInfoA(infolevel : u32, cinfo : u32, pinfo : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeDomainControllerInfoW(infolevel : u32, cinfo : u32, pinfo : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeNameResultA(presult : *const DS_NAME_RESULTA) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeNameResultW(presult : *const DS_NAME_RESULTW) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreePasswordCredentials(authidentity : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeSchemaGuidMapA(pguidmap : *const DS_SCHEMA_GUID_MAPA) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeSchemaGuidMapW(pguidmap : *const DS_SCHEMA_GUID_MAPW) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeSpnArrayA(cspn : u32, rpszspn : *mut ::windows_sys::core::PSTR) -> ());
::windows_targets::link!("ntdsapi.dll" "system" fn DsFreeSpnArrayW(cspn : u32, rpszspn : *mut ::windows_sys::core::PWSTR) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsGetDcCloseW(getdccontexthandle : super::super::Foundation:: HANDLE) -> ());
::windows_targets::link!("netapi32.dll" "system" fn DsGetDcNameA(computername : ::windows_sys::core::PCSTR, domainname : ::windows_sys::core::PCSTR, domainguid : *const ::windows_sys::core::GUID, sitename : ::windows_sys::core::PCSTR, flags : u32, domaincontrollerinfo : *mut *mut DOMAIN_CONTROLLER_INFOA) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsGetDcNameW(computername : ::windows_sys::core::PCWSTR, domainname : ::windows_sys::core::PCWSTR, domainguid : *const ::windows_sys::core::GUID, sitename : ::windows_sys::core::PCWSTR, flags : u32, domaincontrollerinfo : *mut *mut DOMAIN_CONTROLLER_INFOW) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn DsGetDcNextA(getdccontexthandle : super::super::Foundation:: HANDLE, sockaddresscount : *mut u32, sockaddresses : *mut *mut super::WinSock:: SOCKET_ADDRESS, dnshostname : *mut ::windows_sys::core::PSTR) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn DsGetDcNextW(getdccontexthandle : super::super::Foundation:: HANDLE, sockaddresscount : *mut u32, sockaddresses : *mut *mut super::WinSock:: SOCKET_ADDRESS, dnshostname : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsGetDcOpenA(dnsname : ::windows_sys::core::PCSTR, optionflags : u32, sitename : ::windows_sys::core::PCSTR, domainguid : *const ::windows_sys::core::GUID, dnsforestname : ::windows_sys::core::PCSTR, dcflags : u32, retgetdccontext : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsGetDcOpenW(dnsname : ::windows_sys::core::PCWSTR, optionflags : u32, sitename : ::windows_sys::core::PCWSTR, domainguid : *const ::windows_sys::core::GUID, dnsforestname : ::windows_sys::core::PCWSTR, dcflags : u32, retgetdccontext : *mut super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsGetDcSiteCoverageA(servername : ::windows_sys::core::PCSTR, entrycount : *mut u32, sitenames : *mut *mut ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsGetDcSiteCoverageW(servername : ::windows_sys::core::PCWSTR, entrycount : *mut u32, sitenames : *mut *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsGetDomainControllerInfoA(hds : super::super::Foundation:: HANDLE, domainname : ::windows_sys::core::PCSTR, infolevel : u32, pcout : *mut u32, ppinfo : *mut *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsGetDomainControllerInfoW(hds : super::super::Foundation:: HANDLE, domainname : ::windows_sys::core::PCWSTR, infolevel : u32, pcout : *mut u32, ppinfo : *mut *mut ::core::ffi::c_void) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"] fn DsGetForestTrustInformationW(servername : ::windows_sys::core::PCWSTR, trusteddomainname : ::windows_sys::core::PCWSTR, flags : u32, foresttrustinfo : *mut *mut super::super::Security::Authentication::Identity:: LSA_FOREST_TRUST_INFORMATION) -> u32);
::windows_targets::link!("dsuiext.dll" "system" fn DsGetFriendlyClassName(pszobjectclass : ::windows_sys::core::PCWSTR, pszbuffer : ::windows_sys::core::PWSTR, cchbuffer : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("dsuiext.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn DsGetIcon(dwflags : u32, pszobjectclass : ::windows_sys::core::PCWSTR, cximage : i32, cyimage : i32) -> super::super::UI::WindowsAndMessaging:: HICON);
::windows_targets::link!("dsparse.dll" "system" fn DsGetRdnW(ppdn : *mut ::windows_sys::core::PWSTR, pcdn : *mut u32, ppkey : *mut ::windows_sys::core::PWSTR, pckey : *mut u32, ppval : *mut ::windows_sys::core::PWSTR, pcval : *mut u32) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsGetSiteNameA(computername : ::windows_sys::core::PCSTR, sitename : *mut ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsGetSiteNameW(computername : ::windows_sys::core::PCWSTR, sitename : *mut ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsGetSpnA(servicetype : DS_SPN_NAME_TYPE, serviceclass : ::windows_sys::core::PCSTR, servicename : ::windows_sys::core::PCSTR, instanceport : u16, cinstancenames : u16, pinstancenames : *const ::windows_sys::core::PCSTR, pinstanceports : *const u16, pcspn : *mut u32, prpszspn : *mut *mut ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsGetSpnW(servicetype : DS_SPN_NAME_TYPE, serviceclass : ::windows_sys::core::PCWSTR, servicename : ::windows_sys::core::PCWSTR, instanceport : u16, cinstancenames : u16, pinstancenames : *const ::windows_sys::core::PCWSTR, pinstanceports : *const u16, pcspn : *mut u32, prpszspn : *mut *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsInheritSecurityIdentityA(hds : super::super::Foundation:: HANDLE, flags : u32, srcprincipal : ::windows_sys::core::PCSTR, dstprincipal : ::windows_sys::core::PCSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsInheritSecurityIdentityW(hds : super::super::Foundation:: HANDLE, flags : u32, srcprincipal : ::windows_sys::core::PCWSTR, dstprincipal : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsparse.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsIsMangledDnA(pszdn : ::windows_sys::core::PCSTR, edsmanglefor : DS_MANGLE_FOR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsparse.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsIsMangledDnW(pszdn : ::windows_sys::core::PCWSTR, edsmanglefor : DS_MANGLE_FOR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsparse.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsIsMangledRdnValueA(pszrdn : ::windows_sys::core::PCSTR, crdn : u32, edsmanglefordesired : DS_MANGLE_FOR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dsparse.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsIsMangledRdnValueW(pszrdn : ::windows_sys::core::PCWSTR, crdn : u32, edsmanglefordesired : DS_MANGLE_FOR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListDomainsInSiteA(hds : super::super::Foundation:: HANDLE, site : ::windows_sys::core::PCSTR, ppdomains : *mut *mut DS_NAME_RESULTA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListDomainsInSiteW(hds : super::super::Foundation:: HANDLE, site : ::windows_sys::core::PCWSTR, ppdomains : *mut *mut DS_NAME_RESULTW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListInfoForServerA(hds : super::super::Foundation:: HANDLE, server : ::windows_sys::core::PCSTR, ppinfo : *mut *mut DS_NAME_RESULTA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListInfoForServerW(hds : super::super::Foundation:: HANDLE, server : ::windows_sys::core::PCWSTR, ppinfo : *mut *mut DS_NAME_RESULTW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListRolesA(hds : super::super::Foundation:: HANDLE, pproles : *mut *mut DS_NAME_RESULTA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListRolesW(hds : super::super::Foundation:: HANDLE, pproles : *mut *mut DS_NAME_RESULTW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListServersForDomainInSiteA(hds : super::super::Foundation:: HANDLE, domain : ::windows_sys::core::PCSTR, site : ::windows_sys::core::PCSTR, ppservers : *mut *mut DS_NAME_RESULTA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListServersForDomainInSiteW(hds : super::super::Foundation:: HANDLE, domain : ::windows_sys::core::PCWSTR, site : ::windows_sys::core::PCWSTR, ppservers : *mut *mut DS_NAME_RESULTW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListServersInSiteA(hds : super::super::Foundation:: HANDLE, site : ::windows_sys::core::PCSTR, ppservers : *mut *mut DS_NAME_RESULTA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListServersInSiteW(hds : super::super::Foundation:: HANDLE, site : ::windows_sys::core::PCWSTR, ppservers : *mut *mut DS_NAME_RESULTW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListSitesA(hds : super::super::Foundation:: HANDLE, ppsites : *mut *mut DS_NAME_RESULTA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsListSitesW(hds : super::super::Foundation:: HANDLE, ppsites : *mut *mut DS_NAME_RESULTW) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsMakePasswordCredentialsA(user : ::windows_sys::core::PCSTR, domain : ::windows_sys::core::PCSTR, password : ::windows_sys::core::PCSTR, pauthidentity : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsMakePasswordCredentialsW(user : ::windows_sys::core::PCWSTR, domain : ::windows_sys::core::PCWSTR, password : ::windows_sys::core::PCWSTR, pauthidentity : *mut *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsMakeSpnA(serviceclass : ::windows_sys::core::PCSTR, servicename : ::windows_sys::core::PCSTR, instancename : ::windows_sys::core::PCSTR, instanceport : u16, referrer : ::windows_sys::core::PCSTR, pcspnlength : *mut u32, pszspn : ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsMakeSpnW(serviceclass : ::windows_sys::core::PCWSTR, servicename : ::windows_sys::core::PCWSTR, instancename : ::windows_sys::core::PCWSTR, instanceport : u16, referrer : ::windows_sys::core::PCWSTR, pcspnlength : *mut u32, pszspn : ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsMapSchemaGuidsA(hds : super::super::Foundation:: HANDLE, cguids : u32, rguids : *const ::windows_sys::core::GUID, ppguidmap : *mut *mut DS_SCHEMA_GUID_MAPA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsMapSchemaGuidsW(hds : super::super::Foundation:: HANDLE, cguids : u32, rguids : *const ::windows_sys::core::GUID, ppguidmap : *mut *mut DS_SCHEMA_GUID_MAPW) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity"))]
::windows_targets::link!("netapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`"] fn DsMergeForestTrustInformationW(domainname : ::windows_sys::core::PCWSTR, newforesttrustinfo : *const super::super::Security::Authentication::Identity:: LSA_FOREST_TRUST_INFORMATION, oldforesttrustinfo : *const super::super::Security::Authentication::Identity:: LSA_FOREST_TRUST_INFORMATION, mergedforesttrustinfo : *mut *mut super::super::Security::Authentication::Identity:: LSA_FOREST_TRUST_INFORMATION) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsQuerySitesByCostA(hds : super::super::Foundation:: HANDLE, pszfromsite : ::windows_sys::core::PCSTR, rgsztosites : *const ::windows_sys::core::PCSTR, ctosites : u32, dwflags : u32, prgsiteinfo : *mut *mut DS_SITE_COST_INFO) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsQuerySitesByCostW(hds : super::super::Foundation:: HANDLE, pwszfromsite : ::windows_sys::core::PCWSTR, rgwsztosites : *const ::windows_sys::core::PCWSTR, ctosites : u32, dwflags : u32, prgsiteinfo : *mut *mut DS_SITE_COST_INFO) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsQuerySitesFree(rgsiteinfo : *const DS_SITE_COST_INFO) -> ());
::windows_targets::link!("dsparse.dll" "system" fn DsQuoteRdnValueA(cunquotedrdnvaluelength : u32, psunquotedrdnvalue : ::windows_sys::core::PCSTR, pcquotedrdnvaluelength : *mut u32, psquotedrdnvalue : ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsQuoteRdnValueW(cunquotedrdnvaluelength : u32, psunquotedrdnvalue : ::windows_sys::core::PCWSTR, pcquotedrdnvaluelength : *mut u32, psquotedrdnvalue : ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsRemoveDsDomainA(hds : super::super::Foundation:: HANDLE, domaindn : ::windows_sys::core::PCSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsRemoveDsDomainW(hds : super::super::Foundation:: HANDLE, domaindn : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsRemoveDsServerA(hds : super::super::Foundation:: HANDLE, serverdn : ::windows_sys::core::PCSTR, domaindn : ::windows_sys::core::PCSTR, flastdcindomain : *mut super::super::Foundation:: BOOL, fcommit : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsRemoveDsServerW(hds : super::super::Foundation:: HANDLE, serverdn : ::windows_sys::core::PCWSTR, domaindn : ::windows_sys::core::PCWSTR, flastdcindomain : *mut super::super::Foundation:: BOOL, fcommit : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaAddA(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCSTR, sourcedsadn : ::windows_sys::core::PCSTR, transportdn : ::windows_sys::core::PCSTR, sourcedsaaddress : ::windows_sys::core::PCSTR, pschedule : *const SCHEDULE, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaAddW(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCWSTR, sourcedsadn : ::windows_sys::core::PCWSTR, transportdn : ::windows_sys::core::PCWSTR, sourcedsaaddress : ::windows_sys::core::PCWSTR, pschedule : *const SCHEDULE, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaConsistencyCheck(hds : super::super::Foundation:: HANDLE, taskid : DS_KCC_TASKID, dwflags : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaDelA(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCSTR, dsasrc : ::windows_sys::core::PCSTR, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaDelW(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCWSTR, dsasrc : ::windows_sys::core::PCWSTR, options : u32) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsReplicaFreeInfo(infotype : DS_REPL_INFO_TYPE, pinfo : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaGetInfo2W(hds : super::super::Foundation:: HANDLE, infotype : DS_REPL_INFO_TYPE, pszobject : ::windows_sys::core::PCWSTR, puuidforsourcedsaobjguid : *const ::windows_sys::core::GUID, pszattributename : ::windows_sys::core::PCWSTR, pszvalue : ::windows_sys::core::PCWSTR, dwflags : u32, dwenumerationcontext : u32, ppinfo : *mut *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaGetInfoW(hds : super::super::Foundation:: HANDLE, infotype : DS_REPL_INFO_TYPE, pszobject : ::windows_sys::core::PCWSTR, puuidforsourcedsaobjguid : *const ::windows_sys::core::GUID, ppinfo : *mut *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaModifyA(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCSTR, puuidsourcedsa : *const ::windows_sys::core::GUID, transportdn : ::windows_sys::core::PCSTR, sourcedsaaddress : ::windows_sys::core::PCSTR, pschedule : *const SCHEDULE, replicaflags : u32, modifyfields : u32, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaModifyW(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCWSTR, puuidsourcedsa : *const ::windows_sys::core::GUID, transportdn : ::windows_sys::core::PCWSTR, sourcedsaaddress : ::windows_sys::core::PCWSTR, pschedule : *const SCHEDULE, replicaflags : u32, modifyfields : u32, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaSyncA(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCSTR, puuiddsasrc : *const ::windows_sys::core::GUID, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaSyncAllA(hds : super::super::Foundation:: HANDLE, psznamecontext : ::windows_sys::core::PCSTR, ulflags : u32, pfncallback : isize, pcallbackdata : *const ::core::ffi::c_void, perrors : *mut *mut *mut DS_REPSYNCALL_ERRINFOA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaSyncAllW(hds : super::super::Foundation:: HANDLE, psznamecontext : ::windows_sys::core::PCWSTR, ulflags : u32, pfncallback : isize, pcallbackdata : *const ::core::ffi::c_void, perrors : *mut *mut *mut DS_REPSYNCALL_ERRINFOW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaSyncW(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCWSTR, puuiddsasrc : *const ::windows_sys::core::GUID, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaUpdateRefsA(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCSTR, dsadest : ::windows_sys::core::PCSTR, puuiddsadest : *const ::windows_sys::core::GUID, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaUpdateRefsW(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCWSTR, dsadest : ::windows_sys::core::PCWSTR, puuiddsadest : *const ::windows_sys::core::GUID, options : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaVerifyObjectsA(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCSTR, puuiddsasrc : *const ::windows_sys::core::GUID, uloptions : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsReplicaVerifyObjectsW(hds : super::super::Foundation:: HANDLE, namecontext : ::windows_sys::core::PCWSTR, puuiddsasrc : *const ::windows_sys::core::GUID, uloptions : u32) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsRoleFreeMemory(buffer : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("netapi32.dll" "system" fn DsRoleGetPrimaryDomainInformation(lpserver : ::windows_sys::core::PCWSTR, infolevel : DSROLE_PRIMARY_DOMAIN_INFO_LEVEL, buffer : *mut *mut u8) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsServerRegisterSpnA(operation : DS_SPN_WRITE_OP, serviceclass : ::windows_sys::core::PCSTR, userobjectdn : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("ntdsapi.dll" "system" fn DsServerRegisterSpnW(operation : DS_SPN_WRITE_OP, serviceclass : ::windows_sys::core::PCWSTR, userobjectdn : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsUnBindA(phds : *const super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsUnBindW(phds : *const super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsUnquoteRdnValueA(cquotedrdnvaluelength : u32, psquotedrdnvalue : ::windows_sys::core::PCSTR, pcunquotedrdnvaluelength : *mut u32, psunquotedrdnvalue : ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("dsparse.dll" "system" fn DsUnquoteRdnValueW(cquotedrdnvaluelength : u32, psquotedrdnvalue : ::windows_sys::core::PCWSTR, pcunquotedrdnvaluelength : *mut u32, psunquotedrdnvalue : ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsValidateSubnetNameA(subnetname : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("netapi32.dll" "system" fn DsValidateSubnetNameW(subnetname : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsWriteAccountSpnA(hds : super::super::Foundation:: HANDLE, operation : DS_SPN_WRITE_OP, pszaccount : ::windows_sys::core::PCSTR, cspn : u32, rpszspn : *const ::windows_sys::core::PCSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntdsapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DsWriteAccountSpnW(hds : super::super::Foundation:: HANDLE, operation : DS_SPN_WRITE_OP, pszaccount : ::windows_sys::core::PCWSTR, cspn : u32, rpszspn : *const ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FreeADsMem(pmem : *mut ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FreeADsStr(pstr : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn PropVariantToAdsType(pvariant : *mut super::super::System::Variant:: VARIANT, dwnumvariant : u32, ppadsvalues : *mut *mut ADSVALUE, pdwnumvalues : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("activeds.dll" "system" fn ReallocADsMem(poldmem : *mut ::core::ffi::c_void, cbold : u32, cbnew : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReallocADsStr(ppstr : *mut ::windows_sys::core::PWSTR, pstr : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
::windows_targets::link!("activeds.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"] fn SecurityDescriptorToBinarySD(vvarsecdes : super::super::System::Variant:: VARIANT, ppsecuritydescriptor : *mut super::super::Security:: PSECURITY_DESCRIPTOR, pdwsdlength : *mut u32, pszservername : ::windows_sys::core::PCWSTR, username : ::windows_sys::core::PCWSTR, password : ::windows_sys::core::PCWSTR, dwflags : u32) -> ::windows_sys::core::HRESULT);
pub type IADs = *mut ::core::ffi::c_void;
pub type IADsADSystemInfo = *mut ::core::ffi::c_void;
pub type IADsAccessControlEntry = *mut ::core::ffi::c_void;
pub type IADsAccessControlList = *mut ::core::ffi::c_void;
pub type IADsAcl = *mut ::core::ffi::c_void;
pub type IADsAggregatee = *mut ::core::ffi::c_void;
pub type IADsAggregator = *mut ::core::ffi::c_void;
pub type IADsBackLink = *mut ::core::ffi::c_void;
pub type IADsCaseIgnoreList = *mut ::core::ffi::c_void;
pub type IADsClass = *mut ::core::ffi::c_void;
pub type IADsCollection = *mut ::core::ffi::c_void;
pub type IADsComputer = *mut ::core::ffi::c_void;
pub type IADsComputerOperations = *mut ::core::ffi::c_void;
pub type IADsContainer = *mut ::core::ffi::c_void;
pub type IADsDNWithBinary = *mut ::core::ffi::c_void;
pub type IADsDNWithString = *mut ::core::ffi::c_void;
pub type IADsDeleteOps = *mut ::core::ffi::c_void;
pub type IADsDomain = *mut ::core::ffi::c_void;
pub type IADsEmail = *mut ::core::ffi::c_void;
pub type IADsExtension = *mut ::core::ffi::c_void;
pub type IADsFaxNumber = *mut ::core::ffi::c_void;
pub type IADsFileService = *mut ::core::ffi::c_void;
pub type IADsFileServiceOperations = *mut ::core::ffi::c_void;
pub type IADsFileShare = *mut ::core::ffi::c_void;
pub type IADsGroup = *mut ::core::ffi::c_void;
pub type IADsHold = *mut ::core::ffi::c_void;
pub type IADsLargeInteger = *mut ::core::ffi::c_void;
pub type IADsLocality = *mut ::core::ffi::c_void;
pub type IADsMembers = *mut ::core::ffi::c_void;
pub type IADsNameTranslate = *mut ::core::ffi::c_void;
pub type IADsNamespaces = *mut ::core::ffi::c_void;
pub type IADsNetAddress = *mut ::core::ffi::c_void;
pub type IADsO = *mut ::core::ffi::c_void;
pub type IADsOU = *mut ::core::ffi::c_void;
pub type IADsObjectOptions = *mut ::core::ffi::c_void;
pub type IADsOctetList = *mut ::core::ffi::c_void;
pub type IADsOpenDSObject = *mut ::core::ffi::c_void;
pub type IADsPath = *mut ::core::ffi::c_void;
pub type IADsPathname = *mut ::core::ffi::c_void;
pub type IADsPostalAddress = *mut ::core::ffi::c_void;
pub type IADsPrintJob = *mut ::core::ffi::c_void;
pub type IADsPrintJobOperations = *mut ::core::ffi::c_void;
pub type IADsPrintQueue = *mut ::core::ffi::c_void;
pub type IADsPrintQueueOperations = *mut ::core::ffi::c_void;
pub type IADsProperty = *mut ::core::ffi::c_void;
pub type IADsPropertyEntry = *mut ::core::ffi::c_void;
pub type IADsPropertyList = *mut ::core::ffi::c_void;
pub type IADsPropertyValue = *mut ::core::ffi::c_void;
pub type IADsPropertyValue2 = *mut ::core::ffi::c_void;
pub type IADsReplicaPointer = *mut ::core::ffi::c_void;
pub type IADsResource = *mut ::core::ffi::c_void;
pub type IADsSecurityDescriptor = *mut ::core::ffi::c_void;
pub type IADsSecurityUtility = *mut ::core::ffi::c_void;
pub type IADsService = *mut ::core::ffi::c_void;
pub type IADsServiceOperations = *mut ::core::ffi::c_void;
pub type IADsSession = *mut ::core::ffi::c_void;
pub type IADsSyntax = *mut ::core::ffi::c_void;
pub type IADsTimestamp = *mut ::core::ffi::c_void;
pub type IADsTypedName = *mut ::core::ffi::c_void;
pub type IADsUser = *mut ::core::ffi::c_void;
pub type IADsWinNTSystemInfo = *mut ::core::ffi::c_void;
pub type ICommonQuery = *mut ::core::ffi::c_void;
pub type IDirectoryObject = *mut ::core::ffi::c_void;
pub type IDirectorySchemaMgmt = *mut ::core::ffi::c_void;
pub type IDirectorySearch = *mut ::core::ffi::c_void;
pub type IDsAdminCreateObj = *mut ::core::ffi::c_void;
pub type IDsAdminNewObj = *mut ::core::ffi::c_void;
pub type IDsAdminNewObjExt = *mut ::core::ffi::c_void;
pub type IDsAdminNewObjPrimarySite = *mut ::core::ffi::c_void;
pub type IDsAdminNotifyHandler = *mut ::core::ffi::c_void;
pub type IDsBrowseDomainTree = *mut ::core::ffi::c_void;
pub type IDsDisplaySpecifier = *mut ::core::ffi::c_void;
pub type IDsObjectPicker = *mut ::core::ffi::c_void;
pub type IDsObjectPickerCredentials = *mut ::core::ffi::c_void;
pub type IPersistQuery = *mut ::core::ffi::c_void;
pub type IPrivateDispatch = *mut ::core::ffi::c_void;
pub type IPrivateUnknown = *mut ::core::ffi::c_void;
pub type IQueryForm = *mut ::core::ffi::c_void;
pub const ACTRL_DS_CONTROL_ACCESS: u32 = 256u32;
pub const ACTRL_DS_CREATE_CHILD: u32 = 1u32;
pub const ACTRL_DS_DELETE_CHILD: u32 = 2u32;
pub const ACTRL_DS_DELETE_TREE: u32 = 64u32;
pub const ACTRL_DS_LIST: u32 = 4u32;
pub const ACTRL_DS_LIST_OBJECT: u32 = 128u32;
pub const ACTRL_DS_OPEN: u32 = 0u32;
pub const ACTRL_DS_READ_PROP: u32 = 16u32;
pub const ACTRL_DS_SELF: u32 = 8u32;
pub const ACTRL_DS_WRITE_PROP: u32 = 32u32;
pub const ADAM_REPL_AUTHENTICATION_MODE_MUTUAL_AUTH_REQUIRED: u32 = 2u32;
pub const ADAM_REPL_AUTHENTICATION_MODE_NEGOTIATE: u32 = 1u32;
pub const ADAM_REPL_AUTHENTICATION_MODE_NEGOTIATE_PASS_THROUGH: u32 = 0u32;
pub const ADAM_SCP_FSMO_NAMING_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("naming");
pub const ADAM_SCP_FSMO_NAMING_STRING_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("naming");
pub const ADAM_SCP_FSMO_SCHEMA_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("schema");
pub const ADAM_SCP_FSMO_SCHEMA_STRING_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("schema");
pub const ADAM_SCP_FSMO_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("fsmo:");
pub const ADAM_SCP_FSMO_STRING_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("fsmo:");
pub const ADAM_SCP_INSTANCE_NAME_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("instance:");
pub const ADAM_SCP_INSTANCE_NAME_STRING_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("instance:");
pub const ADAM_SCP_PARTITION_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("partition:");
pub const ADAM_SCP_PARTITION_STRING_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("partition:");
pub const ADAM_SCP_SITE_NAME_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("site:");
pub const ADAM_SCP_SITE_NAME_STRING_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("site:");
pub const ADSIPROP_ADSIFLAG: ADS_PREFERENCES_ENUM = 12i32;
pub const ADSIPROP_ASYNCHRONOUS: ADS_PREFERENCES_ENUM = 0i32;
pub const ADSIPROP_ATTRIBTYPES_ONLY: ADS_PREFERENCES_ENUM = 4i32;
pub const ADSIPROP_CACHE_RESULTS: ADS_PREFERENCES_ENUM = 11i32;
pub const ADSIPROP_CHASE_REFERRALS: ADS_PREFERENCES_ENUM = 9i32;
pub const ADSIPROP_DEREF_ALIASES: ADS_PREFERENCES_ENUM = 1i32;
pub const ADSIPROP_PAGED_TIME_LIMIT: ADS_PREFERENCES_ENUM = 8i32;
pub const ADSIPROP_PAGESIZE: ADS_PREFERENCES_ENUM = 7i32;
pub const ADSIPROP_SEARCH_SCOPE: ADS_PREFERENCES_ENUM = 5i32;
pub const ADSIPROP_SIZE_LIMIT: ADS_PREFERENCES_ENUM = 2i32;
pub const ADSIPROP_SORT_ON: ADS_PREFERENCES_ENUM = 10i32;
pub const ADSIPROP_TIMEOUT: ADS_PREFERENCES_ENUM = 6i32;
pub const ADSIPROP_TIME_LIMIT: ADS_PREFERENCES_ENUM = 3i32;
pub const ADSI_DIALECT_LDAP: ADSI_DIALECT_ENUM = 0i32;
pub const ADSI_DIALECT_SQL: ADSI_DIALECT_ENUM = 1i32;
pub const ADSTYPE_BACKLINK: ADSTYPE = 18i32;
pub const ADSTYPE_BOOLEAN: ADSTYPE = 6i32;
pub const ADSTYPE_CASEIGNORE_LIST: ADSTYPE = 13i32;
pub const ADSTYPE_CASE_EXACT_STRING: ADSTYPE = 2i32;
pub const ADSTYPE_CASE_IGNORE_STRING: ADSTYPE = 3i32;
pub const ADSTYPE_DN_STRING: ADSTYPE = 1i32;
pub const ADSTYPE_DN_WITH_BINARY: ADSTYPE = 27i32;
pub const ADSTYPE_DN_WITH_STRING: ADSTYPE = 28i32;
pub const ADSTYPE_EMAIL: ADSTYPE = 24i32;
pub const ADSTYPE_FAXNUMBER: ADSTYPE = 23i32;
pub const ADSTYPE_HOLD: ADSTYPE = 20i32;
pub const ADSTYPE_INTEGER: ADSTYPE = 7i32;
pub const ADSTYPE_INVALID: ADSTYPE = 0i32;
pub const ADSTYPE_LARGE_INTEGER: ADSTYPE = 10i32;
pub const ADSTYPE_NETADDRESS: ADSTYPE = 21i32;
pub const ADSTYPE_NT_SECURITY_DESCRIPTOR: ADSTYPE = 25i32;
pub const ADSTYPE_NUMERIC_STRING: ADSTYPE = 5i32;
pub const ADSTYPE_OBJECT_CLASS: ADSTYPE = 12i32;
pub const ADSTYPE_OCTET_LIST: ADSTYPE = 14i32;
pub const ADSTYPE_OCTET_STRING: ADSTYPE = 8i32;
pub const ADSTYPE_PATH: ADSTYPE = 15i32;
pub const ADSTYPE_POSTALADDRESS: ADSTYPE = 16i32;
pub const ADSTYPE_PRINTABLE_STRING: ADSTYPE = 4i32;
pub const ADSTYPE_PROV_SPECIFIC: ADSTYPE = 11i32;
pub const ADSTYPE_REPLICAPOINTER: ADSTYPE = 22i32;
pub const ADSTYPE_TIMESTAMP: ADSTYPE = 17i32;
pub const ADSTYPE_TYPEDNAME: ADSTYPE = 19i32;
pub const ADSTYPE_UNKNOWN: ADSTYPE = 26i32;
pub const ADSTYPE_UTC_TIME: ADSTYPE = 9i32;
pub const ADS_ACEFLAG_FAILED_ACCESS: ADS_ACEFLAG_ENUM = 128i32;
pub const ADS_ACEFLAG_INHERITED_ACE: ADS_ACEFLAG_ENUM = 16i32;
pub const ADS_ACEFLAG_INHERIT_ACE: ADS_ACEFLAG_ENUM = 2i32;
pub const ADS_ACEFLAG_INHERIT_ONLY_ACE: ADS_ACEFLAG_ENUM = 8i32;
pub const ADS_ACEFLAG_NO_PROPAGATE_INHERIT_ACE: ADS_ACEFLAG_ENUM = 4i32;
pub const ADS_ACEFLAG_SUCCESSFUL_ACCESS: ADS_ACEFLAG_ENUM = 64i32;
pub const ADS_ACEFLAG_VALID_INHERIT_FLAGS: ADS_ACEFLAG_ENUM = 31i32;
pub const ADS_ACETYPE_ACCESS_ALLOWED: ADS_ACETYPE_ENUM = 0i32;
pub const ADS_ACETYPE_ACCESS_ALLOWED_CALLBACK: ADS_ACETYPE_ENUM = 9i32;
pub const ADS_ACETYPE_ACCESS_ALLOWED_CALLBACK_OBJECT: ADS_ACETYPE_ENUM = 11i32;
pub const ADS_ACETYPE_ACCESS_ALLOWED_OBJECT: ADS_ACETYPE_ENUM = 5i32;
pub const ADS_ACETYPE_ACCESS_DENIED: ADS_ACETYPE_ENUM = 1i32;
pub const ADS_ACETYPE_ACCESS_DENIED_CALLBACK: ADS_ACETYPE_ENUM = 10i32;
pub const ADS_ACETYPE_ACCESS_DENIED_CALLBACK_OBJECT: ADS_ACETYPE_ENUM = 12i32;
pub const ADS_ACETYPE_ACCESS_DENIED_OBJECT: ADS_ACETYPE_ENUM = 6i32;
pub const ADS_ACETYPE_SYSTEM_ALARM_CALLBACK: ADS_ACETYPE_ENUM = 14i32;
pub const ADS_ACETYPE_SYSTEM_ALARM_CALLBACK_OBJECT: ADS_ACETYPE_ENUM = 16i32;
pub const ADS_ACETYPE_SYSTEM_ALARM_OBJECT: ADS_ACETYPE_ENUM = 8i32;
pub const ADS_ACETYPE_SYSTEM_AUDIT: ADS_ACETYPE_ENUM = 2i32;
pub const ADS_ACETYPE_SYSTEM_AUDIT_CALLBACK: ADS_ACETYPE_ENUM = 13i32;
pub const ADS_ACETYPE_SYSTEM_AUDIT_CALLBACK_OBJECT: ADS_ACETYPE_ENUM = 15i32;
pub const ADS_ACETYPE_SYSTEM_AUDIT_OBJECT: ADS_ACETYPE_ENUM = 7i32;
pub const ADS_ATTR_APPEND: u32 = 3u32;
pub const ADS_ATTR_CLEAR: u32 = 1u32;
pub const ADS_ATTR_DELETE: u32 = 4u32;
pub const ADS_ATTR_UPDATE: u32 = 2u32;
pub const ADS_AUTH_RESERVED: ADS_AUTHENTICATION_ENUM = 2147483648u32;
pub const ADS_CHASE_REFERRALS_ALWAYS: ADS_CHASE_REFERRALS_ENUM = 96i32;
pub const ADS_CHASE_REFERRALS_EXTERNAL: ADS_CHASE_REFERRALS_ENUM = 64i32;
pub const ADS_CHASE_REFERRALS_NEVER: ADS_CHASE_REFERRALS_ENUM = 0i32;
pub const ADS_CHASE_REFERRALS_SUBORDINATE: ADS_CHASE_REFERRALS_ENUM = 32i32;
pub const ADS_DEREF_ALWAYS: ADS_DEREFENUM = 3i32;
pub const ADS_DEREF_FINDING: ADS_DEREFENUM = 2i32;
pub const ADS_DEREF_NEVER: ADS_DEREFENUM = 0i32;
pub const ADS_DEREF_SEARCHING: ADS_DEREFENUM = 1i32;
pub const ADS_DISPLAY_FULL: ADS_DISPLAY_ENUM = 1i32;
pub const ADS_DISPLAY_VALUE_ONLY: ADS_DISPLAY_ENUM = 2i32;
pub const ADS_ESCAPEDMODE_DEFAULT: ADS_ESCAPE_MODE_ENUM = 1i32;
pub const ADS_ESCAPEDMODE_OFF: ADS_ESCAPE_MODE_ENUM = 3i32;
pub const ADS_ESCAPEDMODE_OFF_EX: ADS_ESCAPE_MODE_ENUM = 4i32;
pub const ADS_ESCAPEDMODE_ON: ADS_ESCAPE_MODE_ENUM = 2i32;
pub const ADS_EXT_INITCREDENTIALS: u32 = 1u32;
pub const ADS_EXT_INITIALIZE_COMPLETE: u32 = 2u32;
pub const ADS_EXT_MAXEXTDISPID: u32 = 16777215u32;
pub const ADS_EXT_MINEXTDISPID: u32 = 1u32;
pub const ADS_FAST_BIND: ADS_AUTHENTICATION_ENUM = 32u32;
pub const ADS_FLAG_INHERITED_OBJECT_TYPE_PRESENT: ADS_FLAGTYPE_ENUM = 2i32;
pub const ADS_FLAG_OBJECT_TYPE_PRESENT: ADS_FLAGTYPE_ENUM = 1i32;
pub const ADS_FORMAT_LEAF: ADS_FORMAT_ENUM = 11i32;
pub const ADS_FORMAT_PROVIDER: ADS_FORMAT_ENUM = 10i32;
pub const ADS_FORMAT_SERVER: ADS_FORMAT_ENUM = 9i32;
pub const ADS_FORMAT_WINDOWS: ADS_FORMAT_ENUM = 1i32;
pub const ADS_FORMAT_WINDOWS_DN: ADS_FORMAT_ENUM = 3i32;
pub const ADS_FORMAT_WINDOWS_NO_SERVER: ADS_FORMAT_ENUM = 2i32;
pub const ADS_FORMAT_WINDOWS_PARENT: ADS_FORMAT_ENUM = 4i32;
pub const ADS_FORMAT_X500: ADS_FORMAT_ENUM = 5i32;
pub const ADS_FORMAT_X500_DN: ADS_FORMAT_ENUM = 7i32;
pub const ADS_FORMAT_X500_NO_SERVER: ADS_FORMAT_ENUM = 6i32;
pub const ADS_FORMAT_X500_PARENT: ADS_FORMAT_ENUM = 8i32;
pub const ADS_GROUP_TYPE_DOMAIN_LOCAL_GROUP: ADS_GROUP_TYPE_ENUM = 4i32;
pub const ADS_GROUP_TYPE_GLOBAL_GROUP: ADS_GROUP_TYPE_ENUM = 2i32;
pub const ADS_GROUP_TYPE_LOCAL_GROUP: ADS_GROUP_TYPE_ENUM = 4i32;
pub const ADS_GROUP_TYPE_SECURITY_ENABLED: ADS_GROUP_TYPE_ENUM = -2147483648i32;
pub const ADS_GROUP_TYPE_UNIVERSAL_GROUP: ADS_GROUP_TYPE_ENUM = 8i32;
pub const ADS_NAME_INITTYPE_DOMAIN: ADS_NAME_INITTYPE_ENUM = 1i32;
pub const ADS_NAME_INITTYPE_GC: ADS_NAME_INITTYPE_ENUM = 3i32;
pub const ADS_NAME_INITTYPE_SERVER: ADS_NAME_INITTYPE_ENUM = 2i32;
pub const ADS_NAME_TYPE_1779: ADS_NAME_TYPE_ENUM = 1i32;
pub const ADS_NAME_TYPE_CANONICAL: ADS_NAME_TYPE_ENUM = 2i32;
pub const ADS_NAME_TYPE_CANONICAL_EX: ADS_NAME_TYPE_ENUM = 10i32;
pub const ADS_NAME_TYPE_DISPLAY: ADS_NAME_TYPE_ENUM = 4i32;
pub const ADS_NAME_TYPE_DOMAIN_SIMPLE: ADS_NAME_TYPE_ENUM = 5i32;
pub const ADS_NAME_TYPE_ENTERPRISE_SIMPLE: ADS_NAME_TYPE_ENUM = 6i32;
pub const ADS_NAME_TYPE_GUID: ADS_NAME_TYPE_ENUM = 7i32;
pub const ADS_NAME_TYPE_NT4: ADS_NAME_TYPE_ENUM = 3i32;
pub const ADS_NAME_TYPE_SERVICE_PRINCIPAL_NAME: ADS_NAME_TYPE_ENUM = 11i32;
pub const ADS_NAME_TYPE_SID_OR_SID_HISTORY_NAME: ADS_NAME_TYPE_ENUM = 12i32;
pub const ADS_NAME_TYPE_UNKNOWN: ADS_NAME_TYPE_ENUM = 8i32;
pub const ADS_NAME_TYPE_USER_PRINCIPAL_NAME: ADS_NAME_TYPE_ENUM = 9i32;
pub const ADS_NO_AUTHENTICATION: ADS_AUTHENTICATION_ENUM = 16u32;
pub const ADS_NO_REFERRAL_CHASING: ADS_AUTHENTICATION_ENUM = 1024u32;
pub const ADS_OPTION_ACCUMULATIVE_MODIFICATION: ADS_OPTION_ENUM = 8i32;
pub const ADS_OPTION_MUTUAL_AUTH_STATUS: ADS_OPTION_ENUM = 4i32;
pub const ADS_OPTION_PAGE_SIZE: ADS_OPTION_ENUM = 2i32;
pub const ADS_OPTION_PASSWORD_METHOD: ADS_OPTION_ENUM = 7i32;
pub const ADS_OPTION_PASSWORD_PORTNUMBER: ADS_OPTION_ENUM = 6i32;
pub const ADS_OPTION_QUOTA: ADS_OPTION_ENUM = 5i32;
pub const ADS_OPTION_REFERRALS: ADS_OPTION_ENUM = 1i32;
pub const ADS_OPTION_SECURITY_MASK: ADS_OPTION_ENUM = 3i32;
pub const ADS_OPTION_SERVERNAME: ADS_OPTION_ENUM = 0i32;
pub const ADS_OPTION_SKIP_SID_LOOKUP: ADS_OPTION_ENUM = 9i32;
pub const ADS_PASSWORD_ENCODE_CLEAR: ADS_PASSWORD_ENCODING_ENUM = 1i32;
pub const ADS_PASSWORD_ENCODE_REQUIRE_SSL: ADS_PASSWORD_ENCODING_ENUM = 0i32;
pub const ADS_PATH_FILE: ADS_PATHTYPE_ENUM = 1i32;
pub const ADS_PATH_FILESHARE: ADS_PATHTYPE_ENUM = 2i32;
pub const ADS_PATH_REGISTRY: ADS_PATHTYPE_ENUM = 3i32;
pub const ADS_PROMPT_CREDENTIALS: ADS_AUTHENTICATION_ENUM = 8u32;
pub const ADS_PROPERTY_APPEND: ADS_PROPERTY_OPERATION_ENUM = 3i32;
pub const ADS_PROPERTY_CLEAR: ADS_PROPERTY_OPERATION_ENUM = 1i32;
pub const ADS_PROPERTY_DELETE: ADS_PROPERTY_OPERATION_ENUM = 4i32;
pub const ADS_PROPERTY_UPDATE: ADS_PROPERTY_OPERATION_ENUM = 2i32;
pub const ADS_READONLY_SERVER: ADS_AUTHENTICATION_ENUM = 4u32;
pub const ADS_RIGHT_ACCESS_SYSTEM_SECURITY: ADS_RIGHTS_ENUM = 16777216i32;
pub const ADS_RIGHT_ACTRL_DS_LIST: ADS_RIGHTS_ENUM = 4i32;
pub const ADS_RIGHT_DELETE: ADS_RIGHTS_ENUM = 65536i32;
pub const ADS_RIGHT_DS_CONTROL_ACCESS: ADS_RIGHTS_ENUM = 256i32;
pub const ADS_RIGHT_DS_CREATE_CHILD: ADS_RIGHTS_ENUM = 1i32;
pub const ADS_RIGHT_DS_DELETE_CHILD: ADS_RIGHTS_ENUM = 2i32;
pub const ADS_RIGHT_DS_DELETE_TREE: ADS_RIGHTS_ENUM = 64i32;
pub const ADS_RIGHT_DS_LIST_OBJECT: ADS_RIGHTS_ENUM = 128i32;
pub const ADS_RIGHT_DS_READ_PROP: ADS_RIGHTS_ENUM = 16i32;
pub const ADS_RIGHT_DS_SELF: ADS_RIGHTS_ENUM = 8i32;
pub const ADS_RIGHT_DS_WRITE_PROP: ADS_RIGHTS_ENUM = 32i32;
pub const ADS_RIGHT_GENERIC_ALL: ADS_RIGHTS_ENUM = 268435456i32;
pub const ADS_RIGHT_GENERIC_EXECUTE: ADS_RIGHTS_ENUM = 536870912i32;
pub const ADS_RIGHT_GENERIC_READ: ADS_RIGHTS_ENUM = -2147483648i32;
pub const ADS_RIGHT_GENERIC_WRITE: ADS_RIGHTS_ENUM = 1073741824i32;
pub const ADS_RIGHT_READ_CONTROL: ADS_RIGHTS_ENUM = 131072i32;
pub const ADS_RIGHT_SYNCHRONIZE: ADS_RIGHTS_ENUM = 1048576i32;
pub const ADS_RIGHT_WRITE_DAC: ADS_RIGHTS_ENUM = 262144i32;
pub const ADS_RIGHT_WRITE_OWNER: ADS_RIGHTS_ENUM = 524288i32;
pub const ADS_SCOPE_BASE: ADS_SCOPEENUM = 0i32;
pub const ADS_SCOPE_ONELEVEL: ADS_SCOPEENUM = 1i32;
pub const ADS_SCOPE_SUBTREE: ADS_SCOPEENUM = 2i32;
pub const ADS_SD_CONTROL_SE_DACL_AUTO_INHERITED: ADS_SD_CONTROL_ENUM = 1024i32;
pub const ADS_SD_CONTROL_SE_DACL_AUTO_INHERIT_REQ: ADS_SD_CONTROL_ENUM = 256i32;
pub const ADS_SD_CONTROL_SE_DACL_DEFAULTED: ADS_SD_CONTROL_ENUM = 8i32;
pub const ADS_SD_CONTROL_SE_DACL_PRESENT: ADS_SD_CONTROL_ENUM = 4i32;
pub const ADS_SD_CONTROL_SE_DACL_PROTECTED: ADS_SD_CONTROL_ENUM = 4096i32;
pub const ADS_SD_CONTROL_SE_GROUP_DEFAULTED: ADS_SD_CONTROL_ENUM = 2i32;
pub const ADS_SD_CONTROL_SE_OWNER_DEFAULTED: ADS_SD_CONTROL_ENUM = 1i32;
pub const ADS_SD_CONTROL_SE_SACL_AUTO_INHERITED: ADS_SD_CONTROL_ENUM = 2048i32;
pub const ADS_SD_CONTROL_SE_SACL_AUTO_INHERIT_REQ: ADS_SD_CONTROL_ENUM = 512i32;
pub const ADS_SD_CONTROL_SE_SACL_DEFAULTED: ADS_SD_CONTROL_ENUM = 32i32;
pub const ADS_SD_CONTROL_SE_SACL_PRESENT: ADS_SD_CONTROL_ENUM = 16i32;
pub const ADS_SD_CONTROL_SE_SACL_PROTECTED: ADS_SD_CONTROL_ENUM = 8192i32;
pub const ADS_SD_CONTROL_SE_SELF_RELATIVE: ADS_SD_CONTROL_ENUM = 32768i32;
pub const ADS_SD_FORMAT_HEXSTRING: ADS_SD_FORMAT_ENUM = 3i32;
pub const ADS_SD_FORMAT_IID: ADS_SD_FORMAT_ENUM = 1i32;
pub const ADS_SD_FORMAT_RAW: ADS_SD_FORMAT_ENUM = 2i32;
pub const ADS_SD_REVISION_DS: ADS_SD_REVISION_ENUM = 4i32;
pub const ADS_SEARCHPREF_ASYNCHRONOUS: ADS_SEARCHPREF_ENUM = 0i32;
pub const ADS_SEARCHPREF_ATTRIBTYPES_ONLY: ADS_SEARCHPREF_ENUM = 4i32;
pub const ADS_SEARCHPREF_ATTRIBUTE_QUERY: ADS_SEARCHPREF_ENUM = 15i32;
pub const ADS_SEARCHPREF_CACHE_RESULTS: ADS_SEARCHPREF_ENUM = 11i32;
pub const ADS_SEARCHPREF_CHASE_REFERRALS: ADS_SEARCHPREF_ENUM = 9i32;
pub const ADS_SEARCHPREF_DEREF_ALIASES: ADS_SEARCHPREF_ENUM = 1i32;
pub const ADS_SEARCHPREF_DIRSYNC: ADS_SEARCHPREF_ENUM = 12i32;
pub const ADS_SEARCHPREF_DIRSYNC_FLAG: ADS_SEARCHPREF_ENUM = 17i32;
pub const ADS_SEARCHPREF_EXTENDED_DN: ADS_SEARCHPREF_ENUM = 18i32;
pub const ADS_SEARCHPREF_PAGED_TIME_LIMIT: ADS_SEARCHPREF_ENUM = 8i32;
pub const ADS_SEARCHPREF_PAGESIZE: ADS_SEARCHPREF_ENUM = 7i32;
pub const ADS_SEARCHPREF_SEARCH_SCOPE: ADS_SEARCHPREF_ENUM = 5i32;
pub const ADS_SEARCHPREF_SECURITY_MASK: ADS_SEARCHPREF_ENUM = 16i32;
pub const ADS_SEARCHPREF_SIZE_LIMIT: ADS_SEARCHPREF_ENUM = 2i32;
pub const ADS_SEARCHPREF_SORT_ON: ADS_SEARCHPREF_ENUM = 10i32;
pub const ADS_SEARCHPREF_TIMEOUT: ADS_SEARCHPREF_ENUM = 6i32;
pub const ADS_SEARCHPREF_TIME_LIMIT: ADS_SEARCHPREF_ENUM = 3i32;
pub const ADS_SEARCHPREF_TOMBSTONE: ADS_SEARCHPREF_ENUM = 13i32;
pub const ADS_SEARCHPREF_VLV: ADS_SEARCHPREF_ENUM = 14i32;
pub const ADS_SECURE_AUTHENTICATION: ADS_AUTHENTICATION_ENUM = 1u32;
pub const ADS_SECURITY_INFO_DACL: ADS_SECURITY_INFO_ENUM = 4i32;
pub const ADS_SECURITY_INFO_GROUP: ADS_SECURITY_INFO_ENUM = 2i32;
pub const ADS_SECURITY_INFO_OWNER: ADS_SECURITY_INFO_ENUM = 1i32;
pub const ADS_SECURITY_INFO_SACL: ADS_SECURITY_INFO_ENUM = 8i32;
pub const ADS_SERVER_BIND: ADS_AUTHENTICATION_ENUM = 512u32;
pub const ADS_SETTYPE_DN: ADS_SETTYPE_ENUM = 4i32;
pub const ADS_SETTYPE_FULL: ADS_SETTYPE_ENUM = 1i32;
pub const ADS_SETTYPE_PROVIDER: ADS_SETTYPE_ENUM = 2i32;
pub const ADS_SETTYPE_SERVER: ADS_SETTYPE_ENUM = 3i32;
pub const ADS_STATUS_INVALID_SEARCHPREF: ADS_STATUSENUM = 1i32;
pub const ADS_STATUS_INVALID_SEARCHPREFVALUE: ADS_STATUSENUM = 2i32;
pub const ADS_STATUS_S_OK: ADS_STATUSENUM = 0i32;
pub const ADS_SYSTEMFLAG_ATTR_IS_CONSTRUCTED: ADS_SYSTEMFLAG_ENUM = 4i32;
pub const ADS_SYSTEMFLAG_ATTR_NOT_REPLICATED: ADS_SYSTEMFLAG_ENUM = 1i32;
pub const ADS_SYSTEMFLAG_CONFIG_ALLOW_LIMITED_MOVE: ADS_SYSTEMFLAG_ENUM = 268435456i32;
pub const ADS_SYSTEMFLAG_CONFIG_ALLOW_MOVE: ADS_SYSTEMFLAG_ENUM = 536870912i32;
pub const ADS_SYSTEMFLAG_CONFIG_ALLOW_RENAME: ADS_SYSTEMFLAG_ENUM = 1073741824i32;
pub const ADS_SYSTEMFLAG_CR_NTDS_DOMAIN: ADS_SYSTEMFLAG_ENUM = 2i32;
pub const ADS_SYSTEMFLAG_CR_NTDS_NC: ADS_SYSTEMFLAG_ENUM = 1i32;
pub const ADS_SYSTEMFLAG_DISALLOW_DELETE: ADS_SYSTEMFLAG_ENUM = -2147483648i32;
pub const ADS_SYSTEMFLAG_DOMAIN_DISALLOW_MOVE: ADS_SYSTEMFLAG_ENUM = 67108864i32;
pub const ADS_SYSTEMFLAG_DOMAIN_DISALLOW_RENAME: ADS_SYSTEMFLAG_ENUM = 134217728i32;
pub const ADS_UF_ACCOUNTDISABLE: ADS_USER_FLAG_ENUM = 2i32;
pub const ADS_UF_DONT_EXPIRE_PASSWD: ADS_USER_FLAG_ENUM = 65536i32;
pub const ADS_UF_DONT_REQUIRE_PREAUTH: ADS_USER_FLAG_ENUM = 4194304i32;
pub const ADS_UF_ENCRYPTED_TEXT_PASSWORD_ALLOWED: ADS_USER_FLAG_ENUM = 128i32;
pub const ADS_UF_HOMEDIR_REQUIRED: ADS_USER_FLAG_ENUM = 8i32;
pub const ADS_UF_INTERDOMAIN_TRUST_ACCOUNT: ADS_USER_FLAG_ENUM = 2048i32;
pub const ADS_UF_LOCKOUT: ADS_USER_FLAG_ENUM = 16i32;
pub const ADS_UF_MNS_LOGON_ACCOUNT: ADS_USER_FLAG_ENUM = 131072i32;
pub const ADS_UF_NORMAL_ACCOUNT: ADS_USER_FLAG_ENUM = 512i32;
pub const ADS_UF_NOT_DELEGATED: ADS_USER_FLAG_ENUM = 1048576i32;
pub const ADS_UF_PASSWD_CANT_CHANGE: ADS_USER_FLAG_ENUM = 64i32;
pub const ADS_UF_PASSWD_NOTREQD: ADS_USER_FLAG_ENUM = 32i32;
pub const ADS_UF_PASSWORD_EXPIRED: ADS_USER_FLAG_ENUM = 8388608i32;
pub const ADS_UF_SCRIPT: ADS_USER_FLAG_ENUM = 1i32;
pub const ADS_UF_SERVER_TRUST_ACCOUNT: ADS_USER_FLAG_ENUM = 8192i32;
pub const ADS_UF_SMARTCARD_REQUIRED: ADS_USER_FLAG_ENUM = 262144i32;
pub const ADS_UF_TEMP_DUPLICATE_ACCOUNT: ADS_USER_FLAG_ENUM = 256i32;
pub const ADS_UF_TRUSTED_FOR_DELEGATION: ADS_USER_FLAG_ENUM = 524288i32;
pub const ADS_UF_TRUSTED_TO_AUTHENTICATE_FOR_DELEGATION: ADS_USER_FLAG_ENUM = 16777216i32;
pub const ADS_UF_USE_DES_KEY_ONLY: ADS_USER_FLAG_ENUM = 2097152i32;
pub const ADS_UF_WORKSTATION_TRUST_ACCOUNT: ADS_USER_FLAG_ENUM = 4096i32;
pub const ADS_USE_DELEGATION: ADS_AUTHENTICATION_ENUM = 256u32;
pub const ADS_USE_ENCRYPTION: ADS_AUTHENTICATION_ENUM = 2u32;
pub const ADS_USE_SEALING: ADS_AUTHENTICATION_ENUM = 128u32;
pub const ADS_USE_SIGNING: ADS_AUTHENTICATION_ENUM = 64u32;
pub const ADS_USE_SSL: ADS_AUTHENTICATION_ENUM = 2u32;
pub const ADSystemInfo: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50b6327f_afd1_11d2_9cb9_0000f87a369e);
pub const ADsSecurityUtility: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf270c64a_ffb8_4ae4_85fe_3a75e5347966);
pub const AccessControlEntry: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb75ac000_9bdd_11d0_852c_00c04fd8d503);
pub const AccessControlList: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb85ea052_9bdd_11d0_852c_00c04fd8d503);
pub const BackLink: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfcbf906f_4080_11d1_a3ac_00c04fb950dc);
pub const CFSTR_DSDISPLAYSPECOPTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DsDisplaySpecOptions");
pub const CFSTR_DSOBJECTNAMES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DsObjectNames");
pub const CFSTR_DSOP_DS_SELECTION_LIST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CFSTR_DSOP_DS_SELECTION_LIST");
pub const CFSTR_DSPROPERTYPAGEINFO: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DsPropPageInfo");
pub const CFSTR_DSQUERYPARAMS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DsQueryParameters");
pub const CFSTR_DSQUERYSCOPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DsQueryScope");
pub const CFSTR_DS_DISPLAY_SPEC_OPTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DsDisplaySpecOptions");
pub const CLSID_CommonQuery: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83bc5ec0_6f2a_11d0_a1c4_00aa00c16e65);
pub const CLSID_DsAdminCreateObj: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe301a009_f901_11d2_82b9_00c04f68928b);
pub const CLSID_DsDisplaySpecifier: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1ab4a8c0_6a0b_11d2_ad49_00c04fa31a86);
pub const CLSID_DsDomainTreeBrowser: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1698790a_e2b4_11d0_b0b1_00c04fd8dca6);
pub const CLSID_DsFindAdvanced: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83ee3fe3_57d9_11d0_b932_00a024ab2dbb);
pub const CLSID_DsFindComputer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x16006700_87ad_11d0_9140_00aa00c16e65);
pub const CLSID_DsFindContainer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc1b3cbf2_886a_11d0_9140_00aa00c16e65);
pub const CLSID_DsFindDomainController: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x538c7b7e_d25e_11d0_9742_00a0c906af45);
pub const CLSID_DsFindFrsMembers: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x94ce4b18_b3d3_11d1_b9b4_00c04fd8d5b0);
pub const CLSID_DsFindObjects: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83ee3fe1_57d9_11d0_b932_00a024ab2dbb);
pub const CLSID_DsFindPeople: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83ee3fe2_57d9_11d0_b932_00a024ab2dbb);
pub const CLSID_DsFindPrinter: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb577f070_7ee2_11d0_913f_00aa00c16e65);
pub const CLSID_DsFindVolume: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc1b3cbf1_886a_11d0_9140_00aa00c16e65);
pub const CLSID_DsFindWriteableDomainController: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7cbef079_aa84_444b_bc70_68e41283eabc);
pub const CLSID_DsFolderProperties: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9e51e0d0_6e0f_11d2_9601_00c04fa31a86);
pub const CLSID_DsObjectPicker: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x17d6ccd8_3b7b_11d2_b9e0_00c04fd8dbf7);
pub const CLSID_DsPropertyPages: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0d45d530_764b_11d0_a1ca_00aa00c16e65);
pub const CLSID_DsQuery: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8a23e65e_31c2_11d0_891c_00a024ab2dbb);
pub const CLSID_MicrosoftDS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfe1290f0_cfbd_11cf_a330_00aa00c16e65);
pub const CQFF_ISOPTIONAL: u32 = 2u32;
pub const CQFF_NOGLOBALPAGES: u32 = 1u32;
pub const CQPM_CLEARFORM: u32 = 6u32;
pub const CQPM_ENABLE: u32 = 3u32;
pub const CQPM_GETPARAMETERS: u32 = 5u32;
pub const CQPM_HANDLERSPECIFIC: u32 = 268435456u32;
pub const CQPM_HELP: u32 = 8u32;
pub const CQPM_INITIALIZE: u32 = 1u32;
pub const CQPM_PERSIST: u32 = 7u32;
pub const CQPM_RELEASE: u32 = 2u32;
pub const CQPM_SETDEFAULTPARAMETERS: u32 = 9u32;
pub const CaseIgnoreList: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x15f88a55_4680_11d1_a3b4_00c04fb950dc);
pub const DBDTF_RETURNEXTERNAL: u32 = 4u32;
pub const DBDTF_RETURNFQDN: u32 = 1u32;
pub const DBDTF_RETURNINBOUND: u32 = 8u32;
pub const DBDTF_RETURNINOUTBOUND: u32 = 16u32;
pub const DBDTF_RETURNMIXEDDOMAINS: u32 = 2u32;
pub const DNWithBinary: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7e99c0a3_f935_11d2_ba96_00c04fb6d0d1);
pub const DNWithString: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x334857cc_f934_11d2_ba96_00c04fb6d0d1);
pub const DSA_NEWOBJ_CTX_CLEANUP: u32 = 4u32;
pub const DSA_NEWOBJ_CTX_COMMIT: u32 = 2u32;
pub const DSA_NEWOBJ_CTX_POSTCOMMIT: u32 = 3u32;
pub const DSA_NEWOBJ_CTX_PRECOMMIT: u32 = 1u32;
pub const DSA_NOTIFY_DEL: u32 = 1u32;
pub const DSA_NOTIFY_FLAG_ADDITIONAL_DATA: u32 = 2u32;
pub const DSA_NOTIFY_FLAG_FORCE_ADDITIONAL_DATA: u32 = 1u32;
pub const DSA_NOTIFY_MOV: u32 = 4u32;
pub const DSA_NOTIFY_PROP: u32 = 8u32;
pub const DSA_NOTIFY_REN: u32 = 2u32;
pub const DSBF_DISPLAYNAME: u32 = 4u32;
pub const DSBF_ICONLOCATION: u32 = 2u32;
pub const DSBF_STATE: u32 = 1u32;
pub const DSBID_BANNER: u32 = 256u32;
pub const DSBID_CONTAINERLIST: u32 = 257u32;
pub const DSBI_CHECKBOXES: u32 = 256u32;
pub const DSBI_DONTSIGNSEAL: u32 = 33554432u32;
pub const DSBI_ENTIREDIRECTORY: u32 = 589824u32;
pub const DSBI_EXPANDONOPEN: u32 = 262144u32;
pub const DSBI_HASCREDENTIALS: u32 = 2097152u32;
pub const DSBI_IGNORETREATASLEAF: u32 = 4194304u32;
pub const DSBI_INCLUDEHIDDEN: u32 = 131072u32;
pub const DSBI_NOBUTTONS: u32 = 1u32;
pub const DSBI_NOLINES: u32 = 2u32;
pub const DSBI_NOLINESATROOT: u32 = 4u32;
pub const DSBI_NOROOT: u32 = 65536u32;
pub const DSBI_RETURNOBJECTCLASS: u32 = 16777216u32;
pub const DSBI_RETURN_FORMAT: u32 = 1048576u32;
pub const DSBI_SIMPLEAUTHENTICATE: u32 = 8388608u32;
pub const DSBM_CHANGEIMAGESTATE: u32 = 102u32;
pub const DSBM_CONTEXTMENU: u32 = 104u32;
pub const DSBM_HELP: u32 = 103u32;
pub const DSBM_QUERYINSERT: u32 = 100u32;
pub const DSBM_QUERYINSERTA: u32 = 101u32;
pub const DSBM_QUERYINSERTW: u32 = 100u32;
pub const DSBS_CHECKED: u32 = 1u32;
pub const DSBS_HIDDEN: u32 = 2u32;
pub const DSBS_ROOT: u32 = 4u32;
pub const DSB_MAX_DISPLAYNAME_CHARS: u32 = 64u32;
pub const DSCCIF_HASWIZARDDIALOG: u32 = 1u32;
pub const DSCCIF_HASWIZARDPRIMARYPAGE: u32 = 2u32;
pub const DSDSOF_DONTSIGNSEAL: u32 = 4u32;
pub const DSDSOF_DSAVAILABLE: u32 = 1073741824u32;
pub const DSDSOF_HASUSERANDSERVERINFO: u32 = 1u32;
pub const DSDSOF_SIMPLEAUTHENTICATE: u32 = 2u32;
pub const DSECAF_NOTLISTED: u32 = 1u32;
pub const DSGIF_DEFAULTISCONTAINER: u32 = 32u32;
pub const DSGIF_GETDEFAULTICON: u32 = 16u32;
pub const DSGIF_ISDISABLED: u32 = 2u32;
pub const DSGIF_ISMASK: u32 = 15u32;
pub const DSGIF_ISNORMAL: u32 = 0u32;
pub const DSGIF_ISOPEN: u32 = 1u32;
pub const DSICCF_IGNORETREATASLEAF: u32 = 1u32;
pub const DSOBJECT_ISCONTAINER: u32 = 1u32;
pub const DSOBJECT_READONLYPAGES: u32 = 2147483648u32;
pub const DSOP_DOWNLEVEL_FILTER_ALL_APP_PACKAGES: u32 = 2281701376u32;
pub const DSOP_DOWNLEVEL_FILTER_ALL_WELLKNOWN_SIDS: u32 = 2147614720u32;
pub const DSOP_DOWNLEVEL_FILTER_ANONYMOUS: u32 = 2147483712u32;
pub const DSOP_DOWNLEVEL_FILTER_AUTHENTICATED_USER: u32 = 2147483680u32;
pub const DSOP_DOWNLEVEL_FILTER_BATCH: u32 = 2147483776u32;
pub const DSOP_DOWNLEVEL_FILTER_COMPUTERS: u32 = 2147483656u32;
pub const DSOP_DOWNLEVEL_FILTER_CREATOR_GROUP: u32 = 2147484160u32;
pub const DSOP_DOWNLEVEL_FILTER_CREATOR_OWNER: u32 = 2147483904u32;
pub const DSOP_DOWNLEVEL_FILTER_DIALUP: u32 = 2147484672u32;
pub const DSOP_DOWNLEVEL_FILTER_EXCLUDE_BUILTIN_GROUPS: u32 = 2147516416u32;
pub const DSOP_DOWNLEVEL_FILTER_GLOBAL_GROUPS: u32 = 2147483652u32;
pub const DSOP_DOWNLEVEL_FILTER_IIS_APP_POOL: u32 = 2214592512u32;
pub const DSOP_DOWNLEVEL_FILTER_INTERACTIVE: u32 = 2147485696u32;
pub const DSOP_DOWNLEVEL_FILTER_INTERNET_USER: u32 = 2149580800u32;
pub const DSOP_DOWNLEVEL_FILTER_LOCAL_ACCOUNTS: u32 = 2415919104u32;
pub const DSOP_DOWNLEVEL_FILTER_LOCAL_GROUPS: u32 = 2147483650u32;
pub const DSOP_DOWNLEVEL_FILTER_LOCAL_LOGON: u32 = 2164260864u32;
pub const DSOP_DOWNLEVEL_FILTER_LOCAL_SERVICE: u32 = 2147745792u32;
pub const DSOP_DOWNLEVEL_FILTER_NETWORK: u32 = 2147487744u32;
pub const DSOP_DOWNLEVEL_FILTER_NETWORK_SERVICE: u32 = 2148007936u32;
pub const DSOP_DOWNLEVEL_FILTER_OWNER_RIGHTS: u32 = 2151677952u32;
pub const DSOP_DOWNLEVEL_FILTER_REMOTE_LOGON: u32 = 2148532224u32;
pub const DSOP_DOWNLEVEL_FILTER_SERVICE: u32 = 2147491840u32;
pub const DSOP_DOWNLEVEL_FILTER_SERVICES: u32 = 2155872256u32;
pub const DSOP_DOWNLEVEL_FILTER_SYSTEM: u32 = 2147500032u32;
pub const DSOP_DOWNLEVEL_FILTER_TERMINAL_SERVER: u32 = 2147549184u32;
pub const DSOP_DOWNLEVEL_FILTER_THIS_ORG_CERT: u32 = 2181038080u32;
pub const DSOP_DOWNLEVEL_FILTER_USERS: u32 = 2147483649u32;
pub const DSOP_DOWNLEVEL_FILTER_WORLD: u32 = 2147483664u32;
pub const DSOP_FILTER_BUILTIN_GROUPS: u32 = 4u32;
pub const DSOP_FILTER_COMPUTERS: u32 = 2048u32;
pub const DSOP_FILTER_CONTACTS: u32 = 1024u32;
pub const DSOP_FILTER_DOMAIN_LOCAL_GROUPS_DL: u32 = 256u32;
pub const DSOP_FILTER_DOMAIN_LOCAL_GROUPS_SE: u32 = 512u32;
pub const DSOP_FILTER_GLOBAL_GROUPS_DL: u32 = 64u32;
pub const DSOP_FILTER_GLOBAL_GROUPS_SE: u32 = 128u32;
pub const DSOP_FILTER_INCLUDE_ADVANCED_VIEW: u32 = 1u32;
pub const DSOP_FILTER_PASSWORDSETTINGS_OBJECTS: u32 = 8192u32;
pub const DSOP_FILTER_SERVICE_ACCOUNTS: u32 = 4096u32;
pub const DSOP_FILTER_UNIVERSAL_GROUPS_DL: u32 = 16u32;
pub const DSOP_FILTER_UNIVERSAL_GROUPS_SE: u32 = 32u32;
pub const DSOP_FILTER_USERS: u32 = 2u32;
pub const DSOP_FILTER_WELL_KNOWN_PRINCIPALS: u32 = 8u32;
pub const DSOP_FLAG_MULTISELECT: u32 = 1u32;
pub const DSOP_FLAG_SKIP_TARGET_COMPUTER_DC_CHECK: u32 = 2u32;
pub const DSOP_SCOPE_FLAG_DEFAULT_FILTER_COMPUTERS: u32 = 256u32;
pub const DSOP_SCOPE_FLAG_DEFAULT_FILTER_CONTACTS: u32 = 512u32;
pub const DSOP_SCOPE_FLAG_DEFAULT_FILTER_GROUPS: u32 = 128u32;
pub const DSOP_SCOPE_FLAG_DEFAULT_FILTER_PASSWORDSETTINGS_OBJECTS: u32 = 2048u32;
pub const DSOP_SCOPE_FLAG_DEFAULT_FILTER_SERVICE_ACCOUNTS: u32 = 1024u32;
pub const DSOP_SCOPE_FLAG_DEFAULT_FILTER_USERS: u32 = 64u32;
pub const DSOP_SCOPE_FLAG_STARTING_SCOPE: u32 = 1u32;
pub const DSOP_SCOPE_FLAG_WANT_DOWNLEVEL_BUILTIN_PATH: u32 = 32u32;
pub const DSOP_SCOPE_FLAG_WANT_PROVIDER_GC: u32 = 8u32;
pub const DSOP_SCOPE_FLAG_WANT_PROVIDER_LDAP: u32 = 4u32;
pub const DSOP_SCOPE_FLAG_WANT_PROVIDER_WINNT: u32 = 2u32;
pub const DSOP_SCOPE_FLAG_WANT_SID_PATH: u32 = 16u32;
pub const DSOP_SCOPE_TYPE_DOWNLEVEL_JOINED_DOMAIN: u32 = 4u32;
pub const DSOP_SCOPE_TYPE_ENTERPRISE_DOMAIN: u32 = 8u32;
pub const DSOP_SCOPE_TYPE_EXTERNAL_DOWNLEVEL_DOMAIN: u32 = 64u32;
pub const DSOP_SCOPE_TYPE_EXTERNAL_UPLEVEL_DOMAIN: u32 = 32u32;
pub const DSOP_SCOPE_TYPE_GLOBAL_CATALOG: u32 = 16u32;
pub const DSOP_SCOPE_TYPE_TARGET_COMPUTER: u32 = 1u32;
pub const DSOP_SCOPE_TYPE_UPLEVEL_JOINED_DOMAIN: u32 = 2u32;
pub const DSOP_SCOPE_TYPE_USER_ENTERED_DOWNLEVEL_SCOPE: u32 = 512u32;
pub const DSOP_SCOPE_TYPE_USER_ENTERED_UPLEVEL_SCOPE: u32 = 256u32;
pub const DSOP_SCOPE_TYPE_WORKGROUP: u32 = 128u32;
pub const DSPROP_ATTRCHANGED_MSG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DsPropAttrChanged");
pub const DSPROVIDER_ADVANCED: u32 = 16u32;
pub const DSPROVIDER_AD_LDS: u32 = 32u32;
pub const DSPROVIDER_UNUSED_0: u32 = 1u32;
pub const DSPROVIDER_UNUSED_1: u32 = 2u32;
pub const DSPROVIDER_UNUSED_2: u32 = 4u32;
pub const DSPROVIDER_UNUSED_3: u32 = 8u32;
pub const DSQPF_ENABLEADMINFEATURES: u32 = 8u32;
pub const DSQPF_ENABLEADVANCEDFEATURES: u32 = 16u32;
pub const DSQPF_HASCREDENTIALS: u32 = 32u32;
pub const DSQPF_NOCHOOSECOLUMNS: u32 = 64u32;
pub const DSQPF_NOSAVE: u32 = 1u32;
pub const DSQPF_SAVELOCATION: u32 = 2u32;
pub const DSQPF_SHOWHIDDENOBJECTS: u32 = 4u32;
pub const DSQPM_GETCLASSLIST: u32 = 268435456u32;
pub const DSQPM_HELPTOPICS: u32 = 268435457u32;
pub const DSROLE_PRIMARY_DOMAIN_GUID_PRESENT: u32 = 16777216u32;
pub const DSROLE_PRIMARY_DS_MIXED_MODE: u32 = 2u32;
pub const DSROLE_PRIMARY_DS_READONLY: u32 = 8u32;
pub const DSROLE_PRIMARY_DS_RUNNING: u32 = 1u32;
pub const DSROLE_UPGRADE_IN_PROGRESS: u32 = 4u32;
pub const DSSSF_DONTSIGNSEAL: u32 = 2u32;
pub const DSSSF_DSAVAILABLE: u32 = 2147483648u32;
pub const DSSSF_SIMPLEAUTHENTICATE: u32 = 1u32;
pub const DS_AVOID_SELF: u32 = 16384u32;
pub const DS_BACKGROUND_ONLY: u32 = 256u32;
pub const DS_BEHAVIOR_LONGHORN: u32 = 3u32;
pub const DS_BEHAVIOR_WIN2000: u32 = 0u32;
pub const DS_BEHAVIOR_WIN2003: u32 = 2u32;
pub const DS_BEHAVIOR_WIN2003_WITH_MIXED_DOMAINS: u32 = 1u32;
pub const DS_BEHAVIOR_WIN2008: u32 = 3u32;
pub const DS_BEHAVIOR_WIN2008R2: u32 = 4u32;
pub const DS_BEHAVIOR_WIN2012: u32 = 5u32;
pub const DS_BEHAVIOR_WIN2012R2: u32 = 6u32;
pub const DS_BEHAVIOR_WIN2016: u32 = 7u32;
pub const DS_BEHAVIOR_WIN7: u32 = 4u32;
pub const DS_BEHAVIOR_WIN8: u32 = 5u32;
pub const DS_BEHAVIOR_WINBLUE: u32 = 6u32;
pub const DS_BEHAVIOR_WINTHRESHOLD: u32 = 7u32;
pub const DS_CANONICAL_NAME: DS_NAME_FORMAT = 7i32;
pub const DS_CANONICAL_NAME_EX: DS_NAME_FORMAT = 9i32;
pub const DS_CLOSEST_FLAG: u32 = 128u32;
pub const DS_DIRECTORY_SERVICE_10_REQUIRED: u32 = 8388608u32;
pub const DS_DIRECTORY_SERVICE_6_REQUIRED: u32 = 524288u32;
pub const DS_DIRECTORY_SERVICE_8_REQUIRED: u32 = 2097152u32;
pub const DS_DIRECTORY_SERVICE_9_REQUIRED: u32 = 4194304u32;
pub const DS_DIRECTORY_SERVICE_PREFERRED: u32 = 32u32;
pub const DS_DIRECTORY_SERVICE_REQUIRED: u32 = 16u32;
pub const DS_DISPLAY_NAME: DS_NAME_FORMAT = 3i32;
pub const DS_DNS_CONTROLLER_FLAG: u32 = 536870912u32;
pub const DS_DNS_DOMAIN_FLAG: u32 = 1073741824u32;
pub const DS_DNS_DOMAIN_NAME: DS_NAME_FORMAT = 12i32;
pub const DS_DNS_FOREST_FLAG: u32 = 2147483648u32;
pub const DS_DOMAIN_DIRECT_INBOUND: u32 = 32u32;
pub const DS_DOMAIN_DIRECT_OUTBOUND: u32 = 2u32;
pub const DS_DOMAIN_IN_FOREST: u32 = 1u32;
pub const DS_DOMAIN_NATIVE_MODE: u32 = 16u32;
pub const DS_DOMAIN_PRIMARY: u32 = 8u32;
pub const DS_DOMAIN_TREE_ROOT: u32 = 4u32;
pub const DS_DS_10_FLAG: u32 = 65536u32;
pub const DS_DS_8_FLAG: u32 = 16384u32;
pub const DS_DS_9_FLAG: u32 = 32768u32;
pub const DS_DS_FLAG: u32 = 16u32;
pub const DS_EXIST_ADVISORY_MODE: u32 = 1u32;
pub const DS_FORCE_REDISCOVERY: u32 = 1u32;
pub const DS_FQDN_1779_NAME: DS_NAME_FORMAT = 1i32;
pub const DS_FULL_SECRET_DOMAIN_6_FLAG: u32 = 4096u32;
pub const DS_GC_FLAG: u32 = 4u32;
pub const DS_GC_SERVER_REQUIRED: u32 = 64u32;
pub const DS_GFTI_UPDATE_TDO: u32 = 1u32;
pub const DS_GFTI_VALID_FLAGS: u32 = 1u32;
pub const DS_GOOD_TIMESERV_FLAG: u32 = 512u32;
pub const DS_GOOD_TIMESERV_PREFERRED: u32 = 8192u32;
pub const DS_INSTANCETYPE_IS_NC_HEAD: u32 = 1u32;
pub const DS_INSTANCETYPE_NC_COMING: u32 = 16u32;
pub const DS_INSTANCETYPE_NC_GOING: u32 = 32u32;
pub const DS_INSTANCETYPE_NC_IS_WRITEABLE: u32 = 4u32;
pub const DS_IP_REQUIRED: u32 = 512u32;
pub const DS_IS_DNS_NAME: u32 = 131072u32;
pub const DS_IS_FLAT_NAME: u32 = 65536u32;
pub const DS_KCC_FLAG_ASYNC_OP: u32 = 1u32;
pub const DS_KCC_FLAG_DAMPED: u32 = 2u32;
pub const DS_KCC_TASKID_UPDATE_TOPOLOGY: DS_KCC_TASKID = 0i32;
pub const DS_KDC_FLAG: u32 = 32u32;
pub const DS_KDC_REQUIRED: u32 = 1024u32;
pub const DS_KEY_LIST_FLAG: u32 = 131072u32;
pub const DS_KEY_LIST_SUPPORT_REQUIRED: u32 = 16777216u32;
pub const DS_LDAP_FLAG: u32 = 8u32;
pub const DS_LIST_ACCOUNT_OBJECT_FOR_SERVER: u32 = 2u32;
pub const DS_LIST_DNS_HOST_NAME_FOR_SERVER: u32 = 1u32;
pub const DS_LIST_DSA_OBJECT_FOR_SERVER: u32 = 0u32;
pub const DS_MANGLE_OBJECT_RDN_FOR_DELETION: DS_MANGLE_FOR = 1i32;
pub const DS_MANGLE_OBJECT_RDN_FOR_NAME_CONFLICT: DS_MANGLE_FOR = 2i32;
pub const DS_MANGLE_UNKNOWN: DS_MANGLE_FOR = 0i32;
pub const DS_NAME_ERROR_DOMAIN_ONLY: DS_NAME_ERROR = 5i32;
pub const DS_NAME_ERROR_NOT_FOUND: DS_NAME_ERROR = 2i32;
pub const DS_NAME_ERROR_NOT_UNIQUE: DS_NAME_ERROR = 3i32;
pub const DS_NAME_ERROR_NO_MAPPING: DS_NAME_ERROR = 4i32;
pub const DS_NAME_ERROR_NO_SYNTACTICAL_MAPPING: DS_NAME_ERROR = 6i32;
pub const DS_NAME_ERROR_RESOLVING: DS_NAME_ERROR = 1i32;
pub const DS_NAME_ERROR_TRUST_REFERRAL: DS_NAME_ERROR = 7i32;
pub const DS_NAME_FLAG_EVAL_AT_DC: DS_NAME_FLAGS = 2i32;
pub const DS_NAME_FLAG_GCVERIFY: DS_NAME_FLAGS = 4i32;
pub const DS_NAME_FLAG_SYNTACTICAL_ONLY: DS_NAME_FLAGS = 1i32;
pub const DS_NAME_FLAG_TRUST_REFERRAL: DS_NAME_FLAGS = 8i32;
pub const DS_NAME_NO_ERROR: DS_NAME_ERROR = 0i32;
pub const DS_NAME_NO_FLAGS: DS_NAME_FLAGS = 0i32;
pub const DS_NDNC_FLAG: u32 = 1024u32;
pub const DS_NOTIFY_AFTER_SITE_RECORDS: u32 = 2u32;
pub const DS_NT4_ACCOUNT_NAME: DS_NAME_FORMAT = 2i32;
pub const DS_ONLY_DO_SITE_NAME: u32 = 1u32;
pub const DS_ONLY_LDAP_NEEDED: u32 = 32768u32;
pub const DS_PDC_FLAG: u32 = 1u32;
pub const DS_PDC_REQUIRED: u32 = 128u32;
pub const DS_PING_FLAGS: u32 = 1048575u32;
pub const DS_PROP_ADMIN_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("admin");
pub const DS_PROP_SHELL_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("shell");
pub const DS_REPADD_ASYNCHRONOUS_OPERATION: u32 = 1u32;
pub const DS_REPADD_ASYNCHRONOUS_REPLICA: u32 = 32u32;
pub const DS_REPADD_CRITICAL: u32 = 2048u32;
pub const DS_REPADD_DISABLE_NOTIFICATION: u32 = 64u32;
pub const DS_REPADD_DISABLE_PERIODIC: u32 = 128u32;
pub const DS_REPADD_INITIAL: u32 = 4u32;
pub const DS_REPADD_INTERSITE_MESSAGING: u32 = 16u32;
pub const DS_REPADD_NEVER_NOTIFY: u32 = 512u32;
pub const DS_REPADD_NONGC_RO_REPLICA: u32 = 16777216u32;
pub const DS_REPADD_PERIODIC: u32 = 8u32;
pub const DS_REPADD_SELECT_SECRETS: u32 = 4096u32;
pub const DS_REPADD_TWO_WAY: u32 = 1024u32;
pub const DS_REPADD_USE_COMPRESSION: u32 = 256u32;
pub const DS_REPADD_WRITEABLE: u32 = 2u32;
pub const DS_REPDEL_ASYNCHRONOUS_OPERATION: u32 = 1u32;
pub const DS_REPDEL_IGNORE_ERRORS: u32 = 8u32;
pub const DS_REPDEL_INTERSITE_MESSAGING: u32 = 4u32;
pub const DS_REPDEL_LOCAL_ONLY: u32 = 16u32;
pub const DS_REPDEL_NO_SOURCE: u32 = 32u32;
pub const DS_REPDEL_REF_OK: u32 = 64u32;
pub const DS_REPDEL_WRITEABLE: u32 = 2u32;
pub const DS_REPL_INFO_CURSORS_2_FOR_NC: DS_REPL_INFO_TYPE = 7i32;
pub const DS_REPL_INFO_CURSORS_3_FOR_NC: DS_REPL_INFO_TYPE = 8i32;
pub const DS_REPL_INFO_CURSORS_FOR_NC: DS_REPL_INFO_TYPE = 1i32;
pub const DS_REPL_INFO_FLAG_IMPROVE_LINKED_ATTRS: u32 = 1u32;
pub const DS_REPL_INFO_KCC_DSA_CONNECT_FAILURES: DS_REPL_INFO_TYPE = 3i32;
pub const DS_REPL_INFO_KCC_DSA_LINK_FAILURES: DS_REPL_INFO_TYPE = 4i32;
pub const DS_REPL_INFO_METADATA_2_FOR_ATTR_VALUE: DS_REPL_INFO_TYPE = 10i32;
pub const DS_REPL_INFO_METADATA_2_FOR_OBJ: DS_REPL_INFO_TYPE = 9i32;
pub const DS_REPL_INFO_METADATA_EXT_FOR_ATTR_VALUE: DS_REPL_INFO_TYPE = 11i32;
pub const DS_REPL_INFO_METADATA_FOR_ATTR_VALUE: DS_REPL_INFO_TYPE = 6i32;
pub const DS_REPL_INFO_METADATA_FOR_OBJ: DS_REPL_INFO_TYPE = 2i32;
pub const DS_REPL_INFO_NEIGHBORS: DS_REPL_INFO_TYPE = 0i32;
pub const DS_REPL_INFO_PENDING_OPS: DS_REPL_INFO_TYPE = 5i32;
pub const DS_REPL_INFO_TYPE_MAX: DS_REPL_INFO_TYPE = 12i32;
pub const DS_REPL_NBR_COMPRESS_CHANGES: u32 = 268435456u32;
pub const DS_REPL_NBR_DISABLE_SCHEDULED_SYNC: u32 = 134217728u32;
pub const DS_REPL_NBR_DO_SCHEDULED_SYNCS: u32 = 64u32;
pub const DS_REPL_NBR_FULL_SYNC_IN_PROGRESS: u32 = 65536u32;
pub const DS_REPL_NBR_FULL_SYNC_NEXT_PACKET: u32 = 131072u32;
pub const DS_REPL_NBR_GCSPN: u32 = 1048576u32;
pub const DS_REPL_NBR_IGNORE_CHANGE_NOTIFICATIONS: u32 = 67108864u32;
pub const DS_REPL_NBR_NEVER_SYNCED: u32 = 2097152u32;
pub const DS_REPL_NBR_NONGC_RO_REPLICA: u32 = 1024u32;
pub const DS_REPL_NBR_NO_CHANGE_NOTIFICATIONS: u32 = 536870912u32;
pub const DS_REPL_NBR_PARTIAL_ATTRIBUTE_SET: u32 = 1073741824u32;
pub const DS_REPL_NBR_PREEMPTED: u32 = 16777216u32;
pub const DS_REPL_NBR_RETURN_OBJECT_PARENTS: u32 = 2048u32;
pub const DS_REPL_NBR_SELECT_SECRETS: u32 = 4096u32;
pub const DS_REPL_NBR_SYNC_ON_STARTUP: u32 = 32u32;
pub const DS_REPL_NBR_TWO_WAY_SYNC: u32 = 512u32;
pub const DS_REPL_NBR_USE_ASYNC_INTERSITE_TRANSPORT: u32 = 128u32;
pub const DS_REPL_NBR_WRITEABLE: u32 = 16u32;
pub const DS_REPL_OP_TYPE_ADD: DS_REPL_OP_TYPE = 1i32;
pub const DS_REPL_OP_TYPE_DELETE: DS_REPL_OP_TYPE = 2i32;
pub const DS_REPL_OP_TYPE_MODIFY: DS_REPL_OP_TYPE = 3i32;
pub const DS_REPL_OP_TYPE_SYNC: DS_REPL_OP_TYPE = 0i32;
pub const DS_REPL_OP_TYPE_UPDATE_REFS: DS_REPL_OP_TYPE = 4i32;
pub const DS_REPMOD_ASYNCHRONOUS_OPERATION: u32 = 1u32;
pub const DS_REPMOD_UPDATE_ADDRESS: u32 = 2u32;
pub const DS_REPMOD_UPDATE_FLAGS: u32 = 1u32;
pub const DS_REPMOD_UPDATE_INSTANCE: u32 = 2u32;
pub const DS_REPMOD_UPDATE_RESULT: u32 = 8u32;
pub const DS_REPMOD_UPDATE_SCHEDULE: u32 = 4u32;
pub const DS_REPMOD_UPDATE_TRANSPORT: u32 = 16u32;
pub const DS_REPMOD_WRITEABLE: u32 = 2u32;
pub const DS_REPSYNCALL_ABORT_IF_SERVER_UNAVAILABLE: u32 = 1u32;
pub const DS_REPSYNCALL_CROSS_SITE_BOUNDARIES: u32 = 64u32;
pub const DS_REPSYNCALL_DO_NOT_SYNC: u32 = 8u32;
pub const DS_REPSYNCALL_EVENT_ERROR: DS_REPSYNCALL_EVENT = 0i32;
pub const DS_REPSYNCALL_EVENT_FINISHED: DS_REPSYNCALL_EVENT = 3i32;
pub const DS_REPSYNCALL_EVENT_SYNC_COMPLETED: DS_REPSYNCALL_EVENT = 2i32;
pub const DS_REPSYNCALL_EVENT_SYNC_STARTED: DS_REPSYNCALL_EVENT = 1i32;
pub const DS_REPSYNCALL_ID_SERVERS_BY_DN: u32 = 4u32;
pub const DS_REPSYNCALL_NO_OPTIONS: u32 = 0u32;
pub const DS_REPSYNCALL_PUSH_CHANGES_OUTWARD: u32 = 32u32;
pub const DS_REPSYNCALL_SERVER_UNREACHABLE: DS_REPSYNCALL_ERROR = 2i32;
pub const DS_REPSYNCALL_SKIP_INITIAL_CHECK: u32 = 16u32;
pub const DS_REPSYNCALL_SYNC_ADJACENT_SERVERS_ONLY: u32 = 2u32;
pub const DS_REPSYNCALL_WIN32_ERROR_CONTACTING_SERVER: DS_REPSYNCALL_ERROR = 0i32;
pub const DS_REPSYNCALL_WIN32_ERROR_REPLICATING: DS_REPSYNCALL_ERROR = 1i32;
pub const DS_REPSYNC_ABANDONED: u32 = 32768u32;
pub const DS_REPSYNC_ADD_REFERENCE: u32 = 512u32;
pub const DS_REPSYNC_ASYNCHRONOUS_OPERATION: u32 = 1u32;
pub const DS_REPSYNC_ASYNCHRONOUS_REPLICA: u32 = 1048576u32;
pub const DS_REPSYNC_CRITICAL: u32 = 2097152u32;
pub const DS_REPSYNC_FORCE: u32 = 256u32;
pub const DS_REPSYNC_FULL: u32 = 32u32;
pub const DS_REPSYNC_FULL_IN_PROGRESS: u32 = 4194304u32;
pub const DS_REPSYNC_INITIAL: u32 = 8192u32;
pub const DS_REPSYNC_INITIAL_IN_PROGRESS: u32 = 65536u32;
pub const DS_REPSYNC_INTERSITE_MESSAGING: u32 = 8u32;
pub const DS_REPSYNC_NEVER_COMPLETED: u32 = 1024u32;
pub const DS_REPSYNC_NEVER_NOTIFY: u32 = 4096u32;
pub const DS_REPSYNC_NONGC_RO_REPLICA: u32 = 16777216u32;
pub const DS_REPSYNC_NOTIFICATION: u32 = 524288u32;
pub const DS_REPSYNC_NO_DISCARD: u32 = 128u32;
pub const DS_REPSYNC_PARTIAL_ATTRIBUTE_SET: u32 = 131072u32;
pub const DS_REPSYNC_PERIODIC: u32 = 4u32;
pub const DS_REPSYNC_PREEMPTED: u32 = 8388608u32;
pub const DS_REPSYNC_REQUEUE: u32 = 262144u32;
pub const DS_REPSYNC_SELECT_SECRETS: u32 = 32768u32;
pub const DS_REPSYNC_TWO_WAY: u32 = 2048u32;
pub const DS_REPSYNC_URGENT: u32 = 64u32;
pub const DS_REPSYNC_USE_COMPRESSION: u32 = 16384u32;
pub const DS_REPSYNC_WRITEABLE: u32 = 2u32;
pub const DS_REPUPD_ADD_REFERENCE: u32 = 4u32;
pub const DS_REPUPD_ASYNCHRONOUS_OPERATION: u32 = 1u32;
pub const DS_REPUPD_DELETE_REFERENCE: u32 = 8u32;
pub const DS_REPUPD_REFERENCE_GCSPN: u32 = 16u32;
pub const DS_REPUPD_WRITEABLE: u32 = 2u32;
pub const DS_RETURN_DNS_NAME: u32 = 1073741824u32;
pub const DS_RETURN_FLAT_NAME: u32 = 2147483648u32;
pub const DS_ROLE_DOMAIN_OWNER: u32 = 1u32;
pub const DS_ROLE_INFRASTRUCTURE_OWNER: u32 = 4u32;
pub const DS_ROLE_PDC_OWNER: u32 = 2u32;
pub const DS_ROLE_RID_OWNER: u32 = 3u32;
pub const DS_ROLE_SCHEMA_OWNER: u32 = 0u32;
pub const DS_SCHEMA_GUID_ATTR: u32 = 1u32;
pub const DS_SCHEMA_GUID_ATTR_SET: u32 = 2u32;
pub const DS_SCHEMA_GUID_CLASS: u32 = 3u32;
pub const DS_SCHEMA_GUID_CONTROL_RIGHT: u32 = 4u32;
pub const DS_SCHEMA_GUID_NOT_FOUND: u32 = 0u32;
pub const DS_SELECT_SECRET_DOMAIN_6_FLAG: u32 = 2048u32;
pub const DS_SERVICE_PRINCIPAL_NAME: DS_NAME_FORMAT = 10i32;
pub const DS_SID_OR_SID_HISTORY_NAME: DS_NAME_FORMAT = 11i32;
pub const DS_SPN_ADD_SPN_OP: DS_SPN_WRITE_OP = 0i32;
pub const DS_SPN_DELETE_SPN_OP: DS_SPN_WRITE_OP = 2i32;
pub const DS_SPN_DNS_HOST: DS_SPN_NAME_TYPE = 0i32;
pub const DS_SPN_DN_HOST: DS_SPN_NAME_TYPE = 1i32;
pub const DS_SPN_DOMAIN: DS_SPN_NAME_TYPE = 3i32;
pub const DS_SPN_NB_DOMAIN: DS_SPN_NAME_TYPE = 4i32;
pub const DS_SPN_NB_HOST: DS_SPN_NAME_TYPE = 2i32;
pub const DS_SPN_REPLACE_SPN_OP: DS_SPN_WRITE_OP = 1i32;
pub const DS_SPN_SERVICE: DS_SPN_NAME_TYPE = 5i32;
pub const DS_SYNCED_EVENT_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("NTDSInitialSyncsCompleted");
pub const DS_SYNCED_EVENT_NAME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTDSInitialSyncsCompleted");
pub const DS_TIMESERV_FLAG: u32 = 64u32;
pub const DS_TIMESERV_REQUIRED: u32 = 2048u32;
pub const DS_TRY_NEXTCLOSEST_SITE: u32 = 262144u32;
pub const DS_UNIQUE_ID_NAME: DS_NAME_FORMAT = 6i32;
pub const DS_UNKNOWN_NAME: DS_NAME_FORMAT = 0i32;
pub const DS_USER_PRINCIPAL_NAME: DS_NAME_FORMAT = 8i32;
pub const DS_WEB_SERVICE_REQUIRED: u32 = 1048576u32;
pub const DS_WRITABLE_FLAG: u32 = 256u32;
pub const DS_WRITABLE_REQUIRED: u32 = 4096u32;
pub const DS_WS_FLAG: u32 = 8192u32;
pub const DsRoleOperationActive: DSROLE_OPERATION_STATE = 1i32;
pub const DsRoleOperationIdle: DSROLE_OPERATION_STATE = 0i32;
pub const DsRoleOperationNeedReboot: DSROLE_OPERATION_STATE = 2i32;
pub const DsRoleOperationState: DSROLE_PRIMARY_DOMAIN_INFO_LEVEL = 3i32;
pub const DsRolePrimaryDomainInfoBasic: DSROLE_PRIMARY_DOMAIN_INFO_LEVEL = 1i32;
pub const DsRoleServerBackup: DSROLE_SERVER_STATE = 2i32;
pub const DsRoleServerPrimary: DSROLE_SERVER_STATE = 1i32;
pub const DsRoleServerUnknown: DSROLE_SERVER_STATE = 0i32;
pub const DsRoleUpgradeStatus: DSROLE_PRIMARY_DOMAIN_INFO_LEVEL = 2i32;
pub const DsRole_RoleBackupDomainController: DSROLE_MACHINE_ROLE = 4i32;
pub const DsRole_RoleMemberServer: DSROLE_MACHINE_ROLE = 3i32;
pub const DsRole_RoleMemberWorkstation: DSROLE_MACHINE_ROLE = 1i32;
pub const DsRole_RolePrimaryDomainController: DSROLE_MACHINE_ROLE = 5i32;
pub const DsRole_RoleStandaloneServer: DSROLE_MACHINE_ROLE = 2i32;
pub const DsRole_RoleStandaloneWorkstation: DSROLE_MACHINE_ROLE = 0i32;
pub const Email: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8f92a857_478e_11d1_a3b4_00c04fb950dc);
pub const FACILITY_BACKUP: u32 = 2047u32;
pub const FACILITY_NTDSB: u32 = 2048u32;
pub const FACILITY_SYSTEM: u32 = 0u32;
pub const FLAG_DISABLABLE_OPTIONAL_FEATURE: u32 = 4u32;
pub const FLAG_DOMAIN_OPTIONAL_FEATURE: u32 = 2u32;
pub const FLAG_FOREST_OPTIONAL_FEATURE: u32 = 1u32;
pub const FLAG_SERVER_OPTIONAL_FEATURE: u32 = 8u32;
pub const FRSCONN_MAX_PRIORITY: u32 = 8u32;
pub const FRSCONN_PRIORITY_MASK: u32 = 1879048192u32;
pub const FaxNumber: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa5062215_4681_11d1_a3b4_00c04fb950dc);
pub const GUID_COMPUTRS_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("aa312825768811d1aded00c04fd8d5cd");
pub const GUID_COMPUTRS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("aa312825768811d1aded00c04fd8d5cd");
pub const GUID_DELETED_OBJECTS_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("18e2ea80684f11d2b9aa00c04f79f805");
pub const GUID_DELETED_OBJECTS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("18e2ea80684f11d2b9aa00c04f79f805");
pub const GUID_DOMAIN_CONTROLLERS_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("a361b2ffffd211d1aa4b00c04fd7d83a");
pub const GUID_DOMAIN_CONTROLLERS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("a361b2ffffd211d1aa4b00c04fd7d83a");
pub const GUID_FOREIGNSECURITYPRINCIPALS_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("22b70c67d56e4efb91e9300fca3dc1aa");
pub const GUID_FOREIGNSECURITYPRINCIPALS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("22b70c67d56e4efb91e9300fca3dc1aa");
pub const GUID_INFRASTRUCTURE_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("2fbac1870ade11d297c400c04fd8d5cd");
pub const GUID_INFRASTRUCTURE_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("2fbac1870ade11d297c400c04fd8d5cd");
pub const GUID_KEYS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("683A24E2E8164BD3AF86AC3C2CF3F981");
pub const GUID_LOSTANDFOUND_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ab8153b7768811d1aded00c04fd8d5cd");
pub const GUID_LOSTANDFOUND_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ab8153b7768811d1aded00c04fd8d5cd");
pub const GUID_MANAGED_SERVICE_ACCOUNTS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1EB93889E40C45DF9F0C64D23BBB6237");
pub const GUID_MICROSOFT_PROGRAM_DATA_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("f4be92a4c777485e878e9421d53087db");
pub const GUID_MICROSOFT_PROGRAM_DATA_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("f4be92a4c777485e878e9421d53087db");
pub const GUID_NTDS_QUOTAS_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("6227f0af1fc2410d8e3bb10615bb5b0f");
pub const GUID_NTDS_QUOTAS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("6227f0af1fc2410d8e3bb10615bb5b0f");
pub const GUID_PRIVILEGED_ACCESS_MANAGEMENT_OPTIONAL_FEATURE_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("73e843ece8cc4046b4ab07ffe4ab5bcd");
pub const GUID_PRIVILEGED_ACCESS_MANAGEMENT_OPTIONAL_FEATURE_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("73e843ece8cc4046b4ab07ffe4ab5bcd");
pub const GUID_PROGRAM_DATA_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("09460c08ae1e4a4ea0f64aee7daa1e5a");
pub const GUID_PROGRAM_DATA_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("09460c08ae1e4a4ea0f64aee7daa1e5a");
pub const GUID_RECYCLE_BIN_OPTIONAL_FEATURE_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("d8dc6d76d0ac5e44f3b9a7f9b6744f2a");
pub const GUID_RECYCLE_BIN_OPTIONAL_FEATURE_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("d8dc6d76d0ac5e44f3b9a7f9b6744f2a");
pub const GUID_SYSTEMS_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ab1d30f3768811d1aded00c04fd8d5cd");
pub const GUID_SYSTEMS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ab1d30f3768811d1aded00c04fd8d5cd");
pub const GUID_USERS_CONTAINER_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("a9d1ca15768811d1aded00c04fd8d5cd");
pub const GUID_USERS_CONTAINER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("a9d1ca15768811d1aded00c04fd8d5cd");
pub const Hold: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb3ad3e13_4080_11d1_a3ac_00c04fb950dc);
pub const LargeInteger: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x927971f5_0939_11d1_8be1_00c04fd8d503);
pub const NTDSAPI_BIND_ALLOW_DELEGATION: u32 = 1u32;
pub const NTDSAPI_BIND_FIND_BINDING: u32 = 2u32;
pub const NTDSAPI_BIND_FORCE_KERBEROS: u32 = 4u32;
pub const NTDSCONN_KCC_GC_TOPOLOGY: u32 = 1u32;
pub const NTDSCONN_KCC_INTERSITE_GC_TOPOLOGY: u32 = 32u32;
pub const NTDSCONN_KCC_INTERSITE_TOPOLOGY: u32 = 64u32;
pub const NTDSCONN_KCC_MINIMIZE_HOPS_TOPOLOGY: u32 = 4u32;
pub const NTDSCONN_KCC_NO_REASON: u32 = 0u32;
pub const NTDSCONN_KCC_OSCILLATING_CONNECTION_TOPOLOGY: u32 = 16u32;
pub const NTDSCONN_KCC_REDUNDANT_SERVER_TOPOLOGY: u32 = 512u32;
pub const NTDSCONN_KCC_RING_TOPOLOGY: u32 = 2u32;
pub const NTDSCONN_KCC_SERVER_FAILOVER_TOPOLOGY: u32 = 128u32;
pub const NTDSCONN_KCC_SITE_FAILOVER_TOPOLOGY: u32 = 256u32;
pub const NTDSCONN_KCC_STALE_SERVERS_TOPOLOGY: u32 = 8u32;
pub const NTDSCONN_OPT_DISABLE_INTERSITE_COMPRESSION: u32 = 16u32;
pub const NTDSCONN_OPT_IGNORE_SCHEDULE_MASK: u32 = 2147483648u32;
pub const NTDSCONN_OPT_IS_GENERATED: u32 = 1u32;
pub const NTDSCONN_OPT_OVERRIDE_NOTIFY_DEFAULT: u32 = 4u32;
pub const NTDSCONN_OPT_RODC_TOPOLOGY: u32 = 64u32;
pub const NTDSCONN_OPT_TWOWAY_SYNC: u32 = 2u32;
pub const NTDSCONN_OPT_USER_OWNED_SCHEDULE: u32 = 32u32;
pub const NTDSCONN_OPT_USE_NOTIFY: u32 = 8u32;
pub const NTDSDSA_OPT_BLOCK_RPC: u32 = 64u32;
pub const NTDSDSA_OPT_DISABLE_INBOUND_REPL: u32 = 2u32;
pub const NTDSDSA_OPT_DISABLE_NTDSCONN_XLATE: u32 = 8u32;
pub const NTDSDSA_OPT_DISABLE_OUTBOUND_REPL: u32 = 4u32;
pub const NTDSDSA_OPT_DISABLE_SPN_REGISTRATION: u32 = 16u32;
pub const NTDSDSA_OPT_GENERATE_OWN_TOPO: u32 = 32u32;
pub const NTDSDSA_OPT_IS_GC: u32 = 1u32;
pub const NTDSSETTINGS_DEFAULT_SERVER_REDUNDANCY: u32 = 2u32;
pub const NTDSSETTINGS_OPT_FORCE_KCC_W2K_ELECTION: u32 = 128u32;
pub const NTDSSETTINGS_OPT_FORCE_KCC_WHISTLER_BEHAVIOR: u32 = 64u32;
pub const NTDSSETTINGS_OPT_IS_AUTO_TOPOLOGY_DISABLED: u32 = 1u32;
pub const NTDSSETTINGS_OPT_IS_GROUP_CACHING_ENABLED: u32 = 32u32;
pub const NTDSSETTINGS_OPT_IS_INTER_SITE_AUTO_TOPOLOGY_DISABLED: u32 = 16u32;
pub const NTDSSETTINGS_OPT_IS_RAND_BH_SELECTION_DISABLED: u32 = 256u32;
pub const NTDSSETTINGS_OPT_IS_REDUNDANT_SERVER_TOPOLOGY_ENABLED: u32 = 1024u32;
pub const NTDSSETTINGS_OPT_IS_SCHEDULE_HASHING_ENABLED: u32 = 512u32;
pub const NTDSSETTINGS_OPT_IS_TOPL_CLEANUP_DISABLED: u32 = 2u32;
pub const NTDSSETTINGS_OPT_IS_TOPL_DETECT_STALE_DISABLED: u32 = 8u32;
pub const NTDSSETTINGS_OPT_IS_TOPL_MIN_HOPS_DISABLED: u32 = 4u32;
pub const NTDSSETTINGS_OPT_W2K3_BRIDGES_REQUIRED: u32 = 4096u32;
pub const NTDSSETTINGS_OPT_W2K3_IGNORE_SCHEDULES: u32 = 2048u32;
pub const NTDSSITECONN_OPT_DISABLE_COMPRESSION: u32 = 4u32;
pub const NTDSSITECONN_OPT_TWOWAY_SYNC: u32 = 2u32;
pub const NTDSSITECONN_OPT_USE_NOTIFY: u32 = 1u32;
pub const NTDSSITELINK_OPT_DISABLE_COMPRESSION: u32 = 4u32;
pub const NTDSSITELINK_OPT_TWOWAY_SYNC: u32 = 2u32;
pub const NTDSSITELINK_OPT_USE_NOTIFY: u32 = 1u32;
pub const NTDSTRANSPORT_OPT_BRIDGES_REQUIRED: u32 = 2u32;
pub const NTDSTRANSPORT_OPT_IGNORE_SCHEDULES: u32 = 1u32;
pub const NameTranslate: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x274fae1f_3626_11d1_a3a4_00c04fb950dc);
pub const NetAddress: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb0b71247_4080_11d1_a3ac_00c04fb950dc);
pub const OQWF_DEFAULTFORM: u32 = 2u32;
pub const OQWF_HIDEMENUS: u32 = 1024u32;
pub const OQWF_HIDESEARCHUI: u32 = 2048u32;
pub const OQWF_ISSUEONOPEN: u32 = 64u32;
pub const OQWF_LOADQUERY: u32 = 8u32;
pub const OQWF_OKCANCEL: u32 = 1u32;
pub const OQWF_PARAMISPROPERTYBAG: u32 = 2147483648u32;
pub const OQWF_REMOVEFORMS: u32 = 32u32;
pub const OQWF_REMOVESCOPES: u32 = 16u32;
pub const OQWF_SAVEQUERYONOK: u32 = 512u32;
pub const OQWF_SHOWOPTIONAL: u32 = 128u32;
pub const OQWF_SINGLESELECT: u32 = 4u32;
pub const OctetList: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1241400f_4680_11d1_a3b4_00c04fb950dc);
pub const Path: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb2538919_4080_11d1_a3ac_00c04fb950dc);
pub const Pathname: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x080d0d78_f421_11d0_a36e_00c04fb950dc);
pub const PostalAddress: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0a75afcd_4680_11d1_a3b4_00c04fb950dc);
pub const PropertyEntry: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x72d3edc2_a4c4_11d0_8533_00c04fd8d503);
pub const PropertyValue: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7b9e38b0_a97c_11d0_8534_00c04fd8d503);
pub const QUERYFORM_CHANGESFORMLIST: u64 = 1u64;
pub const QUERYFORM_CHANGESOPTFORMLIST: u64 = 2u64;
pub const ReplicaPointer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf5d1badf_4080_11d1_a3ac_00c04fb950dc);
pub const SCHEDULE_BANDWIDTH: u32 = 1u32;
pub const SCHEDULE_INTERVAL: u32 = 0u32;
pub const SCHEDULE_PRIORITY: u32 = 2u32;
pub const SecurityDescriptor: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb958f73c_9bdd_11d0_852c_00c04fd8d503);
pub const Timestamp: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb2bed2eb_4080_11d1_a3ac_00c04fb950dc);
pub const TypedName: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb33143cb_4080_11d1_a3ac_00c04fb950dc);
pub const WM_ADSPROP_NOTIFY_APPLY: u32 = 2128u32;
pub const WM_ADSPROP_NOTIFY_CHANGE: u32 = 2127u32;
pub const WM_ADSPROP_NOTIFY_ERROR: u32 = 2134u32;
pub const WM_ADSPROP_NOTIFY_EXIT: u32 = 2131u32;
pub const WM_ADSPROP_NOTIFY_FOREGROUND: u32 = 2130u32;
pub const WM_ADSPROP_NOTIFY_PAGEHWND: u32 = 2126u32;
pub const WM_ADSPROP_NOTIFY_PAGEINIT: u32 = 2125u32;
pub const WM_ADSPROP_NOTIFY_SETFOCUS: u32 = 2129u32;
pub const WinNTSystemInfo: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x66182ec4_afd1_11d2_9cb9_0000f87a369e);
pub const hrAccessDenied: ::windows_sys::core::HRESULT = -939522189i32;
pub const hrAfterInitialization: ::windows_sys::core::HRESULT = -939522246i32;
pub const hrAlreadyInitialized: ::windows_sys::core::HRESULT = -939523066i32;
pub const hrAlreadyOpen: ::windows_sys::core::HRESULT = -939589627i32;
pub const hrAlreadyPrepared: ::windows_sys::core::HRESULT = -939522489i32;
pub const hrBFInUse: ::windows_sys::core::HRESULT = -939523894i32;
pub const hrBFNotSynchronous: ::windows_sys::core::HRESULT = -2013265720i32;
pub const hrBFPageNotFound: ::windows_sys::core::HRESULT = -2013265719i32;
pub const hrBackupDirectoryNotEmpty: ::windows_sys::core::HRESULT = -939523592i32;
pub const hrBackupInProgress: ::windows_sys::core::HRESULT = -939523591i32;
pub const hrBackupNotAllowedYet: ::windows_sys::core::HRESULT = -939523573i32;
pub const hrBadBackupDatabaseSize: ::windows_sys::core::HRESULT = -939523535i32;
pub const hrBadCheckpointSignature: ::windows_sys::core::HRESULT = -939523564i32;
pub const hrBadColumnId: ::windows_sys::core::HRESULT = -939522579i32;
pub const hrBadDbSignature: ::windows_sys::core::HRESULT = -939523565i32;
pub const hrBadItagSequence: ::windows_sys::core::HRESULT = -939522578i32;
pub const hrBadLogSignature: ::windows_sys::core::HRESULT = -939523566i32;
pub const hrBadLogVersion: ::windows_sys::core::HRESULT = -939523582i32;
pub const hrBufferTooSmall: ::windows_sys::core::HRESULT = -939523058i32;
pub const hrBufferTruncated: ::windows_sys::core::HRESULT = -2013264914i32;
pub const hrCannotBeTagged: ::windows_sys::core::HRESULT = -939522575i32;
pub const hrCannotRename: ::windows_sys::core::HRESULT = -939522790i32;
pub const hrCheckpointCorrupt: ::windows_sys::core::HRESULT = -939523563i32;
pub const hrCircularLogging: ::windows_sys::core::HRESULT = -939589621i32;
pub const hrColumn2ndSysMaint: ::windows_sys::core::HRESULT = -939522586i32;
pub const hrColumnCannotIndex: ::windows_sys::core::HRESULT = -939522583i32;
pub const hrColumnDoesNotFit: ::windows_sys::core::HRESULT = -939522593i32;
pub const hrColumnDuplicate: ::windows_sys::core::HRESULT = -939522588i32;
pub const hrColumnInUse: ::windows_sys::core::HRESULT = -939523050i32;
pub const hrColumnIndexed: ::windows_sys::core::HRESULT = -939522591i32;
pub const hrColumnLong: ::windows_sys::core::HRESULT = -939522595i32;
pub const hrColumnMaxTruncated: ::windows_sys::core::HRESULT = -2013264408i32;
pub const hrColumnNotFound: ::windows_sys::core::HRESULT = -939522589i32;
pub const hrColumnNotUpdatable: ::windows_sys::core::HRESULT = -939523048i32;
pub const hrColumnNull: ::windows_sys::core::HRESULT = -2013264916i32;
pub const hrColumnSetNull: ::windows_sys::core::HRESULT = -2013264852i32;
pub const hrColumnTooBig: ::windows_sys::core::HRESULT = -939522590i32;
pub const hrCommunicationError: ::windows_sys::core::HRESULT = -939589619i32;
pub const hrConsistentTimeMismatch: ::windows_sys::core::HRESULT = -939523545i32;
pub const hrContainerNotEmpty: ::windows_sys::core::HRESULT = -939523053i32;
pub const hrContentsExpired: ::windows_sys::core::HRESULT = -939589615i32;
pub const hrCouldNotConnect: ::windows_sys::core::HRESULT = -939589625i32;
pub const hrCreateIndexFailed: ::windows_sys::core::HRESULT = -2013264511i32;
pub const hrCurrencyStackOutOfMemory: ::windows_sys::core::HRESULT = -939523026i32;
pub const hrDatabaseAttached: ::windows_sys::core::HRESULT = -2013264913i32;
pub const hrDatabaseCorrupted: ::windows_sys::core::HRESULT = -939522890i32;
pub const hrDatabaseDuplicate: ::windows_sys::core::HRESULT = -939522895i32;
pub const hrDatabaseInUse: ::windows_sys::core::HRESULT = -939522894i32;
pub const hrDatabaseInconsistent: ::windows_sys::core::HRESULT = -939523546i32;
pub const hrDatabaseInvalidName: ::windows_sys::core::HRESULT = -939522892i32;
pub const hrDatabaseInvalidPages: ::windows_sys::core::HRESULT = -939522891i32;
pub const hrDatabaseLocked: ::windows_sys::core::HRESULT = -939522889i32;
pub const hrDatabaseNotFound: ::windows_sys::core::HRESULT = -939522893i32;
pub const hrDeleteBackupFileFail: ::windows_sys::core::HRESULT = -939523572i32;
pub const hrDensityInvalid: ::windows_sys::core::HRESULT = -939522789i32;
pub const hrDiskFull: ::windows_sys::core::HRESULT = -939522288i32;
pub const hrDiskIO: ::windows_sys::core::HRESULT = -939523074i32;
pub const hrError: ::windows_sys::core::HRESULT = -939589630i32;
pub const hrExistingLogFileHasBadSignature: ::windows_sys::core::HRESULT = -2013265362i32;
pub const hrExistingLogFileIsNotContiguous: ::windows_sys::core::HRESULT = -2013265361i32;
pub const hrFLDKeyTooBig: ::windows_sys::core::HRESULT = -2013265520i32;
pub const hrFLDNullKey: ::windows_sys::core::HRESULT = -2013265518i32;
pub const hrFLDTooManySegments: ::windows_sys::core::HRESULT = -939523695i32;
pub const hrFeatureNotAvailable: ::windows_sys::core::HRESULT = -939523095i32;
pub const hrFileAccessDenied: ::windows_sys::core::HRESULT = -939523064i32;
pub const hrFileClose: ::windows_sys::core::HRESULT = -939523994i32;
pub const hrFileNotFound: ::windows_sys::core::HRESULT = -939522285i32;
pub const hrFileOpenReadOnly: ::windows_sys::core::HRESULT = -2013264107i32;
pub const hrFullBackupNotTaken: ::windows_sys::core::HRESULT = -939589618i32;
pub const hrGivenLogFileHasBadSignature: ::windows_sys::core::HRESULT = -939523541i32;
pub const hrGivenLogFileIsNotContiguous: ::windows_sys::core::HRESULT = -939523540i32;
pub const hrIllegalOperation: ::windows_sys::core::HRESULT = -939522784i32;
pub const hrInTransaction: ::windows_sys::core::HRESULT = -939522988i32;
pub const hrIncrementalBackupDisabled: ::windows_sys::core::HRESULT = -939589623i32;
pub const hrIndexCantBuild: ::windows_sys::core::HRESULT = -939522695i32;
pub const hrIndexDuplicate: ::windows_sys::core::HRESULT = -939522693i32;
pub const hrIndexHasClustered: ::windows_sys::core::HRESULT = -939522688i32;
pub const hrIndexHasPrimary: ::windows_sys::core::HRESULT = -939522694i32;
pub const hrIndexInUse: ::windows_sys::core::HRESULT = -939523045i32;
pub const hrIndexInvalidDef: ::windows_sys::core::HRESULT = -939522690i32;
pub const hrIndexMustStay: ::windows_sys::core::HRESULT = -939522691i32;
pub const hrIndexNotFound: ::windows_sys::core::HRESULT = -939522692i32;
pub const hrInvalidBackup: ::windows_sys::core::HRESULT = -939523570i32;
pub const hrInvalidBackupSequence: ::windows_sys::core::HRESULT = -939523575i32;
pub const hrInvalidBookmark: ::windows_sys::core::HRESULT = -939523051i32;
pub const hrInvalidBufferSize: ::windows_sys::core::HRESULT = -939523049i32;
pub const hrInvalidCodePage: ::windows_sys::core::HRESULT = -939523033i32;
pub const hrInvalidColumnType: ::windows_sys::core::HRESULT = -939522585i32;
pub const hrInvalidCountry: ::windows_sys::core::HRESULT = -939523035i32;
pub const hrInvalidDatabase: ::windows_sys::core::HRESULT = -939523068i32;
pub const hrInvalidDatabaseId: ::windows_sys::core::HRESULT = -939523086i32;
pub const hrInvalidFilename: ::windows_sys::core::HRESULT = -939523052i32;
pub const hrInvalidHandle: ::windows_sys::core::HRESULT = -939589629i32;
pub const hrInvalidLanguageId: ::windows_sys::core::HRESULT = -939523034i32;
pub const hrInvalidLogSequence: ::windows_sys::core::HRESULT = -939523581i32;
pub const hrInvalidName: ::windows_sys::core::HRESULT = -939523094i32;
pub const hrInvalidObject: ::windows_sys::core::HRESULT = -939522780i32;
pub const hrInvalidOnSort: ::windows_sys::core::HRESULT = -939522394i32;
pub const hrInvalidOperation: ::windows_sys::core::HRESULT = -939522190i32;
pub const hrInvalidParam: ::windows_sys::core::HRESULT = -939589631i32;
pub const hrInvalidParameter: ::windows_sys::core::HRESULT = -939523093i32;
pub const hrInvalidPath: ::windows_sys::core::HRESULT = -939523073i32;
pub const hrInvalidRecips: ::windows_sys::core::HRESULT = -939589626i32;
pub const hrInvalidSesid: ::windows_sys::core::HRESULT = -939522992i32;
pub const hrInvalidTableId: ::windows_sys::core::HRESULT = -939522786i32;
pub const hrKeyChanged: ::windows_sys::core::HRESULT = -2013264302i32;
pub const hrKeyDuplicate: ::windows_sys::core::HRESULT = -939522491i32;
pub const hrKeyIsMade: ::windows_sys::core::HRESULT = -939522580i32;
pub const hrKeyNotMade: ::windows_sys::core::HRESULT = -939522488i32;
pub const hrLogBufferTooSmall: ::windows_sys::core::HRESULT = -939523579i32;
pub const hrLogCorrupted: ::windows_sys::core::HRESULT = -939522244i32;
pub const hrLogDiskFull: ::windows_sys::core::HRESULT = -939523567i32;
pub const hrLogFileCorrupt: ::windows_sys::core::HRESULT = -939523595i32;
pub const hrLogFileNotFound: ::windows_sys::core::HRESULT = -939589622i32;
pub const hrLogSequenceEnd: ::windows_sys::core::HRESULT = -939523577i32;
pub const hrLogWriteFail: ::windows_sys::core::HRESULT = -939523586i32;
pub const hrLoggingDisabled: ::windows_sys::core::HRESULT = -939523580i32;
pub const hrMakeBackupDirectoryFail: ::windows_sys::core::HRESULT = -939523571i32;
pub const hrMissingExpiryToken: ::windows_sys::core::HRESULT = -939589617i32;
pub const hrMissingFullBackup: ::windows_sys::core::HRESULT = -939523536i32;
pub const hrMissingLogFile: ::windows_sys::core::HRESULT = -939523568i32;
pub const hrMissingPreviousLogFile: ::windows_sys::core::HRESULT = -939523587i32;
pub const hrMissingRestoreLogFiles: ::windows_sys::core::HRESULT = -939523539i32;
pub const hrNoBackup: ::windows_sys::core::HRESULT = -939523576i32;
pub const hrNoBackupDirectory: ::windows_sys::core::HRESULT = -939523593i32;
pub const hrNoCurrentIndex: ::windows_sys::core::HRESULT = -939522581i32;
pub const hrNoCurrentRecord: ::windows_sys::core::HRESULT = -939522493i32;
pub const hrNoFullRestore: ::windows_sys::core::HRESULT = -939589620i32;
pub const hrNoIdleActivity: ::windows_sys::core::HRESULT = -2013264862i32;
pub const hrNoWriteLock: ::windows_sys::core::HRESULT = -2013264853i32;
pub const hrNone: ::windows_sys::core::HRESULT = 0i32;
pub const hrNotInTransaction: ::windows_sys::core::HRESULT = -939523042i32;
pub const hrNotInitialized: ::windows_sys::core::HRESULT = -939523067i32;
pub const hrNullInvalid: ::windows_sys::core::HRESULT = -939522592i32;
pub const hrNullKeyDisallowed: ::windows_sys::core::HRESULT = -939523043i32;
pub const hrNyi: ::windows_sys::core::HRESULT = -1073741823i32;
pub const hrObjectDuplicate: ::windows_sys::core::HRESULT = -939522782i32;
pub const hrObjectNotFound: ::windows_sys::core::HRESULT = -939522791i32;
pub const hrOutOfBuffers: ::windows_sys::core::HRESULT = -939523082i32;
pub const hrOutOfCursors: ::windows_sys::core::HRESULT = -939523083i32;
pub const hrOutOfDatabaseSpace: ::windows_sys::core::HRESULT = -939523084i32;
pub const hrOutOfFileHandles: ::windows_sys::core::HRESULT = -939523076i32;
pub const hrOutOfMemory: ::windows_sys::core::HRESULT = -939523085i32;
pub const hrOutOfSessions: ::windows_sys::core::HRESULT = -939522995i32;
pub const hrOutOfThreads: ::windows_sys::core::HRESULT = -939523993i32;
pub const hrPMRecDeleted: ::windows_sys::core::HRESULT = -939523794i32;
pub const hrPatchFileMismatch: ::windows_sys::core::HRESULT = -939523544i32;
pub const hrPermissionDenied: ::windows_sys::core::HRESULT = -939522287i32;
pub const hrReadVerifyFailure: ::windows_sys::core::HRESULT = -939523078i32;
pub const hrRecordClusteredChanged: ::windows_sys::core::HRESULT = -939522492i32;
pub const hrRecordDeleted: ::windows_sys::core::HRESULT = -939523079i32;
pub const hrRecordNotFound: ::windows_sys::core::HRESULT = -939522495i32;
pub const hrRecordTooBig: ::windows_sys::core::HRESULT = -939523070i32;
pub const hrRecoveredWithErrors: ::windows_sys::core::HRESULT = -939523569i32;
pub const hrRemainingVersions: ::windows_sys::core::HRESULT = -2013265599i32;
pub const hrRestoreInProgress: ::windows_sys::core::HRESULT = -939589628i32;
pub const hrRestoreLogTooHigh: ::windows_sys::core::HRESULT = -939523542i32;
pub const hrRestoreLogTooLow: ::windows_sys::core::HRESULT = -939523543i32;
pub const hrRestoreMapExists: ::windows_sys::core::HRESULT = -939589624i32;
pub const hrSeekNotEqual: ::windows_sys::core::HRESULT = -2013264881i32;
pub const hrSessionWriteConflict: ::windows_sys::core::HRESULT = -939522989i32;
pub const hrTableDuplicate: ::windows_sys::core::HRESULT = -939522793i32;
pub const hrTableEmpty: ::windows_sys::core::HRESULT = -2013264619i32;
pub const hrTableInUse: ::windows_sys::core::HRESULT = -939522792i32;
pub const hrTableLocked: ::windows_sys::core::HRESULT = -939522794i32;
pub const hrTableNotEmpty: ::windows_sys::core::HRESULT = -939522788i32;
pub const hrTaggedNotNULL: ::windows_sys::core::HRESULT = -939522582i32;
pub const hrTempFileOpenError: ::windows_sys::core::HRESULT = -939522293i32;
pub const hrTermInProgress: ::windows_sys::core::HRESULT = -939523096i32;
pub const hrTooManyActiveUsers: ::windows_sys::core::HRESULT = -939523037i32;
pub const hrTooManyAttachedDatabases: ::windows_sys::core::HRESULT = -939522291i32;
pub const hrTooManyColumns: ::windows_sys::core::HRESULT = -939523056i32;
pub const hrTooManyIO: ::windows_sys::core::HRESULT = -939523991i32;
pub const hrTooManyIndexes: ::windows_sys::core::HRESULT = -939523081i32;
pub const hrTooManyKeys: ::windows_sys::core::HRESULT = -939523080i32;
pub const hrTooManyOpenDatabases: ::windows_sys::core::HRESULT = -939523069i32;
pub const hrTooManyOpenIndexes: ::windows_sys::core::HRESULT = -939522686i32;
pub const hrTooManyOpenTables: ::windows_sys::core::HRESULT = -939522785i32;
pub const hrTooManySorts: ::windows_sys::core::HRESULT = -939522395i32;
pub const hrTransTooDeep: ::windows_sys::core::HRESULT = -939522993i32;
pub const hrUnknownExpiryTokenFormat: ::windows_sys::core::HRESULT = -939589616i32;
pub const hrUpdateNotPrepared: ::windows_sys::core::HRESULT = -939522487i32;
pub const hrVersionStoreOutOfMemory: ::windows_sys::core::HRESULT = -939523027i32;
pub const hrWriteConflict: ::windows_sys::core::HRESULT = -939522994i32;
pub const hrerrDataHasChanged: ::windows_sys::core::HRESULT = -939522485i32;
pub const hrwrnDataHasChanged: ::windows_sys::core::HRESULT = -2013264310i32;
pub type ADSI_DIALECT_ENUM = i32;
pub type ADSTYPE = i32;
pub type ADS_ACEFLAG_ENUM = i32;
pub type ADS_ACETYPE_ENUM = i32;
pub type ADS_AUTHENTICATION_ENUM = u32;
pub type ADS_CHASE_REFERRALS_ENUM = i32;
pub type ADS_DEREFENUM = i32;
pub type ADS_DISPLAY_ENUM = i32;
pub type ADS_ESCAPE_MODE_ENUM = i32;
pub type ADS_FLAGTYPE_ENUM = i32;
pub type ADS_FORMAT_ENUM = i32;
pub type ADS_GROUP_TYPE_ENUM = i32;
pub type ADS_NAME_INITTYPE_ENUM = i32;
pub type ADS_NAME_TYPE_ENUM = i32;
pub type ADS_OPTION_ENUM = i32;
pub type ADS_PASSWORD_ENCODING_ENUM = i32;
pub type ADS_PATHTYPE_ENUM = i32;
pub type ADS_PREFERENCES_ENUM = i32;
pub type ADS_PROPERTY_OPERATION_ENUM = i32;
pub type ADS_RIGHTS_ENUM = i32;
pub type ADS_SCOPEENUM = i32;
pub type ADS_SD_CONTROL_ENUM = i32;
pub type ADS_SD_FORMAT_ENUM = i32;
pub type ADS_SD_REVISION_ENUM = i32;
pub type ADS_SEARCHPREF_ENUM = i32;
pub type ADS_SECURITY_INFO_ENUM = i32;
pub type ADS_SETTYPE_ENUM = i32;
pub type ADS_STATUSENUM = i32;
pub type ADS_SYSTEMFLAG_ENUM = i32;
pub type ADS_USER_FLAG_ENUM = i32;
pub type DSROLE_MACHINE_ROLE = i32;
pub type DSROLE_OPERATION_STATE = i32;
pub type DSROLE_PRIMARY_DOMAIN_INFO_LEVEL = i32;
pub type DSROLE_SERVER_STATE = i32;
pub type DS_KCC_TASKID = i32;
pub type DS_MANGLE_FOR = i32;
pub type DS_NAME_ERROR = i32;
pub type DS_NAME_FLAGS = i32;
pub type DS_NAME_FORMAT = i32;
pub type DS_REPL_INFO_TYPE = i32;
pub type DS_REPL_OP_TYPE = i32;
pub type DS_REPSYNCALL_ERROR = i32;
pub type DS_REPSYNCALL_EVENT = i32;
pub type DS_SPN_NAME_TYPE = i32;
pub type DS_SPN_WRITE_OP = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADSPROPERROR {
    pub hwndPage: super::super::Foundation::HWND,
    pub pszPageTitle: ::windows_sys::core::PWSTR,
    pub pszObjPath: ::windows_sys::core::PWSTR,
    pub pszObjClass: ::windows_sys::core::PWSTR,
    pub hr: ::windows_sys::core::HRESULT,
    pub pszError: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADSPROPERROR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADSPROPERROR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADSPROPINITPARAMS {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub hr: ::windows_sys::core::HRESULT,
    pub pDsObj: IDirectoryObject,
    pub pwzCN: ::windows_sys::core::PWSTR,
    pub pWritableAttrs: *mut ADS_ATTR_INFO,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADSPROPINITPARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADSPROPINITPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADSVALUE {
    pub dwType: ADSTYPE,
    pub Anonymous: ADSVALUE_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADSVALUE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADSVALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union ADSVALUE_0 {
    pub DNString: *mut u16,
    pub CaseExactString: *mut u16,
    pub CaseIgnoreString: *mut u16,
    pub PrintableString: *mut u16,
    pub NumericString: *mut u16,
    pub Boolean: u32,
    pub Integer: u32,
    pub OctetString: ADS_OCTET_STRING,
    pub UTCTime: super::super::Foundation::SYSTEMTIME,
    pub LargeInteger: i64,
    pub ClassName: *mut u16,
    pub ProviderSpecific: ADS_PROV_SPECIFIC,
    pub pCaseIgnoreList: *mut ADS_CASEIGNORE_LIST,
    pub pOctetList: *mut ADS_OCTET_LIST,
    pub pPath: *mut ADS_PATH,
    pub pPostalAddress: *mut ADS_POSTALADDRESS,
    pub Timestamp: ADS_TIMESTAMP,
    pub BackLink: ADS_BACKLINK,
    pub pTypedName: *mut ADS_TYPEDNAME,
    pub Hold: ADS_HOLD,
    pub pNetAddress: *mut ADS_NETADDRESS,
    pub pReplicaPointer: *mut ADS_REPLICAPOINTER,
    pub pFaxNumber: *mut ADS_FAXNUMBER,
    pub Email: ADS_EMAIL,
    pub SecurityDescriptor: ADS_NT_SECURITY_DESCRIPTOR,
    pub pDNWithBinary: *mut ADS_DN_WITH_BINARY,
    pub pDNWithString: *mut ADS_DN_WITH_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADSVALUE_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADSVALUE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADS_ATTR_DEF {
    pub pszAttrName: ::windows_sys::core::PWSTR,
    pub dwADsType: ADSTYPE,
    pub dwMinRange: u32,
    pub dwMaxRange: u32,
    pub fMultiValued: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADS_ATTR_DEF {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADS_ATTR_DEF {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADS_ATTR_INFO {
    pub pszAttrName: ::windows_sys::core::PWSTR,
    pub dwControlCode: u32,
    pub dwADsType: ADSTYPE,
    pub pADsValues: *mut ADSVALUE,
    pub dwNumValues: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADS_ATTR_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADS_ATTR_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_BACKLINK {
    pub RemoteID: u32,
    pub ObjectName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ADS_BACKLINK {}
impl ::core::clone::Clone for ADS_BACKLINK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_CASEIGNORE_LIST {
    pub Next: *mut ADS_CASEIGNORE_LIST,
    pub String: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ADS_CASEIGNORE_LIST {}
impl ::core::clone::Clone for ADS_CASEIGNORE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADS_CLASS_DEF {
    pub pszClassName: ::windows_sys::core::PWSTR,
    pub dwMandatoryAttrs: u32,
    pub ppszMandatoryAttrs: *mut ::windows_sys::core::PWSTR,
    pub optionalAttrs: u32,
    pub ppszOptionalAttrs: *mut *mut ::windows_sys::core::PWSTR,
    pub dwNamingAttrs: u32,
    pub ppszNamingAttrs: *mut *mut ::windows_sys::core::PWSTR,
    pub dwSuperClasses: u32,
    pub ppszSuperClasses: *mut *mut ::windows_sys::core::PWSTR,
    pub fIsContainer: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADS_CLASS_DEF {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADS_CLASS_DEF {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_DN_WITH_BINARY {
    pub dwLength: u32,
    pub lpBinaryValue: *mut u8,
    pub pszDNString: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ADS_DN_WITH_BINARY {}
impl ::core::clone::Clone for ADS_DN_WITH_BINARY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_DN_WITH_STRING {
    pub pszStringValue: ::windows_sys::core::PWSTR,
    pub pszDNString: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ADS_DN_WITH_STRING {}
impl ::core::clone::Clone for ADS_DN_WITH_STRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_EMAIL {
    pub Address: ::windows_sys::core::PWSTR,
    pub Type: u32,
}
impl ::core::marker::Copy for ADS_EMAIL {}
impl ::core::clone::Clone for ADS_EMAIL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_FAXNUMBER {
    pub TelephoneNumber: ::windows_sys::core::PWSTR,
    pub NumberOfBits: u32,
    pub Parameters: *mut u8,
}
impl ::core::marker::Copy for ADS_FAXNUMBER {}
impl ::core::clone::Clone for ADS_FAXNUMBER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_HOLD {
    pub ObjectName: ::windows_sys::core::PWSTR,
    pub Amount: u32,
}
impl ::core::marker::Copy for ADS_HOLD {}
impl ::core::clone::Clone for ADS_HOLD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_NETADDRESS {
    pub AddressType: u32,
    pub AddressLength: u32,
    pub Address: *mut u8,
}
impl ::core::marker::Copy for ADS_NETADDRESS {}
impl ::core::clone::Clone for ADS_NETADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_NT_SECURITY_DESCRIPTOR {
    pub dwLength: u32,
    pub lpValue: *mut u8,
}
impl ::core::marker::Copy for ADS_NT_SECURITY_DESCRIPTOR {}
impl ::core::clone::Clone for ADS_NT_SECURITY_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_OBJECT_INFO {
    pub pszRDN: ::windows_sys::core::PWSTR,
    pub pszObjectDN: ::windows_sys::core::PWSTR,
    pub pszParentDN: ::windows_sys::core::PWSTR,
    pub pszSchemaDN: ::windows_sys::core::PWSTR,
    pub pszClassName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ADS_OBJECT_INFO {}
impl ::core::clone::Clone for ADS_OBJECT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_OCTET_LIST {
    pub Next: *mut ADS_OCTET_LIST,
    pub Length: u32,
    pub Data: *mut u8,
}
impl ::core::marker::Copy for ADS_OCTET_LIST {}
impl ::core::clone::Clone for ADS_OCTET_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_OCTET_STRING {
    pub dwLength: u32,
    pub lpValue: *mut u8,
}
impl ::core::marker::Copy for ADS_OCTET_STRING {}
impl ::core::clone::Clone for ADS_OCTET_STRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_PATH {
    pub Type: u32,
    pub VolumeName: ::windows_sys::core::PWSTR,
    pub Path: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ADS_PATH {}
impl ::core::clone::Clone for ADS_PATH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_POSTALADDRESS {
    pub PostalAddress: [::windows_sys::core::PWSTR; 6],
}
impl ::core::marker::Copy for ADS_POSTALADDRESS {}
impl ::core::clone::Clone for ADS_POSTALADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_PROV_SPECIFIC {
    pub dwLength: u32,
    pub lpValue: *mut u8,
}
impl ::core::marker::Copy for ADS_PROV_SPECIFIC {}
impl ::core::clone::Clone for ADS_PROV_SPECIFIC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_REPLICAPOINTER {
    pub ServerName: ::windows_sys::core::PWSTR,
    pub ReplicaType: u32,
    pub ReplicaNumber: u32,
    pub Count: u32,
    pub ReplicaAddressHints: *mut ADS_NETADDRESS,
}
impl ::core::marker::Copy for ADS_REPLICAPOINTER {}
impl ::core::clone::Clone for ADS_REPLICAPOINTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADS_SEARCHPREF_INFO {
    pub dwSearchPref: ADS_SEARCHPREF_ENUM,
    pub vValue: ADSVALUE,
    pub dwStatus: ADS_STATUSENUM,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADS_SEARCHPREF_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADS_SEARCHPREF_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADS_SEARCH_COLUMN {
    pub pszAttrName: ::windows_sys::core::PWSTR,
    pub dwADsType: ADSTYPE,
    pub pADsValues: *mut ADSVALUE,
    pub dwNumValues: u32,
    pub hReserved: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADS_SEARCH_COLUMN {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADS_SEARCH_COLUMN {
    fn clone(&self) -> Self {
        *self
    }
}
pub type ADS_SEARCH_HANDLE = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ADS_SORTKEY {
    pub pszAttrType: ::windows_sys::core::PWSTR,
    pub pszReserved: ::windows_sys::core::PWSTR,
    pub fReverseorder: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ADS_SORTKEY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ADS_SORTKEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_TIMESTAMP {
    pub WholeSeconds: u32,
    pub EventID: u32,
}
impl ::core::marker::Copy for ADS_TIMESTAMP {}
impl ::core::clone::Clone for ADS_TIMESTAMP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_TYPEDNAME {
    pub ObjectName: ::windows_sys::core::PWSTR,
    pub Level: u32,
    pub Interval: u32,
}
impl ::core::marker::Copy for ADS_TYPEDNAME {}
impl ::core::clone::Clone for ADS_TYPEDNAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADS_VLV {
    pub dwBeforeCount: u32,
    pub dwAfterCount: u32,
    pub dwOffset: u32,
    pub dwContentCount: u32,
    pub pszTarget: ::windows_sys::core::PWSTR,
    pub dwContextIDLength: u32,
    pub lpContextID: *mut u8,
}
impl ::core::marker::Copy for ADS_VLV {}
impl ::core::clone::Clone for ADS_VLV {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub struct CQFORM {
    pub cbStruct: u32,
    pub dwFlags: u32,
    pub clsid: ::windows_sys::core::GUID,
    pub hIcon: super::super::UI::WindowsAndMessaging::HICON,
    pub pszTitle: ::windows_sys::core::PCWSTR,
}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::marker::Copy for CQFORM {}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::clone::Clone for CQFORM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub struct CQPAGE {
    pub cbStruct: u32,
    pub dwFlags: u32,
    pub pPageProc: LPCQPAGEPROC,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub idPageName: i32,
    pub idPageTemplate: i32,
    pub pDlgProc: super::super::UI::WindowsAndMessaging::DLGPROC,
    pub lParam: super::super::Foundation::LPARAM,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::marker::Copy for CQPAGE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
impl ::core::clone::Clone for CQPAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOMAINDESC {
    pub pszName: ::windows_sys::core::PWSTR,
    pub pszPath: ::windows_sys::core::PWSTR,
    pub pszNCName: ::windows_sys::core::PWSTR,
    pub pszTrustParent: ::windows_sys::core::PWSTR,
    pub pszObjectClass: ::windows_sys::core::PWSTR,
    pub ulFlags: u32,
    pub fDownLevel: super::super::Foundation::BOOL,
    pub pdChildList: *mut DOMAINDESC,
    pub pdNextSibling: *mut DOMAINDESC,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOMAINDESC {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOMAINDESC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOMAIN_CONTROLLER_INFOA {
    pub DomainControllerName: ::windows_sys::core::PSTR,
    pub DomainControllerAddress: ::windows_sys::core::PSTR,
    pub DomainControllerAddressType: u32,
    pub DomainGuid: ::windows_sys::core::GUID,
    pub DomainName: ::windows_sys::core::PSTR,
    pub DnsForestName: ::windows_sys::core::PSTR,
    pub Flags: u32,
    pub DcSiteName: ::windows_sys::core::PSTR,
    pub ClientSiteName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for DOMAIN_CONTROLLER_INFOA {}
impl ::core::clone::Clone for DOMAIN_CONTROLLER_INFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOMAIN_CONTROLLER_INFOW {
    pub DomainControllerName: ::windows_sys::core::PWSTR,
    pub DomainControllerAddress: ::windows_sys::core::PWSTR,
    pub DomainControllerAddressType: u32,
    pub DomainGuid: ::windows_sys::core::GUID,
    pub DomainName: ::windows_sys::core::PWSTR,
    pub DnsForestName: ::windows_sys::core::PWSTR,
    pub Flags: u32,
    pub DcSiteName: ::windows_sys::core::PWSTR,
    pub ClientSiteName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for DOMAIN_CONTROLLER_INFOW {}
impl ::core::clone::Clone for DOMAIN_CONTROLLER_INFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DOMAIN_TREE {
    pub dsSize: u32,
    pub dwCount: u32,
    pub aDomains: [DOMAINDESC; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DOMAIN_TREE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DOMAIN_TREE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub struct DSA_NEWOBJ_DISPINFO {
    pub dwSize: u32,
    pub hObjClassIcon: super::super::UI::WindowsAndMessaging::HICON,
    pub lpszWizTitle: ::windows_sys::core::PWSTR,
    pub lpszContDisplayName: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::marker::Copy for DSA_NEWOBJ_DISPINFO {}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::clone::Clone for DSA_NEWOBJ_DISPINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSBITEMA {
    pub cbStruct: u32,
    pub pszADsPath: ::windows_sys::core::PCWSTR,
    pub pszClass: ::windows_sys::core::PCWSTR,
    pub dwMask: u32,
    pub dwState: u32,
    pub dwStateMask: u32,
    pub szDisplayName: [u8; 64],
    pub szIconLocation: [u8; 260],
    pub iIconResID: i32,
}
impl ::core::marker::Copy for DSBITEMA {}
impl ::core::clone::Clone for DSBITEMA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSBITEMW {
    pub cbStruct: u32,
    pub pszADsPath: ::windows_sys::core::PCWSTR,
    pub pszClass: ::windows_sys::core::PCWSTR,
    pub dwMask: u32,
    pub dwState: u32,
    pub dwStateMask: u32,
    pub szDisplayName: [u16; 64],
    pub szIconLocation: [u16; 260],
    pub iIconResID: i32,
}
impl ::core::marker::Copy for DSBITEMW {}
impl ::core::clone::Clone for DSBITEMW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
pub struct DSBROWSEINFOA {
    pub cbStruct: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub pszCaption: ::windows_sys::core::PCSTR,
    pub pszTitle: ::windows_sys::core::PCSTR,
    pub pszRoot: ::windows_sys::core::PCWSTR,
    pub pszPath: ::windows_sys::core::PWSTR,
    pub cchPath: u32,
    pub dwFlags: u32,
    pub pfnCallback: super::super::UI::Shell::BFFCALLBACK,
    pub lParam: super::super::Foundation::LPARAM,
    pub dwReturnFormat: u32,
    pub pUserName: ::windows_sys::core::PCWSTR,
    pub pPassword: ::windows_sys::core::PCWSTR,
    pub pszObjectClass: ::windows_sys::core::PWSTR,
    pub cchObjectClass: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::marker::Copy for DSBROWSEINFOA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::clone::Clone for DSBROWSEINFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Shell\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
pub struct DSBROWSEINFOW {
    pub cbStruct: u32,
    pub hwndOwner: super::super::Foundation::HWND,
    pub pszCaption: ::windows_sys::core::PCWSTR,
    pub pszTitle: ::windows_sys::core::PCWSTR,
    pub pszRoot: ::windows_sys::core::PCWSTR,
    pub pszPath: ::windows_sys::core::PWSTR,
    pub cchPath: u32,
    pub dwFlags: u32,
    pub pfnCallback: super::super::UI::Shell::BFFCALLBACK,
    pub lParam: super::super::Foundation::LPARAM,
    pub dwReturnFormat: u32,
    pub pUserName: ::windows_sys::core::PCWSTR,
    pub pPassword: ::windows_sys::core::PCWSTR,
    pub pszObjectClass: ::windows_sys::core::PWSTR,
    pub cchObjectClass: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::marker::Copy for DSBROWSEINFOW {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Shell"))]
impl ::core::clone::Clone for DSBROWSEINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSCLASSCREATIONINFO {
    pub dwFlags: u32,
    pub clsidWizardDialog: ::windows_sys::core::GUID,
    pub clsidWizardPrimaryPage: ::windows_sys::core::GUID,
    pub cWizardExtensions: u32,
    pub aWizardExtensions: [::windows_sys::core::GUID; 1],
}
impl ::core::marker::Copy for DSCLASSCREATIONINFO {}
impl ::core::clone::Clone for DSCLASSCREATIONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSCOLUMN {
    pub dwFlags: u32,
    pub fmt: i32,
    pub cx: i32,
    pub idsName: i32,
    pub offsetProperty: i32,
    pub dwReserved: u32,
}
impl ::core::marker::Copy for DSCOLUMN {}
impl ::core::clone::Clone for DSCOLUMN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSDISPLAYSPECOPTIONS {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub offsetAttribPrefix: u32,
    pub offsetUserName: u32,
    pub offsetPassword: u32,
    pub offsetServer: u32,
    pub offsetServerConfigPath: u32,
}
impl ::core::marker::Copy for DSDISPLAYSPECOPTIONS {}
impl ::core::clone::Clone for DSDISPLAYSPECOPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSOBJECT {
    pub dwFlags: u32,
    pub dwProviderFlags: u32,
    pub offsetName: u32,
    pub offsetClass: u32,
}
impl ::core::marker::Copy for DSOBJECT {}
impl ::core::clone::Clone for DSOBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSOBJECTNAMES {
    pub clsidNamespace: ::windows_sys::core::GUID,
    pub cItems: u32,
    pub aObjects: [DSOBJECT; 1],
}
impl ::core::marker::Copy for DSOBJECTNAMES {}
impl ::core::clone::Clone for DSOBJECTNAMES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSOP_FILTER_FLAGS {
    pub Uplevel: DSOP_UPLEVEL_FILTER_FLAGS,
    pub flDownlevel: u32,
}
impl ::core::marker::Copy for DSOP_FILTER_FLAGS {}
impl ::core::clone::Clone for DSOP_FILTER_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSOP_INIT_INFO {
    pub cbSize: u32,
    pub pwzTargetComputer: ::windows_sys::core::PCWSTR,
    pub cDsScopeInfos: u32,
    pub aDsScopeInfos: *mut DSOP_SCOPE_INIT_INFO,
    pub flOptions: u32,
    pub cAttributesToFetch: u32,
    pub apwzAttributeNames: *const ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for DSOP_INIT_INFO {}
impl ::core::clone::Clone for DSOP_INIT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSOP_SCOPE_INIT_INFO {
    pub cbSize: u32,
    pub flType: u32,
    pub flScope: u32,
    pub FilterFlags: DSOP_FILTER_FLAGS,
    pub pwzDcName: ::windows_sys::core::PCWSTR,
    pub pwzADsPath: ::windows_sys::core::PCWSTR,
    pub hr: ::windows_sys::core::HRESULT,
}
impl ::core::marker::Copy for DSOP_SCOPE_INIT_INFO {}
impl ::core::clone::Clone for DSOP_SCOPE_INIT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSOP_UPLEVEL_FILTER_FLAGS {
    pub flBothModes: u32,
    pub flMixedModeOnly: u32,
    pub flNativeModeOnly: u32,
}
impl ::core::marker::Copy for DSOP_UPLEVEL_FILTER_FLAGS {}
impl ::core::clone::Clone for DSOP_UPLEVEL_FILTER_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSPROPERTYPAGEINFO {
    pub offsetString: u32,
}
impl ::core::marker::Copy for DSPROPERTYPAGEINFO {}
impl ::core::clone::Clone for DSPROPERTYPAGEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSQUERYCLASSLIST {
    pub cbStruct: u32,
    pub cClasses: i32,
    pub offsetClass: [u32; 1],
}
impl ::core::marker::Copy for DSQUERYCLASSLIST {}
impl ::core::clone::Clone for DSQUERYCLASSLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSQUERYINITPARAMS {
    pub cbStruct: u32,
    pub dwFlags: u32,
    pub pDefaultScope: ::windows_sys::core::PWSTR,
    pub pDefaultSaveLocation: ::windows_sys::core::PWSTR,
    pub pUserName: ::windows_sys::core::PWSTR,
    pub pPassword: ::windows_sys::core::PWSTR,
    pub pServer: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for DSQUERYINITPARAMS {}
impl ::core::clone::Clone for DSQUERYINITPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DSQUERYPARAMS {
    pub cbStruct: u32,
    pub dwFlags: u32,
    pub hInstance: super::super::Foundation::HINSTANCE,
    pub offsetQuery: i32,
    pub iColumns: i32,
    pub dwReserved: u32,
    pub aColumns: [DSCOLUMN; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DSQUERYPARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DSQUERYPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSROLE_OPERATION_STATE_INFO {
    pub OperationState: DSROLE_OPERATION_STATE,
}
impl ::core::marker::Copy for DSROLE_OPERATION_STATE_INFO {}
impl ::core::clone::Clone for DSROLE_OPERATION_STATE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSROLE_PRIMARY_DOMAIN_INFO_BASIC {
    pub MachineRole: DSROLE_MACHINE_ROLE,
    pub Flags: u32,
    pub DomainNameFlat: ::windows_sys::core::PWSTR,
    pub DomainNameDns: ::windows_sys::core::PWSTR,
    pub DomainForestName: ::windows_sys::core::PWSTR,
    pub DomainGuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for DSROLE_PRIMARY_DOMAIN_INFO_BASIC {}
impl ::core::clone::Clone for DSROLE_PRIMARY_DOMAIN_INFO_BASIC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DSROLE_UPGRADE_STATUS_INFO {
    pub OperationState: u32,
    pub PreviousServerState: DSROLE_SERVER_STATE,
}
impl ::core::marker::Copy for DSROLE_UPGRADE_STATUS_INFO {}
impl ::core::clone::Clone for DSROLE_UPGRADE_STATUS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_CONTROLLER_INFO_1A {
    pub NetbiosName: ::windows_sys::core::PSTR,
    pub DnsHostName: ::windows_sys::core::PSTR,
    pub SiteName: ::windows_sys::core::PSTR,
    pub ComputerObjectName: ::windows_sys::core::PSTR,
    pub ServerObjectName: ::windows_sys::core::PSTR,
    pub fIsPdc: super::super::Foundation::BOOL,
    pub fDsEnabled: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_CONTROLLER_INFO_1A {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_CONTROLLER_INFO_1A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_CONTROLLER_INFO_1W {
    pub NetbiosName: ::windows_sys::core::PWSTR,
    pub DnsHostName: ::windows_sys::core::PWSTR,
    pub SiteName: ::windows_sys::core::PWSTR,
    pub ComputerObjectName: ::windows_sys::core::PWSTR,
    pub ServerObjectName: ::windows_sys::core::PWSTR,
    pub fIsPdc: super::super::Foundation::BOOL,
    pub fDsEnabled: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_CONTROLLER_INFO_1W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_CONTROLLER_INFO_1W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_CONTROLLER_INFO_2A {
    pub NetbiosName: ::windows_sys::core::PSTR,
    pub DnsHostName: ::windows_sys::core::PSTR,
    pub SiteName: ::windows_sys::core::PSTR,
    pub SiteObjectName: ::windows_sys::core::PSTR,
    pub ComputerObjectName: ::windows_sys::core::PSTR,
    pub ServerObjectName: ::windows_sys::core::PSTR,
    pub NtdsDsaObjectName: ::windows_sys::core::PSTR,
    pub fIsPdc: super::super::Foundation::BOOL,
    pub fDsEnabled: super::super::Foundation::BOOL,
    pub fIsGc: super::super::Foundation::BOOL,
    pub SiteObjectGuid: ::windows_sys::core::GUID,
    pub ComputerObjectGuid: ::windows_sys::core::GUID,
    pub ServerObjectGuid: ::windows_sys::core::GUID,
    pub NtdsDsaObjectGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_CONTROLLER_INFO_2A {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_CONTROLLER_INFO_2A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_CONTROLLER_INFO_2W {
    pub NetbiosName: ::windows_sys::core::PWSTR,
    pub DnsHostName: ::windows_sys::core::PWSTR,
    pub SiteName: ::windows_sys::core::PWSTR,
    pub SiteObjectName: ::windows_sys::core::PWSTR,
    pub ComputerObjectName: ::windows_sys::core::PWSTR,
    pub ServerObjectName: ::windows_sys::core::PWSTR,
    pub NtdsDsaObjectName: ::windows_sys::core::PWSTR,
    pub fIsPdc: super::super::Foundation::BOOL,
    pub fDsEnabled: super::super::Foundation::BOOL,
    pub fIsGc: super::super::Foundation::BOOL,
    pub SiteObjectGuid: ::windows_sys::core::GUID,
    pub ComputerObjectGuid: ::windows_sys::core::GUID,
    pub ServerObjectGuid: ::windows_sys::core::GUID,
    pub NtdsDsaObjectGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_CONTROLLER_INFO_2W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_CONTROLLER_INFO_2W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_CONTROLLER_INFO_3A {
    pub NetbiosName: ::windows_sys::core::PSTR,
    pub DnsHostName: ::windows_sys::core::PSTR,
    pub SiteName: ::windows_sys::core::PSTR,
    pub SiteObjectName: ::windows_sys::core::PSTR,
    pub ComputerObjectName: ::windows_sys::core::PSTR,
    pub ServerObjectName: ::windows_sys::core::PSTR,
    pub NtdsDsaObjectName: ::windows_sys::core::PSTR,
    pub fIsPdc: super::super::Foundation::BOOL,
    pub fDsEnabled: super::super::Foundation::BOOL,
    pub fIsGc: super::super::Foundation::BOOL,
    pub fIsRodc: super::super::Foundation::BOOL,
    pub SiteObjectGuid: ::windows_sys::core::GUID,
    pub ComputerObjectGuid: ::windows_sys::core::GUID,
    pub ServerObjectGuid: ::windows_sys::core::GUID,
    pub NtdsDsaObjectGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_CONTROLLER_INFO_3A {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_CONTROLLER_INFO_3A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_CONTROLLER_INFO_3W {
    pub NetbiosName: ::windows_sys::core::PWSTR,
    pub DnsHostName: ::windows_sys::core::PWSTR,
    pub SiteName: ::windows_sys::core::PWSTR,
    pub SiteObjectName: ::windows_sys::core::PWSTR,
    pub ComputerObjectName: ::windows_sys::core::PWSTR,
    pub ServerObjectName: ::windows_sys::core::PWSTR,
    pub NtdsDsaObjectName: ::windows_sys::core::PWSTR,
    pub fIsPdc: super::super::Foundation::BOOL,
    pub fDsEnabled: super::super::Foundation::BOOL,
    pub fIsGc: super::super::Foundation::BOOL,
    pub fIsRodc: super::super::Foundation::BOOL,
    pub SiteObjectGuid: ::windows_sys::core::GUID,
    pub ComputerObjectGuid: ::windows_sys::core::GUID,
    pub ServerObjectGuid: ::windows_sys::core::GUID,
    pub NtdsDsaObjectGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_CONTROLLER_INFO_3W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_CONTROLLER_INFO_3W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_TRUSTSA {
    pub NetbiosDomainName: ::windows_sys::core::PSTR,
    pub DnsDomainName: ::windows_sys::core::PSTR,
    pub Flags: u32,
    pub ParentIndex: u32,
    pub TrustType: u32,
    pub TrustAttributes: u32,
    pub DomainSid: super::super::Foundation::PSID,
    pub DomainGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_TRUSTSA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_TRUSTSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_DOMAIN_TRUSTSW {
    pub NetbiosDomainName: ::windows_sys::core::PWSTR,
    pub DnsDomainName: ::windows_sys::core::PWSTR,
    pub Flags: u32,
    pub ParentIndex: u32,
    pub TrustType: u32,
    pub TrustAttributes: u32,
    pub DomainSid: super::super::Foundation::PSID,
    pub DomainGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_DOMAIN_TRUSTSW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_DOMAIN_TRUSTSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_NAME_RESULTA {
    pub cItems: u32,
    pub rItems: *mut DS_NAME_RESULT_ITEMA,
}
impl ::core::marker::Copy for DS_NAME_RESULTA {}
impl ::core::clone::Clone for DS_NAME_RESULTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_NAME_RESULTW {
    pub cItems: u32,
    pub rItems: *mut DS_NAME_RESULT_ITEMW,
}
impl ::core::marker::Copy for DS_NAME_RESULTW {}
impl ::core::clone::Clone for DS_NAME_RESULTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_NAME_RESULT_ITEMA {
    pub status: u32,
    pub pDomain: ::windows_sys::core::PSTR,
    pub pName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for DS_NAME_RESULT_ITEMA {}
impl ::core::clone::Clone for DS_NAME_RESULT_ITEMA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_NAME_RESULT_ITEMW {
    pub status: u32,
    pub pDomain: ::windows_sys::core::PWSTR,
    pub pName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for DS_NAME_RESULT_ITEMW {}
impl ::core::clone::Clone for DS_NAME_RESULT_ITEMW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_ATTR_META_DATA {
    pub pszAttributeName: ::windows_sys::core::PWSTR,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_ATTR_META_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_ATTR_META_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_ATTR_META_DATA_2 {
    pub pszAttributeName: ::windows_sys::core::PWSTR,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
    pub pszLastOriginatingDsaDN: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_ATTR_META_DATA_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_ATTR_META_DATA_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_ATTR_META_DATA_BLOB {
    pub oszAttributeName: u32,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
    pub oszLastOriginatingDsaDN: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_ATTR_META_DATA_BLOB {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_ATTR_META_DATA_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_ATTR_VALUE_META_DATA {
    pub cNumEntries: u32,
    pub dwEnumerationContext: u32,
    pub rgMetaData: [DS_REPL_VALUE_META_DATA; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_ATTR_VALUE_META_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_ATTR_VALUE_META_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_ATTR_VALUE_META_DATA_2 {
    pub cNumEntries: u32,
    pub dwEnumerationContext: u32,
    pub rgMetaData: [DS_REPL_VALUE_META_DATA_2; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_ATTR_VALUE_META_DATA_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_ATTR_VALUE_META_DATA_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_ATTR_VALUE_META_DATA_EXT {
    pub cNumEntries: u32,
    pub dwEnumerationContext: u32,
    pub rgMetaData: [DS_REPL_VALUE_META_DATA_EXT; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_ATTR_VALUE_META_DATA_EXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_ATTR_VALUE_META_DATA_EXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPL_CURSOR {
    pub uuidSourceDsaInvocationID: ::windows_sys::core::GUID,
    pub usnAttributeFilter: i64,
}
impl ::core::marker::Copy for DS_REPL_CURSOR {}
impl ::core::clone::Clone for DS_REPL_CURSOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPL_CURSORS {
    pub cNumCursors: u32,
    pub dwReserved: u32,
    pub rgCursor: [DS_REPL_CURSOR; 1],
}
impl ::core::marker::Copy for DS_REPL_CURSORS {}
impl ::core::clone::Clone for DS_REPL_CURSORS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_CURSORS_2 {
    pub cNumCursors: u32,
    pub dwEnumerationContext: u32,
    pub rgCursor: [DS_REPL_CURSOR_2; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_CURSORS_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_CURSORS_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_CURSORS_3W {
    pub cNumCursors: u32,
    pub dwEnumerationContext: u32,
    pub rgCursor: [DS_REPL_CURSOR_3W; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_CURSORS_3W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_CURSORS_3W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_CURSOR_2 {
    pub uuidSourceDsaInvocationID: ::windows_sys::core::GUID,
    pub usnAttributeFilter: i64,
    pub ftimeLastSyncSuccess: super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_CURSOR_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_CURSOR_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_CURSOR_3W {
    pub uuidSourceDsaInvocationID: ::windows_sys::core::GUID,
    pub usnAttributeFilter: i64,
    pub ftimeLastSyncSuccess: super::super::Foundation::FILETIME,
    pub pszSourceDsaDN: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_CURSOR_3W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_CURSOR_3W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_CURSOR_BLOB {
    pub uuidSourceDsaInvocationID: ::windows_sys::core::GUID,
    pub usnAttributeFilter: i64,
    pub ftimeLastSyncSuccess: super::super::Foundation::FILETIME,
    pub oszSourceDsaDN: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_CURSOR_BLOB {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_CURSOR_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_KCC_DSA_FAILURESW {
    pub cNumEntries: u32,
    pub dwReserved: u32,
    pub rgDsaFailure: [DS_REPL_KCC_DSA_FAILUREW; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_KCC_DSA_FAILURESW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_KCC_DSA_FAILURESW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_KCC_DSA_FAILUREW {
    pub pszDsaDN: ::windows_sys::core::PWSTR,
    pub uuidDsaObjGuid: ::windows_sys::core::GUID,
    pub ftimeFirstFailure: super::super::Foundation::FILETIME,
    pub cNumFailures: u32,
    pub dwLastResult: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_KCC_DSA_FAILUREW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_KCC_DSA_FAILUREW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_KCC_DSA_FAILUREW_BLOB {
    pub oszDsaDN: u32,
    pub uuidDsaObjGuid: ::windows_sys::core::GUID,
    pub ftimeFirstFailure: super::super::Foundation::FILETIME,
    pub cNumFailures: u32,
    pub dwLastResult: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_KCC_DSA_FAILUREW_BLOB {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_KCC_DSA_FAILUREW_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_NEIGHBORSW {
    pub cNumNeighbors: u32,
    pub dwReserved: u32,
    pub rgNeighbor: [DS_REPL_NEIGHBORW; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_NEIGHBORSW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_NEIGHBORSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_NEIGHBORW {
    pub pszNamingContext: ::windows_sys::core::PWSTR,
    pub pszSourceDsaDN: ::windows_sys::core::PWSTR,
    pub pszSourceDsaAddress: ::windows_sys::core::PWSTR,
    pub pszAsyncIntersiteTransportDN: ::windows_sys::core::PWSTR,
    pub dwReplicaFlags: u32,
    pub dwReserved: u32,
    pub uuidNamingContextObjGuid: ::windows_sys::core::GUID,
    pub uuidSourceDsaObjGuid: ::windows_sys::core::GUID,
    pub uuidSourceDsaInvocationID: ::windows_sys::core::GUID,
    pub uuidAsyncIntersiteTransportObjGuid: ::windows_sys::core::GUID,
    pub usnLastObjChangeSynced: i64,
    pub usnAttributeFilter: i64,
    pub ftimeLastSyncSuccess: super::super::Foundation::FILETIME,
    pub ftimeLastSyncAttempt: super::super::Foundation::FILETIME,
    pub dwLastSyncResult: u32,
    pub cNumConsecutiveSyncFailures: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_NEIGHBORW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_NEIGHBORW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_NEIGHBORW_BLOB {
    pub oszNamingContext: u32,
    pub oszSourceDsaDN: u32,
    pub oszSourceDsaAddress: u32,
    pub oszAsyncIntersiteTransportDN: u32,
    pub dwReplicaFlags: u32,
    pub dwReserved: u32,
    pub uuidNamingContextObjGuid: ::windows_sys::core::GUID,
    pub uuidSourceDsaObjGuid: ::windows_sys::core::GUID,
    pub uuidSourceDsaInvocationID: ::windows_sys::core::GUID,
    pub uuidAsyncIntersiteTransportObjGuid: ::windows_sys::core::GUID,
    pub usnLastObjChangeSynced: i64,
    pub usnAttributeFilter: i64,
    pub ftimeLastSyncSuccess: super::super::Foundation::FILETIME,
    pub ftimeLastSyncAttempt: super::super::Foundation::FILETIME,
    pub dwLastSyncResult: u32,
    pub cNumConsecutiveSyncFailures: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_NEIGHBORW_BLOB {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_NEIGHBORW_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_OBJ_META_DATA {
    pub cNumEntries: u32,
    pub dwReserved: u32,
    pub rgMetaData: [DS_REPL_ATTR_META_DATA; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_OBJ_META_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_OBJ_META_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_OBJ_META_DATA_2 {
    pub cNumEntries: u32,
    pub dwReserved: u32,
    pub rgMetaData: [DS_REPL_ATTR_META_DATA_2; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_OBJ_META_DATA_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_OBJ_META_DATA_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_OPW {
    pub ftimeEnqueued: super::super::Foundation::FILETIME,
    pub ulSerialNumber: u32,
    pub ulPriority: u32,
    pub OpType: DS_REPL_OP_TYPE,
    pub ulOptions: u32,
    pub pszNamingContext: ::windows_sys::core::PWSTR,
    pub pszDsaDN: ::windows_sys::core::PWSTR,
    pub pszDsaAddress: ::windows_sys::core::PWSTR,
    pub uuidNamingContextObjGuid: ::windows_sys::core::GUID,
    pub uuidDsaObjGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_OPW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_OPW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_OPW_BLOB {
    pub ftimeEnqueued: super::super::Foundation::FILETIME,
    pub ulSerialNumber: u32,
    pub ulPriority: u32,
    pub OpType: DS_REPL_OP_TYPE,
    pub ulOptions: u32,
    pub oszNamingContext: u32,
    pub oszDsaDN: u32,
    pub oszDsaAddress: u32,
    pub uuidNamingContextObjGuid: ::windows_sys::core::GUID,
    pub uuidDsaObjGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_OPW_BLOB {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_OPW_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_PENDING_OPSW {
    pub ftimeCurrentOpStarted: super::super::Foundation::FILETIME,
    pub cNumPendingOps: u32,
    pub rgPendingOp: [DS_REPL_OPW; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_PENDING_OPSW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_PENDING_OPSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_QUEUE_STATISTICSW {
    pub ftimeCurrentOpStarted: super::super::Foundation::FILETIME,
    pub cNumPendingOps: u32,
    pub ftimeOldestSync: super::super::Foundation::FILETIME,
    pub ftimeOldestAdd: super::super::Foundation::FILETIME,
    pub ftimeOldestMod: super::super::Foundation::FILETIME,
    pub ftimeOldestDel: super::super::Foundation::FILETIME,
    pub ftimeOldestUpdRefs: super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_QUEUE_STATISTICSW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_QUEUE_STATISTICSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_VALUE_META_DATA {
    pub pszAttributeName: ::windows_sys::core::PWSTR,
    pub pszObjectDn: ::windows_sys::core::PWSTR,
    pub cbData: u32,
    pub pbData: *mut u8,
    pub ftimeDeleted: super::super::Foundation::FILETIME,
    pub ftimeCreated: super::super::Foundation::FILETIME,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_VALUE_META_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_VALUE_META_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_VALUE_META_DATA_2 {
    pub pszAttributeName: ::windows_sys::core::PWSTR,
    pub pszObjectDn: ::windows_sys::core::PWSTR,
    pub cbData: u32,
    pub pbData: *mut u8,
    pub ftimeDeleted: super::super::Foundation::FILETIME,
    pub ftimeCreated: super::super::Foundation::FILETIME,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
    pub pszLastOriginatingDsaDN: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_VALUE_META_DATA_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_VALUE_META_DATA_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_VALUE_META_DATA_BLOB {
    pub oszAttributeName: u32,
    pub oszObjectDn: u32,
    pub cbData: u32,
    pub obData: u32,
    pub ftimeDeleted: super::super::Foundation::FILETIME,
    pub ftimeCreated: super::super::Foundation::FILETIME,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
    pub oszLastOriginatingDsaDN: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_VALUE_META_DATA_BLOB {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_VALUE_META_DATA_BLOB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_VALUE_META_DATA_BLOB_EXT {
    pub oszAttributeName: u32,
    pub oszObjectDn: u32,
    pub cbData: u32,
    pub obData: u32,
    pub ftimeDeleted: super::super::Foundation::FILETIME,
    pub ftimeCreated: super::super::Foundation::FILETIME,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
    pub oszLastOriginatingDsaDN: u32,
    pub dwUserIdentifier: u32,
    pub dwPriorLinkState: u32,
    pub dwCurrentLinkState: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_VALUE_META_DATA_BLOB_EXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_VALUE_META_DATA_BLOB_EXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DS_REPL_VALUE_META_DATA_EXT {
    pub pszAttributeName: ::windows_sys::core::PWSTR,
    pub pszObjectDn: ::windows_sys::core::PWSTR,
    pub cbData: u32,
    pub pbData: *mut u8,
    pub ftimeDeleted: super::super::Foundation::FILETIME,
    pub ftimeCreated: super::super::Foundation::FILETIME,
    pub dwVersion: u32,
    pub ftimeLastOriginatingChange: super::super::Foundation::FILETIME,
    pub uuidLastOriginatingDsaInvocationID: ::windows_sys::core::GUID,
    pub usnOriginatingChange: i64,
    pub usnLocalChange: i64,
    pub pszLastOriginatingDsaDN: ::windows_sys::core::PWSTR,
    pub dwUserIdentifier: u32,
    pub dwPriorLinkState: u32,
    pub dwCurrentLinkState: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DS_REPL_VALUE_META_DATA_EXT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DS_REPL_VALUE_META_DATA_EXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPSYNCALL_ERRINFOA {
    pub pszSvrId: ::windows_sys::core::PSTR,
    pub error: DS_REPSYNCALL_ERROR,
    pub dwWin32Err: u32,
    pub pszSrcId: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for DS_REPSYNCALL_ERRINFOA {}
impl ::core::clone::Clone for DS_REPSYNCALL_ERRINFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPSYNCALL_ERRINFOW {
    pub pszSvrId: ::windows_sys::core::PWSTR,
    pub error: DS_REPSYNCALL_ERROR,
    pub dwWin32Err: u32,
    pub pszSrcId: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for DS_REPSYNCALL_ERRINFOW {}
impl ::core::clone::Clone for DS_REPSYNCALL_ERRINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPSYNCALL_SYNCA {
    pub pszSrcId: ::windows_sys::core::PSTR,
    pub pszDstId: ::windows_sys::core::PSTR,
    pub pszNC: ::windows_sys::core::PSTR,
    pub pguidSrc: *mut ::windows_sys::core::GUID,
    pub pguidDst: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for DS_REPSYNCALL_SYNCA {}
impl ::core::clone::Clone for DS_REPSYNCALL_SYNCA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPSYNCALL_SYNCW {
    pub pszSrcId: ::windows_sys::core::PWSTR,
    pub pszDstId: ::windows_sys::core::PWSTR,
    pub pszNC: ::windows_sys::core::PWSTR,
    pub pguidSrc: *mut ::windows_sys::core::GUID,
    pub pguidDst: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for DS_REPSYNCALL_SYNCW {}
impl ::core::clone::Clone for DS_REPSYNCALL_SYNCW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPSYNCALL_UPDATEA {
    pub event: DS_REPSYNCALL_EVENT,
    pub pErrInfo: *mut DS_REPSYNCALL_ERRINFOA,
    pub pSync: *mut DS_REPSYNCALL_SYNCA,
}
impl ::core::marker::Copy for DS_REPSYNCALL_UPDATEA {}
impl ::core::clone::Clone for DS_REPSYNCALL_UPDATEA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_REPSYNCALL_UPDATEW {
    pub event: DS_REPSYNCALL_EVENT,
    pub pErrInfo: *mut DS_REPSYNCALL_ERRINFOW,
    pub pSync: *mut DS_REPSYNCALL_SYNCW,
}
impl ::core::marker::Copy for DS_REPSYNCALL_UPDATEW {}
impl ::core::clone::Clone for DS_REPSYNCALL_UPDATEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_SCHEMA_GUID_MAPA {
    pub guid: ::windows_sys::core::GUID,
    pub guidType: u32,
    pub pName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for DS_SCHEMA_GUID_MAPA {}
impl ::core::clone::Clone for DS_SCHEMA_GUID_MAPA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_SCHEMA_GUID_MAPW {
    pub guid: ::windows_sys::core::GUID,
    pub guidType: u32,
    pub pName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for DS_SCHEMA_GUID_MAPW {}
impl ::core::clone::Clone for DS_SCHEMA_GUID_MAPW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct DS_SELECTION {
    pub pwzName: ::windows_sys::core::PWSTR,
    pub pwzADsPath: ::windows_sys::core::PWSTR,
    pub pwzClass: ::windows_sys::core::PWSTR,
    pub pwzUPN: ::windows_sys::core::PWSTR,
    pub pvarFetchedAttributes: *mut super::super::System::Variant::VARIANT,
    pub flScopeType: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for DS_SELECTION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for DS_SELECTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct DS_SELECTION_LIST {
    pub cItems: u32,
    pub cFetchedAttributes: u32,
    pub aDsSelection: [DS_SELECTION; 1],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for DS_SELECTION_LIST {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for DS_SELECTION_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DS_SITE_COST_INFO {
    pub errorCode: u32,
    pub cost: u32,
}
impl ::core::marker::Copy for DS_SITE_COST_INFO {}
impl ::core::clone::Clone for DS_SITE_COST_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"]
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
pub struct OPENQUERYWINDOW {
    pub cbStruct: u32,
    pub dwFlags: u32,
    pub clsidHandler: ::windows_sys::core::GUID,
    pub pHandlerParameters: *mut ::core::ffi::c_void,
    pub clsidDefaultForm: ::windows_sys::core::GUID,
    pub pPersistQuery: IPersistQuery,
    pub Anonymous: OPENQUERYWINDOW_0,
}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::marker::Copy for OPENQUERYWINDOW {}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::clone::Clone for OPENQUERYWINDOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"]
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
pub union OPENQUERYWINDOW_0 {
    pub pFormParameters: *mut ::core::ffi::c_void,
    pub ppbFormParameters: super::super::System::Com::StructuredStorage::IPropertyBag,
}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::marker::Copy for OPENQUERYWINDOW_0 {}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::clone::Clone for OPENQUERYWINDOW_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCHEDULE {
    pub Size: u32,
    pub Bandwidth: u32,
    pub NumberOfSchedules: u32,
    pub Schedules: [SCHEDULE_HEADER; 1],
}
impl ::core::marker::Copy for SCHEDULE {}
impl ::core::clone::Clone for SCHEDULE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCHEDULE_HEADER {
    pub Type: u32,
    pub Offset: u32,
}
impl ::core::marker::Copy for SCHEDULE_HEADER {}
impl ::core::clone::Clone for SCHEDULE_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub type LPCQADDFORMSPROC = ::core::option::Option<unsafe extern "system" fn(lparam: super::super::Foundation::LPARAM, pform: *mut CQFORM) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub type LPCQADDPAGESPROC = ::core::option::Option<unsafe extern "system" fn(lparam: super::super::Foundation::LPARAM, clsidform: *const ::windows_sys::core::GUID, ppage: *mut CQPAGE) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
pub type LPCQPAGEPROC = ::core::option::Option<unsafe extern "system" fn(ppage: *mut CQPAGE, hwnd: super::super::Foundation::HWND, umsg: u32, wparam: super::super::Foundation::WPARAM, lparam: super::super::Foundation::LPARAM) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPDSENUMATTRIBUTES = ::core::option::Option<unsafe extern "system" fn(lparam: super::super::Foundation::LPARAM, pszattributename: ::windows_sys::core::PCWSTR, pszdisplayname: ::windows_sys::core::PCWSTR, dwflags: u32) -> ::windows_sys::core::HRESULT>;
