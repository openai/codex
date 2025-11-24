::windows_targets::link!("urlmon.dll" "system" fn CoGetClassObjectFromURL(rclassid : *const ::windows_sys::core::GUID, szcode : ::windows_sys::core::PCWSTR, dwfileversionms : u32, dwfileversionls : u32, sztype : ::windows_sys::core::PCWSTR, pbindctx : super:: IBindCtx, dwclscontext : super:: CLSCTX, pvreserved : *const ::core::ffi::c_void, riid : *const ::windows_sys::core::GUID, ppv : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetCombineIUri(pbaseuri : super:: IUri, prelativeuri : super:: IUri, dwcombineflags : u32, ppcombineduri : *mut super:: IUri, dwreserved : usize) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetCombineUrl(pwzbaseurl : ::windows_sys::core::PCWSTR, pwzrelativeurl : ::windows_sys::core::PCWSTR, dwcombineflags : u32, pszresult : ::windows_sys::core::PWSTR, cchresult : u32, pcchresult : *mut u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetCombineUrlEx(pbaseuri : super:: IUri, pwzrelativeurl : ::windows_sys::core::PCWSTR, dwcombineflags : u32, ppcombineduri : *mut super:: IUri, dwreserved : usize) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetCompareUrl(pwzurl1 : ::windows_sys::core::PCWSTR, pwzurl2 : ::windows_sys::core::PCWSTR, dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetCreateSecurityManager(psp : super:: IServiceProvider, ppsm : *mut IInternetSecurityManager, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetCreateZoneManager(psp : super:: IServiceProvider, ppzm : *mut IInternetZoneManager, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetGetProtocolFlags(pwzurl : ::windows_sys::core::PCWSTR, pdwflags : *mut u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetGetSecurityUrl(pwszurl : ::windows_sys::core::PCWSTR, ppwszsecurl : *mut ::windows_sys::core::PWSTR, psuaction : PSUACTION, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetGetSecurityUrlEx(puri : super:: IUri, ppsecuri : *mut super:: IUri, psuaction : PSUACTION, dwreserved : usize) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetGetSession(dwsessionmode : u32, ppiinternetsession : *mut IInternetSession, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetIsFeatureEnabled(featureentry : INTERNETFEATURELIST, dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetIsFeatureEnabledForIUri(featureentry : INTERNETFEATURELIST, dwflags : u32, piuri : super:: IUri, psecmgr : IInternetSecurityManagerEx2) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetIsFeatureEnabledForUrl(featureentry : INTERNETFEATURELIST, dwflags : u32, szurl : ::windows_sys::core::PCWSTR, psecmgr : IInternetSecurityManager) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetIsFeatureZoneElevationEnabled(szfromurl : ::windows_sys::core::PCWSTR, sztourl : ::windows_sys::core::PCWSTR, psecmgr : IInternetSecurityManager, dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetParseIUri(piuri : super:: IUri, parseaction : PARSEACTION, dwflags : u32, pwzresult : ::windows_sys::core::PWSTR, cchresult : u32, pcchresult : *mut u32, dwreserved : usize) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetParseUrl(pwzurl : ::windows_sys::core::PCWSTR, parseaction : PARSEACTION, dwflags : u32, pszresult : ::windows_sys::core::PWSTR, cchresult : u32, pcchresult : *mut u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CoInternetQueryInfo(pwzurl : ::windows_sys::core::PCWSTR, queryoptions : QUERYOPTION, dwqueryflags : u32, pvbuffer : *mut ::core::ffi::c_void, cbbuffer : u32, pcbbuffer : *mut u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CoInternetSetFeatureEnabled(featureentry : INTERNETFEATURELIST, dwflags : u32, fenable : super::super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CompareSecurityIds(pbsecurityid1 : *const u8, dwlen1 : u32, pbsecurityid2 : *const u8, dwlen2 : u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CompatFlagsFromClsid(pclsid : *const ::windows_sys::core::GUID, pdwcompatflags : *mut u32, pdwmiscstatusflags : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Security", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Security\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn CopyBindInfo(pcbisrc : *const super:: BINDINFO, pbidest : *mut super:: BINDINFO) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn CopyStgMedium(pcstgmedsrc : *const super:: STGMEDIUM, pstgmeddest : *mut super:: STGMEDIUM) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CreateAsyncBindCtx(reserved : u32, pbscb : super:: IBindStatusCallback, pefetc : super:: IEnumFORMATETC, ppbc : *mut super:: IBindCtx) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CreateAsyncBindCtxEx(pbc : super:: IBindCtx, dwoptions : u32, pbscb : super:: IBindStatusCallback, penum : super:: IEnumFORMATETC, ppbc : *mut super:: IBindCtx, reserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CreateFormatEnumerator(cfmtetc : u32, rgfmtetc : *const super:: FORMATETC, ppenumfmtetc : *mut super:: IEnumFORMATETC) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CreateURLMoniker(pmkctx : super:: IMoniker, szurl : ::windows_sys::core::PCWSTR, ppmk : *mut super:: IMoniker) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CreateURLMonikerEx(pmkctx : super:: IMoniker, szurl : ::windows_sys::core::PCWSTR, ppmk : *mut super:: IMoniker, dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn CreateURLMonikerEx2(pmkctx : super:: IMoniker, puri : super:: IUri, ppmk : *mut super:: IMoniker, dwflags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FaultInIEFeature(hwnd : super::super::super::Foundation:: HWND, pclassspec : *const super:: uCLSSPEC, pquery : *mut super:: QUERYCONTEXT, dwflags : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn FindMediaType(rgsztypes : ::windows_sys::core::PCSTR, rgcftypes : *mut u16) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn FindMediaTypeClass(pbc : super:: IBindCtx, sztype : ::windows_sys::core::PCSTR, pclsid : *mut ::windows_sys::core::GUID, reserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn FindMimeFromData(pbc : super:: IBindCtx, pwzurl : ::windows_sys::core::PCWSTR, pbuffer : *const ::core::ffi::c_void, cbsize : u32, pwzmimeproposed : ::windows_sys::core::PCWSTR, dwmimeflags : u32, ppwzmimeout : *mut ::windows_sys::core::PWSTR, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn GetClassFileOrMime(pbc : super:: IBindCtx, szfilename : ::windows_sys::core::PCWSTR, pbuffer : *const ::core::ffi::c_void, cbsize : u32, szmime : ::windows_sys::core::PCWSTR, dwreserved : u32, pclsid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn GetClassURL(szurl : ::windows_sys::core::PCWSTR, pclsid : *mut ::windows_sys::core::GUID) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn GetComponentIDFromCLSSPEC(pclassspec : *const super:: uCLSSPEC, ppszcomponentid : *mut ::windows_sys::core::PSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn GetSoftwareUpdateInfo(szdistunit : ::windows_sys::core::PCWSTR, psdi : *mut SOFTDISTINFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn HlinkGoBack(punk : ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn HlinkGoForward(punk : ::windows_sys::core::IUnknown) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn HlinkNavigateMoniker(punk : ::windows_sys::core::IUnknown, pmktarget : super:: IMoniker) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn HlinkNavigateString(punk : ::windows_sys::core::IUnknown, sztarget : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn HlinkSimpleNavigateToMoniker(pmktarget : super:: IMoniker, szlocation : ::windows_sys::core::PCWSTR, sztargetframename : ::windows_sys::core::PCWSTR, punk : ::windows_sys::core::IUnknown, pbc : super:: IBindCtx, param5 : super:: IBindStatusCallback, grfhlnf : u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn HlinkSimpleNavigateToString(sztarget : ::windows_sys::core::PCWSTR, szlocation : ::windows_sys::core::PCWSTR, sztargetframename : ::windows_sys::core::PCWSTR, punk : ::windows_sys::core::IUnknown, pbc : super:: IBindCtx, param5 : super:: IBindStatusCallback, grfhlnf : u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn IEGetUserPrivateNamespaceName() -> ::windows_sys::core::PWSTR);
::windows_targets::link!("urlmon.dll" "system" fn IEInstallScope(pdwscope : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn IsAsyncMoniker(pmk : super:: IMoniker) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsLoggingEnabledA(pszurl : ::windows_sys::core::PCSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsLoggingEnabledW(pwszurl : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("urlmon.dll" "system" fn IsValidURL(pbc : super:: IBindCtx, szurl : ::windows_sys::core::PCWSTR, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn MkParseDisplayNameEx(pbc : super:: IBindCtx, szdisplayname : ::windows_sys::core::PCWSTR, pcheaten : *mut u32, ppmk : *mut super:: IMoniker) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn ObtainUserAgentString(dwoption : u32, pszuaout : ::windows_sys::core::PSTR, cbsize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn RegisterBindStatusCallback(pbc : super:: IBindCtx, pbscb : super:: IBindStatusCallback, ppbscbprev : *mut super:: IBindStatusCallback, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn RegisterFormatEnumerator(pbc : super:: IBindCtx, pefetc : super:: IEnumFORMATETC, reserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn RegisterMediaTypeClass(pbc : super:: IBindCtx, ctypes : u32, rgsztypes : *const ::windows_sys::core::PCSTR, rgclsid : *const ::windows_sys::core::GUID, reserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn RegisterMediaTypes(ctypes : u32, rgsztypes : *const ::windows_sys::core::PCSTR, rgcftypes : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_Security", feature = "Win32_System_Com_StructuredStorage"))]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_Security\"`, `\"Win32_System_Com_StructuredStorage\"`"] fn ReleaseBindInfo(pbindinfo : *mut super:: BINDINFO) -> ());
::windows_targets::link!("urlmon.dll" "system" fn RevokeBindStatusCallback(pbc : super:: IBindCtx, pbscb : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn RevokeFormatEnumerator(pbc : super:: IBindCtx, pefetc : super:: IEnumFORMATETC) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetAccessForIEAppContainer(hobject : super::super::super::Foundation:: HANDLE, ieobjecttype : IEObjectType, dwaccessmask : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn SetSoftwareUpdateAdvertisementState(szdistunit : ::windows_sys::core::PCWSTR, dwadstate : u32, dwadvertisedversionms : u32, dwadvertisedversionls : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLDownloadToCacheFileA(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCSTR, param2 : ::windows_sys::core::PSTR, cchfilename : u32, param4 : u32, param5 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLDownloadToCacheFileW(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCWSTR, param2 : ::windows_sys::core::PWSTR, cchfilename : u32, param4 : u32, param5 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLDownloadToFileA(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCSTR, param2 : ::windows_sys::core::PCSTR, param3 : u32, param4 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLDownloadToFileW(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCWSTR, param2 : ::windows_sys::core::PCWSTR, param3 : u32, param4 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLOpenBlockingStreamA(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCSTR, param2 : *mut super:: IStream, param3 : u32, param4 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLOpenBlockingStreamW(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCWSTR, param2 : *mut super:: IStream, param3 : u32, param4 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLOpenPullStreamA(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCSTR, param2 : u32, param3 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLOpenPullStreamW(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCWSTR, param2 : u32, param3 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLOpenStreamA(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCSTR, param2 : u32, param3 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn URLOpenStreamW(param0 : ::windows_sys::core::IUnknown, param1 : ::windows_sys::core::PCWSTR, param2 : u32, param3 : super:: IBindStatusCallback) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn UrlMkGetSessionOption(dwoption : u32, pbuffer : *mut ::core::ffi::c_void, dwbufferlength : u32, pdwbufferlengthout : *mut u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("urlmon.dll" "system" fn UrlMkSetSessionOption(dwoption : u32, pbuffer : *const ::core::ffi::c_void, dwbufferlength : u32, dwreserved : u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("urlmon.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WriteHitLogging(lplogginginfo : *const HIT_LOGGING_INFO) -> super::super::super::Foundation:: BOOL);
pub type IBindCallbackRedirect = *mut ::core::ffi::c_void;
pub type IBindHttpSecurity = *mut ::core::ffi::c_void;
pub type IBindProtocol = *mut ::core::ffi::c_void;
pub type ICatalogFileInfo = *mut ::core::ffi::c_void;
pub type ICodeInstall = *mut ::core::ffi::c_void;
pub type IDataFilter = *mut ::core::ffi::c_void;
pub type IEncodingFilterFactory = *mut ::core::ffi::c_void;
pub type IGetBindHandle = *mut ::core::ffi::c_void;
pub type IHttpNegotiate = *mut ::core::ffi::c_void;
pub type IHttpNegotiate2 = *mut ::core::ffi::c_void;
pub type IHttpNegotiate3 = *mut ::core::ffi::c_void;
pub type IHttpSecurity = *mut ::core::ffi::c_void;
pub type IInternet = *mut ::core::ffi::c_void;
pub type IInternetBindInfo = *mut ::core::ffi::c_void;
pub type IInternetBindInfoEx = *mut ::core::ffi::c_void;
pub type IInternetHostSecurityManager = *mut ::core::ffi::c_void;
pub type IInternetPriority = *mut ::core::ffi::c_void;
pub type IInternetProtocol = *mut ::core::ffi::c_void;
pub type IInternetProtocolEx = *mut ::core::ffi::c_void;
pub type IInternetProtocolInfo = *mut ::core::ffi::c_void;
pub type IInternetProtocolRoot = *mut ::core::ffi::c_void;
pub type IInternetProtocolSink = *mut ::core::ffi::c_void;
pub type IInternetProtocolSinkStackable = *mut ::core::ffi::c_void;
pub type IInternetSecurityManager = *mut ::core::ffi::c_void;
pub type IInternetSecurityManagerEx = *mut ::core::ffi::c_void;
pub type IInternetSecurityManagerEx2 = *mut ::core::ffi::c_void;
pub type IInternetSecurityMgrSite = *mut ::core::ffi::c_void;
pub type IInternetSession = *mut ::core::ffi::c_void;
pub type IInternetThreadSwitch = *mut ::core::ffi::c_void;
pub type IInternetZoneManager = *mut ::core::ffi::c_void;
pub type IInternetZoneManagerEx = *mut ::core::ffi::c_void;
pub type IInternetZoneManagerEx2 = *mut ::core::ffi::c_void;
pub type IMonikerProp = *mut ::core::ffi::c_void;
pub type IPersistMoniker = *mut ::core::ffi::c_void;
pub type ISoftDistExt = *mut ::core::ffi::c_void;
pub type IUriBuilderFactory = *mut ::core::ffi::c_void;
pub type IUriContainer = *mut ::core::ffi::c_void;
pub type IWinInetCacheHints = *mut ::core::ffi::c_void;
pub type IWinInetCacheHints2 = *mut ::core::ffi::c_void;
pub type IWinInetFileStream = *mut ::core::ffi::c_void;
pub type IWinInetHttpInfo = *mut ::core::ffi::c_void;
pub type IWinInetHttpTimeouts = *mut ::core::ffi::c_void;
pub type IWinInetInfo = *mut ::core::ffi::c_void;
pub type IWindowForBindingUI = *mut ::core::ffi::c_void;
pub type IWrappedProtocol = *mut ::core::ffi::c_void;
pub type IZoneIdentifier = *mut ::core::ffi::c_void;
pub type IZoneIdentifier2 = *mut ::core::ffi::c_void;
pub const AUTHENTICATEF_BASIC: AUTHENTICATEF = 2i32;
pub const AUTHENTICATEF_HTTP: AUTHENTICATEF = 4i32;
pub const AUTHENTICATEF_PROXY: AUTHENTICATEF = 1i32;
pub const BINDF2_ALLOW_PROXY_CRED_PROMPT: BINDF2 = 256i32;
pub const BINDF2_DISABLEAUTOCOOKIEHANDLING: BINDF2 = 2i32;
pub const BINDF2_DISABLEBASICOVERHTTP: BINDF2 = 1i32;
pub const BINDF2_DISABLE_HTTP_REDIRECT_CACHING: BINDF2 = 64i32;
pub const BINDF2_DISABLE_HTTP_REDIRECT_XSECURITYID: BINDF2 = 8i32;
pub const BINDF2_KEEP_CALLBACK_MODULE_LOADED: BINDF2 = 128i32;
pub const BINDF2_READ_DATA_GREATER_THAN_4GB: BINDF2 = 4i32;
pub const BINDF2_RESERVED_1: BINDF2 = -2147483648i32;
pub const BINDF2_RESERVED_10: BINDF2 = 65536i32;
pub const BINDF2_RESERVED_11: BINDF2 = 32768i32;
pub const BINDF2_RESERVED_12: BINDF2 = 16384i32;
pub const BINDF2_RESERVED_13: BINDF2 = 8192i32;
pub const BINDF2_RESERVED_14: BINDF2 = 4096i32;
pub const BINDF2_RESERVED_15: BINDF2 = 2048i32;
pub const BINDF2_RESERVED_16: BINDF2 = 1024i32;
pub const BINDF2_RESERVED_17: BINDF2 = 512i32;
pub const BINDF2_RESERVED_2: BINDF2 = 1073741824i32;
pub const BINDF2_RESERVED_3: BINDF2 = 536870912i32;
pub const BINDF2_RESERVED_4: BINDF2 = 268435456i32;
pub const BINDF2_RESERVED_5: BINDF2 = 134217728i32;
pub const BINDF2_RESERVED_6: BINDF2 = 67108864i32;
pub const BINDF2_RESERVED_7: BINDF2 = 33554432i32;
pub const BINDF2_RESERVED_8: BINDF2 = 16777216i32;
pub const BINDF2_RESERVED_9: BINDF2 = 8388608i32;
pub const BINDF2_RESERVED_A: BINDF2 = 4194304i32;
pub const BINDF2_RESERVED_B: BINDF2 = 2097152i32;
pub const BINDF2_RESERVED_C: BINDF2 = 1048576i32;
pub const BINDF2_RESERVED_D: BINDF2 = 524288i32;
pub const BINDF2_RESERVED_E: BINDF2 = 262144i32;
pub const BINDF2_RESERVED_F: BINDF2 = 131072i32;
pub const BINDF2_SETDOWNLOADMODE: BINDF2 = 32i32;
pub const BINDF_ASYNCHRONOUS: BINDF = 1i32;
pub const BINDF_ASYNCSTORAGE: BINDF = 2i32;
pub const BINDF_DIRECT_READ: BINDF = 131072i32;
pub const BINDF_ENFORCERESTRICTED: BINDF = 8388608i32;
pub const BINDF_FORMS_SUBMIT: BINDF = 262144i32;
pub const BINDF_FREE_THREADED: BINDF = 65536i32;
pub const BINDF_FROMURLMON: BINDF = 1048576i32;
pub const BINDF_FWD_BACK: BINDF = 2097152i32;
pub const BINDF_GETCLASSOBJECT: BINDF = 16384i32;
pub const BINDF_GETFROMCACHE_IF_NET_FAIL: BINDF = 524288i32;
pub const BINDF_GETNEWESTVERSION: BINDF = 16i32;
pub const BINDF_HYPERLINK: BINDF = 1024i32;
pub const BINDF_IGNORESECURITYPROBLEM: BINDF = 256i32;
pub const BINDF_NEEDFILE: BINDF = 64i32;
pub const BINDF_NOPROGRESSIVERENDERING: BINDF = 4i32;
pub const BINDF_NOWRITECACHE: BINDF = 32i32;
pub const BINDF_NO_UI: BINDF = 2048i32;
pub const BINDF_OFFLINEOPERATION: BINDF = 8i32;
pub const BINDF_PRAGMA_NO_CACHE: BINDF = 8192i32;
pub const BINDF_PREFERDEFAULTHANDLER: BINDF = 4194304i32;
pub const BINDF_PULLDATA: BINDF = 128i32;
pub const BINDF_RESERVED_1: BINDF = 32768i32;
pub const BINDF_RESERVED_2: BINDF = -2147483648i32;
pub const BINDF_RESERVED_3: BINDF = 16777216i32;
pub const BINDF_RESERVED_4: BINDF = 33554432i32;
pub const BINDF_RESERVED_5: BINDF = 67108864i32;
pub const BINDF_RESERVED_6: BINDF = 134217728i32;
pub const BINDF_RESERVED_7: BINDF = 1073741824i32;
pub const BINDF_RESERVED_8: BINDF = 536870912i32;
pub const BINDF_RESYNCHRONIZE: BINDF = 512i32;
pub const BINDF_SILENTOPERATION: BINDF = 4096i32;
pub const BINDHANDLETYPES_APPCACHE: BINDHANDLETYPES = 0i32;
pub const BINDHANDLETYPES_COUNT: BINDHANDLETYPES = 2i32;
pub const BINDHANDLETYPES_DEPENDENCY: BINDHANDLETYPES = 1i32;
pub const BINDINFO_OPTIONS_ALLOWCONNECTDATA: BINDINFO_OPTIONS = 536870912i32;
pub const BINDINFO_OPTIONS_BINDTOOBJECT: BINDINFO_OPTIONS = 1048576i32;
pub const BINDINFO_OPTIONS_DISABLEAUTOREDIRECTS: BINDINFO_OPTIONS = 1073741824i32;
pub const BINDINFO_OPTIONS_DISABLE_UTF8: BINDINFO_OPTIONS = 262144i32;
pub const BINDINFO_OPTIONS_ENABLE_UTF8: BINDINFO_OPTIONS = 131072i32;
pub const BINDINFO_OPTIONS_IGNOREHTTPHTTPSREDIRECTS: BINDINFO_OPTIONS = 16777216i32;
pub const BINDINFO_OPTIONS_IGNOREMIMETEXTPLAIN: BINDINFO_OPTIONS = 4194304i32;
pub const BINDINFO_OPTIONS_IGNORE_SSLERRORS_ONCE: BINDINFO_OPTIONS = 33554432i32;
pub const BINDINFO_OPTIONS_SECURITYOPTOUT: BINDINFO_OPTIONS = 2097152i32;
pub const BINDINFO_OPTIONS_SHDOCVW_NAVIGATE: BINDINFO_OPTIONS = -2147483648i32;
pub const BINDINFO_OPTIONS_USEBINDSTRINGCREDS: BINDINFO_OPTIONS = 8388608i32;
pub const BINDINFO_OPTIONS_USE_IE_ENCODING: BINDINFO_OPTIONS = 524288i32;
pub const BINDINFO_OPTIONS_WININETFLAG: BINDINFO_OPTIONS = 65536i32;
pub const BINDINFO_WPC_DOWNLOADBLOCKED: BINDINFO_OPTIONS = 134217728i32;
pub const BINDINFO_WPC_LOGGING_ENABLED: BINDINFO_OPTIONS = 268435456i32;
pub const BINDSTATUS_64BIT_PROGRESS: BINDSTATUS = 56i32;
pub const BINDSTATUS_ACCEPTRANGES: BINDSTATUS = 33i32;
pub const BINDSTATUS_BEGINDOWNLOADCOMPONENTS: BINDSTATUS = 7i32;
pub const BINDSTATUS_BEGINDOWNLOADDATA: BINDSTATUS = 4i32;
pub const BINDSTATUS_BEGINSYNCOPERATION: BINDSTATUS = 15i32;
pub const BINDSTATUS_BEGINUPLOADDATA: BINDSTATUS = 17i32;
pub const BINDSTATUS_CACHECONTROL: BINDSTATUS = 48i32;
pub const BINDSTATUS_CACHEFILENAMEAVAILABLE: BINDSTATUS = 14i32;
pub const BINDSTATUS_CLASSIDAVAILABLE: BINDSTATUS = 12i32;
pub const BINDSTATUS_CLASSINSTALLLOCATION: BINDSTATUS = 23i32;
pub const BINDSTATUS_CLSIDCANINSTANTIATE: BINDSTATUS = 28i32;
pub const BINDSTATUS_COMPACT_POLICY_RECEIVED: BINDSTATUS = 35i32;
pub const BINDSTATUS_CONNECTING: BINDSTATUS = 2i32;
pub const BINDSTATUS_CONTENTDISPOSITIONATTACH: BINDSTATUS = 26i32;
pub const BINDSTATUS_CONTENTDISPOSITIONFILENAME: BINDSTATUS = 49i32;
pub const BINDSTATUS_COOKIE_SENT: BINDSTATUS = 34i32;
pub const BINDSTATUS_COOKIE_STATE_ACCEPT: BINDSTATUS = 38i32;
pub const BINDSTATUS_COOKIE_STATE_DOWNGRADE: BINDSTATUS = 42i32;
pub const BINDSTATUS_COOKIE_STATE_LEASH: BINDSTATUS = 41i32;
pub const BINDSTATUS_COOKIE_STATE_PROMPT: BINDSTATUS = 40i32;
pub const BINDSTATUS_COOKIE_STATE_REJECT: BINDSTATUS = 39i32;
pub const BINDSTATUS_COOKIE_STATE_UNKNOWN: BINDSTATUS = 37i32;
pub const BINDSTATUS_COOKIE_SUPPRESSED: BINDSTATUS = 36i32;
pub const BINDSTATUS_DECODING: BINDSTATUS = 24i32;
pub const BINDSTATUS_DIRECTBIND: BINDSTATUS = 30i32;
pub const BINDSTATUS_DISPLAYNAMEAVAILABLE: BINDSTATUS = 52i32;
pub const BINDSTATUS_DOWNLOADINGDATA: BINDSTATUS = 5i32;
pub const BINDSTATUS_ENCODING: BINDSTATUS = 21i32;
pub const BINDSTATUS_ENDDOWNLOADCOMPONENTS: BINDSTATUS = 9i32;
pub const BINDSTATUS_ENDDOWNLOADDATA: BINDSTATUS = 6i32;
pub const BINDSTATUS_ENDSYNCOPERATION: BINDSTATUS = 16i32;
pub const BINDSTATUS_ENDUPLOADDATA: BINDSTATUS = 19i32;
pub const BINDSTATUS_FILTERREPORTMIMETYPE: BINDSTATUS = 27i32;
pub const BINDSTATUS_FINDINGRESOURCE: BINDSTATUS = 1i32;
pub const BINDSTATUS_INSTALLINGCOMPONENTS: BINDSTATUS = 8i32;
pub const BINDSTATUS_IUNKNOWNAVAILABLE: BINDSTATUS = 29i32;
pub const BINDSTATUS_LAST: BINDSTATUS = 56i32;
pub const BINDSTATUS_LAST_PRIVATE: BINDSTATUS = 77i32;
pub const BINDSTATUS_LOADINGMIMEHANDLER: BINDSTATUS = 25i32;
pub const BINDSTATUS_MIMETEXTPLAINMISMATCH: BINDSTATUS = 50i32;
pub const BINDSTATUS_MIMETYPEAVAILABLE: BINDSTATUS = 13i32;
pub const BINDSTATUS_P3P_HEADER: BINDSTATUS = 44i32;
pub const BINDSTATUS_PERSISTENT_COOKIE_RECEIVED: BINDSTATUS = 46i32;
pub const BINDSTATUS_POLICY_HREF: BINDSTATUS = 43i32;
pub const BINDSTATUS_PROTOCOLCLASSID: BINDSTATUS = 20i32;
pub const BINDSTATUS_PROXYDETECTING: BINDSTATUS = 32i32;
pub const BINDSTATUS_PUBLISHERAVAILABLE: BINDSTATUS = 51i32;
pub const BINDSTATUS_RAWMIMETYPE: BINDSTATUS = 31i32;
pub const BINDSTATUS_REDIRECTING: BINDSTATUS = 3i32;
pub const BINDSTATUS_RESERVED_0: BINDSTATUS = 57i32;
pub const BINDSTATUS_RESERVED_1: BINDSTATUS = 58i32;
pub const BINDSTATUS_RESERVED_10: BINDSTATUS = 73i32;
pub const BINDSTATUS_RESERVED_11: BINDSTATUS = 74i32;
pub const BINDSTATUS_RESERVED_12: BINDSTATUS = 75i32;
pub const BINDSTATUS_RESERVED_13: BINDSTATUS = 76i32;
pub const BINDSTATUS_RESERVED_14: BINDSTATUS = 77i32;
pub const BINDSTATUS_RESERVED_2: BINDSTATUS = 59i32;
pub const BINDSTATUS_RESERVED_3: BINDSTATUS = 60i32;
pub const BINDSTATUS_RESERVED_4: BINDSTATUS = 61i32;
pub const BINDSTATUS_RESERVED_5: BINDSTATUS = 62i32;
pub const BINDSTATUS_RESERVED_6: BINDSTATUS = 63i32;
pub const BINDSTATUS_RESERVED_7: BINDSTATUS = 64i32;
pub const BINDSTATUS_RESERVED_8: BINDSTATUS = 65i32;
pub const BINDSTATUS_RESERVED_9: BINDSTATUS = 66i32;
pub const BINDSTATUS_RESERVED_A: BINDSTATUS = 67i32;
pub const BINDSTATUS_RESERVED_B: BINDSTATUS = 68i32;
pub const BINDSTATUS_RESERVED_C: BINDSTATUS = 69i32;
pub const BINDSTATUS_RESERVED_D: BINDSTATUS = 70i32;
pub const BINDSTATUS_RESERVED_E: BINDSTATUS = 71i32;
pub const BINDSTATUS_RESERVED_F: BINDSTATUS = 72i32;
pub const BINDSTATUS_SENDINGREQUEST: BINDSTATUS = 11i32;
pub const BINDSTATUS_SERVER_MIMETYPEAVAILABLE: BINDSTATUS = 54i32;
pub const BINDSTATUS_SESSION_COOKIES_ALLOWED: BINDSTATUS = 47i32;
pub const BINDSTATUS_SESSION_COOKIE_RECEIVED: BINDSTATUS = 45i32;
pub const BINDSTATUS_SNIFFED_CLASSIDAVAILABLE: BINDSTATUS = 55i32;
pub const BINDSTATUS_SSLUX_NAVBLOCKED: BINDSTATUS = 53i32;
pub const BINDSTATUS_UPLOADINGDATA: BINDSTATUS = 18i32;
pub const BINDSTATUS_USINGCACHEDCOPY: BINDSTATUS = 10i32;
pub const BINDSTATUS_VERIFIEDMIMETYPEAVAILABLE: BINDSTATUS = 22i32;
pub const BINDSTRING_ACCEPT_ENCODINGS: BINDSTRING = 11i32;
pub const BINDSTRING_ACCEPT_MIMES: BINDSTRING = 2i32;
pub const BINDSTRING_DOC_URL: BINDSTRING = 25i32;
pub const BINDSTRING_DOWNLOADPATH: BINDSTRING = 19i32;
pub const BINDSTRING_ENTERPRISE_ID: BINDSTRING = 24i32;
pub const BINDSTRING_EXTRA_URL: BINDSTRING = 3i32;
pub const BINDSTRING_FLAG_BIND_TO_OBJECT: BINDSTRING = 16i32;
pub const BINDSTRING_HEADERS: BINDSTRING = 1i32;
pub const BINDSTRING_IID: BINDSTRING = 15i32;
pub const BINDSTRING_INITIAL_FILENAME: BINDSTRING = 21i32;
pub const BINDSTRING_LANGUAGE: BINDSTRING = 4i32;
pub const BINDSTRING_OS: BINDSTRING = 9i32;
pub const BINDSTRING_PASSWORD: BINDSTRING = 6i32;
pub const BINDSTRING_POST_COOKIE: BINDSTRING = 12i32;
pub const BINDSTRING_POST_DATA_MIME: BINDSTRING = 13i32;
pub const BINDSTRING_PROXY_PASSWORD: BINDSTRING = 23i32;
pub const BINDSTRING_PROXY_USERNAME: BINDSTRING = 22i32;
pub const BINDSTRING_PTR_BIND_CONTEXT: BINDSTRING = 17i32;
pub const BINDSTRING_ROOTDOC_URL: BINDSTRING = 20i32;
pub const BINDSTRING_SAMESITE_COOKIE_LEVEL: BINDSTRING = 26i32;
pub const BINDSTRING_UA_COLOR: BINDSTRING = 8i32;
pub const BINDSTRING_UA_PIXELS: BINDSTRING = 7i32;
pub const BINDSTRING_URL: BINDSTRING = 14i32;
pub const BINDSTRING_USERNAME: BINDSTRING = 5i32;
pub const BINDSTRING_USER_AGENT: BINDSTRING = 10i32;
pub const BINDSTRING_XDR_ORIGIN: BINDSTRING = 18i32;
pub const BINDVERB_CUSTOM: BINDVERB = 3i32;
pub const BINDVERB_GET: BINDVERB = 0i32;
pub const BINDVERB_POST: BINDVERB = 1i32;
pub const BINDVERB_PUT: BINDVERB = 2i32;
pub const BINDVERB_RESERVED1: BINDVERB = 4i32;
pub const BSCF_64BITLENGTHDOWNLOAD: BSCF = 64i32;
pub const BSCF_AVAILABLEDATASIZEUNKNOWN: BSCF = 16i32;
pub const BSCF_DATAFULLYAVAILABLE: BSCF = 8i32;
pub const BSCF_FIRSTDATANOTIFICATION: BSCF = 1i32;
pub const BSCF_INTERMEDIATEDATANOTIFICATION: BSCF = 2i32;
pub const BSCF_LASTDATANOTIFICATION: BSCF = 4i32;
pub const BSCF_SKIPDRAINDATAFORFILEURLS: BSCF = 32i32;
pub const CF_NULL: u32 = 0u32;
pub const CIP_ACCESS_DENIED: CIP_STATUS = 1i32;
pub const CIP_DISK_FULL: CIP_STATUS = 0i32;
pub const CIP_EXE_SELF_REGISTERATION_TIMEOUT: CIP_STATUS = 6i32;
pub const CIP_NAME_CONFLICT: CIP_STATUS = 4i32;
pub const CIP_NEED_REBOOT: CIP_STATUS = 8i32;
pub const CIP_NEED_REBOOT_UI_PERMISSION: CIP_STATUS = 9i32;
pub const CIP_NEWER_VERSION_EXISTS: CIP_STATUS = 2i32;
pub const CIP_OLDER_VERSION_EXISTS: CIP_STATUS = 3i32;
pub const CIP_TRUST_VERIFICATION_COMPONENT_MISSING: CIP_STATUS = 5i32;
pub const CIP_UNSAFE_TO_ABORT: CIP_STATUS = 7i32;
pub const CLASSIDPROP: MONIKERPROPERTY = 2i32;
pub const CONFIRMSAFETYACTION_LOADOBJECT: u32 = 1u32;
pub const E_PENDING: ::windows_sys::core::HRESULT = -2147483638i32;
pub const FEATURE_ADDON_MANAGEMENT: INTERNETFEATURELIST = 13i32;
pub const FEATURE_BEHAVIORS: INTERNETFEATURELIST = 6i32;
pub const FEATURE_BLOCK_INPUT_PROMPTS: INTERNETFEATURELIST = 27i32;
pub const FEATURE_DISABLE_LEGACY_COMPRESSION: INTERNETFEATURELIST = 22i32;
pub const FEATURE_DISABLE_MK_PROTOCOL: INTERNETFEATURELIST = 7i32;
pub const FEATURE_DISABLE_NAVIGATION_SOUNDS: INTERNETFEATURELIST = 21i32;
pub const FEATURE_DISABLE_TELNET_PROTOCOL: INTERNETFEATURELIST = 25i32;
pub const FEATURE_ENTRY_COUNT: INTERNETFEATURELIST = 28i32;
pub const FEATURE_FEEDS: INTERNETFEATURELIST = 26i32;
pub const FEATURE_FORCE_ADDR_AND_STATUS: INTERNETFEATURELIST = 23i32;
pub const FEATURE_GET_URL_DOM_FILEPATH_UNENCODED: INTERNETFEATURELIST = 18i32;
pub const FEATURE_HTTP_USERNAME_PASSWORD_DISABLE: INTERNETFEATURELIST = 15i32;
pub const FEATURE_LOCALMACHINE_LOCKDOWN: INTERNETFEATURELIST = 8i32;
pub const FEATURE_MIME_HANDLING: INTERNETFEATURELIST = 2i32;
pub const FEATURE_MIME_SNIFFING: INTERNETFEATURELIST = 3i32;
pub const FEATURE_OBJECT_CACHING: INTERNETFEATURELIST = 0i32;
pub const FEATURE_PROTOCOL_LOCKDOWN: INTERNETFEATURELIST = 14i32;
pub const FEATURE_RESTRICT_ACTIVEXINSTALL: INTERNETFEATURELIST = 10i32;
pub const FEATURE_RESTRICT_FILEDOWNLOAD: INTERNETFEATURELIST = 12i32;
pub const FEATURE_SAFE_BINDTOOBJECT: INTERNETFEATURELIST = 16i32;
pub const FEATURE_SECURITYBAND: INTERNETFEATURELIST = 9i32;
pub const FEATURE_SSLUX: INTERNETFEATURELIST = 20i32;
pub const FEATURE_TABBED_BROWSING: INTERNETFEATURELIST = 19i32;
pub const FEATURE_UNC_SAVEDFILECHECK: INTERNETFEATURELIST = 17i32;
pub const FEATURE_VALIDATE_NAVIGATE_URL: INTERNETFEATURELIST = 11i32;
pub const FEATURE_WEBOC_POPUPMANAGEMENT: INTERNETFEATURELIST = 5i32;
pub const FEATURE_WINDOW_RESTRICTIONS: INTERNETFEATURELIST = 4i32;
pub const FEATURE_XMLHTTP: INTERNETFEATURELIST = 24i32;
pub const FEATURE_ZONE_ELEVATION: INTERNETFEATURELIST = 1i32;
pub const FIEF_FLAG_FORCE_JITUI: u32 = 1u32;
pub const FIEF_FLAG_PEEK: u32 = 2u32;
pub const FIEF_FLAG_RESERVED_0: u32 = 8u32;
pub const FIEF_FLAG_SKIP_INSTALLED_VERSION_CHECK: u32 = 4u32;
pub const FMFD_DEFAULT: u32 = 0u32;
pub const FMFD_ENABLEMIMESNIFFING: u32 = 2u32;
pub const FMFD_IGNOREMIMETEXTPLAIN: u32 = 4u32;
pub const FMFD_RESERVED_1: u32 = 64u32;
pub const FMFD_RESERVED_2: u32 = 128u32;
pub const FMFD_RESPECTTEXTPLAIN: u32 = 16u32;
pub const FMFD_RETURNUPDATEDIMGMIMES: u32 = 32u32;
pub const FMFD_SERVERMIME: u32 = 8u32;
pub const FMFD_URLASFILENAME: u32 = 1u32;
pub const GET_FEATURE_FROM_PROCESS: u32 = 2u32;
pub const GET_FEATURE_FROM_REGISTRY: u32 = 4u32;
pub const GET_FEATURE_FROM_THREAD: u32 = 1u32;
pub const GET_FEATURE_FROM_THREAD_INTERNET: u32 = 64u32;
pub const GET_FEATURE_FROM_THREAD_INTRANET: u32 = 16u32;
pub const GET_FEATURE_FROM_THREAD_LOCALMACHINE: u32 = 8u32;
pub const GET_FEATURE_FROM_THREAD_RESTRICTED: u32 = 128u32;
pub const GET_FEATURE_FROM_THREAD_TRUSTED: u32 = 32u32;
pub const IE_EPM_OBJECT_EVENT: IEObjectType = 0i32;
pub const IE_EPM_OBJECT_FILE: IEObjectType = 5i32;
pub const IE_EPM_OBJECT_MUTEX: IEObjectType = 1i32;
pub const IE_EPM_OBJECT_NAMED_PIPE: IEObjectType = 6i32;
pub const IE_EPM_OBJECT_REGISTRY: IEObjectType = 7i32;
pub const IE_EPM_OBJECT_SEMAPHORE: IEObjectType = 2i32;
pub const IE_EPM_OBJECT_SHARED_MEMORY: IEObjectType = 3i32;
pub const IE_EPM_OBJECT_WAITABLE_TIMER: IEObjectType = 4i32;
pub const INET_E_AUTHENTICATION_REQUIRED: ::windows_sys::core::HRESULT = -2146697207i32;
pub const INET_E_BLOCKED_ENHANCEDPROTECTEDMODE: ::windows_sys::core::HRESULT = -2146695930i32;
pub const INET_E_BLOCKED_PLUGGABLE_PROTOCOL: ::windows_sys::core::HRESULT = -2146695931i32;
pub const INET_E_BLOCKED_REDIRECT_XSECURITYID: ::windows_sys::core::HRESULT = -2146697189i32;
pub const INET_E_CANNOT_CONNECT: ::windows_sys::core::HRESULT = -2146697212i32;
pub const INET_E_CANNOT_INSTANTIATE_OBJECT: ::windows_sys::core::HRESULT = -2146697200i32;
pub const INET_E_CANNOT_LOAD_DATA: ::windows_sys::core::HRESULT = -2146697201i32;
pub const INET_E_CANNOT_LOCK_REQUEST: ::windows_sys::core::HRESULT = -2146697194i32;
pub const INET_E_CANNOT_REPLACE_SFP_FILE: ::windows_sys::core::HRESULT = -2146696448i32;
pub const INET_E_CODE_DOWNLOAD_DECLINED: ::windows_sys::core::HRESULT = -2146696960i32;
pub const INET_E_CODE_INSTALL_BLOCKED_ARM: ::windows_sys::core::HRESULT = -2146695932i32;
pub const INET_E_CODE_INSTALL_BLOCKED_BITNESS: ::windows_sys::core::HRESULT = -2146695929i32;
pub const INET_E_CODE_INSTALL_BLOCKED_BY_HASH_POLICY: ::windows_sys::core::HRESULT = -2146695936i32;
pub const INET_E_CODE_INSTALL_BLOCKED_IMMERSIVE: ::windows_sys::core::HRESULT = -2146695934i32;
pub const INET_E_CODE_INSTALL_SUPPRESSED: ::windows_sys::core::HRESULT = -2146696192i32;
pub const INET_E_CONNECTION_TIMEOUT: ::windows_sys::core::HRESULT = -2146697205i32;
pub const INET_E_DATA_NOT_AVAILABLE: ::windows_sys::core::HRESULT = -2146697209i32;
pub const INET_E_DEFAULT_ACTION: i32 = -2146697199i32;
pub const INET_E_DOMINJECTIONVALIDATION: ::windows_sys::core::HRESULT = -2146697188i32;
pub const INET_E_DOWNLOAD_BLOCKED_BY_CSP: ::windows_sys::core::HRESULT = -2146695928i32;
pub const INET_E_DOWNLOAD_BLOCKED_BY_INPRIVATE: ::windows_sys::core::HRESULT = -2146695935i32;
pub const INET_E_DOWNLOAD_FAILURE: ::windows_sys::core::HRESULT = -2146697208i32;
pub const INET_E_ERROR_FIRST: ::windows_sys::core::HRESULT = -2146697214i32;
pub const INET_E_ERROR_LAST: i32 = -2146695928i32;
pub const INET_E_FORBIDFRAMING: ::windows_sys::core::HRESULT = -2146695933i32;
pub const INET_E_HSTS_CERTIFICATE_ERROR: ::windows_sys::core::HRESULT = -2146697186i32;
pub const INET_E_INVALID_CERTIFICATE: ::windows_sys::core::HRESULT = -2146697191i32;
pub const INET_E_INVALID_REQUEST: ::windows_sys::core::HRESULT = -2146697204i32;
pub const INET_E_INVALID_URL: ::windows_sys::core::HRESULT = -2146697214i32;
pub const INET_E_NO_SESSION: ::windows_sys::core::HRESULT = -2146697213i32;
pub const INET_E_NO_VALID_MEDIA: ::windows_sys::core::HRESULT = -2146697206i32;
pub const INET_E_OBJECT_NOT_FOUND: ::windows_sys::core::HRESULT = -2146697210i32;
pub const INET_E_QUERYOPTION_UNKNOWN: ::windows_sys::core::HRESULT = -2146697197i32;
pub const INET_E_REDIRECTING: ::windows_sys::core::HRESULT = -2146697196i32;
pub const INET_E_REDIRECT_FAILED: ::windows_sys::core::HRESULT = -2146697196i32;
pub const INET_E_REDIRECT_TO_DIR: ::windows_sys::core::HRESULT = -2146697195i32;
pub const INET_E_RESERVED_1: ::windows_sys::core::HRESULT = -2146697190i32;
pub const INET_E_RESERVED_2: ::windows_sys::core::HRESULT = -2146697185i32;
pub const INET_E_RESERVED_3: ::windows_sys::core::HRESULT = -2146697184i32;
pub const INET_E_RESERVED_4: ::windows_sys::core::HRESULT = -2146697183i32;
pub const INET_E_RESERVED_5: ::windows_sys::core::HRESULT = -2146697182i32;
pub const INET_E_RESOURCE_NOT_FOUND: ::windows_sys::core::HRESULT = -2146697211i32;
pub const INET_E_RESULT_DISPATCHED: ::windows_sys::core::HRESULT = -2146696704i32;
pub const INET_E_SECURITY_PROBLEM: ::windows_sys::core::HRESULT = -2146697202i32;
pub const INET_E_TERMINATED_BIND: ::windows_sys::core::HRESULT = -2146697192i32;
pub const INET_E_UNKNOWN_PROTOCOL: ::windows_sys::core::HRESULT = -2146697203i32;
pub const INET_E_USE_DEFAULT_PROTOCOLHANDLER: ::windows_sys::core::HRESULT = -2146697199i32;
pub const INET_E_USE_DEFAULT_SETTING: ::windows_sys::core::HRESULT = -2146697198i32;
pub const INET_E_USE_EXTEND_BINDING: ::windows_sys::core::HRESULT = -2146697193i32;
pub const INET_E_VTAB_SWITCH_FORCE_ENGINE: ::windows_sys::core::HRESULT = -2146697187i32;
pub const MAX_SIZE_SECURITY_ID: u32 = 512u32;
pub const MAX_ZONE_DESCRIPTION: INET_ZONE_MANAGER_CONSTANTS = 200i32;
pub const MAX_ZONE_PATH: INET_ZONE_MANAGER_CONSTANTS = 260i32;
pub const MIMETYPEPROP: MONIKERPROPERTY = 0i32;
pub const MKSYS_URLMONIKER: u32 = 6u32;
pub const MK_S_ASYNCHRONOUS: ::windows_sys::core::HRESULT = 262632i32;
pub const MUTZ_ACCEPT_WILDCARD_SCHEME: u32 = 128u32;
pub const MUTZ_DONT_UNESCAPE: u32 = 2048u32;
pub const MUTZ_DONT_USE_CACHE: u32 = 4096u32;
pub const MUTZ_ENFORCERESTRICTED: u32 = 256u32;
pub const MUTZ_FORCE_INTRANET_FLAGS: u32 = 8192u32;
pub const MUTZ_IGNORE_ZONE_MAPPINGS: u32 = 16384u32;
pub const MUTZ_ISFILE: u32 = 2u32;
pub const MUTZ_NOSAVEDFILECHECK: u32 = 1u32;
pub const MUTZ_REQUIRESAVEDFILECHECK: u32 = 1024u32;
pub const MUTZ_RESERVED: u32 = 512u32;
pub const OIBDG_APARTMENTTHREADED: OIBDG_FLAGS = 256i32;
pub const OIBDG_DATAONLY: OIBDG_FLAGS = 4096i32;
pub const PARSE_ANCHOR: PARSEACTION = 6i32;
pub const PARSE_CANONICALIZE: PARSEACTION = 1i32;
pub const PARSE_DECODE_IS_ESCAPE: PARSEACTION = 8i32;
pub const PARSE_DOCUMENT: PARSEACTION = 5i32;
pub const PARSE_DOMAIN: PARSEACTION = 15i32;
pub const PARSE_ENCODE_IS_UNESCAPE: PARSEACTION = 7i32;
pub const PARSE_ESCAPE: PARSEACTION = 18i32;
pub const PARSE_FRIENDLY: PARSEACTION = 2i32;
pub const PARSE_LOCATION: PARSEACTION = 16i32;
pub const PARSE_MIME: PARSEACTION = 11i32;
pub const PARSE_PATH_FROM_URL: PARSEACTION = 9i32;
pub const PARSE_ROOTDOCUMENT: PARSEACTION = 4i32;
pub const PARSE_SCHEMA: PARSEACTION = 13i32;
pub const PARSE_SECURITY_DOMAIN: PARSEACTION = 17i32;
pub const PARSE_SECURITY_URL: PARSEACTION = 3i32;
pub const PARSE_SERVER: PARSEACTION = 12i32;
pub const PARSE_SITE: PARSEACTION = 14i32;
pub const PARSE_UNESCAPE: PARSEACTION = 19i32;
pub const PARSE_URL_FROM_PATH: PARSEACTION = 10i32;
pub const PD_FORCE_SWITCH: PI_FLAGS = 65536i32;
pub const PI_APARTMENTTHREADED: PI_FLAGS = 256i32;
pub const PI_CLASSINSTALL: PI_FLAGS = 512i32;
pub const PI_CLSIDLOOKUP: PI_FLAGS = 32i32;
pub const PI_DATAPROGRESS: PI_FLAGS = 64i32;
pub const PI_FILTER_MODE: PI_FLAGS = 2i32;
pub const PI_FORCE_ASYNC: PI_FLAGS = 4i32;
pub const PI_LOADAPPDIRECT: PI_FLAGS = 16384i32;
pub const PI_MIMEVERIFICATION: PI_FLAGS = 16i32;
pub const PI_NOMIMEHANDLER: PI_FLAGS = 32768i32;
pub const PI_PARSE_URL: PI_FLAGS = 1i32;
pub const PI_PASSONBINDCTX: PI_FLAGS = 8192i32;
pub const PI_PREFERDEFAULTHANDLER: PI_FLAGS = 131072i32;
pub const PI_SYNCHRONOUS: PI_FLAGS = 128i32;
pub const PI_USE_WORKERTHREAD: PI_FLAGS = 8i32;
pub const POPUPLEVELPROP: MONIKERPROPERTY = 4i32;
pub const PROTOCOLFLAG_NO_PICS_CHECK: u32 = 1u32;
pub const PSU_DEFAULT: PSUACTION = 1i32;
pub const PSU_SECURITY_URL_ONLY: PSUACTION = 2i32;
pub const PUAFOUT_DEFAULT: PUAFOUT = 0i32;
pub const PUAFOUT_ISLOCKZONEPOLICY: PUAFOUT = 1i32;
pub const PUAF_ACCEPT_WILDCARD_SCHEME: PUAF = 128i32;
pub const PUAF_CHECK_TIFS: PUAF = 16i32;
pub const PUAF_DEFAULT: PUAF = 0i32;
pub const PUAF_DEFAULTZONEPOL: PUAF = 262144i32;
pub const PUAF_DONTCHECKBOXINDIALOG: PUAF = 32i32;
pub const PUAF_DONT_USE_CACHE: PUAF = 4096i32;
pub const PUAF_DRAGPROTOCOLCHECK: PUAF = 2097152i32;
pub const PUAF_ENFORCERESTRICTED: PUAF = 256i32;
pub const PUAF_FORCEUI_FOREGROUND: PUAF = 8i32;
pub const PUAF_ISFILE: PUAF = 2i32;
pub const PUAF_LMZ_LOCKED: PUAF = 131072i32;
pub const PUAF_LMZ_UNLOCKED: PUAF = 65536i32;
pub const PUAF_NOSAVEDFILECHECK: PUAF = 512i32;
pub const PUAF_NOUI: PUAF = 1i32;
pub const PUAF_NOUIIFLOCKED: PUAF = 1048576i32;
pub const PUAF_NPL_USE_LOCKED_IF_RESTRICTED: PUAF = 524288i32;
pub const PUAF_REQUIRESAVEDFILECHECK: PUAF = 1024i32;
pub const PUAF_RESERVED1: PUAF = 8192i32;
pub const PUAF_RESERVED2: PUAF = 16384i32;
pub const PUAF_TRUSTED: PUAF = 64i32;
pub const PUAF_WARN_IF_DENIED: PUAF = 4i32;
pub const QUERY_CAN_NAVIGATE: QUERYOPTION = 7i32;
pub const QUERY_CONTENT_ENCODING: QUERYOPTION = 3i32;
pub const QUERY_CONTENT_TYPE: QUERYOPTION = 4i32;
pub const QUERY_EXPIRATION_DATE: QUERYOPTION = 1i32;
pub const QUERY_IS_CACHED: QUERYOPTION = 9i32;
pub const QUERY_IS_CACHED_AND_USABLE_OFFLINE: QUERYOPTION = 16i32;
pub const QUERY_IS_CACHED_OR_MAPPED: QUERYOPTION = 11i32;
pub const QUERY_IS_INSTALLEDENTRY: QUERYOPTION = 10i32;
pub const QUERY_IS_SAFE: QUERYOPTION = 14i32;
pub const QUERY_IS_SECURE: QUERYOPTION = 13i32;
pub const QUERY_RECOMBINE: QUERYOPTION = 6i32;
pub const QUERY_REFRESH: QUERYOPTION = 5i32;
pub const QUERY_TIME_OF_LAST_CHANGE: QUERYOPTION = 2i32;
pub const QUERY_USES_CACHE: QUERYOPTION = 12i32;
pub const QUERY_USES_HISTORYFOLDER: QUERYOPTION = 15i32;
pub const QUERY_USES_NETWORK: QUERYOPTION = 8i32;
pub const SECURITY_IE_STATE_GREEN: u32 = 0u32;
pub const SECURITY_IE_STATE_RED: u32 = 1u32;
pub const SET_FEATURE_IN_REGISTRY: u32 = 4u32;
pub const SET_FEATURE_ON_PROCESS: u32 = 2u32;
pub const SET_FEATURE_ON_THREAD: u32 = 1u32;
pub const SET_FEATURE_ON_THREAD_INTERNET: u32 = 64u32;
pub const SET_FEATURE_ON_THREAD_INTRANET: u32 = 16u32;
pub const SET_FEATURE_ON_THREAD_LOCALMACHINE: u32 = 8u32;
pub const SET_FEATURE_ON_THREAD_RESTRICTED: u32 = 128u32;
pub const SET_FEATURE_ON_THREAD_TRUSTED: u32 = 32u32;
pub const SOFTDIST_ADSTATE_AVAILABLE: u32 = 1u32;
pub const SOFTDIST_ADSTATE_DOWNLOADED: u32 = 2u32;
pub const SOFTDIST_ADSTATE_INSTALLED: u32 = 3u32;
pub const SOFTDIST_ADSTATE_NONE: u32 = 0u32;
pub const SOFTDIST_FLAG_DELETE_SUBSCRIPTION: u32 = 8u32;
pub const SOFTDIST_FLAG_USAGE_AUTOINSTALL: u32 = 4u32;
pub const SOFTDIST_FLAG_USAGE_EMAIL: u32 = 1u32;
pub const SOFTDIST_FLAG_USAGE_PRECACHE: u32 = 2u32;
pub const SZM_CREATE: SZM_FLAGS = 0i32;
pub const SZM_DELETE: SZM_FLAGS = 1i32;
pub const S_ASYNCHRONOUS: i32 = 262632i32;
pub const TRUSTEDDOWNLOADPROP: MONIKERPROPERTY = 3i32;
pub const UAS_EXACTLEGACY: u32 = 4096u32;
pub const URLACTION_ACTIVEX_ALLOW_TDC: u32 = 4620u32;
pub const URLACTION_ACTIVEX_CONFIRM_NOOBJECTSAFETY: u32 = 4612u32;
pub const URLACTION_ACTIVEX_CURR_MAX: u32 = 4620u32;
pub const URLACTION_ACTIVEX_DYNSRC_VIDEO_AND_ANIMATION: u32 = 4618u32;
pub const URLACTION_ACTIVEX_MAX: u32 = 5119u32;
pub const URLACTION_ACTIVEX_MIN: u32 = 4608u32;
pub const URLACTION_ACTIVEX_NO_WEBOC_SCRIPT: u32 = 4614u32;
pub const URLACTION_ACTIVEX_OVERRIDE_DATA_SAFETY: u32 = 4610u32;
pub const URLACTION_ACTIVEX_OVERRIDE_DOMAINLIST: u32 = 4619u32;
pub const URLACTION_ACTIVEX_OVERRIDE_OBJECT_SAFETY: u32 = 4609u32;
pub const URLACTION_ACTIVEX_OVERRIDE_OPTIN: u32 = 4616u32;
pub const URLACTION_ACTIVEX_OVERRIDE_REPURPOSEDETECTION: u32 = 4615u32;
pub const URLACTION_ACTIVEX_OVERRIDE_SCRIPT_SAFETY: u32 = 4611u32;
pub const URLACTION_ACTIVEX_RUN: u32 = 4608u32;
pub const URLACTION_ACTIVEX_SCRIPTLET_RUN: u32 = 4617u32;
pub const URLACTION_ACTIVEX_TREATASUNTRUSTED: u32 = 4613u32;
pub const URLACTION_ALLOW_ACTIVEX_FILTERING: u32 = 9986u32;
pub const URLACTION_ALLOW_ANTIMALWARE_SCANNING_OF_ACTIVEX: u32 = 9996u32;
pub const URLACTION_ALLOW_APEVALUATION: u32 = 8961u32;
pub const URLACTION_ALLOW_AUDIO_VIDEO: u32 = 9985u32;
pub const URLACTION_ALLOW_AUDIO_VIDEO_PLUGINS: u32 = 9988u32;
pub const URLACTION_ALLOW_CROSSDOMAIN_APPCACHE_MANIFEST: u32 = 9994u32;
pub const URLACTION_ALLOW_CROSSDOMAIN_DROP_ACROSS_WINDOWS: u32 = 9993u32;
pub const URLACTION_ALLOW_CROSSDOMAIN_DROP_WITHIN_WINDOW: u32 = 9992u32;
pub const URLACTION_ALLOW_CSS_EXPRESSIONS: u32 = 9997u32;
pub const URLACTION_ALLOW_JSCRIPT_IE: u32 = 5133u32;
pub const URLACTION_ALLOW_RENDER_LEGACY_DXTFILTERS: u32 = 9995u32;
pub const URLACTION_ALLOW_RESTRICTEDPROTOCOLS: u32 = 8960u32;
pub const URLACTION_ALLOW_STRUCTURED_STORAGE_SNIFFING: u32 = 9987u32;
pub const URLACTION_ALLOW_VBSCRIPT_IE: u32 = 5132u32;
pub const URLACTION_ALLOW_XDOMAIN_SUBFRAME_RESIZE: u32 = 5128u32;
pub const URLACTION_ALLOW_XHR_EVALUATION: u32 = 8962u32;
pub const URLACTION_ALLOW_ZONE_ELEVATION_OPT_OUT_ADDITION: u32 = 9990u32;
pub const URLACTION_ALLOW_ZONE_ELEVATION_VIA_OPT_OUT: u32 = 9989u32;
pub const URLACTION_AUTHENTICATE_CLIENT: u32 = 6657u32;
pub const URLACTION_AUTOMATIC_ACTIVEX_UI: u32 = 8705u32;
pub const URLACTION_AUTOMATIC_DOWNLOAD_UI: u32 = 8704u32;
pub const URLACTION_AUTOMATIC_DOWNLOAD_UI_MIN: u32 = 8704u32;
pub const URLACTION_BEHAVIOR_MIN: u32 = 8192u32;
pub const URLACTION_BEHAVIOR_RUN: u32 = 8192u32;
pub const URLACTION_CHANNEL_SOFTDIST_MAX: u32 = 7935u32;
pub const URLACTION_CHANNEL_SOFTDIST_MIN: u32 = 7680u32;
pub const URLACTION_CHANNEL_SOFTDIST_PERMISSIONS: u32 = 7685u32;
pub const URLACTION_CLIENT_CERT_PROMPT: u32 = 6660u32;
pub const URLACTION_COOKIES: u32 = 6658u32;
pub const URLACTION_COOKIES_ENABLED: u32 = 6672u32;
pub const URLACTION_COOKIES_SESSION: u32 = 6659u32;
pub const URLACTION_COOKIES_SESSION_THIRD_PARTY: u32 = 6662u32;
pub const URLACTION_COOKIES_THIRD_PARTY: u32 = 6661u32;
pub const URLACTION_CREDENTIALS_USE: u32 = 6656u32;
pub const URLACTION_CROSS_DOMAIN_DATA: u32 = 5126u32;
pub const URLACTION_DOTNET_USERCONTROLS: u32 = 8197u32;
pub const URLACTION_DOWNLOAD_CURR_MAX: u32 = 4100u32;
pub const URLACTION_DOWNLOAD_MAX: u32 = 4607u32;
pub const URLACTION_DOWNLOAD_MIN: u32 = 4096u32;
pub const URLACTION_DOWNLOAD_SIGNED_ACTIVEX: u32 = 4097u32;
pub const URLACTION_DOWNLOAD_UNSIGNED_ACTIVEX: u32 = 4100u32;
pub const URLACTION_FEATURE_BLOCK_INPUT_PROMPTS: u32 = 8453u32;
pub const URLACTION_FEATURE_CROSSDOMAIN_FOCUS_CHANGE: u32 = 8455u32;
pub const URLACTION_FEATURE_DATA_BINDING: u32 = 8454u32;
pub const URLACTION_FEATURE_FORCE_ADDR_AND_STATUS: u32 = 8452u32;
pub const URLACTION_FEATURE_MIME_SNIFFING: u32 = 8448u32;
pub const URLACTION_FEATURE_MIN: u32 = 8448u32;
pub const URLACTION_FEATURE_SCRIPT_STATUS_BAR: u32 = 8451u32;
pub const URLACTION_FEATURE_WINDOW_RESTRICTIONS: u32 = 8450u32;
pub const URLACTION_FEATURE_ZONE_ELEVATION: u32 = 8449u32;
pub const URLACTION_HTML_ALLOW_CROSS_DOMAIN_CANVAS: u32 = 5645u32;
pub const URLACTION_HTML_ALLOW_CROSS_DOMAIN_TEXTTRACK: u32 = 5648u32;
pub const URLACTION_HTML_ALLOW_CROSS_DOMAIN_WEBWORKER: u32 = 5647u32;
pub const URLACTION_HTML_ALLOW_INDEXEDDB: u32 = 5649u32;
pub const URLACTION_HTML_ALLOW_INJECTED_DYNAMIC_HTML: u32 = 5643u32;
pub const URLACTION_HTML_ALLOW_WINDOW_CLOSE: u32 = 5646u32;
pub const URLACTION_HTML_FONT_DOWNLOAD: u32 = 5636u32;
pub const URLACTION_HTML_INCLUDE_FILE_PATH: u32 = 5642u32;
pub const URLACTION_HTML_JAVA_RUN: u32 = 5637u32;
pub const URLACTION_HTML_MAX: u32 = 6143u32;
pub const URLACTION_HTML_META_REFRESH: u32 = 5640u32;
pub const URLACTION_HTML_MIN: u32 = 5632u32;
pub const URLACTION_HTML_MIXED_CONTENT: u32 = 5641u32;
pub const URLACTION_HTML_REQUIRE_UTF8_DOCUMENT_CODEPAGE: u32 = 5644u32;
pub const URLACTION_HTML_SUBFRAME_NAVIGATE: u32 = 5639u32;
pub const URLACTION_HTML_SUBMIT_FORMS: u32 = 5633u32;
pub const URLACTION_HTML_SUBMIT_FORMS_FROM: u32 = 5634u32;
pub const URLACTION_HTML_SUBMIT_FORMS_TO: u32 = 5635u32;
pub const URLACTION_HTML_USERDATA_SAVE: u32 = 5638u32;
pub const URLACTION_INFODELIVERY_CURR_MAX: u32 = 7430u32;
pub const URLACTION_INFODELIVERY_MAX: u32 = 7679u32;
pub const URLACTION_INFODELIVERY_MIN: u32 = 7424u32;
pub const URLACTION_INFODELIVERY_NO_ADDING_CHANNELS: u32 = 7424u32;
pub const URLACTION_INFODELIVERY_NO_ADDING_SUBSCRIPTIONS: u32 = 7427u32;
pub const URLACTION_INFODELIVERY_NO_CHANNEL_LOGGING: u32 = 7430u32;
pub const URLACTION_INFODELIVERY_NO_EDITING_CHANNELS: u32 = 7425u32;
pub const URLACTION_INFODELIVERY_NO_EDITING_SUBSCRIPTIONS: u32 = 7428u32;
pub const URLACTION_INFODELIVERY_NO_REMOVING_CHANNELS: u32 = 7426u32;
pub const URLACTION_INFODELIVERY_NO_REMOVING_SUBSCRIPTIONS: u32 = 7429u32;
pub const URLACTION_INPRIVATE_BLOCKING: u32 = 9984u32;
pub const URLACTION_JAVA_CURR_MAX: u32 = 7168u32;
pub const URLACTION_JAVA_MAX: u32 = 7423u32;
pub const URLACTION_JAVA_MIN: u32 = 7168u32;
pub const URLACTION_JAVA_PERMISSIONS: u32 = 7168u32;
pub const URLACTION_LOOSE_XAML: u32 = 9218u32;
pub const URLACTION_LOWRIGHTS: u32 = 9472u32;
pub const URLACTION_MIN: u32 = 4096u32;
pub const URLACTION_NETWORK_CURR_MAX: u32 = 6672u32;
pub const URLACTION_NETWORK_MAX: u32 = 7167u32;
pub const URLACTION_NETWORK_MIN: u32 = 6656u32;
pub const URLACTION_PLUGGABLE_PROTOCOL_XHR: u32 = 5131u32;
pub const URLACTION_SCRIPT_CURR_MAX: u32 = 5133u32;
pub const URLACTION_SCRIPT_JAVA_USE: u32 = 5122u32;
pub const URLACTION_SCRIPT_MAX: u32 = 5631u32;
pub const URLACTION_SCRIPT_MIN: u32 = 5120u32;
pub const URLACTION_SCRIPT_NAVIGATE: u32 = 5130u32;
pub const URLACTION_SCRIPT_OVERRIDE_SAFETY: u32 = 5121u32;
pub const URLACTION_SCRIPT_PASTE: u32 = 5127u32;
pub const URLACTION_SCRIPT_RUN: u32 = 5120u32;
pub const URLACTION_SCRIPT_SAFE_ACTIVEX: u32 = 5125u32;
pub const URLACTION_SCRIPT_XSSFILTER: u32 = 5129u32;
pub const URLACTION_SHELL_ALLOW_CROSS_SITE_SHARE: u32 = 6161u32;
pub const URLACTION_SHELL_CURR_MAX: u32 = 6162u32;
pub const URLACTION_SHELL_ENHANCED_DRAGDROP_SECURITY: u32 = 6155u32;
pub const URLACTION_SHELL_EXECUTE_HIGHRISK: u32 = 6150u32;
pub const URLACTION_SHELL_EXECUTE_LOWRISK: u32 = 6152u32;
pub const URLACTION_SHELL_EXECUTE_MODRISK: u32 = 6151u32;
pub const URLACTION_SHELL_EXTENSIONSECURITY: u32 = 6156u32;
pub const URLACTION_SHELL_FILE_DOWNLOAD: u32 = 6147u32;
pub const URLACTION_SHELL_INSTALL_DTITEMS: u32 = 6144u32;
pub const URLACTION_SHELL_MAX: u32 = 6655u32;
pub const URLACTION_SHELL_MIN: u32 = 6144u32;
pub const URLACTION_SHELL_MOVE_OR_COPY: u32 = 6146u32;
pub const URLACTION_SHELL_POPUPMGR: u32 = 6153u32;
pub const URLACTION_SHELL_PREVIEW: u32 = 6159u32;
pub const URLACTION_SHELL_REMOTEQUERY: u32 = 6158u32;
pub const URLACTION_SHELL_RTF_OBJECTS_LOAD: u32 = 6154u32;
pub const URLACTION_SHELL_SECURE_DRAGSOURCE: u32 = 6157u32;
pub const URLACTION_SHELL_SHARE: u32 = 6160u32;
pub const URLACTION_SHELL_SHELLEXECUTE: u32 = 6150u32;
pub const URLACTION_SHELL_TOCTOU_RISK: u32 = 6162u32;
pub const URLACTION_SHELL_VERB: u32 = 6148u32;
pub const URLACTION_SHELL_WEBVIEW_VERB: u32 = 6149u32;
pub const URLACTION_WINDOWS_BROWSER_APPLICATIONS: u32 = 9216u32;
pub const URLACTION_WINFX_SETUP: u32 = 9728u32;
pub const URLACTION_XPS_DOCUMENTS: u32 = 9217u32;
pub const URLMON_OPTION_URL_ENCODING: u32 = 268435460u32;
pub const URLMON_OPTION_USERAGENT: u32 = 268435457u32;
pub const URLMON_OPTION_USERAGENT_REFRESH: u32 = 268435458u32;
pub const URLMON_OPTION_USE_BINDSTRINGCREDS: u32 = 268435464u32;
pub const URLMON_OPTION_USE_BROWSERAPPSDOCUMENTS: u32 = 268435472u32;
pub const URLOSTRM_GETNEWESTVERSION: u32 = 3u32;
pub const URLOSTRM_USECACHEDCOPY: u32 = 2u32;
pub const URLOSTRM_USECACHEDCOPY_ONLY: u32 = 1u32;
pub const URLPOLICY_ACTIVEX_CHECK_LIST: u32 = 65536u32;
pub const URLPOLICY_ALLOW: u32 = 0u32;
pub const URLPOLICY_AUTHENTICATE_CHALLENGE_RESPONSE: u32 = 65536u32;
pub const URLPOLICY_AUTHENTICATE_CLEARTEXT_OK: u32 = 0u32;
pub const URLPOLICY_AUTHENTICATE_MUTUAL_ONLY: u32 = 196608u32;
pub const URLPOLICY_BEHAVIOR_CHECK_LIST: u32 = 65536u32;
pub const URLPOLICY_CHANNEL_SOFTDIST_AUTOINSTALL: u32 = 196608u32;
pub const URLPOLICY_CHANNEL_SOFTDIST_PRECACHE: u32 = 131072u32;
pub const URLPOLICY_CHANNEL_SOFTDIST_PROHIBIT: u32 = 65536u32;
pub const URLPOLICY_CREDENTIALS_ANONYMOUS_ONLY: u32 = 196608u32;
pub const URLPOLICY_CREDENTIALS_CONDITIONAL_PROMPT: u32 = 131072u32;
pub const URLPOLICY_CREDENTIALS_MUST_PROMPT_USER: u32 = 65536u32;
pub const URLPOLICY_CREDENTIALS_SILENT_LOGON_OK: u32 = 0u32;
pub const URLPOLICY_DISALLOW: u32 = 3u32;
pub const URLPOLICY_DONTCHECKDLGBOX: u32 = 256u32;
pub const URLPOLICY_JAVA_CUSTOM: u32 = 8388608u32;
pub const URLPOLICY_JAVA_HIGH: u32 = 65536u32;
pub const URLPOLICY_JAVA_LOW: u32 = 196608u32;
pub const URLPOLICY_JAVA_MEDIUM: u32 = 131072u32;
pub const URLPOLICY_JAVA_PROHIBIT: u32 = 0u32;
pub const URLPOLICY_LOG_ON_ALLOW: u32 = 64u32;
pub const URLPOLICY_LOG_ON_DISALLOW: u32 = 128u32;
pub const URLPOLICY_MASK_PERMISSIONS: u32 = 15u32;
pub const URLPOLICY_NOTIFY_ON_ALLOW: u32 = 16u32;
pub const URLPOLICY_NOTIFY_ON_DISALLOW: u32 = 32u32;
pub const URLPOLICY_QUERY: u32 = 1u32;
pub const URLTEMPLATE_CUSTOM: URLTEMPLATE = 0i32;
pub const URLTEMPLATE_HIGH: URLTEMPLATE = 73728i32;
pub const URLTEMPLATE_LOW: URLTEMPLATE = 65536i32;
pub const URLTEMPLATE_MEDHIGH: URLTEMPLATE = 70912i32;
pub const URLTEMPLATE_MEDIUM: URLTEMPLATE = 69632i32;
pub const URLTEMPLATE_MEDLOW: URLTEMPLATE = 66816i32;
pub const URLTEMPLATE_PREDEFINED_MAX: URLTEMPLATE = 131072i32;
pub const URLTEMPLATE_PREDEFINED_MIN: URLTEMPLATE = 65536i32;
pub const URLZONEREG_DEFAULT: URLZONEREG = 0i32;
pub const URLZONEREG_HKCU: URLZONEREG = 2i32;
pub const URLZONEREG_HKLM: URLZONEREG = 1i32;
pub const URLZONE_ESC_FLAG: u32 = 256u32;
pub const URLZONE_INTERNET: URLZONE = 3i32;
pub const URLZONE_INTRANET: URLZONE = 1i32;
pub const URLZONE_INVALID: URLZONE = -1i32;
pub const URLZONE_LOCAL_MACHINE: URLZONE = 0i32;
pub const URLZONE_PREDEFINED_MAX: URLZONE = 999i32;
pub const URLZONE_PREDEFINED_MIN: URLZONE = 0i32;
pub const URLZONE_TRUSTED: URLZONE = 2i32;
pub const URLZONE_UNTRUSTED: URLZONE = 4i32;
pub const URLZONE_USER_MAX: URLZONE = 10000i32;
pub const URLZONE_USER_MIN: URLZONE = 1000i32;
pub const URL_ENCODING_DISABLE_UTF8: URL_ENCODING = 536870912i32;
pub const URL_ENCODING_ENABLE_UTF8: URL_ENCODING = 268435456i32;
pub const URL_ENCODING_NONE: URL_ENCODING = 0i32;
pub const URL_MK_LEGACY: u32 = 0u32;
pub const URL_MK_NO_CANONICALIZE: u32 = 2u32;
pub const URL_MK_UNIFORM: u32 = 1u32;
pub const USE_SRC_URL: MONIKERPROPERTY = 1i32;
pub const UriBuilder_USE_ORIGINAL_FLAGS: u32 = 1u32;
pub const Uri_DISPLAY_IDN_HOST: u32 = 4u32;
pub const Uri_DISPLAY_NO_FRAGMENT: u32 = 1u32;
pub const Uri_DISPLAY_NO_PUNYCODE: u32 = 8u32;
pub const Uri_ENCODING_HOST_IS_IDN: u32 = 4u32;
pub const Uri_ENCODING_HOST_IS_PERCENT_ENCODED_CP: u32 = 16u32;
pub const Uri_ENCODING_HOST_IS_PERCENT_ENCODED_UTF8: u32 = 8u32;
pub const Uri_ENCODING_QUERY_AND_FRAGMENT_IS_CP: u32 = 64u32;
pub const Uri_ENCODING_QUERY_AND_FRAGMENT_IS_PERCENT_ENCODED_UTF8: u32 = 32u32;
pub const Uri_ENCODING_USER_INFO_AND_PATH_IS_CP: u32 = 2u32;
pub const Uri_ENCODING_USER_INFO_AND_PATH_IS_PERCENT_ENCODED_UTF8: u32 = 1u32;
pub const Uri_HOST_DNS: Uri_HOST_TYPE = 1i32;
pub const Uri_HOST_IDN: Uri_HOST_TYPE = 4i32;
pub const Uri_HOST_IPV4: Uri_HOST_TYPE = 2i32;
pub const Uri_HOST_IPV6: Uri_HOST_TYPE = 3i32;
pub const Uri_HOST_UNKNOWN: Uri_HOST_TYPE = 0i32;
pub const Uri_PUNYCODE_IDN_HOST: u32 = 2u32;
pub const WININETINFO_OPTION_LOCK_HANDLE: u32 = 65534u32;
pub const ZAFLAGS_ADD_SITES: ZAFLAGS = 2i32;
pub const ZAFLAGS_CUSTOM_EDIT: ZAFLAGS = 1i32;
pub const ZAFLAGS_DETECT_INTRANET: ZAFLAGS = 256i32;
pub const ZAFLAGS_INCLUDE_INTRANET_SITES: ZAFLAGS = 16i32;
pub const ZAFLAGS_INCLUDE_PROXY_OVERRIDE: ZAFLAGS = 8i32;
pub const ZAFLAGS_NO_CACHE: ZAFLAGS = 262144i32;
pub const ZAFLAGS_NO_UI: ZAFLAGS = 32i32;
pub const ZAFLAGS_REQUIRE_VERIFICATION: ZAFLAGS = 4i32;
pub const ZAFLAGS_SUPPORTS_VERIFICATION: ZAFLAGS = 64i32;
pub const ZAFLAGS_UNC_AS_INTRANET: ZAFLAGS = 128i32;
pub const ZAFLAGS_USE_LOCKED_ZONES: ZAFLAGS = 65536i32;
pub const ZAFLAGS_VERIFY_TEMPLATE_SETTINGS: ZAFLAGS = 131072i32;
pub type AUTHENTICATEF = i32;
pub type BINDF = i32;
pub type BINDF2 = i32;
pub type BINDHANDLETYPES = i32;
pub type BINDINFO_OPTIONS = i32;
pub type BINDSTATUS = i32;
pub type BINDSTRING = i32;
pub type BINDVERB = i32;
pub type BSCF = i32;
pub type CIP_STATUS = i32;
pub type IEObjectType = i32;
pub type INET_ZONE_MANAGER_CONSTANTS = i32;
pub type INTERNETFEATURELIST = i32;
pub type MONIKERPROPERTY = i32;
pub type OIBDG_FLAGS = i32;
pub type PARSEACTION = i32;
pub type PI_FLAGS = i32;
pub type PSUACTION = i32;
pub type PUAF = i32;
pub type PUAFOUT = i32;
pub type QUERYOPTION = i32;
pub type SZM_FLAGS = i32;
pub type URLTEMPLATE = i32;
pub type URLZONE = i32;
pub type URLZONEREG = i32;
pub type URL_ENCODING = i32;
pub type Uri_HOST_TYPE = i32;
pub type ZAFLAGS = i32;
#[repr(C)]
pub struct CODEBASEHOLD {
    pub cbSize: u32,
    pub szDistUnit: ::windows_sys::core::PWSTR,
    pub szCodeBase: ::windows_sys::core::PWSTR,
    pub dwVersionMS: u32,
    pub dwVersionLS: u32,
    pub dwStyle: u32,
}
impl ::core::marker::Copy for CODEBASEHOLD {}
impl ::core::clone::Clone for CODEBASEHOLD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CONFIRMSAFETY {
    pub clsid: ::windows_sys::core::GUID,
    pub pUnk: ::windows_sys::core::IUnknown,
    pub dwFlags: u32,
}
impl ::core::marker::Copy for CONFIRMSAFETY {}
impl ::core::clone::Clone for CONFIRMSAFETY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DATAINFO {
    pub ulTotalSize: u32,
    pub ulavrPacketSize: u32,
    pub ulConnectSpeed: u32,
    pub ulProcessorSpeed: u32,
}
impl ::core::marker::Copy for DATAINFO {}
impl ::core::clone::Clone for DATAINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HIT_LOGGING_INFO {
    pub dwStructSize: u32,
    pub lpszLoggedUrlName: ::windows_sys::core::PSTR,
    pub StartTime: super::super::super::Foundation::SYSTEMTIME,
    pub EndTime: super::super::super::Foundation::SYSTEMTIME,
    pub lpszExtendedInfo: ::windows_sys::core::PSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HIT_LOGGING_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HIT_LOGGING_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROTOCOLDATA {
    pub grfFlags: u32,
    pub dwState: u32,
    pub pData: *mut ::core::ffi::c_void,
    pub cbData: u32,
}
impl ::core::marker::Copy for PROTOCOLDATA {}
impl ::core::clone::Clone for PROTOCOLDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROTOCOLFILTERDATA {
    pub cbSize: u32,
    pub pProtocolSink: IInternetProtocolSink,
    pub pProtocol: IInternetProtocol,
    pub pUnk: ::windows_sys::core::IUnknown,
    pub dwFilterFlags: u32,
}
impl ::core::marker::Copy for PROTOCOLFILTERDATA {}
impl ::core::clone::Clone for PROTOCOLFILTERDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PROTOCOL_ARGUMENT {
    pub szMethod: ::windows_sys::core::PCWSTR,
    pub szTargetUrl: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for PROTOCOL_ARGUMENT {}
impl ::core::clone::Clone for PROTOCOL_ARGUMENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct REMSECURITY_ATTRIBUTES {
    pub nLength: u32,
    pub lpSecurityDescriptor: u32,
    pub bInheritHandle: super::super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for REMSECURITY_ATTRIBUTES {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for REMSECURITY_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RemBINDINFO {
    pub cbSize: u32,
    pub szExtraInfo: ::windows_sys::core::PWSTR,
    pub grfBindInfoF: u32,
    pub dwBindVerb: u32,
    pub szCustomVerb: ::windows_sys::core::PWSTR,
    pub cbstgmedData: u32,
    pub dwOptions: u32,
    pub dwOptionsFlags: u32,
    pub dwCodePage: u32,
    pub securityAttributes: REMSECURITY_ATTRIBUTES,
    pub iid: ::windows_sys::core::GUID,
    pub pUnk: ::windows_sys::core::IUnknown,
    pub dwReserved: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RemBINDINFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RemBINDINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RemFORMATETC {
    pub cfFormat: u32,
    pub ptd: u32,
    pub dwAspect: u32,
    pub lindex: i32,
    pub tymed: u32,
}
impl ::core::marker::Copy for RemFORMATETC {}
impl ::core::clone::Clone for RemFORMATETC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SOFTDISTINFO {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub dwAdState: u32,
    pub szTitle: ::windows_sys::core::PWSTR,
    pub szAbstract: ::windows_sys::core::PWSTR,
    pub szHREF: ::windows_sys::core::PWSTR,
    pub dwInstalledVersionMS: u32,
    pub dwInstalledVersionLS: u32,
    pub dwUpdateVersionMS: u32,
    pub dwUpdateVersionLS: u32,
    pub dwAdvertisedVersionMS: u32,
    pub dwAdvertisedVersionLS: u32,
    pub dwReserved: u32,
}
impl ::core::marker::Copy for SOFTDISTINFO {}
impl ::core::clone::Clone for SOFTDISTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct StartParam {
    pub iid: ::windows_sys::core::GUID,
    pub pIBindCtx: super::IBindCtx,
    pub pItf: ::windows_sys::core::IUnknown,
}
impl ::core::marker::Copy for StartParam {}
impl ::core::clone::Clone for StartParam {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ZONEATTRIBUTES {
    pub cbSize: u32,
    pub szDisplayName: [u16; 260],
    pub szDescription: [u16; 200],
    pub szIconPath: [u16; 260],
    pub dwTemplateMinLevel: u32,
    pub dwTemplateRecommended: u32,
    pub dwTemplateCurrentLevel: u32,
    pub dwFlags: u32,
}
impl ::core::marker::Copy for ZONEATTRIBUTES {}
impl ::core::clone::Clone for ZONEATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
