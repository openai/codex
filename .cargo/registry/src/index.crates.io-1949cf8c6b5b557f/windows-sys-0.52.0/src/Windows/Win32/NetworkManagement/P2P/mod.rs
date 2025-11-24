::windows_targets::link!("drt.dll" "system" fn DrtClose(hdrt : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("drt.dll" "system" fn DrtContinueSearch(hsearchcontext : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("drtprov.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn DrtCreateDerivedKey(plocalcert : *const super::super::Security::Cryptography:: CERT_CONTEXT, pkey : *mut DRT_DATA) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("drtprov.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn DrtCreateDerivedKeySecurityProvider(prootcert : *const super::super::Security::Cryptography:: CERT_CONTEXT, plocalcert : *const super::super::Security::Cryptography:: CERT_CONTEXT, ppsecurityprovider : *mut *mut DRT_SECURITY_PROVIDER) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drtprov.dll" "system" fn DrtCreateDnsBootstrapResolver(port : u16, pwszaddress : ::windows_sys::core::PCWSTR, ppmodule : *mut *mut DRT_BOOTSTRAP_PROVIDER) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drttransport.dll" "system" fn DrtCreateIpv6UdpTransport(scope : DRT_SCOPE, dwscopeid : u32, dwlocalitythreshold : u32, pwport : *mut u16, phtransport : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drtprov.dll" "system" fn DrtCreateNullSecurityProvider(ppsecurityprovider : *mut *mut DRT_SECURITY_PROVIDER) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("drtprov.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DrtCreatePnrpBootstrapResolver(fpublish : super::super::Foundation:: BOOL, pwzpeername : ::windows_sys::core::PCWSTR, pwzcloudname : ::windows_sys::core::PCWSTR, pwzpublishingidentity : ::windows_sys::core::PCWSTR, ppresolver : *mut *mut DRT_BOOTSTRAP_PROVIDER) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drtprov.dll" "system" fn DrtDeleteDerivedKeySecurityProvider(psecurityprovider : *const DRT_SECURITY_PROVIDER) -> ());
::windows_targets::link!("drtprov.dll" "system" fn DrtDeleteDnsBootstrapResolver(presolver : *const DRT_BOOTSTRAP_PROVIDER) -> ());
::windows_targets::link!("drttransport.dll" "system" fn DrtDeleteIpv6UdpTransport(htransport : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drtprov.dll" "system" fn DrtDeleteNullSecurityProvider(psecurityprovider : *const DRT_SECURITY_PROVIDER) -> ());
::windows_targets::link!("drtprov.dll" "system" fn DrtDeletePnrpBootstrapResolver(presolver : *const DRT_BOOTSTRAP_PROVIDER) -> ());
::windows_targets::link!("drt.dll" "system" fn DrtEndSearch(hsearchcontext : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("drt.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn DrtGetEventData(hdrt : *const ::core::ffi::c_void, uleventdatalen : u32, peventdata : *mut DRT_EVENT_DATA) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtGetEventDataSize(hdrt : *const ::core::ffi::c_void, puleventdatalen : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtGetInstanceName(hdrt : *const ::core::ffi::c_void, ulcbinstancenamesize : u32, pwzdrtinstancename : ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtGetInstanceNameSize(hdrt : *const ::core::ffi::c_void, pulcbinstancenamesize : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("drt.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn DrtGetSearchPath(hsearchcontext : *const ::core::ffi::c_void, ulsearchpathsize : u32, psearchpath : *mut DRT_ADDRESS_LIST) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtGetSearchPathSize(hsearchcontext : *const ::core::ffi::c_void, pulsearchpathsize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtGetSearchResult(hsearchcontext : *const ::core::ffi::c_void, ulsearchresultsize : u32, psearchresult : *mut DRT_SEARCH_RESULT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtGetSearchResultSize(hsearchcontext : *const ::core::ffi::c_void, pulsearchresultsize : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("drt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DrtOpen(psettings : *const DRT_SETTINGS, hevent : super::super::Foundation:: HANDLE, pvcontext : *const ::core::ffi::c_void, phdrt : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtRegisterKey(hdrt : *const ::core::ffi::c_void, pregistration : *const DRT_REGISTRATION, pvkeycontext : *const ::core::ffi::c_void, phkeyregistration : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("drt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DrtStartSearch(hdrt : *const ::core::ffi::c_void, pkey : *const DRT_DATA, pinfo : *const DRT_SEARCH_INFO, timeout : u32, hevent : super::super::Foundation:: HANDLE, pvcontext : *const ::core::ffi::c_void, hsearchcontext : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("drt.dll" "system" fn DrtUnregisterKey(hkeyregistration : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("drt.dll" "system" fn DrtUpdateKey(hkeyregistration : *const ::core::ffi::c_void, pappdata : *const DRT_DATA) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabAddContact(pwzcontactdata : ::windows_sys::core::PCWSTR, ppcontact : *mut *mut PEER_CONTACT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn PeerCollabAsyncInviteContact(pccontact : *const PEER_CONTACT, pcendpoint : *const PEER_ENDPOINT, pcinvitation : *const PEER_INVITATION, hevent : super::super::Foundation:: HANDLE, phinvitation : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn PeerCollabAsyncInviteEndpoint(pcendpoint : *const PEER_ENDPOINT, pcinvitation : *const PEER_INVITATION, hevent : super::super::Foundation:: HANDLE, phinvitation : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabCancelInvitation(hinvitation : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabCloseHandle(hinvitation : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabDeleteContact(pwzpeername : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabDeleteEndpointData(pcendpoint : *const PEER_ENDPOINT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabDeleteObject(pobjectid : *const ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabEnumApplicationRegistrationInfo(registrationtype : PEER_APPLICATION_REGISTRATION_TYPE, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabEnumApplications(pcendpoint : *const PEER_ENDPOINT, papplicationid : *const ::windows_sys::core::GUID, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabEnumContacts(phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabEnumEndpoints(pccontact : *const PEER_CONTACT, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabEnumObjects(pcendpoint : *const PEER_ENDPOINT, pobjectid : *const ::windows_sys::core::GUID, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabEnumPeopleNearMe(phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabExportContact(pwzpeername : ::windows_sys::core::PCWSTR, ppwzcontactdata : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn PeerCollabGetAppLaunchInfo(pplaunchinfo : *mut *mut PEER_APP_LAUNCH_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabGetApplicationRegistrationInfo(papplicationid : *const ::windows_sys::core::GUID, registrationtype : PEER_APPLICATION_REGISTRATION_TYPE, ppapplication : *mut *mut PEER_APPLICATION_REGISTRATION_INFO) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabGetContact(pwzpeername : ::windows_sys::core::PCWSTR, ppcontact : *mut *mut PEER_CONTACT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabGetEndpointName(ppwzendpointname : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn PeerCollabGetEventData(hpeerevent : *const ::core::ffi::c_void, ppeventdata : *mut *mut PEER_COLLAB_EVENT_DATA) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabGetInvitationResponse(hinvitation : super::super::Foundation:: HANDLE, ppinvitationresponse : *mut *mut PEER_INVITATION_RESPONSE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabGetPresenceInfo(pcendpoint : *const PEER_ENDPOINT, pppresenceinfo : *mut *mut PEER_PRESENCE_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabGetSigninOptions(pdwsigninoptions : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"] fn PeerCollabInviteContact(pccontact : *const PEER_CONTACT, pcendpoint : *const PEER_ENDPOINT, pcinvitation : *const PEER_INVITATION, ppresponse : *mut *mut PEER_INVITATION_RESPONSE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabInviteEndpoint(pcendpoint : *const PEER_ENDPOINT, pcinvitation : *const PEER_INVITATION, ppresponse : *mut *mut PEER_INVITATION_RESPONSE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabParseContact(pwzcontactdata : ::windows_sys::core::PCWSTR, ppcontact : *mut *mut PEER_CONTACT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabQueryContactData(pcendpoint : *const PEER_ENDPOINT, ppwzcontactdata : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabRefreshEndpointData(pcendpoint : *const PEER_ENDPOINT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabRegisterApplication(pcapplication : *const PEER_APPLICATION_REGISTRATION_INFO, registrationtype : PEER_APPLICATION_REGISTRATION_TYPE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabRegisterEvent(hevent : super::super::Foundation:: HANDLE, ceventregistration : u32, peventregistrations : *const PEER_COLLAB_EVENT_REGISTRATION, phpeerevent : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabSetEndpointName(pwzendpointname : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabSetObject(pcobject : *const PEER_OBJECT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabSetPresenceInfo(pcpresenceinfo : *const PEER_PRESENCE_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabShutdown() -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabSignin(hwndparent : super::super::Foundation:: HWND, dwsigninoptions : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabSignout(dwsigninoptions : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabStartup(wversionrequested : u16) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabSubscribeEndpointData(pcendpoint : *const PEER_ENDPOINT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabUnregisterApplication(papplicationid : *const ::windows_sys::core::GUID, registrationtype : PEER_APPLICATION_REGISTRATION_TYPE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCollabUnregisterEvent(hpeerevent : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerCollabUnsubscribeEndpointData(pcendpoint : *const PEER_ENDPOINT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerCollabUpdateContact(pcontact : *const PEER_CONTACT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerCreatePeerName(pwzidentity : ::windows_sys::core::PCWSTR, pwzclassifier : ::windows_sys::core::PCWSTR, ppwzpeername : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistClientAddContentInformation(hpeerdist : isize, hcontenthandle : isize, cbnumberofbytes : u32, pbuffer : *const u8, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistClientAddData(hpeerdist : isize, hcontenthandle : isize, cbnumberofbytes : u32, pbuffer : *const u8, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistClientBlockRead(hpeerdist : isize, hcontenthandle : isize, cbmaxnumberofbytes : u32, pbuffer : *mut u8, dwtimeoutinmilliseconds : u32, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistClientCancelAsyncOperation(hpeerdist : isize, hcontenthandle : isize, poverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistClientCloseContent(hpeerdist : isize, hcontenthandle : isize) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistClientCompleteContentInformation(hpeerdist : isize, hcontenthandle : isize, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistClientFlushContent(hpeerdist : isize, pcontenttag : *const PEERDIST_CONTENT_TAG, hcompletionport : super::super::Foundation:: HANDLE, ulcompletionkey : usize, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistClientGetInformationByHandle(hpeerdist : isize, hcontenthandle : isize, peerdistclientinfoclass : PEERDIST_CLIENT_INFO_BY_HANDLE_CLASS, dwbuffersize : u32, lpinformation : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerDistClientOpenContent(hpeerdist : isize, pcontenttag : *const PEERDIST_CONTENT_TAG, hcompletionport : super::super::Foundation:: HANDLE, ulcompletionkey : usize, phcontenthandle : *mut isize) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistClientStreamRead(hpeerdist : isize, hcontenthandle : isize, cbmaxnumberofbytes : u32, pbuffer : *mut u8, dwtimeoutinmilliseconds : u32, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistGetOverlappedResult(lpoverlapped : *const super::super::System::IO:: OVERLAPPED, lpnumberofbytestransferred : *mut u32, bwait : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistGetStatus(hpeerdist : isize, ppeerdiststatus : *mut PEERDIST_STATUS) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistGetStatusEx(hpeerdist : isize, ppeerdiststatus : *mut PEERDIST_STATUS_INFO) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistRegisterForStatusChangeNotification(hpeerdist : isize, hcompletionport : super::super::Foundation:: HANDLE, ulcompletionkey : usize, lpoverlapped : *const super::super::System::IO:: OVERLAPPED, ppeerdiststatus : *mut PEERDIST_STATUS) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistRegisterForStatusChangeNotificationEx(hpeerdist : isize, hcompletionport : super::super::Foundation:: HANDLE, ulcompletionkey : usize, lpoverlapped : *const super::super::System::IO:: OVERLAPPED, ppeerdiststatus : *mut PEERDIST_STATUS_INFO) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistServerCancelAsyncOperation(hpeerdist : isize, cbcontentidentifier : u32, pcontentidentifier : *const u8, poverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistServerCloseContentInformation(hpeerdist : isize, hcontentinfo : isize) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistServerCloseStreamHandle(hpeerdist : isize, hstream : isize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerDistServerOpenContentInformation(hpeerdist : isize, cbcontentidentifier : u32, pcontentidentifier : *const u8, ullcontentoffset : u64, cbcontentlength : u64, hcompletionport : super::super::Foundation:: HANDLE, ulcompletionkey : usize, phcontentinfo : *mut isize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerDistServerOpenContentInformationEx(hpeerdist : isize, cbcontentidentifier : u32, pcontentidentifier : *const u8, ullcontentoffset : u64, cbcontentlength : u64, pretrievaloptions : *const PEERDIST_RETRIEVAL_OPTIONS, hcompletionport : super::super::Foundation:: HANDLE, ulcompletionkey : usize, phcontentinfo : *mut isize) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistServerPublishAddToStream(hpeerdist : isize, hstream : isize, cbnumberofbytes : u32, pbuffer : *const u8, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistServerPublishCompleteStream(hpeerdist : isize, hstream : isize, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerDistServerPublishStream(hpeerdist : isize, cbcontentidentifier : u32, pcontentidentifier : *const u8, cbcontentlength : u64, ppublishoptions : *const PEERDIST_PUBLICATION_OPTIONS, hcompletionport : super::super::Foundation:: HANDLE, ulcompletionkey : usize, phstream : *mut isize) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("peerdist.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn PeerDistServerRetrieveContentInformation(hpeerdist : isize, hcontentinfo : isize, cbmaxnumberofbytes : u32, pbuffer : *mut u8, lpoverlapped : *const super::super::System::IO:: OVERLAPPED) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistServerUnpublish(hpeerdist : isize, cbcontentidentifier : u32, pcontentidentifier : *const u8) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistShutdown(hpeerdist : isize) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistStartup(dwversionrequested : u32, phpeerdist : *mut isize, pdwsupportedversion : *mut u32) -> u32);
::windows_targets::link!("peerdist.dll" "system" fn PeerDistUnregisterForStatusChangeNotification(hpeerdist : isize) -> u32);
::windows_targets::link!("p2p.dll" "system" fn PeerEndEnumeration(hpeerenum : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerEnumGroups(pwzidentity : ::windows_sys::core::PCWSTR, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerEnumIdentities(phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerFreeData(pvdata : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("p2p.dll" "system" fn PeerGetItemCount(hpeerenum : *const ::core::ffi::c_void, pcount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGetNextItem(hpeerenum : *const ::core::ffi::c_void, pcount : *mut u32, pppvitems : *mut *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphAddRecord(hgraph : *const ::core::ffi::c_void, precord : *const PEER_RECORD, precordid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphClose(hgraph : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphCloseDirectConnection(hgraph : *const ::core::ffi::c_void, ullconnectionid : u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerGraphConnect(hgraph : *const ::core::ffi::c_void, pwzpeerid : ::windows_sys::core::PCWSTR, paddress : *const PEER_ADDRESS, pullconnectionid : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphCreate(pgraphproperties : *const PEER_GRAPH_PROPERTIES, pwzdatabasename : ::windows_sys::core::PCWSTR, psecurityinterface : *const PEER_SECURITY_INTERFACE, phgraph : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphDelete(pwzgraphid : ::windows_sys::core::PCWSTR, pwzpeerid : ::windows_sys::core::PCWSTR, pwzdatabasename : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphDeleteRecord(hgraph : *const ::core::ffi::c_void, precordid : *const ::windows_sys::core::GUID, flocal : super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphEndEnumeration(hpeerenum : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphEnumConnections(hgraph : *const ::core::ffi::c_void, dwflags : u32, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphEnumNodes(hgraph : *const ::core::ffi::c_void, pwzpeerid : ::windows_sys::core::PCWSTR, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphEnumRecords(hgraph : *const ::core::ffi::c_void, precordtype : *const ::windows_sys::core::GUID, pwzpeerid : ::windows_sys::core::PCWSTR, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphExportDatabase(hgraph : *const ::core::ffi::c_void, pwzfilepath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphFreeData(pvdata : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphGetEventData(hpeerevent : *const ::core::ffi::c_void, ppeventdata : *mut *mut PEER_GRAPH_EVENT_DATA) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphGetItemCount(hpeerenum : *const ::core::ffi::c_void, pcount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphGetNextItem(hpeerenum : *const ::core::ffi::c_void, pcount : *mut u32, pppvitems : *mut *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerGraphGetNodeInfo(hgraph : *const ::core::ffi::c_void, ullnodeid : u64, ppnodeinfo : *mut *mut PEER_NODE_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphGetProperties(hgraph : *const ::core::ffi::c_void, ppgraphproperties : *mut *mut PEER_GRAPH_PROPERTIES) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphGetRecord(hgraph : *const ::core::ffi::c_void, precordid : *const ::windows_sys::core::GUID, pprecord : *mut *mut PEER_RECORD) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphGetStatus(hgraph : *const ::core::ffi::c_void, pdwstatus : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphImportDatabase(hgraph : *const ::core::ffi::c_void, pwzfilepath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphListen(hgraph : *const ::core::ffi::c_void, dwscope : u32, dwscopeid : u32, wport : u16) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphOpen(pwzgraphid : ::windows_sys::core::PCWSTR, pwzpeerid : ::windows_sys::core::PCWSTR, pwzdatabasename : ::windows_sys::core::PCWSTR, psecurityinterface : *const PEER_SECURITY_INTERFACE, crecordtypesyncprecedence : u32, precordtypesyncprecedence : *const ::windows_sys::core::GUID, phgraph : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerGraphOpenDirectConnection(hgraph : *const ::core::ffi::c_void, pwzpeerid : ::windows_sys::core::PCWSTR, paddress : *const PEER_ADDRESS, pullconnectionid : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphPeerTimeToUniversalTime(hgraph : *const ::core::ffi::c_void, pftpeertime : *const super::super::Foundation:: FILETIME, pftuniversaltime : *mut super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphRegisterEvent(hgraph : *const ::core::ffi::c_void, hevent : super::super::Foundation:: HANDLE, ceventregistrations : u32, peventregistrations : *const PEER_GRAPH_EVENT_REGISTRATION, phpeerevent : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphSearchRecords(hgraph : *const ::core::ffi::c_void, pwzcriteria : ::windows_sys::core::PCWSTR, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphSendData(hgraph : *const ::core::ffi::c_void, ullconnectionid : u64, ptype : *const ::windows_sys::core::GUID, cbdata : u32, pvdata : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphSetNodeAttributes(hgraph : *const ::core::ffi::c_void, pwzattributes : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphSetPresence(hgraph : *const ::core::ffi::c_void, fpresent : super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphSetProperties(hgraph : *const ::core::ffi::c_void, pgraphproperties : *const PEER_GRAPH_PROPERTIES) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphShutdown() -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphStartup(wversionrequested : u16, pversiondata : *mut PEER_VERSION_DATA) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphUniversalTimeToPeerTime(hgraph : *const ::core::ffi::c_void, pftuniversaltime : *const super::super::Foundation:: FILETIME, pftpeertime : *mut super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphUnregisterEvent(hpeerevent : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2pgraph.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGraphUpdateRecord(hgraph : *const ::core::ffi::c_void, precord : *const PEER_RECORD) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2pgraph.dll" "system" fn PeerGraphValidateDeferredRecords(hgraph : *const ::core::ffi::c_void, crecordids : u32, precordids : *const ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupAddRecord(hgroup : *const ::core::ffi::c_void, precord : *const PEER_RECORD, precordid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupClose(hgroup : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupCloseDirectConnection(hgroup : *const ::core::ffi::c_void, ullconnectionid : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupConnect(hgroup : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerGroupConnectByAddress(hgroup : *const ::core::ffi::c_void, caddresses : u32, paddresses : *const PEER_ADDRESS) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupCreate(pproperties : *const PEER_GROUP_PROPERTIES, phgroup : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupCreateInvitation(hgroup : *const ::core::ffi::c_void, pwzidentityinfo : ::windows_sys::core::PCWSTR, pftexpiration : *const super::super::Foundation:: FILETIME, croles : u32, proles : *const ::windows_sys::core::GUID, ppwzinvitation : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupCreatePasswordInvitation(hgroup : *const ::core::ffi::c_void, ppwzinvitation : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupDelete(pwzidentity : ::windows_sys::core::PCWSTR, pwzgrouppeername : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupDeleteRecord(hgroup : *const ::core::ffi::c_void, precordid : *const ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupEnumConnections(hgroup : *const ::core::ffi::c_void, dwflags : u32, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupEnumMembers(hgroup : *const ::core::ffi::c_void, dwflags : u32, pwzidentity : ::windows_sys::core::PCWSTR, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupEnumRecords(hgroup : *const ::core::ffi::c_void, precordtype : *const ::windows_sys::core::GUID, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupExportConfig(hgroup : *const ::core::ffi::c_void, pwzpassword : ::windows_sys::core::PCWSTR, ppwzxml : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupExportDatabase(hgroup : *const ::core::ffi::c_void, pwzfilepath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupGetEventData(hpeerevent : *const ::core::ffi::c_void, ppeventdata : *mut *mut PEER_GROUP_EVENT_DATA) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupGetProperties(hgroup : *const ::core::ffi::c_void, ppproperties : *mut *mut PEER_GROUP_PROPERTIES) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupGetRecord(hgroup : *const ::core::ffi::c_void, precordid : *const ::windows_sys::core::GUID, pprecord : *mut *mut PEER_RECORD) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupGetStatus(hgroup : *const ::core::ffi::c_void, pdwstatus : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupImportConfig(pwzxml : ::windows_sys::core::PCWSTR, pwzpassword : ::windows_sys::core::PCWSTR, foverwrite : super::super::Foundation:: BOOL, ppwzidentity : *mut ::windows_sys::core::PWSTR, ppwzgroup : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupImportDatabase(hgroup : *const ::core::ffi::c_void, pwzfilepath : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn PeerGroupIssueCredentials(hgroup : *const ::core::ffi::c_void, pwzsubjectidentity : ::windows_sys::core::PCWSTR, pcredentialinfo : *const PEER_CREDENTIAL_INFO, dwflags : u32, ppwzinvitation : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupJoin(pwzidentity : ::windows_sys::core::PCWSTR, pwzinvitation : ::windows_sys::core::PCWSTR, pwzcloud : ::windows_sys::core::PCWSTR, phgroup : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupOpen(pwzidentity : ::windows_sys::core::PCWSTR, pwzgrouppeername : ::windows_sys::core::PCWSTR, pwzcloud : ::windows_sys::core::PCWSTR, phgroup : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerGroupOpenDirectConnection(hgroup : *const ::core::ffi::c_void, pwzidentity : ::windows_sys::core::PCWSTR, paddress : *const PEER_ADDRESS, pullconnectionid : *mut u64) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn PeerGroupParseInvitation(pwzinvitation : ::windows_sys::core::PCWSTR, ppinvitationinfo : *mut *mut PEER_INVITATION_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupPasswordJoin(pwzidentity : ::windows_sys::core::PCWSTR, pwzinvitation : ::windows_sys::core::PCWSTR, pwzpassword : ::windows_sys::core::PCWSTR, pwzcloud : ::windows_sys::core::PCWSTR, phgroup : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupPeerTimeToUniversalTime(hgroup : *const ::core::ffi::c_void, pftpeertime : *const super::super::Foundation:: FILETIME, pftuniversaltime : *mut super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupRegisterEvent(hgroup : *const ::core::ffi::c_void, hevent : super::super::Foundation:: HANDLE, ceventregistration : u32, peventregistrations : *const PEER_GROUP_EVENT_REGISTRATION, phpeerevent : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupResumePasswordAuthentication(hgroup : *const ::core::ffi::c_void, hpeereventhandle : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupSearchRecords(hgroup : *const ::core::ffi::c_void, pwzcriteria : ::windows_sys::core::PCWSTR, phpeerenum : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupSendData(hgroup : *const ::core::ffi::c_void, ullconnectionid : u64, ptype : *const ::windows_sys::core::GUID, cbdata : u32, pvdata : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupSetProperties(hgroup : *const ::core::ffi::c_void, pproperties : *const PEER_GROUP_PROPERTIES) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupShutdown() -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupStartup(wversionrequested : u16, pversiondata : *mut PEER_VERSION_DATA) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupUniversalTimeToPeerTime(hgroup : *const ::core::ffi::c_void, pftuniversaltime : *const super::super::Foundation:: FILETIME, pftpeertime : *mut super::super::Foundation:: FILETIME) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerGroupUnregisterEvent(hpeerevent : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerGroupUpdateRecord(hgroup : *const ::core::ffi::c_void, precord : *const PEER_RECORD) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerHostNameToPeerName(pwzhostname : ::windows_sys::core::PCWSTR, ppwzpeername : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityCreate(pwzclassifier : ::windows_sys::core::PCWSTR, pwzfriendlyname : ::windows_sys::core::PCWSTR, hcryptprov : usize, ppwzidentity : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityDelete(pwzidentity : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityExport(pwzidentity : ::windows_sys::core::PCWSTR, pwzpassword : ::windows_sys::core::PCWSTR, ppwzexportxml : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityGetCryptKey(pwzidentity : ::windows_sys::core::PCWSTR, phcryptprov : *mut usize) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityGetDefault(ppwzpeername : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityGetFriendlyName(pwzidentity : ::windows_sys::core::PCWSTR, ppwzfriendlyname : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityGetXML(pwzidentity : ::windows_sys::core::PCWSTR, ppwzidentityxml : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentityImport(pwzimportxml : ::windows_sys::core::PCWSTR, pwzpassword : ::windows_sys::core::PCWSTR, ppwzidentity : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerIdentitySetFriendlyName(pwzidentity : ::windows_sys::core::PCWSTR, pwzfriendlyname : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerNameToPeerHostName(pwzpeername : ::windows_sys::core::PCWSTR, ppwzhostname : *mut ::windows_sys::core::PWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerPnrpEndResolve(hresolve : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerPnrpGetCloudInfo(pcnumclouds : *mut u32, ppcloudinfo : *mut *mut PEER_PNRP_CLOUD_INFO) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerPnrpGetEndpoint(hresolve : *const ::core::ffi::c_void, ppendpoint : *mut *mut PEER_PNRP_ENDPOINT_INFO) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerPnrpRegister(pcwzpeername : ::windows_sys::core::PCWSTR, pregistrationinfo : *const PEER_PNRP_REGISTRATION_INFO, phregistration : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerPnrpResolve(pcwzpeername : ::windows_sys::core::PCWSTR, pcwzcloudname : ::windows_sys::core::PCWSTR, pcendpoints : *mut u32, ppendpoints : *mut *mut PEER_PNRP_ENDPOINT_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerPnrpShutdown() -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PeerPnrpStartResolve(pcwzpeername : ::windows_sys::core::PCWSTR, pcwzcloudname : ::windows_sys::core::PCWSTR, cmaxendpoints : u32, hevent : super::super::Foundation:: HANDLE, phresolve : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerPnrpStartup(wversionrequested : u16) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("p2p.dll" "system" fn PeerPnrpUnregister(hregistration : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Networking_WinSock")]
::windows_targets::link!("p2p.dll" "system" #[doc = "Required features: `\"Win32_Networking_WinSock\"`"] fn PeerPnrpUpdateRegistration(hregistration : *const ::core::ffi::c_void, pregistrationinfo : *const PEER_PNRP_REGISTRATION_INFO) -> ::windows_sys::core::HRESULT);
pub const DRT_ACTIVE: DRT_STATUS = 0i32;
pub const DRT_ADDRESS_FLAG_ACCEPTED: DRT_ADDRESS_FLAGS = 1i32;
pub const DRT_ADDRESS_FLAG_BAD_VALIDATE_ID: DRT_ADDRESS_FLAGS = 32i32;
pub const DRT_ADDRESS_FLAG_INQUIRE: DRT_ADDRESS_FLAGS = 128i32;
pub const DRT_ADDRESS_FLAG_LOOP: DRT_ADDRESS_FLAGS = 8i32;
pub const DRT_ADDRESS_FLAG_REJECTED: DRT_ADDRESS_FLAGS = 2i32;
pub const DRT_ADDRESS_FLAG_SUSPECT_UNREGISTERED_ID: DRT_ADDRESS_FLAGS = 64i32;
pub const DRT_ADDRESS_FLAG_TOO_BUSY: DRT_ADDRESS_FLAGS = 16i32;
pub const DRT_ADDRESS_FLAG_UNREACHABLE: DRT_ADDRESS_FLAGS = 4i32;
pub const DRT_ALONE: DRT_STATUS = 1i32;
pub const DRT_EVENT_LEAFSET_KEY_CHANGED: DRT_EVENT_TYPE = 1i32;
pub const DRT_EVENT_REGISTRATION_STATE_CHANGED: DRT_EVENT_TYPE = 2i32;
pub const DRT_EVENT_STATUS_CHANGED: DRT_EVENT_TYPE = 0i32;
pub const DRT_E_BOOTSTRAPPROVIDER_IN_USE: ::windows_sys::core::HRESULT = -2141052914i32;
pub const DRT_E_BOOTSTRAPPROVIDER_NOT_ATTACHED: ::windows_sys::core::HRESULT = -2141052913i32;
pub const DRT_E_CAPABILITY_MISMATCH: ::windows_sys::core::HRESULT = -2141052657i32;
pub const DRT_E_DUPLICATE_KEY: ::windows_sys::core::HRESULT = -2141052919i32;
pub const DRT_E_FAULTED: ::windows_sys::core::HRESULT = -2141052662i32;
pub const DRT_E_INSUFFICIENT_BUFFER: ::windows_sys::core::HRESULT = -2141052660i32;
pub const DRT_E_INVALID_ADDRESS: ::windows_sys::core::HRESULT = -2141052923i32;
pub const DRT_E_INVALID_BOOTSTRAP_PROVIDER: ::windows_sys::core::HRESULT = -2141052924i32;
pub const DRT_E_INVALID_CERT_CHAIN: ::windows_sys::core::HRESULT = -2141057020i32;
pub const DRT_E_INVALID_INSTANCE_PREFIX: ::windows_sys::core::HRESULT = -2141052659i32;
pub const DRT_E_INVALID_KEY: ::windows_sys::core::HRESULT = -2141057015i32;
pub const DRT_E_INVALID_KEY_SIZE: ::windows_sys::core::HRESULT = -2141057022i32;
pub const DRT_E_INVALID_MAX_ADDRESSES: ::windows_sys::core::HRESULT = -2141057017i32;
pub const DRT_E_INVALID_MAX_ENDPOINTS: ::windows_sys::core::HRESULT = -2141057007i32;
pub const DRT_E_INVALID_MESSAGE: ::windows_sys::core::HRESULT = -2141057019i32;
pub const DRT_E_INVALID_PORT: ::windows_sys::core::HRESULT = -2141052928i32;
pub const DRT_E_INVALID_SCOPE: ::windows_sys::core::HRESULT = -2141052922i32;
pub const DRT_E_INVALID_SEARCH_INFO: ::windows_sys::core::HRESULT = -2141052663i32;
pub const DRT_E_INVALID_SEARCH_RANGE: ::windows_sys::core::HRESULT = -2141057006i32;
pub const DRT_E_INVALID_SECURITY_MODE: ::windows_sys::core::HRESULT = -2141052658i32;
pub const DRT_E_INVALID_SECURITY_PROVIDER: ::windows_sys::core::HRESULT = -2141052926i32;
pub const DRT_E_INVALID_SETTINGS: ::windows_sys::core::HRESULT = -2141052664i32;
pub const DRT_E_INVALID_TRANSPORT_PROVIDER: ::windows_sys::core::HRESULT = -2141052927i32;
pub const DRT_E_NO_ADDRESSES_AVAILABLE: ::windows_sys::core::HRESULT = -2141052920i32;
pub const DRT_E_NO_MORE: ::windows_sys::core::HRESULT = -2141057018i32;
pub const DRT_E_SEARCH_IN_PROGRESS: ::windows_sys::core::HRESULT = -2141057016i32;
pub const DRT_E_SECURITYPROVIDER_IN_USE: ::windows_sys::core::HRESULT = -2141052916i32;
pub const DRT_E_SECURITYPROVIDER_NOT_ATTACHED: ::windows_sys::core::HRESULT = -2141052915i32;
pub const DRT_E_STILL_IN_USE: ::windows_sys::core::HRESULT = -2141052925i32;
pub const DRT_E_TIMEOUT: ::windows_sys::core::HRESULT = -2141057023i32;
pub const DRT_E_TRANSPORTPROVIDER_IN_USE: ::windows_sys::core::HRESULT = -2141052918i32;
pub const DRT_E_TRANSPORTPROVIDER_NOT_ATTACHED: ::windows_sys::core::HRESULT = -2141052917i32;
pub const DRT_E_TRANSPORT_ALREADY_BOUND: ::windows_sys::core::HRESULT = -2141052671i32;
pub const DRT_E_TRANSPORT_ALREADY_EXISTS_FOR_SCOPE: ::windows_sys::core::HRESULT = -2141052665i32;
pub const DRT_E_TRANSPORT_EXECUTING_CALLBACK: ::windows_sys::core::HRESULT = -2141052666i32;
pub const DRT_E_TRANSPORT_INVALID_ARGUMENT: ::windows_sys::core::HRESULT = -2141052668i32;
pub const DRT_E_TRANSPORT_NOT_BOUND: ::windows_sys::core::HRESULT = -2141052670i32;
pub const DRT_E_TRANSPORT_NO_DEST_ADDRESSES: ::windows_sys::core::HRESULT = -2141052667i32;
pub const DRT_E_TRANSPORT_SHUTTING_DOWN: ::windows_sys::core::HRESULT = -2141052921i32;
pub const DRT_E_TRANSPORT_STILL_BOUND: ::windows_sys::core::HRESULT = -2141052661i32;
pub const DRT_E_TRANSPORT_UNEXPECTED: ::windows_sys::core::HRESULT = -2141052669i32;
pub const DRT_FAULTED: DRT_STATUS = 20i32;
pub const DRT_GLOBAL_SCOPE: DRT_SCOPE = 1i32;
pub const DRT_LEAFSET_KEY_ADDED: DRT_LEAFSET_KEY_CHANGE_TYPE = 0i32;
pub const DRT_LEAFSET_KEY_DELETED: DRT_LEAFSET_KEY_CHANGE_TYPE = 1i32;
pub const DRT_LINK_LOCAL_ISATAP_SCOPEID: u32 = 4294967295u32;
pub const DRT_LINK_LOCAL_SCOPE: DRT_SCOPE = 3i32;
pub const DRT_MATCH_EXACT: DRT_MATCH_TYPE = 0i32;
pub const DRT_MATCH_INTERMEDIATE: DRT_MATCH_TYPE = 2i32;
pub const DRT_MATCH_NEAR: DRT_MATCH_TYPE = 1i32;
pub const DRT_MAX_INSTANCE_PREFIX_LEN: u32 = 128u32;
pub const DRT_MAX_PAYLOAD_SIZE: u32 = 5120u32;
pub const DRT_MAX_ROUTING_ADDRESSES: u32 = 20u32;
pub const DRT_MIN_ROUTING_ADDRESSES: u32 = 1u32;
pub const DRT_NO_NETWORK: DRT_STATUS = 10i32;
pub const DRT_PAYLOAD_REVOKED: u32 = 1u32;
pub const DRT_REGISTRATION_STATE_UNRESOLVEABLE: DRT_REGISTRATION_STATE = 1i32;
pub const DRT_SECURE_CONFIDENTIALPAYLOAD: DRT_SECURITY_MODE = 2i32;
pub const DRT_SECURE_MEMBERSHIP: DRT_SECURITY_MODE = 1i32;
pub const DRT_SECURE_RESOLVE: DRT_SECURITY_MODE = 0i32;
pub const DRT_SITE_LOCAL_SCOPE: DRT_SCOPE = 2i32;
pub const DRT_S_RETRY: ::windows_sys::core::HRESULT = 6426640i32;
pub const FACILITY_DRT: u32 = 98u32;
pub const MaximumPeerDistClientInfoByHandlesClass: PEERDIST_CLIENT_INFO_BY_HANDLE_CLASS = 1i32;
pub const NS_PNRPCLOUD: u32 = 39u32;
pub const NS_PNRPNAME: u32 = 38u32;
pub const NS_PROVIDER_PNRPCLOUD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x03fe89ce_766d_4976_b9c1_bb9bc42c7b4d);
pub const NS_PROVIDER_PNRPNAME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x03fe89cd_766d_4976_b9c1_bb9bc42c7b4d);
pub const PEERDIST_PUBLICATION_OPTIONS_VERSION: i32 = 2i32;
pub const PEERDIST_PUBLICATION_OPTIONS_VERSION_1: i32 = 1i32;
pub const PEERDIST_PUBLICATION_OPTIONS_VERSION_2: i32 = 2i32;
pub const PEERDIST_READ_TIMEOUT_DEFAULT: u32 = 4294967294u32;
pub const PEERDIST_READ_TIMEOUT_LOCAL_CACHE_ONLY: u32 = 0u32;
pub const PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION: PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_VALUE = 2u32;
pub const PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_1: PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_VALUE = 1u32;
pub const PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_2: PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_VALUE = 2u32;
pub const PEERDIST_STATUS_AVAILABLE: PEERDIST_STATUS = 2i32;
pub const PEERDIST_STATUS_DISABLED: PEERDIST_STATUS = 0i32;
pub const PEERDIST_STATUS_UNAVAILABLE: PEERDIST_STATUS = 1i32;
pub const PEER_APPLICATION_ALL_USERS: PEER_APPLICATION_REGISTRATION_TYPE = 1i32;
pub const PEER_APPLICATION_CURRENT_USER: PEER_APPLICATION_REGISTRATION_TYPE = 0i32;
pub const PEER_CHANGE_ADDED: PEER_CHANGE_TYPE = 0i32;
pub const PEER_CHANGE_DELETED: PEER_CHANGE_TYPE = 1i32;
pub const PEER_CHANGE_UPDATED: PEER_CHANGE_TYPE = 2i32;
pub const PEER_COLLAB_OBJECTID_USER_PICTURE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdd15f41f_fc4e_4922_b035_4c06a754d01d);
pub const PEER_CONNECTED: PEER_CONNECTION_STATUS = 1i32;
pub const PEER_CONNECTION_DIRECT: PEER_CONNECTION_FLAGS = 2i32;
pub const PEER_CONNECTION_FAILED: PEER_CONNECTION_STATUS = 3i32;
pub const PEER_CONNECTION_NEIGHBOR: PEER_CONNECTION_FLAGS = 1i32;
pub const PEER_DEFER_EXPIRATION: PEER_GROUP_PROPERTY_FLAGS = 4i32;
pub const PEER_DISABLE_PRESENCE: PEER_GROUP_PROPERTY_FLAGS = 2i32;
pub const PEER_DISCONNECTED: PEER_CONNECTION_STATUS = 2i32;
pub const PEER_EVENT_ENDPOINT_APPLICATION_CHANGED: PEER_COLLAB_EVENT_TYPE = 4i32;
pub const PEER_EVENT_ENDPOINT_CHANGED: PEER_COLLAB_EVENT_TYPE = 2i32;
pub const PEER_EVENT_ENDPOINT_OBJECT_CHANGED: PEER_COLLAB_EVENT_TYPE = 5i32;
pub const PEER_EVENT_ENDPOINT_PRESENCE_CHANGED: PEER_COLLAB_EVENT_TYPE = 3i32;
pub const PEER_EVENT_MY_APPLICATION_CHANGED: PEER_COLLAB_EVENT_TYPE = 8i32;
pub const PEER_EVENT_MY_ENDPOINT_CHANGED: PEER_COLLAB_EVENT_TYPE = 6i32;
pub const PEER_EVENT_MY_OBJECT_CHANGED: PEER_COLLAB_EVENT_TYPE = 9i32;
pub const PEER_EVENT_MY_PRESENCE_CHANGED: PEER_COLLAB_EVENT_TYPE = 7i32;
pub const PEER_EVENT_PEOPLE_NEAR_ME_CHANGED: PEER_COLLAB_EVENT_TYPE = 10i32;
pub const PEER_EVENT_REQUEST_STATUS_CHANGED: PEER_COLLAB_EVENT_TYPE = 11i32;
pub const PEER_EVENT_WATCHLIST_CHANGED: PEER_COLLAB_EVENT_TYPE = 1i32;
pub const PEER_E_ALREADY_EXISTS: ::windows_sys::core::HRESULT = -2147024713i32;
pub const PEER_E_CLIENT_INVALID_COMPARTMENT_ID: ::windows_sys::core::HRESULT = -2147013390i32;
pub const PEER_E_CLOUD_DISABLED: ::windows_sys::core::HRESULT = -2147013394i32;
pub const PEER_E_CLOUD_IS_DEAD: ::windows_sys::core::HRESULT = -2147013387i32;
pub const PEER_E_CLOUD_IS_SEARCH_ONLY: ::windows_sys::core::HRESULT = -2147013391i32;
pub const PEER_E_CLOUD_NOT_FOUND: ::windows_sys::core::HRESULT = -2147013395i32;
pub const PEER_E_DISK_FULL: ::windows_sys::core::HRESULT = -2147024784i32;
pub const PEER_E_DUPLICATE_PEER_NAME: ::windows_sys::core::HRESULT = -2147013388i32;
pub const PEER_E_INVALID_IDENTITY: ::windows_sys::core::HRESULT = -2147013393i32;
pub const PEER_E_NOT_FOUND: ::windows_sys::core::HRESULT = -2147023728i32;
pub const PEER_E_TOO_MUCH_LOAD: ::windows_sys::core::HRESULT = -2147013392i32;
pub const PEER_GRAPH_EVENT_CONNECTION_REQUIRED: PEER_GRAPH_EVENT_TYPE = 7i32;
pub const PEER_GRAPH_EVENT_DIRECT_CONNECTION: PEER_GRAPH_EVENT_TYPE = 4i32;
pub const PEER_GRAPH_EVENT_INCOMING_DATA: PEER_GRAPH_EVENT_TYPE = 6i32;
pub const PEER_GRAPH_EVENT_NEIGHBOR_CONNECTION: PEER_GRAPH_EVENT_TYPE = 5i32;
pub const PEER_GRAPH_EVENT_NODE_CHANGED: PEER_GRAPH_EVENT_TYPE = 8i32;
pub const PEER_GRAPH_EVENT_PROPERTY_CHANGED: PEER_GRAPH_EVENT_TYPE = 2i32;
pub const PEER_GRAPH_EVENT_RECORD_CHANGED: PEER_GRAPH_EVENT_TYPE = 3i32;
pub const PEER_GRAPH_EVENT_STATUS_CHANGED: PEER_GRAPH_EVENT_TYPE = 1i32;
pub const PEER_GRAPH_EVENT_SYNCHRONIZED: PEER_GRAPH_EVENT_TYPE = 9i32;
pub const PEER_GRAPH_PROPERTY_DEFER_EXPIRATION: PEER_GRAPH_PROPERTY_FLAGS = 2i32;
pub const PEER_GRAPH_PROPERTY_HEARTBEATS: PEER_GRAPH_PROPERTY_FLAGS = 1i32;
pub const PEER_GRAPH_SCOPE_ANY: PEER_GRAPH_SCOPE = 0i32;
pub const PEER_GRAPH_SCOPE_GLOBAL: PEER_GRAPH_SCOPE = 1i32;
pub const PEER_GRAPH_SCOPE_LINKLOCAL: PEER_GRAPH_SCOPE = 3i32;
pub const PEER_GRAPH_SCOPE_LOOPBACK: PEER_GRAPH_SCOPE = 4i32;
pub const PEER_GRAPH_SCOPE_SITELOCAL: PEER_GRAPH_SCOPE = 2i32;
pub const PEER_GRAPH_STATUS_HAS_CONNECTIONS: PEER_GRAPH_STATUS_FLAGS = 2i32;
pub const PEER_GRAPH_STATUS_LISTENING: PEER_GRAPH_STATUS_FLAGS = 1i32;
pub const PEER_GRAPH_STATUS_SYNCHRONIZED: PEER_GRAPH_STATUS_FLAGS = 4i32;
pub const PEER_GROUP_EVENT_AUTHENTICATION_FAILED: PEER_GROUP_EVENT_TYPE = 11i32;
pub const PEER_GROUP_EVENT_CONNECTION_FAILED: PEER_GROUP_EVENT_TYPE = 10i32;
pub const PEER_GROUP_EVENT_DIRECT_CONNECTION: PEER_GROUP_EVENT_TYPE = 4i32;
pub const PEER_GROUP_EVENT_INCOMING_DATA: PEER_GROUP_EVENT_TYPE = 6i32;
pub const PEER_GROUP_EVENT_MEMBER_CHANGED: PEER_GROUP_EVENT_TYPE = 8i32;
pub const PEER_GROUP_EVENT_NEIGHBOR_CONNECTION: PEER_GROUP_EVENT_TYPE = 5i32;
pub const PEER_GROUP_EVENT_PROPERTY_CHANGED: PEER_GROUP_EVENT_TYPE = 2i32;
pub const PEER_GROUP_EVENT_RECORD_CHANGED: PEER_GROUP_EVENT_TYPE = 3i32;
pub const PEER_GROUP_EVENT_STATUS_CHANGED: PEER_GROUP_EVENT_TYPE = 1i32;
pub const PEER_GROUP_GMC_AUTHENTICATION: PEER_GROUP_AUTHENTICATION_SCHEME = 1i32;
pub const PEER_GROUP_PASSWORD_AUTHENTICATION: PEER_GROUP_AUTHENTICATION_SCHEME = 2i32;
pub const PEER_GROUP_ROLE_ADMIN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x04387127_aa56_450a_8ce5_4f565c6790f4);
pub const PEER_GROUP_ROLE_INVITING_MEMBER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4370fd89_dc18_4cfb_8dbf_9853a8a9f905);
pub const PEER_GROUP_ROLE_MEMBER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf12dc4c7_0857_4ca0_93fc_b1bb19a3d8c2);
pub const PEER_GROUP_STATUS_HAS_CONNECTIONS: PEER_GROUP_STATUS = 2i32;
pub const PEER_GROUP_STATUS_LISTENING: PEER_GROUP_STATUS = 1i32;
pub const PEER_GROUP_STORE_CREDENTIALS: PEER_GROUP_ISSUE_CREDENTIAL_FLAGS = 1i32;
pub const PEER_INVITATION_RESPONSE_ACCEPTED: PEER_INVITATION_RESPONSE_TYPE = 1i32;
pub const PEER_INVITATION_RESPONSE_DECLINED: PEER_INVITATION_RESPONSE_TYPE = 0i32;
pub const PEER_INVITATION_RESPONSE_ERROR: PEER_INVITATION_RESPONSE_TYPE = 3i32;
pub const PEER_INVITATION_RESPONSE_EXPIRED: PEER_INVITATION_RESPONSE_TYPE = 2i32;
pub const PEER_MEMBER_CONNECTED: PEER_MEMBER_CHANGE_TYPE = 1i32;
pub const PEER_MEMBER_DATA_OPTIONAL: PEER_GROUP_PROPERTY_FLAGS = 1i32;
pub const PEER_MEMBER_DISCONNECTED: PEER_MEMBER_CHANGE_TYPE = 2i32;
pub const PEER_MEMBER_JOINED: PEER_MEMBER_CHANGE_TYPE = 4i32;
pub const PEER_MEMBER_LEFT: PEER_MEMBER_CHANGE_TYPE = 5i32;
pub const PEER_MEMBER_PRESENT: PEER_MEMBER_FLAGS = 1i32;
pub const PEER_MEMBER_UPDATED: PEER_MEMBER_CHANGE_TYPE = 3i32;
pub const PEER_NODE_CHANGE_CONNECTED: PEER_NODE_CHANGE_TYPE = 1i32;
pub const PEER_NODE_CHANGE_DISCONNECTED: PEER_NODE_CHANGE_TYPE = 2i32;
pub const PEER_NODE_CHANGE_UPDATED: PEER_NODE_CHANGE_TYPE = 3i32;
pub const PEER_PNRP_ALL_LINK_CLOUDS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PEER_PNRP_ALL_LINKS");
pub const PEER_PRESENCE_AWAY: PEER_PRESENCE_STATUS = 2i32;
pub const PEER_PRESENCE_BE_RIGHT_BACK: PEER_PRESENCE_STATUS = 3i32;
pub const PEER_PRESENCE_BUSY: PEER_PRESENCE_STATUS = 5i32;
pub const PEER_PRESENCE_IDLE: PEER_PRESENCE_STATUS = 4i32;
pub const PEER_PRESENCE_OFFLINE: PEER_PRESENCE_STATUS = 0i32;
pub const PEER_PRESENCE_ONLINE: PEER_PRESENCE_STATUS = 7i32;
pub const PEER_PRESENCE_ON_THE_PHONE: PEER_PRESENCE_STATUS = 6i32;
pub const PEER_PRESENCE_OUT_TO_LUNCH: PEER_PRESENCE_STATUS = 1i32;
pub const PEER_PUBLICATION_SCOPE_ALL: PEER_PUBLICATION_SCOPE = 3i32;
pub const PEER_PUBLICATION_SCOPE_INTERNET: PEER_PUBLICATION_SCOPE = 2i32;
pub const PEER_PUBLICATION_SCOPE_NEAR_ME: PEER_PUBLICATION_SCOPE = 1i32;
pub const PEER_PUBLICATION_SCOPE_NONE: PEER_PUBLICATION_SCOPE = 0i32;
pub const PEER_RECORD_ADDED: PEER_RECORD_CHANGE_TYPE = 1i32;
pub const PEER_RECORD_DELETED: PEER_RECORD_CHANGE_TYPE = 3i32;
pub const PEER_RECORD_EXPIRED: PEER_RECORD_CHANGE_TYPE = 4i32;
pub const PEER_RECORD_FLAG_AUTOREFRESH: PEER_RECORD_FLAGS = 1i32;
pub const PEER_RECORD_FLAG_DELETED: PEER_RECORD_FLAGS = 2i32;
pub const PEER_RECORD_UPDATED: PEER_RECORD_CHANGE_TYPE = 2i32;
pub const PEER_SIGNIN_ALL: PEER_SIGNIN_FLAGS = 3i32;
pub const PEER_SIGNIN_INTERNET: PEER_SIGNIN_FLAGS = 2i32;
pub const PEER_SIGNIN_NEAR_ME: PEER_SIGNIN_FLAGS = 1i32;
pub const PEER_SIGNIN_NONE: PEER_SIGNIN_FLAGS = 0i32;
pub const PEER_WATCH_ALLOWED: PEER_WATCH_PERMISSION = 1i32;
pub const PEER_WATCH_BLOCKED: PEER_WATCH_PERMISSION = 0i32;
pub const PNRPINFO_HINT: u32 = 1u32;
pub const PNRP_CLOUD_FULL_PARTICIPANT: PNRP_CLOUD_FLAGS = 4i32;
pub const PNRP_CLOUD_NAME_LOCAL: PNRP_CLOUD_FLAGS = 1i32;
pub const PNRP_CLOUD_NO_FLAGS: PNRP_CLOUD_FLAGS = 0i32;
pub const PNRP_CLOUD_RESOLVE_ONLY: PNRP_CLOUD_FLAGS = 2i32;
pub const PNRP_CLOUD_STATE_ACTIVE: PNRP_CLOUD_STATE = 2i32;
pub const PNRP_CLOUD_STATE_ALONE: PNRP_CLOUD_STATE = 6i32;
pub const PNRP_CLOUD_STATE_DEAD: PNRP_CLOUD_STATE = 3i32;
pub const PNRP_CLOUD_STATE_DISABLED: PNRP_CLOUD_STATE = 4i32;
pub const PNRP_CLOUD_STATE_NO_NET: PNRP_CLOUD_STATE = 5i32;
pub const PNRP_CLOUD_STATE_SYNCHRONISING: PNRP_CLOUD_STATE = 1i32;
pub const PNRP_CLOUD_STATE_VIRTUAL: PNRP_CLOUD_STATE = 0i32;
pub const PNRP_EXTENDED_PAYLOAD_TYPE_BINARY: PNRP_EXTENDED_PAYLOAD_TYPE = 1i32;
pub const PNRP_EXTENDED_PAYLOAD_TYPE_NONE: PNRP_EXTENDED_PAYLOAD_TYPE = 0i32;
pub const PNRP_EXTENDED_PAYLOAD_TYPE_STRING: PNRP_EXTENDED_PAYLOAD_TYPE = 2i32;
pub const PNRP_GLOBAL_SCOPE: PNRP_SCOPE = 1i32;
pub const PNRP_LINK_LOCAL_SCOPE: PNRP_SCOPE = 3i32;
pub const PNRP_MAX_ENDPOINT_ADDRESSES: u32 = 10u32;
pub const PNRP_MAX_EXTENDED_PAYLOAD_BYTES: u32 = 4096u32;
pub const PNRP_REGISTERED_ID_STATE_OK: PNRP_REGISTERED_ID_STATE = 1i32;
pub const PNRP_REGISTERED_ID_STATE_PROBLEM: PNRP_REGISTERED_ID_STATE = 2i32;
pub const PNRP_RESOLVE_CRITERIA_ANY_PEER_NAME: PNRP_RESOLVE_CRITERIA = 5i32;
pub const PNRP_RESOLVE_CRITERIA_DEFAULT: PNRP_RESOLVE_CRITERIA = 0i32;
pub const PNRP_RESOLVE_CRITERIA_NEAREST_NON_CURRENT_PROCESS_PEER_NAME: PNRP_RESOLVE_CRITERIA = 4i32;
pub const PNRP_RESOLVE_CRITERIA_NEAREST_PEER_NAME: PNRP_RESOLVE_CRITERIA = 6i32;
pub const PNRP_RESOLVE_CRITERIA_NEAREST_REMOTE_PEER_NAME: PNRP_RESOLVE_CRITERIA = 2i32;
pub const PNRP_RESOLVE_CRITERIA_NON_CURRENT_PROCESS_PEER_NAME: PNRP_RESOLVE_CRITERIA = 3i32;
pub const PNRP_RESOLVE_CRITERIA_REMOTE_PEER_NAME: PNRP_RESOLVE_CRITERIA = 1i32;
pub const PNRP_SCOPE_ANY: PNRP_SCOPE = 0i32;
pub const PNRP_SITE_LOCAL_SCOPE: PNRP_SCOPE = 2i32;
pub const PeerDistClientBasicInfo: PEERDIST_CLIENT_INFO_BY_HANDLE_CLASS = 0i32;
pub const SVCID_PNRPCLOUD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2239ce6_00c0_4fbf_bad6_18139385a49a);
pub const SVCID_PNRPNAME_V1: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2239ce5_00c0_4fbf_bad6_18139385a49a);
pub const SVCID_PNRPNAME_V2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2239ce7_00c0_4fbf_bad6_18139385a49a);
pub const WSA_PNRP_CLIENT_INVALID_COMPARTMENT_ID: u32 = 11506u32;
pub const WSA_PNRP_CLOUD_DISABLED: u32 = 11502u32;
pub const WSA_PNRP_CLOUD_IS_DEAD: u32 = 11509u32;
pub const WSA_PNRP_CLOUD_IS_SEARCH_ONLY: u32 = 11505u32;
pub const WSA_PNRP_CLOUD_NOT_FOUND: u32 = 11501u32;
pub const WSA_PNRP_DUPLICATE_PEER_NAME: u32 = 11508u32;
pub const WSA_PNRP_ERROR_BASE: u32 = 11500u32;
pub const WSA_PNRP_INVALID_IDENTITY: u32 = 11503u32;
pub const WSA_PNRP_TOO_MUCH_LOAD: u32 = 11504u32;
pub const WSZ_SCOPE_GLOBAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GLOBAL");
pub const WSZ_SCOPE_LINKLOCAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LINKLOCAL");
pub const WSZ_SCOPE_SITELOCAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SITELOCAL");
pub type DRT_ADDRESS_FLAGS = i32;
pub type DRT_EVENT_TYPE = i32;
pub type DRT_LEAFSET_KEY_CHANGE_TYPE = i32;
pub type DRT_MATCH_TYPE = i32;
pub type DRT_REGISTRATION_STATE = i32;
pub type DRT_SCOPE = i32;
pub type DRT_SECURITY_MODE = i32;
pub type DRT_STATUS = i32;
pub type PEERDIST_CLIENT_INFO_BY_HANDLE_CLASS = i32;
pub type PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_VALUE = u32;
pub type PEERDIST_STATUS = i32;
pub type PEER_APPLICATION_REGISTRATION_TYPE = i32;
pub type PEER_CHANGE_TYPE = i32;
pub type PEER_COLLAB_EVENT_TYPE = i32;
pub type PEER_CONNECTION_FLAGS = i32;
pub type PEER_CONNECTION_STATUS = i32;
pub type PEER_GRAPH_EVENT_TYPE = i32;
pub type PEER_GRAPH_PROPERTY_FLAGS = i32;
pub type PEER_GRAPH_SCOPE = i32;
pub type PEER_GRAPH_STATUS_FLAGS = i32;
pub type PEER_GROUP_AUTHENTICATION_SCHEME = i32;
pub type PEER_GROUP_EVENT_TYPE = i32;
pub type PEER_GROUP_ISSUE_CREDENTIAL_FLAGS = i32;
pub type PEER_GROUP_PROPERTY_FLAGS = i32;
pub type PEER_GROUP_STATUS = i32;
pub type PEER_INVITATION_RESPONSE_TYPE = i32;
pub type PEER_MEMBER_CHANGE_TYPE = i32;
pub type PEER_MEMBER_FLAGS = i32;
pub type PEER_NODE_CHANGE_TYPE = i32;
pub type PEER_PRESENCE_STATUS = i32;
pub type PEER_PUBLICATION_SCOPE = i32;
pub type PEER_RECORD_CHANGE_TYPE = i32;
pub type PEER_RECORD_FLAGS = i32;
pub type PEER_SIGNIN_FLAGS = i32;
pub type PEER_WATCH_PERMISSION = i32;
pub type PNRP_CLOUD_FLAGS = i32;
pub type PNRP_CLOUD_STATE = i32;
pub type PNRP_EXTENDED_PAYLOAD_TYPE = i32;
pub type PNRP_REGISTERED_ID_STATE = i32;
pub type PNRP_RESOLVE_CRITERIA = i32;
pub type PNRP_SCOPE = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct DRT_ADDRESS {
    pub socketAddress: super::super::Networking::WinSock::SOCKADDR_STORAGE,
    pub flags: u32,
    pub nearness: i32,
    pub latency: u32,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_ADDRESS {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct DRT_ADDRESS_LIST {
    pub AddressCount: u32,
    pub AddressList: [DRT_ADDRESS; 1],
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_ADDRESS_LIST {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_ADDRESS_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRT_BOOTSTRAP_PROVIDER {
    pub pvContext: *mut ::core::ffi::c_void,
    pub Attach: isize,
    pub Detach: isize,
    pub InitResolve: isize,
    pub IssueResolve: isize,
    pub EndResolve: isize,
    pub Register: isize,
    pub Unregister: isize,
}
impl ::core::marker::Copy for DRT_BOOTSTRAP_PROVIDER {}
impl ::core::clone::Clone for DRT_BOOTSTRAP_PROVIDER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRT_DATA {
    pub cb: u32,
    pub pb: *mut u8,
}
impl ::core::marker::Copy for DRT_DATA {}
impl ::core::clone::Clone for DRT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct DRT_EVENT_DATA {
    pub r#type: DRT_EVENT_TYPE,
    pub hr: ::windows_sys::core::HRESULT,
    pub pvContext: *mut ::core::ffi::c_void,
    pub Anonymous: DRT_EVENT_DATA_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_EVENT_DATA {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_EVENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub union DRT_EVENT_DATA_0 {
    pub leafsetKeyChange: DRT_EVENT_DATA_0_0,
    pub registrationStateChange: DRT_EVENT_DATA_0_1,
    pub statusChange: DRT_EVENT_DATA_0_2,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_EVENT_DATA_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_EVENT_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct DRT_EVENT_DATA_0_0 {
    pub change: DRT_LEAFSET_KEY_CHANGE_TYPE,
    pub localKey: DRT_DATA,
    pub remoteKey: DRT_DATA,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_EVENT_DATA_0_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_EVENT_DATA_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct DRT_EVENT_DATA_0_1 {
    pub state: DRT_REGISTRATION_STATE,
    pub localKey: DRT_DATA,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_EVENT_DATA_0_1 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_EVENT_DATA_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct DRT_EVENT_DATA_0_2 {
    pub status: DRT_STATUS,
    pub bootstrapAddresses: DRT_EVENT_DATA_0_2_0,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_EVENT_DATA_0_2 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_EVENT_DATA_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct DRT_EVENT_DATA_0_2_0 {
    pub cntAddress: u32,
    pub pAddresses: *mut super::super::Networking::WinSock::SOCKADDR_STORAGE,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for DRT_EVENT_DATA_0_2_0 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for DRT_EVENT_DATA_0_2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRT_REGISTRATION {
    pub key: DRT_DATA,
    pub appData: DRT_DATA,
}
impl ::core::marker::Copy for DRT_REGISTRATION {}
impl ::core::clone::Clone for DRT_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DRT_SEARCH_INFO {
    pub dwSize: u32,
    pub fIterative: super::super::Foundation::BOOL,
    pub fAllowCurrentInstanceMatch: super::super::Foundation::BOOL,
    pub fAnyMatchInRange: super::super::Foundation::BOOL,
    pub cMaxEndpoints: u32,
    pub pMaximumKey: *mut DRT_DATA,
    pub pMinimumKey: *mut DRT_DATA,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DRT_SEARCH_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DRT_SEARCH_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRT_SEARCH_RESULT {
    pub dwSize: u32,
    pub r#type: DRT_MATCH_TYPE,
    pub pvContext: *mut ::core::ffi::c_void,
    pub registration: DRT_REGISTRATION,
}
impl ::core::marker::Copy for DRT_SEARCH_RESULT {}
impl ::core::clone::Clone for DRT_SEARCH_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRT_SECURITY_PROVIDER {
    pub pvContext: *mut ::core::ffi::c_void,
    pub Attach: isize,
    pub Detach: isize,
    pub RegisterKey: isize,
    pub UnregisterKey: isize,
    pub ValidateAndUnpackPayload: isize,
    pub SecureAndPackPayload: isize,
    pub FreeData: isize,
    pub EncryptData: isize,
    pub DecryptData: isize,
    pub GetSerializedCredential: isize,
    pub ValidateRemoteCredential: isize,
    pub SignData: isize,
    pub VerifyData: isize,
}
impl ::core::marker::Copy for DRT_SECURITY_PROVIDER {}
impl ::core::clone::Clone for DRT_SECURITY_PROVIDER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DRT_SETTINGS {
    pub dwSize: u32,
    pub cbKey: u32,
    pub bProtocolMajorVersion: u8,
    pub bProtocolMinorVersion: u8,
    pub ulMaxRoutingAddresses: u32,
    pub pwzDrtInstancePrefix: ::windows_sys::core::PWSTR,
    pub hTransport: *mut ::core::ffi::c_void,
    pub pSecurityProvider: *mut DRT_SECURITY_PROVIDER,
    pub pBootstrapProvider: *mut DRT_BOOTSTRAP_PROVIDER,
    pub eSecurityMode: DRT_SECURITY_MODE,
}
impl ::core::marker::Copy for DRT_SETTINGS {}
impl ::core::clone::Clone for DRT_SETTINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PEERDIST_CLIENT_BASIC_INFO {
    pub fFlashCrowd: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PEERDIST_CLIENT_BASIC_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PEERDIST_CLIENT_BASIC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEERDIST_CONTENT_TAG {
    pub Data: [u8; 16],
}
impl ::core::marker::Copy for PEERDIST_CONTENT_TAG {}
impl ::core::clone::Clone for PEERDIST_CONTENT_TAG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEERDIST_PUBLICATION_OPTIONS {
    pub dwVersion: u32,
    pub dwFlags: u32,
}
impl ::core::marker::Copy for PEERDIST_PUBLICATION_OPTIONS {}
impl ::core::clone::Clone for PEERDIST_PUBLICATION_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEERDIST_RETRIEVAL_OPTIONS {
    pub cbSize: u32,
    pub dwContentInfoMinVersion: u32,
    pub dwContentInfoMaxVersion: u32,
    pub dwReserved: u32,
}
impl ::core::marker::Copy for PEERDIST_RETRIEVAL_OPTIONS {}
impl ::core::clone::Clone for PEERDIST_RETRIEVAL_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEERDIST_STATUS_INFO {
    pub cbSize: u32,
    pub status: PEERDIST_STATUS,
    pub dwMinVer: PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_VALUE,
    pub dwMaxVer: PEERDIST_RETRIEVAL_OPTIONS_CONTENTINFO_VERSION_VALUE,
}
impl ::core::marker::Copy for PEERDIST_STATUS_INFO {}
impl ::core::clone::Clone for PEERDIST_STATUS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_ADDRESS {
    pub dwSize: u32,
    pub sin6: super::super::Networking::WinSock::SOCKADDR_IN6,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_ADDRESS {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_APPLICATION {
    pub id: ::windows_sys::core::GUID,
    pub data: PEER_DATA,
    pub pwzDescription: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for PEER_APPLICATION {}
impl ::core::clone::Clone for PEER_APPLICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_APPLICATION_REGISTRATION_INFO {
    pub application: PEER_APPLICATION,
    pub pwzApplicationToLaunch: ::windows_sys::core::PWSTR,
    pub pwzApplicationArguments: ::windows_sys::core::PWSTR,
    pub dwPublicationScope: u32,
}
impl ::core::marker::Copy for PEER_APPLICATION_REGISTRATION_INFO {}
impl ::core::clone::Clone for PEER_APPLICATION_REGISTRATION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct PEER_APP_LAUNCH_INFO {
    pub pContact: *mut PEER_CONTACT,
    pub pEndpoint: *mut PEER_ENDPOINT,
    pub pInvitation: *mut PEER_INVITATION,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for PEER_APP_LAUNCH_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for PEER_APP_LAUNCH_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct PEER_COLLAB_EVENT_DATA {
    pub eventType: PEER_COLLAB_EVENT_TYPE,
    pub Anonymous: PEER_COLLAB_EVENT_DATA_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for PEER_COLLAB_EVENT_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for PEER_COLLAB_EVENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub union PEER_COLLAB_EVENT_DATA_0 {
    pub watchListChangedData: PEER_EVENT_WATCHLIST_CHANGED_DATA,
    pub presenceChangedData: PEER_EVENT_PRESENCE_CHANGED_DATA,
    pub applicationChangedData: PEER_EVENT_APPLICATION_CHANGED_DATA,
    pub objectChangedData: PEER_EVENT_OBJECT_CHANGED_DATA,
    pub endpointChangedData: PEER_EVENT_ENDPOINT_CHANGED_DATA,
    pub peopleNearMeChangedData: PEER_EVENT_PEOPLE_NEAR_ME_CHANGED_DATA,
    pub requestStatusChangedData: PEER_EVENT_REQUEST_STATUS_CHANGED_DATA,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for PEER_COLLAB_EVENT_DATA_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for PEER_COLLAB_EVENT_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_COLLAB_EVENT_REGISTRATION {
    pub eventType: PEER_COLLAB_EVENT_TYPE,
    pub pInstance: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for PEER_COLLAB_EVENT_REGISTRATION {}
impl ::core::clone::Clone for PEER_COLLAB_EVENT_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_CONNECTION_INFO {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub ullConnectionId: u64,
    pub ullNodeId: u64,
    pub pwzPeerId: ::windows_sys::core::PWSTR,
    pub address: PEER_ADDRESS,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_CONNECTION_INFO {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_CONNECTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PEER_CONTACT {
    pub pwzPeerName: ::windows_sys::core::PWSTR,
    pub pwzNickName: ::windows_sys::core::PWSTR,
    pub pwzDisplayName: ::windows_sys::core::PWSTR,
    pub pwzEmailAddress: ::windows_sys::core::PWSTR,
    pub fWatch: super::super::Foundation::BOOL,
    pub WatcherPermissions: PEER_WATCH_PERMISSION,
    pub credentials: PEER_DATA,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PEER_CONTACT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PEER_CONTACT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct PEER_CREDENTIAL_INFO {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub pwzFriendlyName: ::windows_sys::core::PWSTR,
    pub pPublicKey: *mut super::super::Security::Cryptography::CERT_PUBLIC_KEY_INFO,
    pub pwzIssuerPeerName: ::windows_sys::core::PWSTR,
    pub pwzIssuerFriendlyName: ::windows_sys::core::PWSTR,
    pub ftValidityStart: super::super::Foundation::FILETIME,
    pub ftValidityEnd: super::super::Foundation::FILETIME,
    pub cRoles: u32,
    pub pRoles: *mut ::windows_sys::core::GUID,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for PEER_CREDENTIAL_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for PEER_CREDENTIAL_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_DATA {
    pub cbData: u32,
    pub pbData: *mut u8,
}
impl ::core::marker::Copy for PEER_DATA {}
impl ::core::clone::Clone for PEER_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_ENDPOINT {
    pub address: PEER_ADDRESS,
    pub pwzEndpointName: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_ENDPOINT {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_ENDPOINT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct PEER_EVENT_APPLICATION_CHANGED_DATA {
    pub pContact: *mut PEER_CONTACT,
    pub pEndpoint: *mut PEER_ENDPOINT,
    pub changeType: PEER_CHANGE_TYPE,
    pub pApplication: *mut PEER_APPLICATION,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for PEER_EVENT_APPLICATION_CHANGED_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for PEER_EVENT_APPLICATION_CHANGED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_EVENT_CONNECTION_CHANGE_DATA {
    pub dwSize: u32,
    pub status: PEER_CONNECTION_STATUS,
    pub ullConnectionId: u64,
    pub ullNodeId: u64,
    pub ullNextConnectionId: u64,
    pub hrConnectionFailedReason: ::windows_sys::core::HRESULT,
}
impl ::core::marker::Copy for PEER_EVENT_CONNECTION_CHANGE_DATA {}
impl ::core::clone::Clone for PEER_EVENT_CONNECTION_CHANGE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct PEER_EVENT_ENDPOINT_CHANGED_DATA {
    pub pContact: *mut PEER_CONTACT,
    pub pEndpoint: *mut PEER_ENDPOINT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for PEER_EVENT_ENDPOINT_CHANGED_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for PEER_EVENT_ENDPOINT_CHANGED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_EVENT_INCOMING_DATA {
    pub dwSize: u32,
    pub ullConnectionId: u64,
    pub r#type: ::windows_sys::core::GUID,
    pub data: PEER_DATA,
}
impl ::core::marker::Copy for PEER_EVENT_INCOMING_DATA {}
impl ::core::clone::Clone for PEER_EVENT_INCOMING_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_EVENT_MEMBER_CHANGE_DATA {
    pub dwSize: u32,
    pub changeType: PEER_MEMBER_CHANGE_TYPE,
    pub pwzIdentity: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for PEER_EVENT_MEMBER_CHANGE_DATA {}
impl ::core::clone::Clone for PEER_EVENT_MEMBER_CHANGE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_EVENT_NODE_CHANGE_DATA {
    pub dwSize: u32,
    pub changeType: PEER_NODE_CHANGE_TYPE,
    pub ullNodeId: u64,
    pub pwzPeerId: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for PEER_EVENT_NODE_CHANGE_DATA {}
impl ::core::clone::Clone for PEER_EVENT_NODE_CHANGE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct PEER_EVENT_OBJECT_CHANGED_DATA {
    pub pContact: *mut PEER_CONTACT,
    pub pEndpoint: *mut PEER_ENDPOINT,
    pub changeType: PEER_CHANGE_TYPE,
    pub pObject: *mut PEER_OBJECT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for PEER_EVENT_OBJECT_CHANGED_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for PEER_EVENT_OBJECT_CHANGED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_EVENT_PEOPLE_NEAR_ME_CHANGED_DATA {
    pub changeType: PEER_CHANGE_TYPE,
    pub pPeopleNearMe: *mut PEER_PEOPLE_NEAR_ME,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_EVENT_PEOPLE_NEAR_ME_CHANGED_DATA {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_EVENT_PEOPLE_NEAR_ME_CHANGED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub struct PEER_EVENT_PRESENCE_CHANGED_DATA {
    pub pContact: *mut PEER_CONTACT,
    pub pEndpoint: *mut PEER_ENDPOINT,
    pub changeType: PEER_CHANGE_TYPE,
    pub pPresenceInfo: *mut PEER_PRESENCE_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::marker::Copy for PEER_EVENT_PRESENCE_CHANGED_DATA {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
impl ::core::clone::Clone for PEER_EVENT_PRESENCE_CHANGED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_EVENT_RECORD_CHANGE_DATA {
    pub dwSize: u32,
    pub changeType: PEER_RECORD_CHANGE_TYPE,
    pub recordId: ::windows_sys::core::GUID,
    pub recordType: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for PEER_EVENT_RECORD_CHANGE_DATA {}
impl ::core::clone::Clone for PEER_EVENT_RECORD_CHANGE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_EVENT_REQUEST_STATUS_CHANGED_DATA {
    pub pEndpoint: *mut PEER_ENDPOINT,
    pub hrChange: ::windows_sys::core::HRESULT,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_EVENT_REQUEST_STATUS_CHANGED_DATA {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_EVENT_REQUEST_STATUS_CHANGED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_EVENT_SYNCHRONIZED_DATA {
    pub dwSize: u32,
    pub recordType: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for PEER_EVENT_SYNCHRONIZED_DATA {}
impl ::core::clone::Clone for PEER_EVENT_SYNCHRONIZED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PEER_EVENT_WATCHLIST_CHANGED_DATA {
    pub pContact: *mut PEER_CONTACT,
    pub changeType: PEER_CHANGE_TYPE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PEER_EVENT_WATCHLIST_CHANGED_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PEER_EVENT_WATCHLIST_CHANGED_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_GRAPH_EVENT_DATA {
    pub eventType: PEER_GRAPH_EVENT_TYPE,
    pub Anonymous: PEER_GRAPH_EVENT_DATA_0,
}
impl ::core::marker::Copy for PEER_GRAPH_EVENT_DATA {}
impl ::core::clone::Clone for PEER_GRAPH_EVENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union PEER_GRAPH_EVENT_DATA_0 {
    pub dwStatus: PEER_GRAPH_STATUS_FLAGS,
    pub incomingData: PEER_EVENT_INCOMING_DATA,
    pub recordChangeData: PEER_EVENT_RECORD_CHANGE_DATA,
    pub connectionChangeData: PEER_EVENT_CONNECTION_CHANGE_DATA,
    pub nodeChangeData: PEER_EVENT_NODE_CHANGE_DATA,
    pub synchronizedData: PEER_EVENT_SYNCHRONIZED_DATA,
}
impl ::core::marker::Copy for PEER_GRAPH_EVENT_DATA_0 {}
impl ::core::clone::Clone for PEER_GRAPH_EVENT_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_GRAPH_EVENT_REGISTRATION {
    pub eventType: PEER_GRAPH_EVENT_TYPE,
    pub pType: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for PEER_GRAPH_EVENT_REGISTRATION {}
impl ::core::clone::Clone for PEER_GRAPH_EVENT_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_GRAPH_PROPERTIES {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub dwScope: u32,
    pub dwMaxRecordSize: u32,
    pub pwzGraphId: ::windows_sys::core::PWSTR,
    pub pwzCreatorId: ::windows_sys::core::PWSTR,
    pub pwzFriendlyName: ::windows_sys::core::PWSTR,
    pub pwzComment: ::windows_sys::core::PWSTR,
    pub ulPresenceLifetime: u32,
    pub cPresenceMax: u32,
}
impl ::core::marker::Copy for PEER_GRAPH_PROPERTIES {}
impl ::core::clone::Clone for PEER_GRAPH_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_GROUP_EVENT_DATA {
    pub eventType: PEER_GROUP_EVENT_TYPE,
    pub Anonymous: PEER_GROUP_EVENT_DATA_0,
}
impl ::core::marker::Copy for PEER_GROUP_EVENT_DATA {}
impl ::core::clone::Clone for PEER_GROUP_EVENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union PEER_GROUP_EVENT_DATA_0 {
    pub dwStatus: PEER_GROUP_STATUS,
    pub incomingData: PEER_EVENT_INCOMING_DATA,
    pub recordChangeData: PEER_EVENT_RECORD_CHANGE_DATA,
    pub connectionChangeData: PEER_EVENT_CONNECTION_CHANGE_DATA,
    pub memberChangeData: PEER_EVENT_MEMBER_CHANGE_DATA,
    pub hrConnectionFailedReason: ::windows_sys::core::HRESULT,
}
impl ::core::marker::Copy for PEER_GROUP_EVENT_DATA_0 {}
impl ::core::clone::Clone for PEER_GROUP_EVENT_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_GROUP_EVENT_REGISTRATION {
    pub eventType: PEER_GROUP_EVENT_TYPE,
    pub pType: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for PEER_GROUP_EVENT_REGISTRATION {}
impl ::core::clone::Clone for PEER_GROUP_EVENT_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_GROUP_PROPERTIES {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub pwzCloud: ::windows_sys::core::PWSTR,
    pub pwzClassifier: ::windows_sys::core::PWSTR,
    pub pwzGroupPeerName: ::windows_sys::core::PWSTR,
    pub pwzCreatorPeerName: ::windows_sys::core::PWSTR,
    pub pwzFriendlyName: ::windows_sys::core::PWSTR,
    pub pwzComment: ::windows_sys::core::PWSTR,
    pub ulMemberDataLifetime: u32,
    pub ulPresenceLifetime: u32,
    pub dwAuthenticationSchemes: u32,
    pub pwzGroupPassword: ::windows_sys::core::PWSTR,
    pub groupPasswordRole: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for PEER_GROUP_PROPERTIES {}
impl ::core::clone::Clone for PEER_GROUP_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_INVITATION {
    pub applicationId: ::windows_sys::core::GUID,
    pub applicationData: PEER_DATA,
    pub pwzMessage: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for PEER_INVITATION {}
impl ::core::clone::Clone for PEER_INVITATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub struct PEER_INVITATION_INFO {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub pwzCloudName: ::windows_sys::core::PWSTR,
    pub dwScope: u32,
    pub dwCloudFlags: u32,
    pub pwzGroupPeerName: ::windows_sys::core::PWSTR,
    pub pwzIssuerPeerName: ::windows_sys::core::PWSTR,
    pub pwzSubjectPeerName: ::windows_sys::core::PWSTR,
    pub pwzGroupFriendlyName: ::windows_sys::core::PWSTR,
    pub pwzIssuerFriendlyName: ::windows_sys::core::PWSTR,
    pub pwzSubjectFriendlyName: ::windows_sys::core::PWSTR,
    pub ftValidityStart: super::super::Foundation::FILETIME,
    pub ftValidityEnd: super::super::Foundation::FILETIME,
    pub cRoles: u32,
    pub pRoles: *mut ::windows_sys::core::GUID,
    pub cClassifiers: u32,
    pub ppwzClassifiers: *mut ::windows_sys::core::PWSTR,
    pub pSubjectPublicKey: *mut super::super::Security::Cryptography::CERT_PUBLIC_KEY_INFO,
    pub authScheme: PEER_GROUP_AUTHENTICATION_SCHEME,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for PEER_INVITATION_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for PEER_INVITATION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_INVITATION_RESPONSE {
    pub action: PEER_INVITATION_RESPONSE_TYPE,
    pub pwzMessage: ::windows_sys::core::PWSTR,
    pub hrExtendedInfo: ::windows_sys::core::HRESULT,
}
impl ::core::marker::Copy for PEER_INVITATION_RESPONSE {}
impl ::core::clone::Clone for PEER_INVITATION_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
pub struct PEER_MEMBER {
    pub dwSize: u32,
    pub dwFlags: u32,
    pub pwzIdentity: ::windows_sys::core::PWSTR,
    pub pwzAttributes: ::windows_sys::core::PWSTR,
    pub ullNodeId: u64,
    pub cAddresses: u32,
    pub pAddresses: *mut PEER_ADDRESS,
    pub pCredentialInfo: *mut PEER_CREDENTIAL_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
impl ::core::marker::Copy for PEER_MEMBER {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock", feature = "Win32_Security_Cryptography"))]
impl ::core::clone::Clone for PEER_MEMBER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_NAME_PAIR {
    pub dwSize: u32,
    pub pwzPeerName: ::windows_sys::core::PWSTR,
    pub pwzFriendlyName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for PEER_NAME_PAIR {}
impl ::core::clone::Clone for PEER_NAME_PAIR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_NODE_INFO {
    pub dwSize: u32,
    pub ullNodeId: u64,
    pub pwzPeerId: ::windows_sys::core::PWSTR,
    pub cAddresses: u32,
    pub pAddresses: *mut PEER_ADDRESS,
    pub pwzAttributes: ::windows_sys::core::PWSTR,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_NODE_INFO {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_NODE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_OBJECT {
    pub id: ::windows_sys::core::GUID,
    pub data: PEER_DATA,
    pub dwPublicationScope: u32,
}
impl ::core::marker::Copy for PEER_OBJECT {}
impl ::core::clone::Clone for PEER_OBJECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_PEOPLE_NEAR_ME {
    pub pwzNickName: ::windows_sys::core::PWSTR,
    pub endpoint: PEER_ENDPOINT,
    pub id: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_PEOPLE_NEAR_ME {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_PEOPLE_NEAR_ME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_PNRP_CLOUD_INFO {
    pub pwzCloudName: ::windows_sys::core::PWSTR,
    pub dwScope: PNRP_SCOPE,
    pub dwScopeId: u32,
}
impl ::core::marker::Copy for PEER_PNRP_CLOUD_INFO {}
impl ::core::clone::Clone for PEER_PNRP_CLOUD_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_PNRP_ENDPOINT_INFO {
    pub pwzPeerName: ::windows_sys::core::PWSTR,
    pub cAddresses: u32,
    pub ppAddresses: *mut *mut super::super::Networking::WinSock::SOCKADDR,
    pub pwzComment: ::windows_sys::core::PWSTR,
    pub payload: PEER_DATA,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_PNRP_ENDPOINT_INFO {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_PNRP_ENDPOINT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PEER_PNRP_REGISTRATION_INFO {
    pub pwzCloudName: ::windows_sys::core::PWSTR,
    pub pwzPublishingIdentity: ::windows_sys::core::PWSTR,
    pub cAddresses: u32,
    pub ppAddresses: *mut *mut super::super::Networking::WinSock::SOCKADDR,
    pub wPort: u16,
    pub pwzComment: ::windows_sys::core::PWSTR,
    pub payload: PEER_DATA,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PEER_PNRP_REGISTRATION_INFO {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PEER_PNRP_REGISTRATION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_PRESENCE_INFO {
    pub status: PEER_PRESENCE_STATUS,
    pub pwzDescriptiveText: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for PEER_PRESENCE_INFO {}
impl ::core::clone::Clone for PEER_PRESENCE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PEER_RECORD {
    pub dwSize: u32,
    pub r#type: ::windows_sys::core::GUID,
    pub id: ::windows_sys::core::GUID,
    pub dwVersion: u32,
    pub dwFlags: u32,
    pub pwzCreatorId: ::windows_sys::core::PWSTR,
    pub pwzModifiedById: ::windows_sys::core::PWSTR,
    pub pwzAttributes: ::windows_sys::core::PWSTR,
    pub ftCreation: super::super::Foundation::FILETIME,
    pub ftExpiration: super::super::Foundation::FILETIME,
    pub ftLastModified: super::super::Foundation::FILETIME,
    pub securityData: PEER_DATA,
    pub data: PEER_DATA,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PEER_RECORD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PEER_RECORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PEER_SECURITY_INTERFACE {
    pub dwSize: u32,
    pub pwzSspFilename: ::windows_sys::core::PWSTR,
    pub pwzPackageName: ::windows_sys::core::PWSTR,
    pub cbSecurityInfo: u32,
    pub pbSecurityInfo: *mut u8,
    pub pvContext: *mut ::core::ffi::c_void,
    pub pfnValidateRecord: PFNPEER_VALIDATE_RECORD,
    pub pfnSecureRecord: PFNPEER_SECURE_RECORD,
    pub pfnFreeSecurityData: PFNPEER_FREE_SECURITY_DATA,
    pub pfnAuthFailed: PFNPEER_ON_PASSWORD_AUTH_FAILED,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PEER_SECURITY_INTERFACE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PEER_SECURITY_INTERFACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PEER_VERSION_DATA {
    pub wVersion: u16,
    pub wHighestVersion: u16,
}
impl ::core::marker::Copy for PEER_VERSION_DATA {}
impl ::core::clone::Clone for PEER_VERSION_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PNRPCLOUDINFO {
    pub dwSize: u32,
    pub Cloud: PNRP_CLOUD_ID,
    pub enCloudState: PNRP_CLOUD_STATE,
    pub enCloudFlags: PNRP_CLOUD_FLAGS,
}
impl ::core::marker::Copy for PNRPCLOUDINFO {}
impl ::core::clone::Clone for PNRPCLOUDINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`"]
#[cfg(feature = "Win32_Networking_WinSock")]
pub struct PNRPINFO_V1 {
    pub dwSize: u32,
    pub lpwszIdentity: ::windows_sys::core::PWSTR,
    pub nMaxResolve: u32,
    pub dwTimeout: u32,
    pub dwLifetime: u32,
    pub enResolveCriteria: PNRP_RESOLVE_CRITERIA,
    pub dwFlags: u32,
    pub saHint: super::super::Networking::WinSock::SOCKET_ADDRESS,
    pub enNameState: PNRP_REGISTERED_ID_STATE,
}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::marker::Copy for PNRPINFO_V1 {}
#[cfg(feature = "Win32_Networking_WinSock")]
impl ::core::clone::Clone for PNRPINFO_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_System_Com"))]
pub struct PNRPINFO_V2 {
    pub dwSize: u32,
    pub lpwszIdentity: ::windows_sys::core::PWSTR,
    pub nMaxResolve: u32,
    pub dwTimeout: u32,
    pub dwLifetime: u32,
    pub enResolveCriteria: PNRP_RESOLVE_CRITERIA,
    pub dwFlags: u32,
    pub saHint: super::super::Networking::WinSock::SOCKET_ADDRESS,
    pub enNameState: PNRP_REGISTERED_ID_STATE,
    pub enExtendedPayloadType: PNRP_EXTENDED_PAYLOAD_TYPE,
    pub Anonymous: PNRPINFO_V2_0,
}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for PNRPINFO_V2 {}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for PNRPINFO_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Networking_WinSock\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_System_Com"))]
pub union PNRPINFO_V2_0 {
    pub blobPayload: super::super::System::Com::BLOB,
    pub pwszPayload: ::windows_sys::core::PWSTR,
}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for PNRPINFO_V2_0 {}
#[cfg(all(feature = "Win32_Networking_WinSock", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for PNRPINFO_V2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PNRP_CLOUD_ID {
    pub AddressFamily: i32,
    pub Scope: PNRP_SCOPE,
    pub ScopeId: u32,
}
impl ::core::marker::Copy for PNRP_CLOUD_ID {}
impl ::core::clone::Clone for PNRP_CLOUD_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Networking_WinSock\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Networking_WinSock"))]
pub type DRT_BOOTSTRAP_RESOLVE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(hr: ::windows_sys::core::HRESULT, pvcontext: *mut ::core::ffi::c_void, paddresses: *mut super::super::Networking::WinSock::SOCKET_ADDRESS_LIST, ffatalerror: super::super::Foundation::BOOL) -> ()>;
pub type PFNPEER_FREE_SECURITY_DATA = ::core::option::Option<unsafe extern "system" fn(hgraph: *const ::core::ffi::c_void, pvcontext: *const ::core::ffi::c_void, psecuritydata: *const PEER_DATA) -> ::windows_sys::core::HRESULT>;
pub type PFNPEER_ON_PASSWORD_AUTH_FAILED = ::core::option::Option<unsafe extern "system" fn(hgraph: *const ::core::ffi::c_void, pvcontext: *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNPEER_SECURE_RECORD = ::core::option::Option<unsafe extern "system" fn(hgraph: *const ::core::ffi::c_void, pvcontext: *const ::core::ffi::c_void, precord: *const PEER_RECORD, changetype: PEER_RECORD_CHANGE_TYPE, ppsecuritydata: *mut *mut PEER_DATA) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNPEER_VALIDATE_RECORD = ::core::option::Option<unsafe extern "system" fn(hgraph: *const ::core::ffi::c_void, pvcontext: *const ::core::ffi::c_void, precord: *const PEER_RECORD, changetype: PEER_RECORD_CHANGE_TYPE) -> ::windows_sys::core::HRESULT>;
