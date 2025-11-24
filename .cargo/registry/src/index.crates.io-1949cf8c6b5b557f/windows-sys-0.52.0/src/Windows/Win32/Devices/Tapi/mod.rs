#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn GetTnefStreamCodepage(lpstream : super::super::System::Com:: IStream, lpulcodepage : *mut u32, lpulsubcodepage : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_System_AddressBook", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_System_AddressBook\"`, `\"Win32_System_Com\"`"] fn OpenTnefStream(lpvsupport : *mut ::core::ffi::c_void, lpstream : super::super::System::Com:: IStream, lpszstreamname : *const i8, ulflags : u32, lpmessage : super::super::System::AddressBook:: IMessage, wkeyval : u16, lpptnef : *mut ITnef) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_System_AddressBook", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_System_AddressBook\"`, `\"Win32_System_Com\"`"] fn OpenTnefStreamEx(lpvsupport : *mut ::core::ffi::c_void, lpstream : super::super::System::Com:: IStream, lpszstreamname : *const i8, ulflags : u32, lpmessage : super::super::System::AddressBook:: IMessage, wkeyval : u16, lpadressbook : super::super::System::AddressBook:: IAddrBook, lpptnef : *mut ITnef) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("tapi32.dll" "system" fn lineAccept(hcall : u32, lpsuseruserinfo : ::windows_sys::core::PCSTR, dwsize : u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineAddProvider(lpszproviderfilename : ::windows_sys::core::PCSTR, hwndowner : super::super::Foundation:: HWND, lpdwpermanentproviderid : *mut u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineAddProviderA(lpszproviderfilename : ::windows_sys::core::PCSTR, hwndowner : super::super::Foundation:: HWND, lpdwpermanentproviderid : *mut u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineAddProviderW(lpszproviderfilename : ::windows_sys::core::PCWSTR, hwndowner : super::super::Foundation:: HWND, lpdwpermanentproviderid : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineAddToConference(hconfcall : u32, hconsultcall : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineAgentSpecific(hline : u32, dwaddressid : u32, dwagentextensionidindex : u32, lpparams : *mut ::core::ffi::c_void, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineAnswer(hcall : u32, lpsuseruserinfo : ::windows_sys::core::PCSTR, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineBlindTransfer(hcall : u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineBlindTransferA(hcall : u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineBlindTransferW(hcall : u32, lpszdestaddressw : ::windows_sys::core::PCWSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineClose(hline : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineCompleteCall(hcall : u32, lpdwcompletionid : *mut u32, dwcompletionmode : u32, dwmessageid : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineCompleteTransfer(hcall : u32, hconsultcall : u32, lphconfcall : *mut u32, dwtransfermode : u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineConfigDialog(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineConfigDialogA(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineConfigDialogEdit(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCSTR, lpdeviceconfigin : *const ::core::ffi::c_void, dwsize : u32, lpdeviceconfigout : *mut VARSTRING) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineConfigDialogEditA(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCSTR, lpdeviceconfigin : *const ::core::ffi::c_void, dwsize : u32, lpdeviceconfigout : *mut VARSTRING) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineConfigDialogEditW(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCWSTR, lpdeviceconfigin : *const ::core::ffi::c_void, dwsize : u32, lpdeviceconfigout : *mut VARSTRING) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineConfigDialogW(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineConfigProvider(hwndowner : super::super::Foundation:: HWND, dwpermanentproviderid : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineCreateAgentA(hline : u32, lpszagentid : ::windows_sys::core::PCSTR, lpszagentpin : ::windows_sys::core::PCSTR, lphagent : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineCreateAgentSessionA(hline : u32, hagent : u32, lpszagentpin : ::windows_sys::core::PCSTR, dwworkingaddressid : u32, lpgroupid : *mut ::windows_sys::core::GUID, lphagentsession : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineCreateAgentSessionW(hline : u32, hagent : u32, lpszagentpin : ::windows_sys::core::PCWSTR, dwworkingaddressid : u32, lpgroupid : *mut ::windows_sys::core::GUID, lphagentsession : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineCreateAgentW(hline : u32, lpszagentid : ::windows_sys::core::PCWSTR, lpszagentpin : ::windows_sys::core::PCWSTR, lphagent : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineDeallocateCall(hcall : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineDevSpecific(hline : u32, dwaddressid : u32, hcall : u32, lpparams : *mut ::core::ffi::c_void, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineDevSpecificFeature(hline : u32, dwfeature : u32, lpparams : *mut ::core::ffi::c_void, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineDial(hcall : u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineDialA(hcall : u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineDialW(hcall : u32, lpszdestaddress : ::windows_sys::core::PCWSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineDrop(hcall : u32, lpsuseruserinfo : ::windows_sys::core::PCSTR, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineForward(hline : u32, balladdresses : u32, dwaddressid : u32, lpforwardlist : *const LINEFORWARDLIST, dwnumringsnoanswer : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineForwardA(hline : u32, balladdresses : u32, dwaddressid : u32, lpforwardlist : *const LINEFORWARDLIST, dwnumringsnoanswer : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineForwardW(hline : u32, balladdresses : u32, dwaddressid : u32, lpforwardlist : *const LINEFORWARDLIST, dwnumringsnoanswer : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGatherDigits(hcall : u32, dwdigitmodes : u32, lpsdigits : ::windows_sys::core::PSTR, dwnumdigits : u32, lpszterminationdigits : ::windows_sys::core::PCSTR, dwfirstdigittimeout : u32, dwinterdigittimeout : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGatherDigitsA(hcall : u32, dwdigitmodes : u32, lpsdigits : ::windows_sys::core::PSTR, dwnumdigits : u32, lpszterminationdigits : ::windows_sys::core::PCSTR, dwfirstdigittimeout : u32, dwinterdigittimeout : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGatherDigitsW(hcall : u32, dwdigitmodes : u32, lpsdigits : ::windows_sys::core::PWSTR, dwnumdigits : u32, lpszterminationdigits : ::windows_sys::core::PCWSTR, dwfirstdigittimeout : u32, dwinterdigittimeout : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGenerateDigits(hcall : u32, dwdigitmode : u32, lpszdigits : ::windows_sys::core::PCSTR, dwduration : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGenerateDigitsA(hcall : u32, dwdigitmode : u32, lpszdigits : ::windows_sys::core::PCSTR, dwduration : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGenerateDigitsW(hcall : u32, dwdigitmode : u32, lpszdigits : ::windows_sys::core::PCWSTR, dwduration : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGenerateTone(hcall : u32, dwtonemode : u32, dwduration : u32, dwnumtones : u32, lptones : *const LINEGENERATETONE) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressCaps(hlineapp : u32, dwdeviceid : u32, dwaddressid : u32, dwapiversion : u32, dwextversion : u32, lpaddresscaps : *mut LINEADDRESSCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressCapsA(hlineapp : u32, dwdeviceid : u32, dwaddressid : u32, dwapiversion : u32, dwextversion : u32, lpaddresscaps : *mut LINEADDRESSCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressCapsW(hlineapp : u32, dwdeviceid : u32, dwaddressid : u32, dwapiversion : u32, dwextversion : u32, lpaddresscaps : *mut LINEADDRESSCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressID(hline : u32, lpdwaddressid : *mut u32, dwaddressmode : u32, lpsaddress : ::windows_sys::core::PCSTR, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressIDA(hline : u32, lpdwaddressid : *mut u32, dwaddressmode : u32, lpsaddress : ::windows_sys::core::PCSTR, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressIDW(hline : u32, lpdwaddressid : *mut u32, dwaddressmode : u32, lpsaddress : ::windows_sys::core::PCWSTR, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressStatus(hline : u32, dwaddressid : u32, lpaddressstatus : *mut LINEADDRESSSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressStatusA(hline : u32, dwaddressid : u32, lpaddressstatus : *mut LINEADDRESSSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAddressStatusW(hline : u32, dwaddressid : u32, lpaddressstatus : *mut LINEADDRESSSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentActivityListA(hline : u32, dwaddressid : u32, lpagentactivitylist : *mut LINEAGENTACTIVITYLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentActivityListW(hline : u32, dwaddressid : u32, lpagentactivitylist : *mut LINEAGENTACTIVITYLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentCapsA(hlineapp : u32, dwdeviceid : u32, dwaddressid : u32, dwappapiversion : u32, lpagentcaps : *mut LINEAGENTCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentCapsW(hlineapp : u32, dwdeviceid : u32, dwaddressid : u32, dwappapiversion : u32, lpagentcaps : *mut LINEAGENTCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentGroupListA(hline : u32, dwaddressid : u32, lpagentgrouplist : *mut LINEAGENTGROUPLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentGroupListW(hline : u32, dwaddressid : u32, lpagentgrouplist : *mut LINEAGENTGROUPLIST) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn lineGetAgentInfo(hline : u32, hagent : u32, lpagentinfo : *mut LINEAGENTINFO) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn lineGetAgentSessionInfo(hline : u32, hagentsession : u32, lpagentsessioninfo : *mut LINEAGENTSESSIONINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentSessionList(hline : u32, hagent : u32, lpagentsessionlist : *mut LINEAGENTSESSIONLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentStatusA(hline : u32, dwaddressid : u32, lpagentstatus : *mut LINEAGENTSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAgentStatusW(hline : u32, dwaddressid : u32, lpagentstatus : *mut LINEAGENTSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAppPriority(lpszappfilename : ::windows_sys::core::PCSTR, dwmediamode : u32, lpextensionid : *mut LINEEXTENSIONID, dwrequestmode : u32, lpextensionname : *mut VARSTRING, lpdwpriority : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAppPriorityA(lpszappfilename : ::windows_sys::core::PCSTR, dwmediamode : u32, lpextensionid : *mut LINEEXTENSIONID, dwrequestmode : u32, lpextensionname : *mut VARSTRING, lpdwpriority : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetAppPriorityW(lpszappfilename : ::windows_sys::core::PCWSTR, dwmediamode : u32, lpextensionid : *mut LINEEXTENSIONID, dwrequestmode : u32, lpextensionname : *mut VARSTRING, lpdwpriority : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetCallInfo(hcall : u32, lpcallinfo : *mut LINECALLINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetCallInfoA(hcall : u32, lpcallinfo : *mut LINECALLINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetCallInfoW(hcall : u32, lpcallinfo : *mut LINECALLINFO) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineGetCallStatus(hcall : u32, lpcallstatus : *mut LINECALLSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetConfRelatedCalls(hcall : u32, lpcalllist : *mut LINECALLLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetCountry(dwcountryid : u32, dwapiversion : u32, lplinecountrylist : *mut LINECOUNTRYLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetCountryA(dwcountryid : u32, dwapiversion : u32, lplinecountrylist : *mut LINECOUNTRYLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetCountryW(dwcountryid : u32, dwapiversion : u32, lplinecountrylist : *mut LINECOUNTRYLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetDevCaps(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextversion : u32, lplinedevcaps : *mut LINEDEVCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetDevCapsA(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextversion : u32, lplinedevcaps : *mut LINEDEVCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetDevCapsW(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextversion : u32, lplinedevcaps : *mut LINEDEVCAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetDevConfig(dwdeviceid : u32, lpdeviceconfig : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetDevConfigA(dwdeviceid : u32, lpdeviceconfig : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetDevConfigW(dwdeviceid : u32, lpdeviceconfig : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetGroupListA(hline : u32, lpgrouplist : *mut LINEAGENTGROUPLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetGroupListW(hline : u32, lpgrouplist : *mut LINEAGENTGROUPLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetID(hline : u32, dwaddressid : u32, hcall : u32, dwselect : u32, lpdeviceid : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetIDA(hline : u32, dwaddressid : u32, hcall : u32, dwselect : u32, lpdeviceid : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetIDW(hline : u32, dwaddressid : u32, hcall : u32, dwselect : u32, lpdeviceid : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn lineGetIcon(dwdeviceid : u32, lpszdeviceclass : ::windows_sys::core::PCSTR, lphicon : *mut super::super::UI::WindowsAndMessaging:: HICON) -> i32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn lineGetIconA(dwdeviceid : u32, lpszdeviceclass : ::windows_sys::core::PCSTR, lphicon : *mut super::super::UI::WindowsAndMessaging:: HICON) -> i32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn lineGetIconW(dwdeviceid : u32, lpszdeviceclass : ::windows_sys::core::PCWSTR, lphicon : *mut super::super::UI::WindowsAndMessaging:: HICON) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetLineDevStatus(hline : u32, lplinedevstatus : *mut LINEDEVSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetLineDevStatusA(hline : u32, lplinedevstatus : *mut LINEDEVSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetLineDevStatusW(hline : u32, lplinedevstatus : *mut LINEDEVSTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetMessage(hlineapp : u32, lpmessage : *mut LINEMESSAGE, dwtimeout : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetNewCalls(hline : u32, dwaddressid : u32, dwselect : u32, lpcalllist : *mut LINECALLLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetNumRings(hline : u32, dwaddressid : u32, lpdwnumrings : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetProviderList(dwapiversion : u32, lpproviderlist : *mut LINEPROVIDERLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetProviderListA(dwapiversion : u32, lpproviderlist : *mut LINEPROVIDERLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetProviderListW(dwapiversion : u32, lpproviderlist : *mut LINEPROVIDERLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetProxyStatus(hlineapp : u32, dwdeviceid : u32, dwappapiversion : u32, lplineproxyreqestlist : *mut LINEPROXYREQUESTLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetQueueInfo(hline : u32, dwqueueid : u32, lplinequeueinfo : *mut LINEQUEUEINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetQueueListA(hline : u32, lpgroupid : *mut ::windows_sys::core::GUID, lpqueuelist : *mut LINEQUEUELIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetQueueListW(hline : u32, lpgroupid : *mut ::windows_sys::core::GUID, lpqueuelist : *mut LINEQUEUELIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetRequest(hlineapp : u32, dwrequestmode : u32, lprequestbuffer : *mut ::core::ffi::c_void) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetRequestA(hlineapp : u32, dwrequestmode : u32, lprequestbuffer : *mut ::core::ffi::c_void) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetRequestW(hlineapp : u32, dwrequestmode : u32, lprequestbuffer : *mut ::core::ffi::c_void) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetStatusMessages(hline : u32, lpdwlinestates : *mut u32, lpdwaddressstates : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetTranslateCaps(hlineapp : u32, dwapiversion : u32, lptranslatecaps : *mut LINETRANSLATECAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetTranslateCapsA(hlineapp : u32, dwapiversion : u32, lptranslatecaps : *mut LINETRANSLATECAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineGetTranslateCapsW(hlineapp : u32, dwapiversion : u32, lptranslatecaps : *mut LINETRANSLATECAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineHandoff(hcall : u32, lpszfilename : ::windows_sys::core::PCSTR, dwmediamode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineHandoffA(hcall : u32, lpszfilename : ::windows_sys::core::PCSTR, dwmediamode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineHandoffW(hcall : u32, lpszfilename : ::windows_sys::core::PCWSTR, dwmediamode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineHold(hcall : u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineInitialize(lphlineapp : *mut u32, hinstance : super::super::Foundation:: HINSTANCE, lpfncallback : LINECALLBACK, lpszappname : ::windows_sys::core::PCSTR, lpdwnumdevs : *mut u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineInitializeExA(lphlineapp : *mut u32, hinstance : super::super::Foundation:: HINSTANCE, lpfncallback : LINECALLBACK, lpszfriendlyappname : ::windows_sys::core::PCSTR, lpdwnumdevs : *mut u32, lpdwapiversion : *mut u32, lplineinitializeexparams : *mut LINEINITIALIZEEXPARAMS) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineInitializeExW(lphlineapp : *mut u32, hinstance : super::super::Foundation:: HINSTANCE, lpfncallback : LINECALLBACK, lpszfriendlyappname : ::windows_sys::core::PCWSTR, lpdwnumdevs : *mut u32, lpdwapiversion : *mut u32, lplineinitializeexparams : *mut LINEINITIALIZEEXPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineMakeCall(hline : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineMakeCallA(hline : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineMakeCallW(hline : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCWSTR, dwcountrycode : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineMonitorDigits(hcall : u32, dwdigitmodes : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineMonitorMedia(hcall : u32, dwmediamodes : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineMonitorTones(hcall : u32, lptonelist : *const LINEMONITORTONE, dwnumentries : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineNegotiateAPIVersion(hlineapp : u32, dwdeviceid : u32, dwapilowversion : u32, dwapihighversion : u32, lpdwapiversion : *mut u32, lpextensionid : *mut LINEEXTENSIONID) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineNegotiateExtVersion(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextlowversion : u32, dwexthighversion : u32, lpdwextversion : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineOpen(hlineapp : u32, dwdeviceid : u32, lphline : *mut u32, dwapiversion : u32, dwextversion : u32, dwcallbackinstance : usize, dwprivileges : u32, dwmediamodes : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineOpenA(hlineapp : u32, dwdeviceid : u32, lphline : *mut u32, dwapiversion : u32, dwextversion : u32, dwcallbackinstance : usize, dwprivileges : u32, dwmediamodes : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineOpenW(hlineapp : u32, dwdeviceid : u32, lphline : *mut u32, dwapiversion : u32, dwextversion : u32, dwcallbackinstance : usize, dwprivileges : u32, dwmediamodes : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn linePark(hcall : u32, dwparkmode : u32, lpszdiraddress : ::windows_sys::core::PCSTR, lpnondiraddress : *mut VARSTRING) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineParkA(hcall : u32, dwparkmode : u32, lpszdiraddress : ::windows_sys::core::PCSTR, lpnondiraddress : *mut VARSTRING) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineParkW(hcall : u32, dwparkmode : u32, lpszdiraddress : ::windows_sys::core::PCWSTR, lpnondiraddress : *mut VARSTRING) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn linePickup(hline : u32, dwaddressid : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCSTR, lpszgroupid : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn linePickupA(hline : u32, dwaddressid : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCSTR, lpszgroupid : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn linePickupW(hline : u32, dwaddressid : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCWSTR, lpszgroupid : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn linePrepareAddToConference(hconfcall : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn linePrepareAddToConferenceA(hconfcall : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn linePrepareAddToConferenceW(hconfcall : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineProxyMessage(hline : u32, hcall : u32, dwmsg : u32, dwparam1 : u32, dwparam2 : u32, dwparam3 : u32) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn lineProxyResponse(hline : u32, lpproxyrequest : *mut LINEPROXYREQUEST, dwresult : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineRedirect(hcall : u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineRedirectA(hcall : u32, lpszdestaddress : ::windows_sys::core::PCSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineRedirectW(hcall : u32, lpszdestaddress : ::windows_sys::core::PCWSTR, dwcountrycode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineRegisterRequestRecipient(hlineapp : u32, dwregistrationinstance : u32, dwrequestmode : u32, benable : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineReleaseUserUserInfo(hcall : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineRemoveFromConference(hcall : u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineRemoveProvider(dwpermanentproviderid : u32, hwndowner : super::super::Foundation:: HWND) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSecureCall(hcall : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSendUserUserInfo(hcall : u32, lpsuseruserinfo : ::windows_sys::core::PCSTR, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAgentActivity(hline : u32, dwaddressid : u32, dwactivityid : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAgentGroup(hline : u32, dwaddressid : u32, lpagentgrouplist : *mut LINEAGENTGROUPLIST) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAgentMeasurementPeriod(hline : u32, hagent : u32, dwmeasurementperiod : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAgentSessionState(hline : u32, hagentsession : u32, dwagentsessionstate : u32, dwnextagentsessionstate : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAgentState(hline : u32, dwaddressid : u32, dwagentstate : u32, dwnextagentstate : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAgentStateEx(hline : u32, hagent : u32, dwagentstate : u32, dwnextagentstate : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAppPriority(lpszappfilename : ::windows_sys::core::PCSTR, dwmediamode : u32, lpextensionid : *mut LINEEXTENSIONID, dwrequestmode : u32, lpszextensionname : ::windows_sys::core::PCSTR, dwpriority : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAppPriorityA(lpszappfilename : ::windows_sys::core::PCSTR, dwmediamode : u32, lpextensionid : *mut LINEEXTENSIONID, dwrequestmode : u32, lpszextensionname : ::windows_sys::core::PCSTR, dwpriority : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAppPriorityW(lpszappfilename : ::windows_sys::core::PCWSTR, dwmediamode : u32, lpextensionid : *mut LINEEXTENSIONID, dwrequestmode : u32, lpszextensionname : ::windows_sys::core::PCWSTR, dwpriority : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetAppSpecific(hcall : u32, dwappspecific : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetCallData(hcall : u32, lpcalldata : *mut ::core::ffi::c_void, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetCallParams(hcall : u32, dwbearermode : u32, dwminrate : u32, dwmaxrate : u32, lpdialparams : *const LINEDIALPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetCallPrivilege(hcall : u32, dwcallprivilege : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetCallQualityOfService(hcall : u32, lpsendingflowspec : *mut ::core::ffi::c_void, dwsendingflowspecsize : u32, lpreceivingflowspec : *mut ::core::ffi::c_void, dwreceivingflowspecsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetCallTreatment(hcall : u32, dwtreatment : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetCurrentLocation(hlineapp : u32, dwlocation : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetDevConfig(dwdeviceid : u32, lpdeviceconfig : *const ::core::ffi::c_void, dwsize : u32, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetDevConfigA(dwdeviceid : u32, lpdeviceconfig : *const ::core::ffi::c_void, dwsize : u32, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetDevConfigW(dwdeviceid : u32, lpdeviceconfig : *const ::core::ffi::c_void, dwsize : u32, lpszdeviceclass : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetLineDevStatus(hline : u32, dwstatustochange : u32, fstatus : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetMediaControl(hline : u32, dwaddressid : u32, hcall : u32, dwselect : u32, lpdigitlist : *const LINEMEDIACONTROLDIGIT, dwdigitnumentries : u32, lpmedialist : *const LINEMEDIACONTROLMEDIA, dwmedianumentries : u32, lptonelist : *const LINEMEDIACONTROLTONE, dwtonenumentries : u32, lpcallstatelist : *const LINEMEDIACONTROLCALLSTATE, dwcallstatenumentries : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetMediaMode(hcall : u32, dwmediamodes : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetNumRings(hline : u32, dwaddressid : u32, dwnumrings : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetQueueMeasurementPeriod(hline : u32, dwqueueid : u32, dwmeasurementperiod : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetStatusMessages(hline : u32, dwlinestates : u32, dwaddressstates : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetTerminal(hline : u32, dwaddressid : u32, hcall : u32, dwselect : u32, dwterminalmodes : u32, dwterminalid : u32, benable : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetTollList(hlineapp : u32, dwdeviceid : u32, lpszaddressin : ::windows_sys::core::PCSTR, dwtolllistoption : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetTollListA(hlineapp : u32, dwdeviceid : u32, lpszaddressin : ::windows_sys::core::PCSTR, dwtolllistoption : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetTollListW(hlineapp : u32, dwdeviceid : u32, lpszaddressinw : ::windows_sys::core::PCWSTR, dwtolllistoption : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetupConference(hcall : u32, hline : u32, lphconfcall : *mut u32, lphconsultcall : *mut u32, dwnumparties : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetupConferenceA(hcall : u32, hline : u32, lphconfcall : *mut u32, lphconsultcall : *mut u32, dwnumparties : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetupConferenceW(hcall : u32, hline : u32, lphconfcall : *mut u32, lphconsultcall : *mut u32, dwnumparties : u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetupTransfer(hcall : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetupTransferA(hcall : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSetupTransferW(hcall : u32, lphconsultcall : *mut u32, lpcallparams : *const LINECALLPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineShutdown(hlineapp : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineSwapHold(hactivecall : u32, hheldcall : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineTranslateAddress(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, lpszaddressin : ::windows_sys::core::PCSTR, dwcard : u32, dwtranslateoptions : u32, lptranslateoutput : *mut LINETRANSLATEOUTPUT) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineTranslateAddressA(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, lpszaddressin : ::windows_sys::core::PCSTR, dwcard : u32, dwtranslateoptions : u32, lptranslateoutput : *mut LINETRANSLATEOUTPUT) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineTranslateAddressW(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, lpszaddressin : ::windows_sys::core::PCWSTR, dwcard : u32, dwtranslateoptions : u32, lptranslateoutput : *mut LINETRANSLATEOUTPUT) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineTranslateDialog(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, hwndowner : super::super::Foundation:: HWND, lpszaddressin : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineTranslateDialogA(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, hwndowner : super::super::Foundation:: HWND, lpszaddressin : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn lineTranslateDialogW(hlineapp : u32, dwdeviceid : u32, dwapiversion : u32, hwndowner : super::super::Foundation:: HWND, lpszaddressin : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineUncompleteCall(hline : u32, dwcompletionid : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineUnhold(hcall : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineUnpark(hline : u32, dwaddressid : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineUnparkA(hline : u32, dwaddressid : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn lineUnparkW(hline : u32, dwaddressid : u32, lphcall : *mut u32, lpszdestaddress : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneClose(hphone : u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn phoneConfigDialog(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn phoneConfigDialogA(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn phoneConfigDialogW(dwdeviceid : u32, hwndowner : super::super::Foundation:: HWND, lpszdeviceclass : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneDevSpecific(hphone : u32, lpparams : *mut ::core::ffi::c_void, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetButtonInfo(hphone : u32, dwbuttonlampid : u32, lpbuttoninfo : *mut PHONEBUTTONINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetButtonInfoA(hphone : u32, dwbuttonlampid : u32, lpbuttoninfo : *mut PHONEBUTTONINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetButtonInfoW(hphone : u32, dwbuttonlampid : u32, lpbuttoninfo : *mut PHONEBUTTONINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetData(hphone : u32, dwdataid : u32, lpdata : *mut ::core::ffi::c_void, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetDevCaps(hphoneapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextversion : u32, lpphonecaps : *mut PHONECAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetDevCapsA(hphoneapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextversion : u32, lpphonecaps : *mut PHONECAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetDevCapsW(hphoneapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextversion : u32, lpphonecaps : *mut PHONECAPS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetDisplay(hphone : u32, lpdisplay : *mut VARSTRING) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetGain(hphone : u32, dwhookswitchdev : u32, lpdwgain : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetHookSwitch(hphone : u32, lpdwhookswitchdevs : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetID(hphone : u32, lpdeviceid : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetIDA(hphone : u32, lpdeviceid : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetIDW(hphone : u32, lpdeviceid : *mut VARSTRING, lpszdeviceclass : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn phoneGetIcon(dwdeviceid : u32, lpszdeviceclass : ::windows_sys::core::PCSTR, lphicon : *mut super::super::UI::WindowsAndMessaging:: HICON) -> i32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn phoneGetIconA(dwdeviceid : u32, lpszdeviceclass : ::windows_sys::core::PCSTR, lphicon : *mut super::super::UI::WindowsAndMessaging:: HICON) -> i32);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn phoneGetIconW(dwdeviceid : u32, lpszdeviceclass : ::windows_sys::core::PCWSTR, lphicon : *mut super::super::UI::WindowsAndMessaging:: HICON) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetLamp(hphone : u32, dwbuttonlampid : u32, lpdwlampmode : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetMessage(hphoneapp : u32, lpmessage : *mut PHONEMESSAGE, dwtimeout : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetRing(hphone : u32, lpdwringmode : *mut u32, lpdwvolume : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetStatus(hphone : u32, lpphonestatus : *mut PHONESTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetStatusA(hphone : u32, lpphonestatus : *mut PHONESTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetStatusMessages(hphone : u32, lpdwphonestates : *mut u32, lpdwbuttonmodes : *mut u32, lpdwbuttonstates : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetStatusW(hphone : u32, lpphonestatus : *mut PHONESTATUS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneGetVolume(hphone : u32, dwhookswitchdev : u32, lpdwvolume : *mut u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn phoneInitialize(lphphoneapp : *mut u32, hinstance : super::super::Foundation:: HINSTANCE, lpfncallback : PHONECALLBACK, lpszappname : ::windows_sys::core::PCSTR, lpdwnumdevs : *mut u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn phoneInitializeExA(lphphoneapp : *mut u32, hinstance : super::super::Foundation:: HINSTANCE, lpfncallback : PHONECALLBACK, lpszfriendlyappname : ::windows_sys::core::PCSTR, lpdwnumdevs : *mut u32, lpdwapiversion : *mut u32, lpphoneinitializeexparams : *mut PHONEINITIALIZEEXPARAMS) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn phoneInitializeExW(lphphoneapp : *mut u32, hinstance : super::super::Foundation:: HINSTANCE, lpfncallback : PHONECALLBACK, lpszfriendlyappname : ::windows_sys::core::PCWSTR, lpdwnumdevs : *mut u32, lpdwapiversion : *mut u32, lpphoneinitializeexparams : *mut PHONEINITIALIZEEXPARAMS) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneNegotiateAPIVersion(hphoneapp : u32, dwdeviceid : u32, dwapilowversion : u32, dwapihighversion : u32, lpdwapiversion : *mut u32, lpextensionid : *mut PHONEEXTENSIONID) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneNegotiateExtVersion(hphoneapp : u32, dwdeviceid : u32, dwapiversion : u32, dwextlowversion : u32, dwexthighversion : u32, lpdwextversion : *mut u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneOpen(hphoneapp : u32, dwdeviceid : u32, lphphone : *mut u32, dwapiversion : u32, dwextversion : u32, dwcallbackinstance : usize, dwprivilege : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetButtonInfo(hphone : u32, dwbuttonlampid : u32, lpbuttoninfo : *const PHONEBUTTONINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetButtonInfoA(hphone : u32, dwbuttonlampid : u32, lpbuttoninfo : *const PHONEBUTTONINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetButtonInfoW(hphone : u32, dwbuttonlampid : u32, lpbuttoninfo : *const PHONEBUTTONINFO) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetData(hphone : u32, dwdataid : u32, lpdata : *const ::core::ffi::c_void, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetDisplay(hphone : u32, dwrow : u32, dwcolumn : u32, lpsdisplay : ::windows_sys::core::PCSTR, dwsize : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetGain(hphone : u32, dwhookswitchdev : u32, dwgain : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetHookSwitch(hphone : u32, dwhookswitchdevs : u32, dwhookswitchmode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetLamp(hphone : u32, dwbuttonlampid : u32, dwlampmode : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetRing(hphone : u32, dwringmode : u32, dwvolume : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetStatusMessages(hphone : u32, dwphonestates : u32, dwbuttonmodes : u32, dwbuttonstates : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneSetVolume(hphone : u32, dwhookswitchdev : u32, dwvolume : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn phoneShutdown(hphoneapp : u32) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn tapiGetLocationInfo(lpszcountrycode : ::windows_sys::core::PSTR, lpszcitycode : ::windows_sys::core::PSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn tapiGetLocationInfoA(lpszcountrycode : ::windows_sys::core::PSTR, lpszcitycode : ::windows_sys::core::PSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn tapiGetLocationInfoW(lpszcountrycodew : ::windows_sys::core::PWSTR, lpszcitycodew : ::windows_sys::core::PWSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn tapiRequestDrop(hwnd : super::super::Foundation:: HWND, wrequestid : super::super::Foundation:: WPARAM) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn tapiRequestMakeCall(lpszdestaddress : ::windows_sys::core::PCSTR, lpszappname : ::windows_sys::core::PCSTR, lpszcalledparty : ::windows_sys::core::PCSTR, lpszcomment : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn tapiRequestMakeCallA(lpszdestaddress : ::windows_sys::core::PCSTR, lpszappname : ::windows_sys::core::PCSTR, lpszcalledparty : ::windows_sys::core::PCSTR, lpszcomment : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("tapi32.dll" "system" fn tapiRequestMakeCallW(lpszdestaddress : ::windows_sys::core::PCWSTR, lpszappname : ::windows_sys::core::PCWSTR, lpszcalledparty : ::windows_sys::core::PCWSTR, lpszcomment : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn tapiRequestMediaCall(hwnd : super::super::Foundation:: HWND, wrequestid : super::super::Foundation:: WPARAM, lpszdeviceclass : ::windows_sys::core::PCSTR, lpdeviceid : ::windows_sys::core::PCSTR, dwsize : u32, dwsecure : u32, lpszdestaddress : ::windows_sys::core::PCSTR, lpszappname : ::windows_sys::core::PCSTR, lpszcalledparty : ::windows_sys::core::PCSTR, lpszcomment : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn tapiRequestMediaCallA(hwnd : super::super::Foundation:: HWND, wrequestid : super::super::Foundation:: WPARAM, lpszdeviceclass : ::windows_sys::core::PCSTR, lpdeviceid : ::windows_sys::core::PCSTR, dwsize : u32, dwsecure : u32, lpszdestaddress : ::windows_sys::core::PCSTR, lpszappname : ::windows_sys::core::PCSTR, lpszcalledparty : ::windows_sys::core::PCSTR, lpszcomment : ::windows_sys::core::PCSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("tapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn tapiRequestMediaCallW(hwnd : super::super::Foundation:: HWND, wrequestid : super::super::Foundation:: WPARAM, lpszdeviceclass : ::windows_sys::core::PCWSTR, lpdeviceid : ::windows_sys::core::PCWSTR, dwsize : u32, dwsecure : u32, lpszdestaddress : ::windows_sys::core::PCWSTR, lpszappname : ::windows_sys::core::PCWSTR, lpszcalledparty : ::windows_sys::core::PCWSTR, lpszcomment : ::windows_sys::core::PCWSTR) -> i32);
pub type IEnumACDGroup = *mut ::core::ffi::c_void;
pub type IEnumAddress = *mut ::core::ffi::c_void;
pub type IEnumAgent = *mut ::core::ffi::c_void;
pub type IEnumAgentHandler = *mut ::core::ffi::c_void;
pub type IEnumAgentSession = *mut ::core::ffi::c_void;
pub type IEnumBstr = *mut ::core::ffi::c_void;
pub type IEnumCall = *mut ::core::ffi::c_void;
pub type IEnumCallHub = *mut ::core::ffi::c_void;
pub type IEnumCallingCard = *mut ::core::ffi::c_void;
pub type IEnumDialableAddrs = *mut ::core::ffi::c_void;
pub type IEnumDirectory = *mut ::core::ffi::c_void;
pub type IEnumDirectoryObject = *mut ::core::ffi::c_void;
pub type IEnumLocation = *mut ::core::ffi::c_void;
pub type IEnumMcastScope = *mut ::core::ffi::c_void;
pub type IEnumPhone = *mut ::core::ffi::c_void;
pub type IEnumPluggableSuperclassInfo = *mut ::core::ffi::c_void;
pub type IEnumPluggableTerminalClassInfo = *mut ::core::ffi::c_void;
pub type IEnumQueue = *mut ::core::ffi::c_void;
pub type IEnumStream = *mut ::core::ffi::c_void;
pub type IEnumSubStream = *mut ::core::ffi::c_void;
pub type IEnumTerminal = *mut ::core::ffi::c_void;
pub type IEnumTerminalClass = *mut ::core::ffi::c_void;
pub type IMcastAddressAllocation = *mut ::core::ffi::c_void;
pub type IMcastLeaseInfo = *mut ::core::ffi::c_void;
pub type IMcastScope = *mut ::core::ffi::c_void;
pub type ITACDGroup = *mut ::core::ffi::c_void;
pub type ITACDGroupEvent = *mut ::core::ffi::c_void;
pub type ITAMMediaFormat = *mut ::core::ffi::c_void;
pub type ITASRTerminalEvent = *mut ::core::ffi::c_void;
pub type ITAddress = *mut ::core::ffi::c_void;
pub type ITAddress2 = *mut ::core::ffi::c_void;
pub type ITAddressCapabilities = *mut ::core::ffi::c_void;
pub type ITAddressDeviceSpecificEvent = *mut ::core::ffi::c_void;
pub type ITAddressEvent = *mut ::core::ffi::c_void;
pub type ITAddressTranslation = *mut ::core::ffi::c_void;
pub type ITAddressTranslationInfo = *mut ::core::ffi::c_void;
pub type ITAgent = *mut ::core::ffi::c_void;
pub type ITAgentEvent = *mut ::core::ffi::c_void;
pub type ITAgentHandler = *mut ::core::ffi::c_void;
pub type ITAgentHandlerEvent = *mut ::core::ffi::c_void;
pub type ITAgentSession = *mut ::core::ffi::c_void;
pub type ITAgentSessionEvent = *mut ::core::ffi::c_void;
pub type ITAllocatorProperties = *mut ::core::ffi::c_void;
pub type ITAutomatedPhoneControl = *mut ::core::ffi::c_void;
pub type ITBasicAudioTerminal = *mut ::core::ffi::c_void;
pub type ITBasicCallControl = *mut ::core::ffi::c_void;
pub type ITBasicCallControl2 = *mut ::core::ffi::c_void;
pub type ITCallHub = *mut ::core::ffi::c_void;
pub type ITCallHubEvent = *mut ::core::ffi::c_void;
pub type ITCallInfo = *mut ::core::ffi::c_void;
pub type ITCallInfo2 = *mut ::core::ffi::c_void;
pub type ITCallInfoChangeEvent = *mut ::core::ffi::c_void;
pub type ITCallMediaEvent = *mut ::core::ffi::c_void;
pub type ITCallNotificationEvent = *mut ::core::ffi::c_void;
pub type ITCallStateEvent = *mut ::core::ffi::c_void;
pub type ITCallingCard = *mut ::core::ffi::c_void;
pub type ITCollection = *mut ::core::ffi::c_void;
pub type ITCollection2 = *mut ::core::ffi::c_void;
pub type ITCustomTone = *mut ::core::ffi::c_void;
pub type ITDetectTone = *mut ::core::ffi::c_void;
pub type ITDigitDetectionEvent = *mut ::core::ffi::c_void;
pub type ITDigitGenerationEvent = *mut ::core::ffi::c_void;
pub type ITDigitsGatheredEvent = *mut ::core::ffi::c_void;
pub type ITDirectory = *mut ::core::ffi::c_void;
pub type ITDirectoryObject = *mut ::core::ffi::c_void;
pub type ITDirectoryObjectConference = *mut ::core::ffi::c_void;
pub type ITDirectoryObjectUser = *mut ::core::ffi::c_void;
pub type ITDispatchMapper = *mut ::core::ffi::c_void;
pub type ITFileTerminalEvent = *mut ::core::ffi::c_void;
pub type ITFileTrack = *mut ::core::ffi::c_void;
pub type ITForwardInformation = *mut ::core::ffi::c_void;
pub type ITForwardInformation2 = *mut ::core::ffi::c_void;
pub type ITILSConfig = *mut ::core::ffi::c_void;
pub type ITLegacyAddressMediaControl = *mut ::core::ffi::c_void;
pub type ITLegacyAddressMediaControl2 = *mut ::core::ffi::c_void;
pub type ITLegacyCallMediaControl = *mut ::core::ffi::c_void;
pub type ITLegacyCallMediaControl2 = *mut ::core::ffi::c_void;
pub type ITLegacyWaveSupport = *mut ::core::ffi::c_void;
pub type ITLocationInfo = *mut ::core::ffi::c_void;
pub type ITMSPAddress = *mut ::core::ffi::c_void;
pub type ITMediaControl = *mut ::core::ffi::c_void;
pub type ITMediaPlayback = *mut ::core::ffi::c_void;
pub type ITMediaRecord = *mut ::core::ffi::c_void;
pub type ITMediaSupport = *mut ::core::ffi::c_void;
pub type ITMultiTrackTerminal = *mut ::core::ffi::c_void;
pub type ITPhone = *mut ::core::ffi::c_void;
pub type ITPhoneDeviceSpecificEvent = *mut ::core::ffi::c_void;
pub type ITPhoneEvent = *mut ::core::ffi::c_void;
pub type ITPluggableTerminalClassInfo = *mut ::core::ffi::c_void;
pub type ITPluggableTerminalEventSink = *mut ::core::ffi::c_void;
pub type ITPluggableTerminalEventSinkRegistration = *mut ::core::ffi::c_void;
pub type ITPluggableTerminalSuperclassInfo = *mut ::core::ffi::c_void;
pub type ITPrivateEvent = *mut ::core::ffi::c_void;
pub type ITQOSEvent = *mut ::core::ffi::c_void;
pub type ITQueue = *mut ::core::ffi::c_void;
pub type ITQueueEvent = *mut ::core::ffi::c_void;
pub type ITRendezvous = *mut ::core::ffi::c_void;
pub type ITRequest = *mut ::core::ffi::c_void;
pub type ITRequestEvent = *mut ::core::ffi::c_void;
pub type ITScriptableAudioFormat = *mut ::core::ffi::c_void;
pub type ITStaticAudioTerminal = *mut ::core::ffi::c_void;
pub type ITStream = *mut ::core::ffi::c_void;
pub type ITStreamControl = *mut ::core::ffi::c_void;
pub type ITSubStream = *mut ::core::ffi::c_void;
pub type ITSubStreamControl = *mut ::core::ffi::c_void;
pub type ITTAPI = *mut ::core::ffi::c_void;
pub type ITTAPI2 = *mut ::core::ffi::c_void;
pub type ITTAPICallCenter = *mut ::core::ffi::c_void;
pub type ITTAPIDispatchEventNotification = *mut ::core::ffi::c_void;
pub type ITTAPIEventNotification = *mut ::core::ffi::c_void;
pub type ITTAPIObjectEvent = *mut ::core::ffi::c_void;
pub type ITTAPIObjectEvent2 = *mut ::core::ffi::c_void;
pub type ITTTSTerminalEvent = *mut ::core::ffi::c_void;
pub type ITTerminal = *mut ::core::ffi::c_void;
pub type ITTerminalSupport = *mut ::core::ffi::c_void;
pub type ITTerminalSupport2 = *mut ::core::ffi::c_void;
pub type ITToneDetectionEvent = *mut ::core::ffi::c_void;
pub type ITToneTerminalEvent = *mut ::core::ffi::c_void;
pub type ITnef = *mut ::core::ffi::c_void;
pub const ACDGE_GROUP_REMOVED: ACDGROUP_EVENT = 1i32;
pub const ACDGE_NEW_GROUP: ACDGROUP_EVENT = 0i32;
pub const ACDQE_NEW_QUEUE: ACDQUEUE_EVENT = 0i32;
pub const ACDQE_QUEUE_REMOVED: ACDQUEUE_EVENT = 1i32;
pub const ACS_ADDRESSDEVICESPECIFIC: ADDRESS_CAPABILITY_STRING = 1i32;
pub const ACS_LINEDEVICESPECIFIC: ADDRESS_CAPABILITY_STRING = 2i32;
pub const ACS_PERMANENTDEVICEGUID: ADDRESS_CAPABILITY_STRING = 5i32;
pub const ACS_PROTOCOL: ADDRESS_CAPABILITY_STRING = 0i32;
pub const ACS_PROVIDERSPECIFIC: ADDRESS_CAPABILITY_STRING = 3i32;
pub const ACS_SWITCHSPECIFIC: ADDRESS_CAPABILITY_STRING = 4i32;
pub const AC_ADDRESSCAPFLAGS: ADDRESS_CAPABILITY = 23i32;
pub const AC_ADDRESSFEATURES: ADDRESS_CAPABILITY = 29i32;
pub const AC_ADDRESSID: ADDRESS_CAPABILITY = 33i32;
pub const AC_ADDRESSTYPES: ADDRESS_CAPABILITY = 0i32;
pub const AC_ANSWERMODES: ADDRESS_CAPABILITY = 14i32;
pub const AC_BEARERMODES: ADDRESS_CAPABILITY = 1i32;
pub const AC_CALLCOMPLETIONCONDITIONS: ADDRESS_CAPABILITY = 40i32;
pub const AC_CALLCOMPLETIONMODES: ADDRESS_CAPABILITY = 41i32;
pub const AC_CALLEDIDSUPPORT: ADDRESS_CAPABILITY = 19i32;
pub const AC_CALLERIDSUPPORT: ADDRESS_CAPABILITY = 18i32;
pub const AC_CALLFEATURES1: ADDRESS_CAPABILITY = 24i32;
pub const AC_CALLFEATURES2: ADDRESS_CAPABILITY = 25i32;
pub const AC_CONNECTEDIDSUPPORT: ADDRESS_CAPABILITY = 20i32;
pub const AC_DEVCAPFLAGS: ADDRESS_CAPABILITY = 13i32;
pub const AC_FORWARDMODES: ADDRESS_CAPABILITY = 34i32;
pub const AC_GATHERDIGITSMAXTIMEOUT: ADDRESS_CAPABILITY = 44i32;
pub const AC_GATHERDIGITSMINTIMEOUT: ADDRESS_CAPABILITY = 43i32;
pub const AC_GENERATEDIGITDEFAULTDURATION: ADDRESS_CAPABILITY = 47i32;
pub const AC_GENERATEDIGITMAXDURATION: ADDRESS_CAPABILITY = 46i32;
pub const AC_GENERATEDIGITMINDURATION: ADDRESS_CAPABILITY = 45i32;
pub const AC_GENERATEDIGITSUPPORT: ADDRESS_CAPABILITY = 8i32;
pub const AC_GENERATETONEMAXNUMFREQ: ADDRESS_CAPABILITY = 10i32;
pub const AC_GENERATETONEMODES: ADDRESS_CAPABILITY = 9i32;
pub const AC_LINEFEATURES: ADDRESS_CAPABILITY = 15i32;
pub const AC_LINEID: ADDRESS_CAPABILITY = 32i32;
pub const AC_MAXACTIVECALLS: ADDRESS_CAPABILITY = 2i32;
pub const AC_MAXCALLCOMPLETIONS: ADDRESS_CAPABILITY = 39i32;
pub const AC_MAXCALLDATASIZE: ADDRESS_CAPABILITY = 31i32;
pub const AC_MAXFORWARDENTRIES: ADDRESS_CAPABILITY = 35i32;
pub const AC_MAXFWDNUMRINGS: ADDRESS_CAPABILITY = 38i32;
pub const AC_MAXNUMCONFERENCE: ADDRESS_CAPABILITY = 5i32;
pub const AC_MAXNUMTRANSCONF: ADDRESS_CAPABILITY = 6i32;
pub const AC_MAXONHOLDCALLS: ADDRESS_CAPABILITY = 3i32;
pub const AC_MAXONHOLDPENDINGCALLS: ADDRESS_CAPABILITY = 4i32;
pub const AC_MAXSPECIFICENTRIES: ADDRESS_CAPABILITY = 36i32;
pub const AC_MINFWDNUMRINGS: ADDRESS_CAPABILITY = 37i32;
pub const AC_MONITORDIGITSUPPORT: ADDRESS_CAPABILITY = 7i32;
pub const AC_MONITORTONEMAXNUMENTRIES: ADDRESS_CAPABILITY = 12i32;
pub const AC_MONITORTONEMAXNUMFREQ: ADDRESS_CAPABILITY = 11i32;
pub const AC_PARKSUPPORT: ADDRESS_CAPABILITY = 17i32;
pub const AC_PERMANENTDEVICEID: ADDRESS_CAPABILITY = 42i32;
pub const AC_PREDICTIVEAUTOTRANSFERSTATES: ADDRESS_CAPABILITY = 30i32;
pub const AC_REDIRECTINGIDSUPPORT: ADDRESS_CAPABILITY = 22i32;
pub const AC_REDIRECTIONIDSUPPORT: ADDRESS_CAPABILITY = 21i32;
pub const AC_REMOVEFROMCONFCAPS: ADDRESS_CAPABILITY = 26i32;
pub const AC_REMOVEFROMCONFSTATE: ADDRESS_CAPABILITY = 27i32;
pub const AC_SETTABLEDEVSTATUS: ADDRESS_CAPABILITY = 16i32;
pub const AC_TRANSFERMODES: ADDRESS_CAPABILITY = 28i32;
pub const ADDRESS_TERMINAL_AVAILABLE: MSP_ADDRESS_EVENT = 0i32;
pub const ADDRESS_TERMINAL_UNAVAILABLE: MSP_ADDRESS_EVENT = 1i32;
pub const AE_BUSY_ACD: AGENT_EVENT = 2i32;
pub const AE_BUSY_INCOMING: AGENT_EVENT = 3i32;
pub const AE_BUSY_OUTGOING: AGENT_EVENT = 4i32;
pub const AE_CAPSCHANGE: ADDRESS_EVENT = 1i32;
pub const AE_CONFIGCHANGE: ADDRESS_EVENT = 3i32;
pub const AE_FORWARD: ADDRESS_EVENT = 4i32;
pub const AE_LASTITEM: ADDRESS_EVENT = 8i32;
pub const AE_MSGWAITOFF: ADDRESS_EVENT = 8i32;
pub const AE_MSGWAITON: ADDRESS_EVENT = 7i32;
pub const AE_NEWTERMINAL: ADDRESS_EVENT = 5i32;
pub const AE_NOT_READY: AGENT_EVENT = 0i32;
pub const AE_READY: AGENT_EVENT = 1i32;
pub const AE_REMOVETERMINAL: ADDRESS_EVENT = 6i32;
pub const AE_RINGING: ADDRESS_EVENT = 2i32;
pub const AE_STATE: ADDRESS_EVENT = 0i32;
pub const AE_UNKNOWN: AGENT_EVENT = 5i32;
pub const AHE_AGENTHANDLER_REMOVED: AGENTHANDLER_EVENT = 1i32;
pub const AHE_NEW_AGENTHANDLER: AGENTHANDLER_EVENT = 0i32;
pub const ASE_BUSY: AGENT_SESSION_EVENT = 3i32;
pub const ASE_END: AGENT_SESSION_EVENT = 5i32;
pub const ASE_NEW_SESSION: AGENT_SESSION_EVENT = 0i32;
pub const ASE_NOT_READY: AGENT_SESSION_EVENT = 1i32;
pub const ASE_READY: AGENT_SESSION_EVENT = 2i32;
pub const ASE_WRAPUP: AGENT_SESSION_EVENT = 4i32;
pub const ASST_BUSY_ON_CALL: AGENT_SESSION_STATE = 2i32;
pub const ASST_BUSY_WRAPUP: AGENT_SESSION_STATE = 3i32;
pub const ASST_NOT_READY: AGENT_SESSION_STATE = 0i32;
pub const ASST_READY: AGENT_SESSION_STATE = 1i32;
pub const ASST_SESSION_ENDED: AGENT_SESSION_STATE = 4i32;
pub const AS_BUSY_ACD: AGENT_STATE = 2i32;
pub const AS_BUSY_INCOMING: AGENT_STATE = 3i32;
pub const AS_BUSY_OUTGOING: AGENT_STATE = 4i32;
pub const AS_INSERVICE: ADDRESS_STATE = 0i32;
pub const AS_NOT_READY: AGENT_STATE = 0i32;
pub const AS_OUTOFSERVICE: ADDRESS_STATE = 1i32;
pub const AS_READY: AGENT_STATE = 1i32;
pub const AS_UNKNOWN: AGENT_STATE = 5i32;
pub const CALL_CAUSE_BAD_DEVICE: MSP_CALL_EVENT_CAUSE = 1i32;
pub const CALL_CAUSE_CONNECT_FAIL: MSP_CALL_EVENT_CAUSE = 2i32;
pub const CALL_CAUSE_LOCAL_REQUEST: MSP_CALL_EVENT_CAUSE = 3i32;
pub const CALL_CAUSE_MEDIA_RECOVERED: MSP_CALL_EVENT_CAUSE = 6i32;
pub const CALL_CAUSE_MEDIA_TIMEOUT: MSP_CALL_EVENT_CAUSE = 5i32;
pub const CALL_CAUSE_QUALITY_OF_SERVICE: MSP_CALL_EVENT_CAUSE = 7i32;
pub const CALL_CAUSE_REMOTE_REQUEST: MSP_CALL_EVENT_CAUSE = 4i32;
pub const CALL_CAUSE_UNKNOWN: MSP_CALL_EVENT_CAUSE = 0i32;
pub const CALL_NEW_STREAM: MSP_CALL_EVENT = 0i32;
pub const CALL_STREAM_ACTIVE: MSP_CALL_EVENT = 4i32;
pub const CALL_STREAM_FAIL: MSP_CALL_EVENT = 1i32;
pub const CALL_STREAM_INACTIVE: MSP_CALL_EVENT = 5i32;
pub const CALL_STREAM_NOT_USED: MSP_CALL_EVENT = 3i32;
pub const CALL_TERMINAL_FAIL: MSP_CALL_EVENT = 2i32;
pub const CEC_DISCONNECT_BADADDRESS: CALL_STATE_EVENT_CAUSE = 3i32;
pub const CEC_DISCONNECT_BLOCKED: CALL_STATE_EVENT_CAUSE = 8i32;
pub const CEC_DISCONNECT_BUSY: CALL_STATE_EVENT_CAUSE = 2i32;
pub const CEC_DISCONNECT_CANCELLED: CALL_STATE_EVENT_CAUSE = 5i32;
pub const CEC_DISCONNECT_FAILED: CALL_STATE_EVENT_CAUSE = 7i32;
pub const CEC_DISCONNECT_NOANSWER: CALL_STATE_EVENT_CAUSE = 4i32;
pub const CEC_DISCONNECT_NORMAL: CALL_STATE_EVENT_CAUSE = 1i32;
pub const CEC_DISCONNECT_REJECTED: CALL_STATE_EVENT_CAUSE = 6i32;
pub const CEC_NONE: CALL_STATE_EVENT_CAUSE = 0i32;
pub const CHE_CALLHUBIDLE: CALLHUB_EVENT = 3i32;
pub const CHE_CALLHUBNEW: CALLHUB_EVENT = 2i32;
pub const CHE_CALLJOIN: CALLHUB_EVENT = 0i32;
pub const CHE_CALLLEAVE: CALLHUB_EVENT = 1i32;
pub const CHE_LASTITEM: CALLHUB_EVENT = 3i32;
pub const CHS_ACTIVE: CALLHUB_STATE = 0i32;
pub const CHS_IDLE: CALLHUB_STATE = 1i32;
pub const CIB_CALLDATABUFFER: CALLINFO_BUFFER = 2i32;
pub const CIB_CHARGINGINFOBUFFER: CALLINFO_BUFFER = 3i32;
pub const CIB_DEVSPECIFICBUFFER: CALLINFO_BUFFER = 1i32;
pub const CIB_HIGHLEVELCOMPATIBILITYBUFFER: CALLINFO_BUFFER = 4i32;
pub const CIB_LOWLEVELCOMPATIBILITYBUFFER: CALLINFO_BUFFER = 5i32;
pub const CIB_USERUSERINFO: CALLINFO_BUFFER = 0i32;
pub const CIC_APPSPECIFIC: CALLINFOCHANGE_CAUSE = 4i32;
pub const CIC_BEARERMODE: CALLINFOCHANGE_CAUSE = 2i32;
pub const CIC_CALLDATA: CALLINFOCHANGE_CAUSE = 24i32;
pub const CIC_CALLEDID: CALLINFOCHANGE_CAUSE = 15i32;
pub const CIC_CALLERID: CALLINFOCHANGE_CAUSE = 14i32;
pub const CIC_CALLID: CALLINFOCHANGE_CAUSE = 5i32;
pub const CIC_CHARGINGINFO: CALLINFOCHANGE_CAUSE = 22i32;
pub const CIC_COMPLETIONID: CALLINFOCHANGE_CAUSE = 9i32;
pub const CIC_CONNECTEDID: CALLINFOCHANGE_CAUSE = 16i32;
pub const CIC_DEVSPECIFIC: CALLINFOCHANGE_CAUSE = 1i32;
pub const CIC_HIGHLEVELCOMP: CALLINFOCHANGE_CAUSE = 20i32;
pub const CIC_LASTITEM: CALLINFOCHANGE_CAUSE = 26i32;
pub const CIC_LOWLEVELCOMP: CALLINFOCHANGE_CAUSE = 21i32;
pub const CIC_MEDIATYPE: CALLINFOCHANGE_CAUSE = 26i32;
pub const CIC_NUMMONITORS: CALLINFOCHANGE_CAUSE = 12i32;
pub const CIC_NUMOWNERDECR: CALLINFOCHANGE_CAUSE = 11i32;
pub const CIC_NUMOWNERINCR: CALLINFOCHANGE_CAUSE = 10i32;
pub const CIC_ORIGIN: CALLINFOCHANGE_CAUSE = 7i32;
pub const CIC_OTHER: CALLINFOCHANGE_CAUSE = 0i32;
pub const CIC_PRIVILEGE: CALLINFOCHANGE_CAUSE = 25i32;
pub const CIC_RATE: CALLINFOCHANGE_CAUSE = 3i32;
pub const CIC_REASON: CALLINFOCHANGE_CAUSE = 8i32;
pub const CIC_REDIRECTINGID: CALLINFOCHANGE_CAUSE = 18i32;
pub const CIC_REDIRECTIONID: CALLINFOCHANGE_CAUSE = 17i32;
pub const CIC_RELATEDCALLID: CALLINFOCHANGE_CAUSE = 6i32;
pub const CIC_TREATMENT: CALLINFOCHANGE_CAUSE = 23i32;
pub const CIC_TRUNK: CALLINFOCHANGE_CAUSE = 13i32;
pub const CIC_USERUSERINFO: CALLINFOCHANGE_CAUSE = 19i32;
pub const CIL_APPSPECIFIC: CALLINFO_LONG = 9i32;
pub const CIL_BEARERMODE: CALLINFO_LONG = 1i32;
pub const CIL_CALLEDIDADDRESSTYPE: CALLINFO_LONG = 3i32;
pub const CIL_CALLERIDADDRESSTYPE: CALLINFO_LONG = 2i32;
pub const CIL_CALLID: CALLINFO_LONG = 15i32;
pub const CIL_CALLPARAMSFLAGS: CALLINFO_LONG = 10i32;
pub const CIL_CALLTREATMENT: CALLINFO_LONG = 11i32;
pub const CIL_COMPLETIONID: CALLINFO_LONG = 17i32;
pub const CIL_CONNECTEDIDADDRESSTYPE: CALLINFO_LONG = 4i32;
pub const CIL_COUNTRYCODE: CALLINFO_LONG = 14i32;
pub const CIL_GENERATEDIGITDURATION: CALLINFO_LONG = 22i32;
pub const CIL_MAXRATE: CALLINFO_LONG = 13i32;
pub const CIL_MEDIATYPESAVAILABLE: CALLINFO_LONG = 0i32;
pub const CIL_MINRATE: CALLINFO_LONG = 12i32;
pub const CIL_MONITORDIGITMODES: CALLINFO_LONG = 23i32;
pub const CIL_MONITORMEDIAMODES: CALLINFO_LONG = 24i32;
pub const CIL_NUMBEROFMONITORS: CALLINFO_LONG = 19i32;
pub const CIL_NUMBEROFOWNERS: CALLINFO_LONG = 18i32;
pub const CIL_ORIGIN: CALLINFO_LONG = 7i32;
pub const CIL_RATE: CALLINFO_LONG = 21i32;
pub const CIL_REASON: CALLINFO_LONG = 8i32;
pub const CIL_REDIRECTINGIDADDRESSTYPE: CALLINFO_LONG = 6i32;
pub const CIL_REDIRECTIONIDADDRESSTYPE: CALLINFO_LONG = 5i32;
pub const CIL_RELATEDCALLID: CALLINFO_LONG = 16i32;
pub const CIL_TRUNK: CALLINFO_LONG = 20i32;
pub const CIS_CALLEDIDNAME: CALLINFO_STRING = 2i32;
pub const CIS_CALLEDIDNUMBER: CALLINFO_STRING = 3i32;
pub const CIS_CALLEDPARTYFRIENDLYNAME: CALLINFO_STRING = 10i32;
pub const CIS_CALLERIDNAME: CALLINFO_STRING = 0i32;
pub const CIS_CALLERIDNUMBER: CALLINFO_STRING = 1i32;
pub const CIS_CALLINGPARTYID: CALLINFO_STRING = 13i32;
pub const CIS_COMMENT: CALLINFO_STRING = 11i32;
pub const CIS_CONNECTEDIDNAME: CALLINFO_STRING = 4i32;
pub const CIS_CONNECTEDIDNUMBER: CALLINFO_STRING = 5i32;
pub const CIS_DISPLAYABLEADDRESS: CALLINFO_STRING = 12i32;
pub const CIS_REDIRECTINGIDNAME: CALLINFO_STRING = 8i32;
pub const CIS_REDIRECTINGIDNUMBER: CALLINFO_STRING = 9i32;
pub const CIS_REDIRECTIONIDNAME: CALLINFO_STRING = 6i32;
pub const CIS_REDIRECTIONIDNUMBER: CALLINFO_STRING = 7i32;
pub const CMC_BAD_DEVICE: CALL_MEDIA_EVENT_CAUSE = 1i32;
pub const CMC_CONNECT_FAIL: CALL_MEDIA_EVENT_CAUSE = 2i32;
pub const CMC_LOCAL_REQUEST: CALL_MEDIA_EVENT_CAUSE = 3i32;
pub const CMC_MEDIA_RECOVERED: CALL_MEDIA_EVENT_CAUSE = 6i32;
pub const CMC_MEDIA_TIMEOUT: CALL_MEDIA_EVENT_CAUSE = 5i32;
pub const CMC_QUALITY_OF_SERVICE: CALL_MEDIA_EVENT_CAUSE = 7i32;
pub const CMC_REMOTE_REQUEST: CALL_MEDIA_EVENT_CAUSE = 4i32;
pub const CMC_UNKNOWN: CALL_MEDIA_EVENT_CAUSE = 0i32;
pub const CME_LASTITEM: CALL_MEDIA_EVENT = 5i32;
pub const CME_NEW_STREAM: CALL_MEDIA_EVENT = 0i32;
pub const CME_STREAM_ACTIVE: CALL_MEDIA_EVENT = 4i32;
pub const CME_STREAM_FAIL: CALL_MEDIA_EVENT = 1i32;
pub const CME_STREAM_INACTIVE: CALL_MEDIA_EVENT = 5i32;
pub const CME_STREAM_NOT_USED: CALL_MEDIA_EVENT = 3i32;
pub const CME_TERMINAL_FAIL: CALL_MEDIA_EVENT = 2i32;
pub const CNE_LASTITEM: CALL_NOTIFICATION_EVENT = 1i32;
pub const CNE_MONITOR: CALL_NOTIFICATION_EVENT = 1i32;
pub const CNE_OWNER: CALL_NOTIFICATION_EVENT = 0i32;
pub const CP_MONITOR: CALL_PRIVILEGE = 1i32;
pub const CP_OWNER: CALL_PRIVILEGE = 0i32;
pub const CS_CONNECTED: CALL_STATE = 2i32;
pub const CS_DISCONNECTED: CALL_STATE = 3i32;
pub const CS_HOLD: CALL_STATE = 5i32;
pub const CS_IDLE: CALL_STATE = 0i32;
pub const CS_INPROGRESS: CALL_STATE = 1i32;
pub const CS_LASTITEM: CALL_STATE = 6i32;
pub const CS_OFFERING: CALL_STATE = 4i32;
pub const CS_QUEUED: CALL_STATE = 6i32;
pub const DC_NOANSWER: DISCONNECT_CODE = 1i32;
pub const DC_NORMAL: DISCONNECT_CODE = 0i32;
pub const DC_REJECTED: DISCONNECT_CODE = 2i32;
pub const DISPIDMASK: u32 = 65535u32;
pub const DT_ILS: DIRECTORY_TYPE = 2i32;
pub const DT_NTDS: DIRECTORY_TYPE = 1i32;
pub const DispatchMapper: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe9225296_c759_11d1_a02b_00c04fb6809f);
pub const FDS_NOTSUPPORTED: FULLDUPLEX_SUPPORT = 1i32;
pub const FDS_SUPPORTED: FULLDUPLEX_SUPPORT = 0i32;
pub const FDS_UNKNOWN: FULLDUPLEX_SUPPORT = 2i32;
pub const FM_ASCONFERENCE: FINISH_MODE = 1i32;
pub const FM_ASTRANSFER: FINISH_MODE = 0i32;
pub const FTEC_END_OF_FILE: FT_STATE_EVENT_CAUSE = 1i32;
pub const FTEC_NORMAL: FT_STATE_EVENT_CAUSE = 0i32;
pub const FTEC_READ_ERROR: FT_STATE_EVENT_CAUSE = 2i32;
pub const FTEC_WRITE_ERROR: FT_STATE_EVENT_CAUSE = 3i32;
pub const GETTNEFSTREAMCODEPAGE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("GetTnefStreamCodePage");
pub const IDISPADDRESS: u32 = 65536u32;
pub const IDISPADDRESSCAPABILITIES: u32 = 131072u32;
pub const IDISPADDRESSTRANSLATION: u32 = 262144u32;
pub const IDISPAGGREGATEDMSPADDRESSOBJ: u32 = 393216u32;
pub const IDISPAGGREGATEDMSPCALLOBJ: u32 = 262144u32;
pub const IDISPAPC: u32 = 131072u32;
pub const IDISPBASICCALLCONTROL: u32 = 131072u32;
pub const IDISPCALLINFO: u32 = 65536u32;
pub const IDISPDIRECTORY: u32 = 65536u32;
pub const IDISPDIROBJCONFERENCE: u32 = 131072u32;
pub const IDISPDIROBJECT: u32 = 65536u32;
pub const IDISPDIROBJUSER: u32 = 196608u32;
pub const IDISPFILETRACK: u32 = 65536u32;
pub const IDISPILSCONFIG: u32 = 131072u32;
pub const IDISPLEGACYADDRESSMEDIACONTROL: u32 = 327680u32;
pub const IDISPLEGACYCALLMEDIACONTROL: u32 = 196608u32;
pub const IDISPMEDIACONTROL: u32 = 131072u32;
pub const IDISPMEDIAPLAYBACK: u32 = 262144u32;
pub const IDISPMEDIARECORD: u32 = 196608u32;
pub const IDISPMEDIASUPPORT: u32 = 196608u32;
pub const IDISPMULTITRACK: u32 = 65536u32;
pub const IDISPPHONE: u32 = 65536u32;
pub const IDISPTAPI: u32 = 65536u32;
pub const IDISPTAPICALLCENTER: u32 = 131072u32;
pub const INITIALIZE_NEGOTIATION: u32 = 4294967295u32;
pub const INTERFACEMASK: u32 = 16711680u32;
pub const LAST_LINEMEDIAMODE: u32 = 32768u32;
pub const LAST_LINEREQUESTMODE: u32 = 2u32;
pub const LINEADDRCAPFLAGS_ACCEPTTOALERT: u32 = 1048576u32;
pub const LINEADDRCAPFLAGS_ACDGROUP: u32 = 1073741824u32;
pub const LINEADDRCAPFLAGS_AUTORECONNECT: u32 = 1024u32;
pub const LINEADDRCAPFLAGS_BLOCKIDDEFAULT: u32 = 8u32;
pub const LINEADDRCAPFLAGS_BLOCKIDOVERRIDE: u32 = 16u32;
pub const LINEADDRCAPFLAGS_COMPLETIONID: u32 = 2048u32;
pub const LINEADDRCAPFLAGS_CONFDROP: u32 = 2097152u32;
pub const LINEADDRCAPFLAGS_CONFERENCEHELD: u32 = 16384u32;
pub const LINEADDRCAPFLAGS_CONFERENCEMAKE: u32 = 32768u32;
pub const LINEADDRCAPFLAGS_DESTOFFHOOK: u32 = 128u32;
pub const LINEADDRCAPFLAGS_DIALED: u32 = 32u32;
pub const LINEADDRCAPFLAGS_FWDBUSYNAADDR: u32 = 524288u32;
pub const LINEADDRCAPFLAGS_FWDCONSULT: u32 = 256u32;
pub const LINEADDRCAPFLAGS_FWDINTEXTADDR: u32 = 262144u32;
pub const LINEADDRCAPFLAGS_FWDNUMRINGS: u32 = 1u32;
pub const LINEADDRCAPFLAGS_FWDSTATUSVALID: u32 = 131072u32;
pub const LINEADDRCAPFLAGS_HOLDMAKESNEW: u32 = 67108864u32;
pub const LINEADDRCAPFLAGS_NOEXTERNALCALLS: u32 = 268435456u32;
pub const LINEADDRCAPFLAGS_NOINTERNALCALLS: u32 = 134217728u32;
pub const LINEADDRCAPFLAGS_NOPSTNADDRESSTRANSLATION: u32 = 2147483648u32;
pub const LINEADDRCAPFLAGS_ORIGOFFHOOK: u32 = 64u32;
pub const LINEADDRCAPFLAGS_PARTIALDIAL: u32 = 65536u32;
pub const LINEADDRCAPFLAGS_PICKUPCALLWAIT: u32 = 4194304u32;
pub const LINEADDRCAPFLAGS_PICKUPGROUPID: u32 = 2u32;
pub const LINEADDRCAPFLAGS_PREDICTIVEDIALER: u32 = 8388608u32;
pub const LINEADDRCAPFLAGS_QUEUE: u32 = 16777216u32;
pub const LINEADDRCAPFLAGS_ROUTEPOINT: u32 = 33554432u32;
pub const LINEADDRCAPFLAGS_SECURE: u32 = 4u32;
pub const LINEADDRCAPFLAGS_SETCALLINGID: u32 = 536870912u32;
pub const LINEADDRCAPFLAGS_SETUPCONFNULL: u32 = 512u32;
pub const LINEADDRCAPFLAGS_TRANSFERHELD: u32 = 4096u32;
pub const LINEADDRCAPFLAGS_TRANSFERMAKE: u32 = 8192u32;
pub const LINEADDRESSMODE_ADDRESSID: u32 = 1u32;
pub const LINEADDRESSMODE_DIALABLEADDR: u32 = 2u32;
pub const LINEADDRESSSHARING_BRIDGEDEXCL: u32 = 2u32;
pub const LINEADDRESSSHARING_BRIDGEDNEW: u32 = 4u32;
pub const LINEADDRESSSHARING_BRIDGEDSHARED: u32 = 8u32;
pub const LINEADDRESSSHARING_MONITORED: u32 = 16u32;
pub const LINEADDRESSSHARING_PRIVATE: u32 = 1u32;
pub const LINEADDRESSSTATE_CAPSCHANGE: u32 = 256u32;
pub const LINEADDRESSSTATE_DEVSPECIFIC: u32 = 2u32;
pub const LINEADDRESSSTATE_FORWARD: u32 = 64u32;
pub const LINEADDRESSSTATE_INUSEMANY: u32 = 16u32;
pub const LINEADDRESSSTATE_INUSEONE: u32 = 8u32;
pub const LINEADDRESSSTATE_INUSEZERO: u32 = 4u32;
pub const LINEADDRESSSTATE_NUMCALLS: u32 = 32u32;
pub const LINEADDRESSSTATE_OTHER: u32 = 1u32;
pub const LINEADDRESSSTATE_TERMINALS: u32 = 128u32;
pub const LINEADDRESSTYPE_DOMAINNAME: u32 = 8u32;
pub const LINEADDRESSTYPE_EMAILNAME: u32 = 4u32;
pub const LINEADDRESSTYPE_IPADDRESS: u32 = 16u32;
pub const LINEADDRESSTYPE_PHONENUMBER: u32 = 1u32;
pub const LINEADDRESSTYPE_SDP: u32 = 2u32;
pub const LINEADDRFEATURE_FORWARD: u32 = 1u32;
pub const LINEADDRFEATURE_FORWARDDND: u32 = 8192u32;
pub const LINEADDRFEATURE_FORWARDFWD: u32 = 4096u32;
pub const LINEADDRFEATURE_MAKECALL: u32 = 2u32;
pub const LINEADDRFEATURE_PICKUP: u32 = 4u32;
pub const LINEADDRFEATURE_PICKUPDIRECT: u32 = 1024u32;
pub const LINEADDRFEATURE_PICKUPGROUP: u32 = 512u32;
pub const LINEADDRFEATURE_PICKUPHELD: u32 = 256u32;
pub const LINEADDRFEATURE_PICKUPWAITING: u32 = 2048u32;
pub const LINEADDRFEATURE_SETMEDIACONTROL: u32 = 8u32;
pub const LINEADDRFEATURE_SETTERMINAL: u32 = 16u32;
pub const LINEADDRFEATURE_SETUPCONF: u32 = 32u32;
pub const LINEADDRFEATURE_UNCOMPLETECALL: u32 = 64u32;
pub const LINEADDRFEATURE_UNPARK: u32 = 128u32;
pub const LINEAGENTFEATURE_AGENTSPECIFIC: u32 = 8u32;
pub const LINEAGENTFEATURE_GETAGENTACTIVITYLIST: u32 = 16u32;
pub const LINEAGENTFEATURE_GETAGENTGROUP: u32 = 32u32;
pub const LINEAGENTFEATURE_SETAGENTACTIVITY: u32 = 4u32;
pub const LINEAGENTFEATURE_SETAGENTGROUP: u32 = 1u32;
pub const LINEAGENTFEATURE_SETAGENTSTATE: u32 = 2u32;
pub const LINEAGENTSESSIONSTATE_BUSYONCALL: u32 = 4u32;
pub const LINEAGENTSESSIONSTATE_BUSYWRAPUP: u32 = 8u32;
pub const LINEAGENTSESSIONSTATE_ENDED: u32 = 16u32;
pub const LINEAGENTSESSIONSTATE_NOTREADY: u32 = 1u32;
pub const LINEAGENTSESSIONSTATE_READY: u32 = 2u32;
pub const LINEAGENTSESSIONSTATE_RELEASED: u32 = 32u32;
pub const LINEAGENTSESSIONSTATUS_NEWSESSION: u32 = 1u32;
pub const LINEAGENTSESSIONSTATUS_STATE: u32 = 2u32;
pub const LINEAGENTSESSIONSTATUS_UPDATEINFO: u32 = 4u32;
pub const LINEAGENTSTATEEX_BUSYACD: u32 = 4u32;
pub const LINEAGENTSTATEEX_BUSYINCOMING: u32 = 8u32;
pub const LINEAGENTSTATEEX_BUSYOUTGOING: u32 = 16u32;
pub const LINEAGENTSTATEEX_NOTREADY: u32 = 1u32;
pub const LINEAGENTSTATEEX_READY: u32 = 2u32;
pub const LINEAGENTSTATEEX_RELEASED: u32 = 64u32;
pub const LINEAGENTSTATEEX_UNKNOWN: u32 = 32u32;
pub const LINEAGENTSTATE_BUSYACD: u32 = 8u32;
pub const LINEAGENTSTATE_BUSYINCOMING: u32 = 16u32;
pub const LINEAGENTSTATE_BUSYOTHER: u32 = 64u32;
pub const LINEAGENTSTATE_BUSYOUTBOUND: u32 = 32u32;
pub const LINEAGENTSTATE_LOGGEDOFF: u32 = 1u32;
pub const LINEAGENTSTATE_NOTREADY: u32 = 2u32;
pub const LINEAGENTSTATE_READY: u32 = 4u32;
pub const LINEAGENTSTATE_UNAVAIL: u32 = 512u32;
pub const LINEAGENTSTATE_UNKNOWN: u32 = 256u32;
pub const LINEAGENTSTATE_WORKINGAFTERCALL: u32 = 128u32;
pub const LINEAGENTSTATUSEX_NEWAGENT: u32 = 1u32;
pub const LINEAGENTSTATUSEX_STATE: u32 = 2u32;
pub const LINEAGENTSTATUSEX_UPDATEINFO: u32 = 4u32;
pub const LINEAGENTSTATUS_ACTIVITY: u32 = 8u32;
pub const LINEAGENTSTATUS_ACTIVITYLIST: u32 = 16u32;
pub const LINEAGENTSTATUS_CAPSCHANGE: u32 = 64u32;
pub const LINEAGENTSTATUS_GROUP: u32 = 1u32;
pub const LINEAGENTSTATUS_GROUPLIST: u32 = 32u32;
pub const LINEAGENTSTATUS_NEXTSTATE: u32 = 4u32;
pub const LINEAGENTSTATUS_STATE: u32 = 2u32;
pub const LINEAGENTSTATUS_VALIDNEXTSTATES: u32 = 256u32;
pub const LINEAGENTSTATUS_VALIDSTATES: u32 = 128u32;
pub const LINEANSWERMODE_DROP: u32 = 2u32;
pub const LINEANSWERMODE_HOLD: u32 = 4u32;
pub const LINEANSWERMODE_NONE: u32 = 1u32;
pub const LINEBEARERMODE_ALTSPEECHDATA: u32 = 16u32;
pub const LINEBEARERMODE_DATA: u32 = 8u32;
pub const LINEBEARERMODE_MULTIUSE: u32 = 4u32;
pub const LINEBEARERMODE_NONCALLSIGNALING: u32 = 32u32;
pub const LINEBEARERMODE_PASSTHROUGH: u32 = 64u32;
pub const LINEBEARERMODE_RESTRICTEDDATA: u32 = 128u32;
pub const LINEBEARERMODE_SPEECH: u32 = 2u32;
pub const LINEBEARERMODE_VOICE: u32 = 1u32;
pub const LINEBUSYMODE_STATION: u32 = 1u32;
pub const LINEBUSYMODE_TRUNK: u32 = 2u32;
pub const LINEBUSYMODE_UNAVAIL: u32 = 8u32;
pub const LINEBUSYMODE_UNKNOWN: u32 = 4u32;
pub const LINECALLCOMPLCOND_BUSY: u32 = 1u32;
pub const LINECALLCOMPLCOND_NOANSWER: u32 = 2u32;
pub const LINECALLCOMPLMODE_CALLBACK: u32 = 2u32;
pub const LINECALLCOMPLMODE_CAMPON: u32 = 1u32;
pub const LINECALLCOMPLMODE_INTRUDE: u32 = 4u32;
pub const LINECALLCOMPLMODE_MESSAGE: u32 = 8u32;
pub const LINECALLFEATURE2_COMPLCALLBACK: u32 = 8u32;
pub const LINECALLFEATURE2_COMPLCAMPON: u32 = 4u32;
pub const LINECALLFEATURE2_COMPLINTRUDE: u32 = 16u32;
pub const LINECALLFEATURE2_COMPLMESSAGE: u32 = 32u32;
pub const LINECALLFEATURE2_NOHOLDCONFERENCE: u32 = 1u32;
pub const LINECALLFEATURE2_ONESTEPTRANSFER: u32 = 2u32;
pub const LINECALLFEATURE2_PARKDIRECT: u32 = 256u32;
pub const LINECALLFEATURE2_PARKNONDIRECT: u32 = 512u32;
pub const LINECALLFEATURE2_TRANSFERCONF: u32 = 128u32;
pub const LINECALLFEATURE2_TRANSFERNORM: u32 = 64u32;
pub const LINECALLFEATURE_ACCEPT: u32 = 1u32;
pub const LINECALLFEATURE_ADDTOCONF: u32 = 2u32;
pub const LINECALLFEATURE_ANSWER: u32 = 4u32;
pub const LINECALLFEATURE_BLINDTRANSFER: u32 = 8u32;
pub const LINECALLFEATURE_COMPLETECALL: u32 = 16u32;
pub const LINECALLFEATURE_COMPLETETRANSF: u32 = 32u32;
pub const LINECALLFEATURE_DIAL: u32 = 64u32;
pub const LINECALLFEATURE_DROP: u32 = 128u32;
pub const LINECALLFEATURE_GATHERDIGITS: u32 = 256u32;
pub const LINECALLFEATURE_GENERATEDIGITS: u32 = 512u32;
pub const LINECALLFEATURE_GENERATETONE: u32 = 1024u32;
pub const LINECALLFEATURE_HOLD: u32 = 2048u32;
pub const LINECALLFEATURE_MONITORDIGITS: u32 = 4096u32;
pub const LINECALLFEATURE_MONITORMEDIA: u32 = 8192u32;
pub const LINECALLFEATURE_MONITORTONES: u32 = 16384u32;
pub const LINECALLFEATURE_PARK: u32 = 32768u32;
pub const LINECALLFEATURE_PREPAREADDCONF: u32 = 65536u32;
pub const LINECALLFEATURE_REDIRECT: u32 = 131072u32;
pub const LINECALLFEATURE_RELEASEUSERUSERINFO: u32 = 268435456u32;
pub const LINECALLFEATURE_REMOVEFROMCONF: u32 = 262144u32;
pub const LINECALLFEATURE_SECURECALL: u32 = 524288u32;
pub const LINECALLFEATURE_SENDUSERUSER: u32 = 1048576u32;
pub const LINECALLFEATURE_SETCALLDATA: u32 = 2147483648u32;
pub const LINECALLFEATURE_SETCALLPARAMS: u32 = 2097152u32;
pub const LINECALLFEATURE_SETMEDIACONTROL: u32 = 4194304u32;
pub const LINECALLFEATURE_SETQOS: u32 = 1073741824u32;
pub const LINECALLFEATURE_SETTERMINAL: u32 = 8388608u32;
pub const LINECALLFEATURE_SETTREATMENT: u32 = 536870912u32;
pub const LINECALLFEATURE_SETUPCONF: u32 = 16777216u32;
pub const LINECALLFEATURE_SETUPTRANSFER: u32 = 33554432u32;
pub const LINECALLFEATURE_SWAPHOLD: u32 = 67108864u32;
pub const LINECALLFEATURE_UNHOLD: u32 = 134217728u32;
pub const LINECALLHUBTRACKING_ALLCALLS: u32 = 2u32;
pub const LINECALLHUBTRACKING_NONE: u32 = 0u32;
pub const LINECALLHUBTRACKING_PROVIDERLEVEL: u32 = 1u32;
pub const LINECALLINFOSTATE_APPSPECIFIC: u32 = 32u32;
pub const LINECALLINFOSTATE_BEARERMODE: u32 = 4u32;
pub const LINECALLINFOSTATE_CALLDATA: u32 = 1073741824u32;
pub const LINECALLINFOSTATE_CALLEDID: u32 = 65536u32;
pub const LINECALLINFOSTATE_CALLERID: u32 = 32768u32;
pub const LINECALLINFOSTATE_CALLID: u32 = 64u32;
pub const LINECALLINFOSTATE_CHARGINGINFO: u32 = 16777216u32;
pub const LINECALLINFOSTATE_COMPLETIONID: u32 = 1024u32;
pub const LINECALLINFOSTATE_CONNECTEDID: u32 = 131072u32;
pub const LINECALLINFOSTATE_DEVSPECIFIC: u32 = 2u32;
pub const LINECALLINFOSTATE_DIALPARAMS: u32 = 67108864u32;
pub const LINECALLINFOSTATE_DISPLAY: u32 = 1048576u32;
pub const LINECALLINFOSTATE_HIGHLEVELCOMP: u32 = 4194304u32;
pub const LINECALLINFOSTATE_LOWLEVELCOMP: u32 = 8388608u32;
pub const LINECALLINFOSTATE_MEDIAMODE: u32 = 16u32;
pub const LINECALLINFOSTATE_MONITORMODES: u32 = 134217728u32;
pub const LINECALLINFOSTATE_NUMMONITORS: u32 = 8192u32;
pub const LINECALLINFOSTATE_NUMOWNERDECR: u32 = 4096u32;
pub const LINECALLINFOSTATE_NUMOWNERINCR: u32 = 2048u32;
pub const LINECALLINFOSTATE_ORIGIN: u32 = 256u32;
pub const LINECALLINFOSTATE_OTHER: u32 = 1u32;
pub const LINECALLINFOSTATE_QOS: u32 = 536870912u32;
pub const LINECALLINFOSTATE_RATE: u32 = 8u32;
pub const LINECALLINFOSTATE_REASON: u32 = 512u32;
pub const LINECALLINFOSTATE_REDIRECTINGID: u32 = 524288u32;
pub const LINECALLINFOSTATE_REDIRECTIONID: u32 = 262144u32;
pub const LINECALLINFOSTATE_RELATEDCALLID: u32 = 128u32;
pub const LINECALLINFOSTATE_TERMINAL: u32 = 33554432u32;
pub const LINECALLINFOSTATE_TREATMENT: u32 = 268435456u32;
pub const LINECALLINFOSTATE_TRUNK: u32 = 16384u32;
pub const LINECALLINFOSTATE_USERUSERINFO: u32 = 2097152u32;
pub const LINECALLORIGIN_CONFERENCE: u32 = 64u32;
pub const LINECALLORIGIN_EXTERNAL: u32 = 4u32;
pub const LINECALLORIGIN_INBOUND: u32 = 128u32;
pub const LINECALLORIGIN_INTERNAL: u32 = 2u32;
pub const LINECALLORIGIN_OUTBOUND: u32 = 1u32;
pub const LINECALLORIGIN_UNAVAIL: u32 = 32u32;
pub const LINECALLORIGIN_UNKNOWN: u32 = 16u32;
pub const LINECALLPARAMFLAGS_BLOCKID: u32 = 4u32;
pub const LINECALLPARAMFLAGS_DESTOFFHOOK: u32 = 16u32;
pub const LINECALLPARAMFLAGS_IDLE: u32 = 2u32;
pub const LINECALLPARAMFLAGS_NOHOLDCONFERENCE: u32 = 32u32;
pub const LINECALLPARAMFLAGS_ONESTEPTRANSFER: u32 = 128u32;
pub const LINECALLPARAMFLAGS_ORIGOFFHOOK: u32 = 8u32;
pub const LINECALLPARAMFLAGS_PREDICTIVEDIAL: u32 = 64u32;
pub const LINECALLPARAMFLAGS_SECURE: u32 = 1u32;
pub const LINECALLPARTYID_ADDRESS: u32 = 8u32;
pub const LINECALLPARTYID_BLOCKED: u32 = 1u32;
pub const LINECALLPARTYID_NAME: u32 = 4u32;
pub const LINECALLPARTYID_OUTOFAREA: u32 = 2u32;
pub const LINECALLPARTYID_PARTIAL: u32 = 16u32;
pub const LINECALLPARTYID_UNAVAIL: u32 = 64u32;
pub const LINECALLPARTYID_UNKNOWN: u32 = 32u32;
pub const LINECALLPRIVILEGE_MONITOR: u32 = 2u32;
pub const LINECALLPRIVILEGE_NONE: u32 = 1u32;
pub const LINECALLPRIVILEGE_OWNER: u32 = 4u32;
pub const LINECALLREASON_CALLCOMPLETION: u32 = 128u32;
pub const LINECALLREASON_CAMPEDON: u32 = 16384u32;
pub const LINECALLREASON_DIRECT: u32 = 1u32;
pub const LINECALLREASON_FWDBUSY: u32 = 2u32;
pub const LINECALLREASON_FWDNOANSWER: u32 = 4u32;
pub const LINECALLREASON_FWDUNCOND: u32 = 8u32;
pub const LINECALLREASON_INTRUDE: u32 = 4096u32;
pub const LINECALLREASON_PARKED: u32 = 8192u32;
pub const LINECALLREASON_PICKUP: u32 = 16u32;
pub const LINECALLREASON_REDIRECT: u32 = 64u32;
pub const LINECALLREASON_REMINDER: u32 = 512u32;
pub const LINECALLREASON_ROUTEREQUEST: u32 = 32768u32;
pub const LINECALLREASON_TRANSFER: u32 = 256u32;
pub const LINECALLREASON_UNAVAIL: u32 = 2048u32;
pub const LINECALLREASON_UNKNOWN: u32 = 1024u32;
pub const LINECALLREASON_UNPARK: u32 = 32u32;
pub const LINECALLSELECT_ADDRESS: u32 = 2u32;
pub const LINECALLSELECT_CALL: u32 = 4u32;
pub const LINECALLSELECT_CALLID: u32 = 16u32;
pub const LINECALLSELECT_DEVICEID: u32 = 8u32;
pub const LINECALLSELECT_LINE: u32 = 1u32;
pub const LINECALLSTATE_ACCEPTED: u32 = 4u32;
pub const LINECALLSTATE_BUSY: u32 = 64u32;
pub const LINECALLSTATE_CONFERENCED: u32 = 2048u32;
pub const LINECALLSTATE_CONNECTED: u32 = 256u32;
pub const LINECALLSTATE_DIALING: u32 = 16u32;
pub const LINECALLSTATE_DIALTONE: u32 = 8u32;
pub const LINECALLSTATE_DISCONNECTED: u32 = 16384u32;
pub const LINECALLSTATE_IDLE: u32 = 1u32;
pub const LINECALLSTATE_OFFERING: u32 = 2u32;
pub const LINECALLSTATE_ONHOLD: u32 = 1024u32;
pub const LINECALLSTATE_ONHOLDPENDCONF: u32 = 4096u32;
pub const LINECALLSTATE_ONHOLDPENDTRANSFER: u32 = 8192u32;
pub const LINECALLSTATE_PROCEEDING: u32 = 512u32;
pub const LINECALLSTATE_RINGBACK: u32 = 32u32;
pub const LINECALLSTATE_SPECIALINFO: u32 = 128u32;
pub const LINECALLSTATE_UNKNOWN: u32 = 32768u32;
pub const LINECALLTREATMENT_BUSY: u32 = 3u32;
pub const LINECALLTREATMENT_MUSIC: u32 = 4u32;
pub const LINECALLTREATMENT_RINGBACK: u32 = 2u32;
pub const LINECALLTREATMENT_SILENCE: u32 = 1u32;
pub const LINECARDOPTION_HIDDEN: u32 = 2u32;
pub const LINECARDOPTION_PREDEFINED: u32 = 1u32;
pub const LINECONNECTEDMODE_ACTIVE: u32 = 1u32;
pub const LINECONNECTEDMODE_ACTIVEHELD: u32 = 4u32;
pub const LINECONNECTEDMODE_CONFIRMED: u32 = 16u32;
pub const LINECONNECTEDMODE_INACTIVE: u32 = 2u32;
pub const LINECONNECTEDMODE_INACTIVEHELD: u32 = 8u32;
pub const LINEDEVCAPFLAGS_CALLHUB: u32 = 1024u32;
pub const LINEDEVCAPFLAGS_CALLHUBTRACKING: u32 = 2048u32;
pub const LINEDEVCAPFLAGS_CLOSEDROP: u32 = 32u32;
pub const LINEDEVCAPFLAGS_CROSSADDRCONF: u32 = 1u32;
pub const LINEDEVCAPFLAGS_DIALBILLING: u32 = 64u32;
pub const LINEDEVCAPFLAGS_DIALDIALTONE: u32 = 256u32;
pub const LINEDEVCAPFLAGS_DIALQUIET: u32 = 128u32;
pub const LINEDEVCAPFLAGS_HIGHLEVCOMP: u32 = 2u32;
pub const LINEDEVCAPFLAGS_LOCAL: u32 = 8192u32;
pub const LINEDEVCAPFLAGS_LOWLEVCOMP: u32 = 4u32;
pub const LINEDEVCAPFLAGS_MEDIACONTROL: u32 = 8u32;
pub const LINEDEVCAPFLAGS_MSP: u32 = 512u32;
pub const LINEDEVCAPFLAGS_MULTIPLEADDR: u32 = 16u32;
pub const LINEDEVCAPFLAGS_PRIVATEOBJECTS: u32 = 4096u32;
pub const LINEDEVSTATE_BATTERY: u32 = 32768u32;
pub const LINEDEVSTATE_CAPSCHANGE: u32 = 1048576u32;
pub const LINEDEVSTATE_CLOSE: u32 = 1024u32;
pub const LINEDEVSTATE_COMPLCANCEL: u32 = 8388608u32;
pub const LINEDEVSTATE_CONFIGCHANGE: u32 = 2097152u32;
pub const LINEDEVSTATE_CONNECTED: u32 = 4u32;
pub const LINEDEVSTATE_DEVSPECIFIC: u32 = 131072u32;
pub const LINEDEVSTATE_DISCONNECTED: u32 = 8u32;
pub const LINEDEVSTATE_INSERVICE: u32 = 64u32;
pub const LINEDEVSTATE_LOCK: u32 = 524288u32;
pub const LINEDEVSTATE_MAINTENANCE: u32 = 256u32;
pub const LINEDEVSTATE_MSGWAITOFF: u32 = 32u32;
pub const LINEDEVSTATE_MSGWAITON: u32 = 16u32;
pub const LINEDEVSTATE_NUMCALLS: u32 = 2048u32;
pub const LINEDEVSTATE_NUMCOMPLETIONS: u32 = 4096u32;
pub const LINEDEVSTATE_OPEN: u32 = 512u32;
pub const LINEDEVSTATE_OTHER: u32 = 1u32;
pub const LINEDEVSTATE_OUTOFSERVICE: u32 = 128u32;
pub const LINEDEVSTATE_REINIT: u32 = 262144u32;
pub const LINEDEVSTATE_REMOVED: u32 = 16777216u32;
pub const LINEDEVSTATE_RINGING: u32 = 2u32;
pub const LINEDEVSTATE_ROAMMODE: u32 = 16384u32;
pub const LINEDEVSTATE_SIGNAL: u32 = 65536u32;
pub const LINEDEVSTATE_TERMINALS: u32 = 8192u32;
pub const LINEDEVSTATE_TRANSLATECHANGE: u32 = 4194304u32;
pub const LINEDEVSTATUSFLAGS_CONNECTED: u32 = 1u32;
pub const LINEDEVSTATUSFLAGS_INSERVICE: u32 = 4u32;
pub const LINEDEVSTATUSFLAGS_LOCKED: u32 = 8u32;
pub const LINEDEVSTATUSFLAGS_MSGWAIT: u32 = 2u32;
pub const LINEDIALTONEMODE_EXTERNAL: u32 = 8u32;
pub const LINEDIALTONEMODE_INTERNAL: u32 = 4u32;
pub const LINEDIALTONEMODE_NORMAL: u32 = 1u32;
pub const LINEDIALTONEMODE_SPECIAL: u32 = 2u32;
pub const LINEDIALTONEMODE_UNAVAIL: u32 = 32u32;
pub const LINEDIALTONEMODE_UNKNOWN: u32 = 16u32;
pub const LINEDIGITMODE_DTMF: u32 = 2u32;
pub const LINEDIGITMODE_DTMFEND: u32 = 4u32;
pub const LINEDIGITMODE_PULSE: u32 = 1u32;
pub const LINEDISCONNECTMODE_BADADDRESS: u32 = 128u32;
pub const LINEDISCONNECTMODE_BLOCKED: u32 = 131072u32;
pub const LINEDISCONNECTMODE_BUSY: u32 = 32u32;
pub const LINEDISCONNECTMODE_CANCELLED: u32 = 524288u32;
pub const LINEDISCONNECTMODE_CONGESTION: u32 = 512u32;
pub const LINEDISCONNECTMODE_DESTINATIONBARRED: u32 = 1048576u32;
pub const LINEDISCONNECTMODE_DONOTDISTURB: u32 = 262144u32;
pub const LINEDISCONNECTMODE_FDNRESTRICT: u32 = 2097152u32;
pub const LINEDISCONNECTMODE_FORWARDED: u32 = 16u32;
pub const LINEDISCONNECTMODE_INCOMPATIBLE: u32 = 1024u32;
pub const LINEDISCONNECTMODE_NOANSWER: u32 = 64u32;
pub const LINEDISCONNECTMODE_NODIALTONE: u32 = 4096u32;
pub const LINEDISCONNECTMODE_NORMAL: u32 = 1u32;
pub const LINEDISCONNECTMODE_NUMBERCHANGED: u32 = 8192u32;
pub const LINEDISCONNECTMODE_OUTOFORDER: u32 = 16384u32;
pub const LINEDISCONNECTMODE_PICKUP: u32 = 8u32;
pub const LINEDISCONNECTMODE_QOSUNAVAIL: u32 = 65536u32;
pub const LINEDISCONNECTMODE_REJECT: u32 = 4u32;
pub const LINEDISCONNECTMODE_TEMPFAILURE: u32 = 32768u32;
pub const LINEDISCONNECTMODE_UNAVAIL: u32 = 2048u32;
pub const LINEDISCONNECTMODE_UNKNOWN: u32 = 2u32;
pub const LINEDISCONNECTMODE_UNREACHABLE: u32 = 256u32;
pub const LINEEQOSINFO_ADMISSIONFAILURE: u32 = 2u32;
pub const LINEEQOSINFO_GENERICERROR: u32 = 4u32;
pub const LINEEQOSINFO_NOQOS: u32 = 1u32;
pub const LINEEQOSINFO_POLICYFAILURE: u32 = 3u32;
pub const LINEERR_ADDRESSBLOCKED: u32 = 2147483731u32;
pub const LINEERR_ALLOCATED: u32 = 2147483649u32;
pub const LINEERR_BADDEVICEID: u32 = 2147483650u32;
pub const LINEERR_BEARERMODEUNAVAIL: u32 = 2147483651u32;
pub const LINEERR_BILLINGREJECTED: u32 = 2147483732u32;
pub const LINEERR_CALLUNAVAIL: u32 = 2147483653u32;
pub const LINEERR_COMPLETIONOVERRUN: u32 = 2147483654u32;
pub const LINEERR_CONFERENCEFULL: u32 = 2147483655u32;
pub const LINEERR_DIALBILLING: u32 = 2147483656u32;
pub const LINEERR_DIALDIALTONE: u32 = 2147483657u32;
pub const LINEERR_DIALPROMPT: u32 = 2147483658u32;
pub const LINEERR_DIALQUIET: u32 = 2147483659u32;
pub const LINEERR_DIALVOICEDETECT: u32 = 2147483740u32;
pub const LINEERR_DISCONNECTED: u32 = 2147483744u32;
pub const LINEERR_INCOMPATIBLEAPIVERSION: u32 = 2147483660u32;
pub const LINEERR_INCOMPATIBLEEXTVERSION: u32 = 2147483661u32;
pub const LINEERR_INIFILECORRUPT: u32 = 2147483662u32;
pub const LINEERR_INUSE: u32 = 2147483663u32;
pub const LINEERR_INVALADDRESS: u32 = 2147483664u32;
pub const LINEERR_INVALADDRESSID: u32 = 2147483665u32;
pub const LINEERR_INVALADDRESSMODE: u32 = 2147483666u32;
pub const LINEERR_INVALADDRESSSTATE: u32 = 2147483667u32;
pub const LINEERR_INVALADDRESSTYPE: u32 = 2147483742u32;
pub const LINEERR_INVALAGENTACTIVITY: u32 = 2147483739u32;
pub const LINEERR_INVALAGENTGROUP: u32 = 2147483736u32;
pub const LINEERR_INVALAGENTID: u32 = 2147483735u32;
pub const LINEERR_INVALAGENTSESSIONSTATE: u32 = 2147483743u32;
pub const LINEERR_INVALAGENTSTATE: u32 = 2147483738u32;
pub const LINEERR_INVALAPPHANDLE: u32 = 2147483668u32;
pub const LINEERR_INVALAPPNAME: u32 = 2147483669u32;
pub const LINEERR_INVALBEARERMODE: u32 = 2147483670u32;
pub const LINEERR_INVALCALLCOMPLMODE: u32 = 2147483671u32;
pub const LINEERR_INVALCALLHANDLE: u32 = 2147483672u32;
pub const LINEERR_INVALCALLPARAMS: u32 = 2147483673u32;
pub const LINEERR_INVALCALLPRIVILEGE: u32 = 2147483674u32;
pub const LINEERR_INVALCALLSELECT: u32 = 2147483675u32;
pub const LINEERR_INVALCALLSTATE: u32 = 2147483676u32;
pub const LINEERR_INVALCALLSTATELIST: u32 = 2147483677u32;
pub const LINEERR_INVALCARD: u32 = 2147483678u32;
pub const LINEERR_INVALCOMPLETIONID: u32 = 2147483679u32;
pub const LINEERR_INVALCONFCALLHANDLE: u32 = 2147483680u32;
pub const LINEERR_INVALCONSULTCALLHANDLE: u32 = 2147483681u32;
pub const LINEERR_INVALCOUNTRYCODE: u32 = 2147483682u32;
pub const LINEERR_INVALDEVICECLASS: u32 = 2147483683u32;
pub const LINEERR_INVALDEVICEHANDLE: u32 = 2147483684u32;
pub const LINEERR_INVALDIALPARAMS: u32 = 2147483685u32;
pub const LINEERR_INVALDIGITLIST: u32 = 2147483686u32;
pub const LINEERR_INVALDIGITMODE: u32 = 2147483687u32;
pub const LINEERR_INVALDIGITS: u32 = 2147483688u32;
pub const LINEERR_INVALEXTVERSION: u32 = 2147483689u32;
pub const LINEERR_INVALFEATURE: u32 = 2147483733u32;
pub const LINEERR_INVALGROUPID: u32 = 2147483690u32;
pub const LINEERR_INVALLINEHANDLE: u32 = 2147483691u32;
pub const LINEERR_INVALLINESTATE: u32 = 2147483692u32;
pub const LINEERR_INVALLOCATION: u32 = 2147483693u32;
pub const LINEERR_INVALMEDIALIST: u32 = 2147483694u32;
pub const LINEERR_INVALMEDIAMODE: u32 = 2147483695u32;
pub const LINEERR_INVALMESSAGEID: u32 = 2147483696u32;
pub const LINEERR_INVALPARAM: u32 = 2147483698u32;
pub const LINEERR_INVALPARKID: u32 = 2147483699u32;
pub const LINEERR_INVALPARKMODE: u32 = 2147483700u32;
pub const LINEERR_INVALPASSWORD: u32 = 2147483737u32;
pub const LINEERR_INVALPOINTER: u32 = 2147483701u32;
pub const LINEERR_INVALPRIVSELECT: u32 = 2147483702u32;
pub const LINEERR_INVALRATE: u32 = 2147483703u32;
pub const LINEERR_INVALREQUESTMODE: u32 = 2147483704u32;
pub const LINEERR_INVALTERMINALID: u32 = 2147483705u32;
pub const LINEERR_INVALTERMINALMODE: u32 = 2147483706u32;
pub const LINEERR_INVALTIMEOUT: u32 = 2147483707u32;
pub const LINEERR_INVALTONE: u32 = 2147483708u32;
pub const LINEERR_INVALTONELIST: u32 = 2147483709u32;
pub const LINEERR_INVALTONEMODE: u32 = 2147483710u32;
pub const LINEERR_INVALTRANSFERMODE: u32 = 2147483711u32;
pub const LINEERR_LINEMAPPERFAILED: u32 = 2147483712u32;
pub const LINEERR_NOCONFERENCE: u32 = 2147483713u32;
pub const LINEERR_NODEVICE: u32 = 2147483714u32;
pub const LINEERR_NODRIVER: u32 = 2147483715u32;
pub const LINEERR_NOMEM: u32 = 2147483716u32;
pub const LINEERR_NOMULTIPLEINSTANCE: u32 = 2147483734u32;
pub const LINEERR_NOREQUEST: u32 = 2147483717u32;
pub const LINEERR_NOTOWNER: u32 = 2147483718u32;
pub const LINEERR_NOTREGISTERED: u32 = 2147483719u32;
pub const LINEERR_OPERATIONFAILED: u32 = 2147483720u32;
pub const LINEERR_OPERATIONUNAVAIL: u32 = 2147483721u32;
pub const LINEERR_RATEUNAVAIL: u32 = 2147483722u32;
pub const LINEERR_REINIT: u32 = 2147483730u32;
pub const LINEERR_REQUESTOVERRUN: u32 = 2147483724u32;
pub const LINEERR_RESOURCEUNAVAIL: u32 = 2147483723u32;
pub const LINEERR_SERVICE_NOT_RUNNING: u32 = 2147483745u32;
pub const LINEERR_STRUCTURETOOSMALL: u32 = 2147483725u32;
pub const LINEERR_TARGETNOTFOUND: u32 = 2147483726u32;
pub const LINEERR_TARGETSELF: u32 = 2147483727u32;
pub const LINEERR_UNINITIALIZED: u32 = 2147483728u32;
pub const LINEERR_USERCANCELLED: u32 = 2147483741u32;
pub const LINEERR_USERUSERINFOTOOBIG: u32 = 2147483729u32;
pub const LINEFEATURE_DEVSPECIFIC: u32 = 1u32;
pub const LINEFEATURE_DEVSPECIFICFEAT: u32 = 2u32;
pub const LINEFEATURE_FORWARD: u32 = 4u32;
pub const LINEFEATURE_FORWARDDND: u32 = 256u32;
pub const LINEFEATURE_FORWARDFWD: u32 = 128u32;
pub const LINEFEATURE_MAKECALL: u32 = 8u32;
pub const LINEFEATURE_SETDEVSTATUS: u32 = 64u32;
pub const LINEFEATURE_SETMEDIACONTROL: u32 = 16u32;
pub const LINEFEATURE_SETTERMINAL: u32 = 32u32;
pub const LINEFORWARDMODE_BUSY: u32 = 16u32;
pub const LINEFORWARDMODE_BUSYEXTERNAL: u32 = 64u32;
pub const LINEFORWARDMODE_BUSYINTERNAL: u32 = 32u32;
pub const LINEFORWARDMODE_BUSYNA: u32 = 4096u32;
pub const LINEFORWARDMODE_BUSYNAEXTERNAL: u32 = 16384u32;
pub const LINEFORWARDMODE_BUSYNAINTERNAL: u32 = 8192u32;
pub const LINEFORWARDMODE_BUSYNASPECIFIC: u32 = 32768u32;
pub const LINEFORWARDMODE_BUSYSPECIFIC: u32 = 128u32;
pub const LINEFORWARDMODE_NOANSW: u32 = 256u32;
pub const LINEFORWARDMODE_NOANSWEXTERNAL: u32 = 1024u32;
pub const LINEFORWARDMODE_NOANSWINTERNAL: u32 = 512u32;
pub const LINEFORWARDMODE_NOANSWSPECIFIC: u32 = 2048u32;
pub const LINEFORWARDMODE_UNAVAIL: u32 = 131072u32;
pub const LINEFORWARDMODE_UNCOND: u32 = 1u32;
pub const LINEFORWARDMODE_UNCONDEXTERNAL: u32 = 4u32;
pub const LINEFORWARDMODE_UNCONDINTERNAL: u32 = 2u32;
pub const LINEFORWARDMODE_UNCONDSPECIFIC: u32 = 8u32;
pub const LINEFORWARDMODE_UNKNOWN: u32 = 65536u32;
pub const LINEGATHERTERM_BUFFERFULL: u32 = 1u32;
pub const LINEGATHERTERM_CANCEL: u32 = 16u32;
pub const LINEGATHERTERM_FIRSTTIMEOUT: u32 = 4u32;
pub const LINEGATHERTERM_INTERTIMEOUT: u32 = 8u32;
pub const LINEGATHERTERM_TERMDIGIT: u32 = 2u32;
pub const LINEGENERATETERM_CANCEL: u32 = 2u32;
pub const LINEGENERATETERM_DONE: u32 = 1u32;
pub const LINEGROUPSTATUS_GROUPREMOVED: u32 = 2u32;
pub const LINEGROUPSTATUS_NEWGROUP: u32 = 1u32;
pub const LINEINITIALIZEEXOPTION_CALLHUBTRACKING: u32 = 2147483648u32;
pub const LINEINITIALIZEEXOPTION_USECOMPLETIONPORT: u32 = 3u32;
pub const LINEINITIALIZEEXOPTION_USEEVENT: u32 = 2u32;
pub const LINEINITIALIZEEXOPTION_USEHIDDENWINDOW: u32 = 1u32;
pub const LINELOCATIONOPTION_PULSEDIAL: u32 = 1u32;
pub const LINEMAPPER: u32 = 4294967295u32;
pub const LINEMEDIACONTROL_NONE: u32 = 1u32;
pub const LINEMEDIACONTROL_PAUSE: u32 = 8u32;
pub const LINEMEDIACONTROL_RATEDOWN: u32 = 64u32;
pub const LINEMEDIACONTROL_RATENORMAL: u32 = 128u32;
pub const LINEMEDIACONTROL_RATEUP: u32 = 32u32;
pub const LINEMEDIACONTROL_RESET: u32 = 4u32;
pub const LINEMEDIACONTROL_RESUME: u32 = 16u32;
pub const LINEMEDIACONTROL_START: u32 = 2u32;
pub const LINEMEDIACONTROL_VOLUMEDOWN: u32 = 512u32;
pub const LINEMEDIACONTROL_VOLUMENORMAL: u32 = 1024u32;
pub const LINEMEDIACONTROL_VOLUMEUP: u32 = 256u32;
pub const LINEMEDIAMODE_ADSI: u32 = 8192u32;
pub const LINEMEDIAMODE_AUTOMATEDVOICE: u32 = 8u32;
pub const LINEMEDIAMODE_DATAMODEM: u32 = 16u32;
pub const LINEMEDIAMODE_DIGITALDATA: u32 = 256u32;
pub const LINEMEDIAMODE_G3FAX: u32 = 32u32;
pub const LINEMEDIAMODE_G4FAX: u32 = 128u32;
pub const LINEMEDIAMODE_INTERACTIVEVOICE: u32 = 4u32;
pub const LINEMEDIAMODE_MIXED: u32 = 4096u32;
pub const LINEMEDIAMODE_TDD: u32 = 64u32;
pub const LINEMEDIAMODE_TELETEX: u32 = 512u32;
pub const LINEMEDIAMODE_TELEX: u32 = 2048u32;
pub const LINEMEDIAMODE_UNKNOWN: u32 = 2u32;
pub const LINEMEDIAMODE_VIDEO: u32 = 32768u32;
pub const LINEMEDIAMODE_VIDEOTEX: u32 = 1024u32;
pub const LINEMEDIAMODE_VOICEVIEW: u32 = 16384u32;
pub const LINEOFFERINGMODE_ACTIVE: u32 = 1u32;
pub const LINEOFFERINGMODE_INACTIVE: u32 = 2u32;
pub const LINEOPENOPTION_PROXY: u32 = 1073741824u32;
pub const LINEOPENOPTION_SINGLEADDRESS: u32 = 2147483648u32;
pub const LINEPARKMODE_DIRECTED: u32 = 1u32;
pub const LINEPARKMODE_NONDIRECTED: u32 = 2u32;
pub const LINEPROXYREQUEST_AGENTSPECIFIC: u32 = 6u32;
pub const LINEPROXYREQUEST_CREATEAGENT: u32 = 9u32;
pub const LINEPROXYREQUEST_CREATEAGENTSESSION: u32 = 12u32;
pub const LINEPROXYREQUEST_GETAGENTACTIVITYLIST: u32 = 7u32;
pub const LINEPROXYREQUEST_GETAGENTCAPS: u32 = 4u32;
pub const LINEPROXYREQUEST_GETAGENTGROUPLIST: u32 = 8u32;
pub const LINEPROXYREQUEST_GETAGENTINFO: u32 = 11u32;
pub const LINEPROXYREQUEST_GETAGENTSESSIONINFO: u32 = 15u32;
pub const LINEPROXYREQUEST_GETAGENTSESSIONLIST: u32 = 13u32;
pub const LINEPROXYREQUEST_GETAGENTSTATUS: u32 = 5u32;
pub const LINEPROXYREQUEST_GETGROUPLIST: u32 = 19u32;
pub const LINEPROXYREQUEST_GETQUEUEINFO: u32 = 18u32;
pub const LINEPROXYREQUEST_GETQUEUELIST: u32 = 16u32;
pub const LINEPROXYREQUEST_SETAGENTACTIVITY: u32 = 3u32;
pub const LINEPROXYREQUEST_SETAGENTGROUP: u32 = 1u32;
pub const LINEPROXYREQUEST_SETAGENTMEASUREMENTPERIOD: u32 = 10u32;
pub const LINEPROXYREQUEST_SETAGENTSESSIONSTATE: u32 = 14u32;
pub const LINEPROXYREQUEST_SETAGENTSTATE: u32 = 2u32;
pub const LINEPROXYREQUEST_SETAGENTSTATEEX: u32 = 20u32;
pub const LINEPROXYREQUEST_SETQUEUEMEASUREMENTPERIOD: u32 = 17u32;
pub const LINEPROXYSTATUS_ALLOPENFORACD: u32 = 4u32;
pub const LINEPROXYSTATUS_CLOSE: u32 = 2u32;
pub const LINEPROXYSTATUS_OPEN: u32 = 1u32;
pub const LINEQOSREQUESTTYPE_SERVICELEVEL: u32 = 1u32;
pub const LINEQOSSERVICELEVEL_BESTEFFORT: u32 = 3u32;
pub const LINEQOSSERVICELEVEL_IFAVAILABLE: u32 = 2u32;
pub const LINEQOSSERVICELEVEL_NEEDED: u32 = 1u32;
pub const LINEQUEUESTATUS_NEWQUEUE: u32 = 2u32;
pub const LINEQUEUESTATUS_QUEUEREMOVED: u32 = 4u32;
pub const LINEQUEUESTATUS_UPDATEINFO: u32 = 1u32;
pub const LINEREMOVEFROMCONF_ANY: u32 = 3u32;
pub const LINEREMOVEFROMCONF_LAST: u32 = 2u32;
pub const LINEREMOVEFROMCONF_NONE: u32 = 1u32;
pub const LINEREQUESTMODE_DROP: u32 = 4u32;
pub const LINEREQUESTMODE_MAKECALL: u32 = 1u32;
pub const LINEREQUESTMODE_MEDIACALL: u32 = 2u32;
pub const LINEROAMMODE_HOME: u32 = 4u32;
pub const LINEROAMMODE_ROAMA: u32 = 8u32;
pub const LINEROAMMODE_ROAMB: u32 = 16u32;
pub const LINEROAMMODE_UNAVAIL: u32 = 2u32;
pub const LINEROAMMODE_UNKNOWN: u32 = 1u32;
pub const LINESPECIALINFO_CUSTIRREG: u32 = 2u32;
pub const LINESPECIALINFO_NOCIRCUIT: u32 = 1u32;
pub const LINESPECIALINFO_REORDER: u32 = 4u32;
pub const LINESPECIALINFO_UNAVAIL: u32 = 16u32;
pub const LINESPECIALINFO_UNKNOWN: u32 = 8u32;
pub const LINETERMDEV_HEADSET: u32 = 2u32;
pub const LINETERMDEV_PHONE: u32 = 1u32;
pub const LINETERMDEV_SPEAKER: u32 = 4u32;
pub const LINETERMMODE_BUTTONS: u32 = 1u32;
pub const LINETERMMODE_DISPLAY: u32 = 4u32;
pub const LINETERMMODE_HOOKSWITCH: u32 = 16u32;
pub const LINETERMMODE_LAMPS: u32 = 2u32;
pub const LINETERMMODE_MEDIABIDIRECT: u32 = 128u32;
pub const LINETERMMODE_MEDIAFROMLINE: u32 = 64u32;
pub const LINETERMMODE_MEDIATOLINE: u32 = 32u32;
pub const LINETERMMODE_RINGER: u32 = 8u32;
pub const LINETERMSHARING_PRIVATE: u32 = 1u32;
pub const LINETERMSHARING_SHAREDCONF: u32 = 4u32;
pub const LINETERMSHARING_SHAREDEXCL: u32 = 2u32;
pub const LINETOLLLISTOPTION_ADD: u32 = 1u32;
pub const LINETOLLLISTOPTION_REMOVE: u32 = 2u32;
pub const LINETONEMODE_BEEP: u32 = 8u32;
pub const LINETONEMODE_BILLING: u32 = 16u32;
pub const LINETONEMODE_BUSY: u32 = 4u32;
pub const LINETONEMODE_CUSTOM: u32 = 1u32;
pub const LINETONEMODE_RINGBACK: u32 = 2u32;
pub const LINETRANSFERMODE_CONFERENCE: u32 = 2u32;
pub const LINETRANSFERMODE_TRANSFER: u32 = 1u32;
pub const LINETRANSLATEOPTION_CANCELCALLWAITING: u32 = 2u32;
pub const LINETRANSLATEOPTION_CARDOVERRIDE: u32 = 1u32;
pub const LINETRANSLATEOPTION_FORCELD: u32 = 8u32;
pub const LINETRANSLATEOPTION_FORCELOCAL: u32 = 4u32;
pub const LINETRANSLATERESULT_CANONICAL: u32 = 1u32;
pub const LINETRANSLATERESULT_DIALBILLING: u32 = 64u32;
pub const LINETRANSLATERESULT_DIALDIALTONE: u32 = 256u32;
pub const LINETRANSLATERESULT_DIALPROMPT: u32 = 512u32;
pub const LINETRANSLATERESULT_DIALQUIET: u32 = 128u32;
pub const LINETRANSLATERESULT_INTERNATIONAL: u32 = 2u32;
pub const LINETRANSLATERESULT_INTOLLLIST: u32 = 16u32;
pub const LINETRANSLATERESULT_LOCAL: u32 = 8u32;
pub const LINETRANSLATERESULT_LONGDISTANCE: u32 = 4u32;
pub const LINETRANSLATERESULT_NOTINTOLLLIST: u32 = 32u32;
pub const LINETRANSLATERESULT_NOTRANSLATION: u32 = 2048u32;
pub const LINETRANSLATERESULT_VOICEDETECT: u32 = 1024u32;
pub const LINETSPIOPTION_NONREENTRANT: u32 = 1u32;
pub const LINE_ADDRESSSTATE: i32 = 0i32;
pub const LINE_AGENTSESSIONSTATUS: i32 = 27i32;
pub const LINE_AGENTSPECIFIC: i32 = 21i32;
pub const LINE_AGENTSTATUS: i32 = 22i32;
pub const LINE_AGENTSTATUSEX: i32 = 29i32;
pub const LINE_APPNEWCALL: i32 = 23i32;
pub const LINE_APPNEWCALLHUB: i32 = 32i32;
pub const LINE_CALLHUBCLOSE: i32 = 33i32;
pub const LINE_CALLINFO: i32 = 1i32;
pub const LINE_CALLSTATE: i32 = 2i32;
pub const LINE_CLOSE: i32 = 3i32;
pub const LINE_CREATE: i32 = 19i32;
pub const LINE_DEVSPECIFIC: i32 = 4i32;
pub const LINE_DEVSPECIFICEX: i32 = 34i32;
pub const LINE_DEVSPECIFICFEATURE: i32 = 5i32;
pub const LINE_GATHERDIGITS: i32 = 6i32;
pub const LINE_GENERATE: i32 = 7i32;
pub const LINE_GROUPSTATUS: i32 = 30i32;
pub const LINE_LINEDEVSTATE: i32 = 8i32;
pub const LINE_MONITORDIGITS: i32 = 9i32;
pub const LINE_MONITORMEDIA: i32 = 10i32;
pub const LINE_MONITORTONE: i32 = 11i32;
pub const LINE_PROXYREQUEST: i32 = 24i32;
pub const LINE_PROXYSTATUS: i32 = 31i32;
pub const LINE_QUEUESTATUS: i32 = 28i32;
pub const LINE_REMOVE: i32 = 25i32;
pub const LINE_REPLY: i32 = 12i32;
pub const LINE_REQUEST: i32 = 13i32;
pub const LM_BROKENFLUTTER: PHONE_LAMP_MODE = 64i32;
pub const LM_DUMMY: PHONE_LAMP_MODE = 1i32;
pub const LM_FLASH: PHONE_LAMP_MODE = 16i32;
pub const LM_FLUTTER: PHONE_LAMP_MODE = 32i32;
pub const LM_OFF: PHONE_LAMP_MODE = 2i32;
pub const LM_STEADY: PHONE_LAMP_MODE = 4i32;
pub const LM_UNKNOWN: PHONE_LAMP_MODE = 128i32;
pub const LM_WINK: PHONE_LAMP_MODE = 8i32;
pub const ME_ADDRESS_EVENT: MSP_EVENT = 0i32;
pub const ME_ASR_TERMINAL_EVENT: MSP_EVENT = 4i32;
pub const ME_CALL_EVENT: MSP_EVENT = 1i32;
pub const ME_FILE_TERMINAL_EVENT: MSP_EVENT = 6i32;
pub const ME_PRIVATE_EVENT: MSP_EVENT = 3i32;
pub const ME_TONE_TERMINAL_EVENT: MSP_EVENT = 7i32;
pub const ME_TSP_DATA: MSP_EVENT = 2i32;
pub const ME_TTS_TERMINAL_EVENT: MSP_EVENT = 5i32;
pub const McastAddressAllocation: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xdf0daef2_a289_11d1_8697_006008b0e5d2);
pub const OPENTNEFSTREAM: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("OpenTnefStream");
pub const OPENTNEFSTREAMEX: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("OpenTnefStreamEx");
pub const OT_CONFERENCE: DIRECTORY_OBJECT_TYPE = 1i32;
pub const OT_USER: DIRECTORY_OBJECT_TYPE = 2i32;
pub const PBF_ABBREVDIAL: PHONE_BUTTON_FUNCTION = 11i32;
pub const PBF_BRIDGEDAPP: PHONE_BUTTON_FUNCTION = 28i32;
pub const PBF_BUSY: PHONE_BUTTON_FUNCTION = 29i32;
pub const PBF_CALLAPP: PHONE_BUTTON_FUNCTION = 30i32;
pub const PBF_CALLID: PHONE_BUTTON_FUNCTION = 34i32;
pub const PBF_CAMPON: PHONE_BUTTON_FUNCTION = 43i32;
pub const PBF_CONFERENCE: PHONE_BUTTON_FUNCTION = 1i32;
pub const PBF_CONNECT: PHONE_BUTTON_FUNCTION = 7i32;
pub const PBF_COVER: PHONE_BUTTON_FUNCTION = 33i32;
pub const PBF_DATAOFF: PHONE_BUTTON_FUNCTION = 25i32;
pub const PBF_DATAON: PHONE_BUTTON_FUNCTION = 24i32;
pub const PBF_DATETIME: PHONE_BUTTON_FUNCTION = 31i32;
pub const PBF_DIRECTORY: PHONE_BUTTON_FUNCTION = 32i32;
pub const PBF_DISCONNECT: PHONE_BUTTON_FUNCTION = 6i32;
pub const PBF_DONOTDISTURB: PHONE_BUTTON_FUNCTION = 26i32;
pub const PBF_DROP: PHONE_BUTTON_FUNCTION = 3i32;
pub const PBF_FLASH: PHONE_BUTTON_FUNCTION = 23i32;
pub const PBF_FORWARD: PHONE_BUTTON_FUNCTION = 12i32;
pub const PBF_HOLD: PHONE_BUTTON_FUNCTION = 4i32;
pub const PBF_INTERCOM: PHONE_BUTTON_FUNCTION = 27i32;
pub const PBF_LASTNUM: PHONE_BUTTON_FUNCTION = 35i32;
pub const PBF_MSGINDICATOR: PHONE_BUTTON_FUNCTION = 38i32;
pub const PBF_MSGWAITOFF: PHONE_BUTTON_FUNCTION = 9i32;
pub const PBF_MSGWAITON: PHONE_BUTTON_FUNCTION = 8i32;
pub const PBF_MUTE: PHONE_BUTTON_FUNCTION = 18i32;
pub const PBF_NIGHTSRV: PHONE_BUTTON_FUNCTION = 36i32;
pub const PBF_NONE: PHONE_BUTTON_FUNCTION = 46i32;
pub const PBF_PARK: PHONE_BUTTON_FUNCTION = 15i32;
pub const PBF_PICKUP: PHONE_BUTTON_FUNCTION = 13i32;
pub const PBF_QUEUECALL: PHONE_BUTTON_FUNCTION = 45i32;
pub const PBF_RECALL: PHONE_BUTTON_FUNCTION = 5i32;
pub const PBF_REDIRECT: PHONE_BUTTON_FUNCTION = 17i32;
pub const PBF_REJECT: PHONE_BUTTON_FUNCTION = 16i32;
pub const PBF_REPDIAL: PHONE_BUTTON_FUNCTION = 39i32;
pub const PBF_RINGAGAIN: PHONE_BUTTON_FUNCTION = 14i32;
pub const PBF_SAVEREPEAT: PHONE_BUTTON_FUNCTION = 44i32;
pub const PBF_SELECTRING: PHONE_BUTTON_FUNCTION = 10i32;
pub const PBF_SEND: PHONE_BUTTON_FUNCTION = 47i32;
pub const PBF_SENDCALLS: PHONE_BUTTON_FUNCTION = 37i32;
pub const PBF_SETREPDIAL: PHONE_BUTTON_FUNCTION = 40i32;
pub const PBF_SPEAKEROFF: PHONE_BUTTON_FUNCTION = 22i32;
pub const PBF_SPEAKERON: PHONE_BUTTON_FUNCTION = 21i32;
pub const PBF_STATIONSPEED: PHONE_BUTTON_FUNCTION = 42i32;
pub const PBF_SYSTEMSPEED: PHONE_BUTTON_FUNCTION = 41i32;
pub const PBF_TRANSFER: PHONE_BUTTON_FUNCTION = 2i32;
pub const PBF_UNKNOWN: PHONE_BUTTON_FUNCTION = 0i32;
pub const PBF_VOLUMEDOWN: PHONE_BUTTON_FUNCTION = 20i32;
pub const PBF_VOLUMEUP: PHONE_BUTTON_FUNCTION = 19i32;
pub const PBM_CALL: PHONE_BUTTON_MODE = 1i32;
pub const PBM_DISPLAY: PHONE_BUTTON_MODE = 5i32;
pub const PBM_DUMMY: PHONE_BUTTON_MODE = 0i32;
pub const PBM_FEATURE: PHONE_BUTTON_MODE = 2i32;
pub const PBM_KEYPAD: PHONE_BUTTON_MODE = 3i32;
pub const PBM_LOCAL: PHONE_BUTTON_MODE = 4i32;
pub const PBS_DOWN: PHONE_BUTTON_STATE = 2i32;
pub const PBS_UNAVAIL: PHONE_BUTTON_STATE = 8i32;
pub const PBS_UNKNOWN: PHONE_BUTTON_STATE = 4i32;
pub const PBS_UP: PHONE_BUTTON_STATE = 1i32;
pub const PCB_DEVSPECIFICBUFFER: PHONECAPS_BUFFER = 0i32;
pub const PCL_DISPLAYNUMCOLUMNS: PHONECAPS_LONG = 5i32;
pub const PCL_DISPLAYNUMROWS: PHONECAPS_LONG = 4i32;
pub const PCL_GENERICPHONE: PHONECAPS_LONG = 8i32;
pub const PCL_HANDSETHOOKSWITCHMODES: PHONECAPS_LONG = 1i32;
pub const PCL_HEADSETHOOKSWITCHMODES: PHONECAPS_LONG = 2i32;
pub const PCL_HOOKSWITCHES: PHONECAPS_LONG = 0i32;
pub const PCL_NUMBUTTONLAMPS: PHONECAPS_LONG = 7i32;
pub const PCL_NUMRINGMODES: PHONECAPS_LONG = 6i32;
pub const PCL_SPEAKERPHONEHOOKSWITCHMODES: PHONECAPS_LONG = 3i32;
pub const PCS_PHONEINFO: PHONECAPS_STRING = 1i32;
pub const PCS_PHONENAME: PHONECAPS_STRING = 0i32;
pub const PCS_PROVIDERINFO: PHONECAPS_STRING = 2i32;
pub const PE_ANSWER: PHONE_EVENT = 10i32;
pub const PE_BUTTON: PHONE_EVENT = 6i32;
pub const PE_CAPSCHANGE: PHONE_EVENT = 5i32;
pub const PE_CLOSE: PHONE_EVENT = 7i32;
pub const PE_DIALING: PHONE_EVENT = 9i32;
pub const PE_DISCONNECT: PHONE_EVENT = 11i32;
pub const PE_DISPLAY: PHONE_EVENT = 0i32;
pub const PE_HOOKSWITCH: PHONE_EVENT = 4i32;
pub const PE_LAMPMODE: PHONE_EVENT = 1i32;
pub const PE_LASTITEM: PHONE_EVENT = 11i32;
pub const PE_NUMBERGATHERED: PHONE_EVENT = 8i32;
pub const PE_RINGMODE: PHONE_EVENT = 2i32;
pub const PE_RINGVOLUME: PHONE_EVENT = 3i32;
pub const PHONEBUTTONFUNCTION_ABBREVDIAL: u32 = 11u32;
pub const PHONEBUTTONFUNCTION_BRIDGEDAPP: u32 = 28u32;
pub const PHONEBUTTONFUNCTION_BUSY: u32 = 29u32;
pub const PHONEBUTTONFUNCTION_CALLAPP: u32 = 30u32;
pub const PHONEBUTTONFUNCTION_CALLID: u32 = 34u32;
pub const PHONEBUTTONFUNCTION_CAMPON: u32 = 43u32;
pub const PHONEBUTTONFUNCTION_CONFERENCE: u32 = 1u32;
pub const PHONEBUTTONFUNCTION_CONNECT: u32 = 7u32;
pub const PHONEBUTTONFUNCTION_COVER: u32 = 33u32;
pub const PHONEBUTTONFUNCTION_DATAOFF: u32 = 25u32;
pub const PHONEBUTTONFUNCTION_DATAON: u32 = 24u32;
pub const PHONEBUTTONFUNCTION_DATETIME: u32 = 31u32;
pub const PHONEBUTTONFUNCTION_DIRECTORY: u32 = 32u32;
pub const PHONEBUTTONFUNCTION_DISCONNECT: u32 = 6u32;
pub const PHONEBUTTONFUNCTION_DONOTDISTURB: u32 = 26u32;
pub const PHONEBUTTONFUNCTION_DROP: u32 = 3u32;
pub const PHONEBUTTONFUNCTION_FLASH: u32 = 23u32;
pub const PHONEBUTTONFUNCTION_FORWARD: u32 = 12u32;
pub const PHONEBUTTONFUNCTION_HOLD: u32 = 4u32;
pub const PHONEBUTTONFUNCTION_INTERCOM: u32 = 27u32;
pub const PHONEBUTTONFUNCTION_LASTNUM: u32 = 35u32;
pub const PHONEBUTTONFUNCTION_MSGINDICATOR: u32 = 38u32;
pub const PHONEBUTTONFUNCTION_MSGWAITOFF: u32 = 9u32;
pub const PHONEBUTTONFUNCTION_MSGWAITON: u32 = 8u32;
pub const PHONEBUTTONFUNCTION_MUTE: u32 = 18u32;
pub const PHONEBUTTONFUNCTION_NIGHTSRV: u32 = 36u32;
pub const PHONEBUTTONFUNCTION_NONE: u32 = 46u32;
pub const PHONEBUTTONFUNCTION_PARK: u32 = 15u32;
pub const PHONEBUTTONFUNCTION_PICKUP: u32 = 13u32;
pub const PHONEBUTTONFUNCTION_QUEUECALL: u32 = 45u32;
pub const PHONEBUTTONFUNCTION_RECALL: u32 = 5u32;
pub const PHONEBUTTONFUNCTION_REDIRECT: u32 = 17u32;
pub const PHONEBUTTONFUNCTION_REJECT: u32 = 16u32;
pub const PHONEBUTTONFUNCTION_REPDIAL: u32 = 39u32;
pub const PHONEBUTTONFUNCTION_RINGAGAIN: u32 = 14u32;
pub const PHONEBUTTONFUNCTION_SAVEREPEAT: u32 = 44u32;
pub const PHONEBUTTONFUNCTION_SELECTRING: u32 = 10u32;
pub const PHONEBUTTONFUNCTION_SEND: u32 = 47u32;
pub const PHONEBUTTONFUNCTION_SENDCALLS: u32 = 37u32;
pub const PHONEBUTTONFUNCTION_SETREPDIAL: u32 = 40u32;
pub const PHONEBUTTONFUNCTION_SPEAKEROFF: u32 = 22u32;
pub const PHONEBUTTONFUNCTION_SPEAKERON: u32 = 21u32;
pub const PHONEBUTTONFUNCTION_STATIONSPEED: u32 = 42u32;
pub const PHONEBUTTONFUNCTION_SYSTEMSPEED: u32 = 41u32;
pub const PHONEBUTTONFUNCTION_TRANSFER: u32 = 2u32;
pub const PHONEBUTTONFUNCTION_UNKNOWN: u32 = 0u32;
pub const PHONEBUTTONFUNCTION_VOLUMEDOWN: u32 = 20u32;
pub const PHONEBUTTONFUNCTION_VOLUMEUP: u32 = 19u32;
pub const PHONEBUTTONMODE_CALL: u32 = 2u32;
pub const PHONEBUTTONMODE_DISPLAY: u32 = 32u32;
pub const PHONEBUTTONMODE_DUMMY: u32 = 1u32;
pub const PHONEBUTTONMODE_FEATURE: u32 = 4u32;
pub const PHONEBUTTONMODE_KEYPAD: u32 = 8u32;
pub const PHONEBUTTONMODE_LOCAL: u32 = 16u32;
pub const PHONEBUTTONSTATE_DOWN: u32 = 2u32;
pub const PHONEBUTTONSTATE_UNAVAIL: u32 = 8u32;
pub const PHONEBUTTONSTATE_UNKNOWN: u32 = 4u32;
pub const PHONEBUTTONSTATE_UP: u32 = 1u32;
pub const PHONEERR_ALLOCATED: u32 = 2415919105u32;
pub const PHONEERR_BADDEVICEID: u32 = 2415919106u32;
pub const PHONEERR_DISCONNECTED: u32 = 2415919140u32;
pub const PHONEERR_INCOMPATIBLEAPIVERSION: u32 = 2415919107u32;
pub const PHONEERR_INCOMPATIBLEEXTVERSION: u32 = 2415919108u32;
pub const PHONEERR_INIFILECORRUPT: u32 = 2415919109u32;
pub const PHONEERR_INUSE: u32 = 2415919110u32;
pub const PHONEERR_INVALAPPHANDLE: u32 = 2415919111u32;
pub const PHONEERR_INVALAPPNAME: u32 = 2415919112u32;
pub const PHONEERR_INVALBUTTONLAMPID: u32 = 2415919113u32;
pub const PHONEERR_INVALBUTTONMODE: u32 = 2415919114u32;
pub const PHONEERR_INVALBUTTONSTATE: u32 = 2415919115u32;
pub const PHONEERR_INVALDATAID: u32 = 2415919116u32;
pub const PHONEERR_INVALDEVICECLASS: u32 = 2415919117u32;
pub const PHONEERR_INVALEXTVERSION: u32 = 2415919118u32;
pub const PHONEERR_INVALHOOKSWITCHDEV: u32 = 2415919119u32;
pub const PHONEERR_INVALHOOKSWITCHMODE: u32 = 2415919120u32;
pub const PHONEERR_INVALLAMPMODE: u32 = 2415919121u32;
pub const PHONEERR_INVALPARAM: u32 = 2415919122u32;
pub const PHONEERR_INVALPHONEHANDLE: u32 = 2415919123u32;
pub const PHONEERR_INVALPHONESTATE: u32 = 2415919124u32;
pub const PHONEERR_INVALPOINTER: u32 = 2415919125u32;
pub const PHONEERR_INVALPRIVILEGE: u32 = 2415919126u32;
pub const PHONEERR_INVALRINGMODE: u32 = 2415919127u32;
pub const PHONEERR_NODEVICE: u32 = 2415919128u32;
pub const PHONEERR_NODRIVER: u32 = 2415919129u32;
pub const PHONEERR_NOMEM: u32 = 2415919130u32;
pub const PHONEERR_NOTOWNER: u32 = 2415919131u32;
pub const PHONEERR_OPERATIONFAILED: u32 = 2415919132u32;
pub const PHONEERR_OPERATIONUNAVAIL: u32 = 2415919133u32;
pub const PHONEERR_REINIT: u32 = 2415919139u32;
pub const PHONEERR_REQUESTOVERRUN: u32 = 2415919136u32;
pub const PHONEERR_RESOURCEUNAVAIL: u32 = 2415919135u32;
pub const PHONEERR_SERVICE_NOT_RUNNING: u32 = 2415919141u32;
pub const PHONEERR_STRUCTURETOOSMALL: u32 = 2415919137u32;
pub const PHONEERR_UNINITIALIZED: u32 = 2415919138u32;
pub const PHONEFEATURE_GENERICPHONE: u32 = 268435456u32;
pub const PHONEFEATURE_GETBUTTONINFO: u32 = 1u32;
pub const PHONEFEATURE_GETDATA: u32 = 2u32;
pub const PHONEFEATURE_GETDISPLAY: u32 = 4u32;
pub const PHONEFEATURE_GETGAINHANDSET: u32 = 8u32;
pub const PHONEFEATURE_GETGAINHEADSET: u32 = 32u32;
pub const PHONEFEATURE_GETGAINSPEAKER: u32 = 16u32;
pub const PHONEFEATURE_GETHOOKSWITCHHANDSET: u32 = 64u32;
pub const PHONEFEATURE_GETHOOKSWITCHHEADSET: u32 = 256u32;
pub const PHONEFEATURE_GETHOOKSWITCHSPEAKER: u32 = 128u32;
pub const PHONEFEATURE_GETLAMP: u32 = 512u32;
pub const PHONEFEATURE_GETRING: u32 = 1024u32;
pub const PHONEFEATURE_GETVOLUMEHANDSET: u32 = 2048u32;
pub const PHONEFEATURE_GETVOLUMEHEADSET: u32 = 8192u32;
pub const PHONEFEATURE_GETVOLUMESPEAKER: u32 = 4096u32;
pub const PHONEFEATURE_SETBUTTONINFO: u32 = 16384u32;
pub const PHONEFEATURE_SETDATA: u32 = 32768u32;
pub const PHONEFEATURE_SETDISPLAY: u32 = 65536u32;
pub const PHONEFEATURE_SETGAINHANDSET: u32 = 131072u32;
pub const PHONEFEATURE_SETGAINHEADSET: u32 = 524288u32;
pub const PHONEFEATURE_SETGAINSPEAKER: u32 = 262144u32;
pub const PHONEFEATURE_SETHOOKSWITCHHANDSET: u32 = 1048576u32;
pub const PHONEFEATURE_SETHOOKSWITCHHEADSET: u32 = 4194304u32;
pub const PHONEFEATURE_SETHOOKSWITCHSPEAKER: u32 = 2097152u32;
pub const PHONEFEATURE_SETLAMP: u32 = 8388608u32;
pub const PHONEFEATURE_SETRING: u32 = 16777216u32;
pub const PHONEFEATURE_SETVOLUMEHANDSET: u32 = 33554432u32;
pub const PHONEFEATURE_SETVOLUMEHEADSET: u32 = 134217728u32;
pub const PHONEFEATURE_SETVOLUMESPEAKER: u32 = 67108864u32;
pub const PHONEHOOKSWITCHDEV_HANDSET: u32 = 1u32;
pub const PHONEHOOKSWITCHDEV_HEADSET: u32 = 4u32;
pub const PHONEHOOKSWITCHDEV_SPEAKER: u32 = 2u32;
pub const PHONEHOOKSWITCHMODE_MIC: u32 = 2u32;
pub const PHONEHOOKSWITCHMODE_MICSPEAKER: u32 = 8u32;
pub const PHONEHOOKSWITCHMODE_ONHOOK: u32 = 1u32;
pub const PHONEHOOKSWITCHMODE_SPEAKER: u32 = 4u32;
pub const PHONEHOOKSWITCHMODE_UNKNOWN: u32 = 16u32;
pub const PHONEINITIALIZEEXOPTION_USECOMPLETIONPORT: u32 = 3u32;
pub const PHONEINITIALIZEEXOPTION_USEEVENT: u32 = 2u32;
pub const PHONEINITIALIZEEXOPTION_USEHIDDENWINDOW: u32 = 1u32;
pub const PHONELAMPMODE_BROKENFLUTTER: u32 = 64u32;
pub const PHONELAMPMODE_DUMMY: u32 = 1u32;
pub const PHONELAMPMODE_FLASH: u32 = 16u32;
pub const PHONELAMPMODE_FLUTTER: u32 = 32u32;
pub const PHONELAMPMODE_OFF: u32 = 2u32;
pub const PHONELAMPMODE_STEADY: u32 = 4u32;
pub const PHONELAMPMODE_UNKNOWN: u32 = 128u32;
pub const PHONELAMPMODE_WINK: u32 = 8u32;
pub const PHONEPRIVILEGE_MONITOR: u32 = 1u32;
pub const PHONEPRIVILEGE_OWNER: u32 = 2u32;
pub const PHONESTATE_CAPSCHANGE: u32 = 4194304u32;
pub const PHONESTATE_CONNECTED: u32 = 2u32;
pub const PHONESTATE_DEVSPECIFIC: u32 = 1048576u32;
pub const PHONESTATE_DISCONNECTED: u32 = 4u32;
pub const PHONESTATE_DISPLAY: u32 = 32u32;
pub const PHONESTATE_HANDSETGAIN: u32 = 2048u32;
pub const PHONESTATE_HANDSETHOOKSWITCH: u32 = 512u32;
pub const PHONESTATE_HANDSETVOLUME: u32 = 1024u32;
pub const PHONESTATE_HEADSETGAIN: u32 = 131072u32;
pub const PHONESTATE_HEADSETHOOKSWITCH: u32 = 32768u32;
pub const PHONESTATE_HEADSETVOLUME: u32 = 65536u32;
pub const PHONESTATE_LAMP: u32 = 64u32;
pub const PHONESTATE_MONITORS: u32 = 16u32;
pub const PHONESTATE_OTHER: u32 = 1u32;
pub const PHONESTATE_OWNER: u32 = 8u32;
pub const PHONESTATE_REINIT: u32 = 2097152u32;
pub const PHONESTATE_REMOVED: u32 = 8388608u32;
pub const PHONESTATE_RESUME: u32 = 524288u32;
pub const PHONESTATE_RINGMODE: u32 = 128u32;
pub const PHONESTATE_RINGVOLUME: u32 = 256u32;
pub const PHONESTATE_SPEAKERGAIN: u32 = 16384u32;
pub const PHONESTATE_SPEAKERHOOKSWITCH: u32 = 4096u32;
pub const PHONESTATE_SPEAKERVOLUME: u32 = 8192u32;
pub const PHONESTATE_SUSPEND: u32 = 262144u32;
pub const PHONESTATUSFLAGS_CONNECTED: u32 = 1u32;
pub const PHONESTATUSFLAGS_SUSPENDED: u32 = 2u32;
pub const PHONE_BUTTON: i32 = 14i32;
pub const PHONE_CLOSE: i32 = 15i32;
pub const PHONE_CREATE: i32 = 20i32;
pub const PHONE_DEVSPECIFIC: i32 = 16i32;
pub const PHONE_REMOVE: i32 = 26i32;
pub const PHONE_REPLY: i32 = 17i32;
pub const PHONE_STATE: i32 = 18i32;
pub const PHSD_HANDSET: PHONE_HOOK_SWITCH_DEVICE = 1i32;
pub const PHSD_HEADSET: PHONE_HOOK_SWITCH_DEVICE = 4i32;
pub const PHSD_SPEAKERPHONE: PHONE_HOOK_SWITCH_DEVICE = 2i32;
pub const PHSS_OFFHOOK: PHONE_HOOK_SWITCH_STATE = 8i32;
pub const PHSS_OFFHOOK_MIC_ONLY: PHONE_HOOK_SWITCH_STATE = 2i32;
pub const PHSS_OFFHOOK_SPEAKER_ONLY: PHONE_HOOK_SWITCH_STATE = 4i32;
pub const PHSS_ONHOOK: PHONE_HOOK_SWITCH_STATE = 1i32;
pub const PP_MONITOR: PHONE_PRIVILEGE = 1i32;
pub const PP_OWNER: PHONE_PRIVILEGE = 0i32;
pub const PRIVATEOBJECT_ADDRESS: u32 = 6u32;
pub const PRIVATEOBJECT_CALL: u32 = 4u32;
pub const PRIVATEOBJECT_CALLID: u32 = 2u32;
pub const PRIVATEOBJECT_LINE: u32 = 3u32;
pub const PRIVATEOBJECT_NONE: u32 = 1u32;
pub const PRIVATEOBJECT_PHONE: u32 = 5u32;
pub const PT_BUSY: PHONE_TONE = 18i32;
pub const PT_ERRORTONE: PHONE_TONE = 20i32;
pub const PT_EXTERNALDIALTONE: PHONE_TONE = 17i32;
pub const PT_KEYPADA: PHONE_TONE = 12i32;
pub const PT_KEYPADB: PHONE_TONE = 13i32;
pub const PT_KEYPADC: PHONE_TONE = 14i32;
pub const PT_KEYPADD: PHONE_TONE = 15i32;
pub const PT_KEYPADEIGHT: PHONE_TONE = 8i32;
pub const PT_KEYPADFIVE: PHONE_TONE = 5i32;
pub const PT_KEYPADFOUR: PHONE_TONE = 4i32;
pub const PT_KEYPADNINE: PHONE_TONE = 9i32;
pub const PT_KEYPADONE: PHONE_TONE = 1i32;
pub const PT_KEYPADPOUND: PHONE_TONE = 11i32;
pub const PT_KEYPADSEVEN: PHONE_TONE = 7i32;
pub const PT_KEYPADSIX: PHONE_TONE = 6i32;
pub const PT_KEYPADSTAR: PHONE_TONE = 10i32;
pub const PT_KEYPADTHREE: PHONE_TONE = 3i32;
pub const PT_KEYPADTWO: PHONE_TONE = 2i32;
pub const PT_KEYPADZERO: PHONE_TONE = 0i32;
pub const PT_NORMALDIALTONE: PHONE_TONE = 16i32;
pub const PT_RINGBACK: PHONE_TONE = 19i32;
pub const PT_SILENCE: PHONE_TONE = 21i32;
pub const QE_ADMISSIONFAILURE: QOS_EVENT = 2i32;
pub const QE_GENERICERROR: QOS_EVENT = 4i32;
pub const QE_LASTITEM: QOS_EVENT = 4i32;
pub const QE_NOQOS: QOS_EVENT = 1i32;
pub const QE_POLICYFAILURE: QOS_EVENT = 3i32;
pub const QSL_BEST_EFFORT: QOS_SERVICE_LEVEL = 3i32;
pub const QSL_IF_AVAILABLE: QOS_SERVICE_LEVEL = 2i32;
pub const QSL_NEEDED: QOS_SERVICE_LEVEL = 1i32;
pub const RAS_LOCAL: RND_ADVERTISING_SCOPE = 1i32;
pub const RAS_REGION: RND_ADVERTISING_SCOPE = 3i32;
pub const RAS_SITE: RND_ADVERTISING_SCOPE = 2i32;
pub const RAS_WORLD: RND_ADVERTISING_SCOPE = 4i32;
pub const RENDBIND_AUTHENTICATE: u32 = 1u32;
pub const RENDBIND_DEFAULTCREDENTIALS: u32 = 14u32;
pub const RENDBIND_DEFAULTDOMAINNAME: u32 = 2u32;
pub const RENDBIND_DEFAULTPASSWORD: u32 = 8u32;
pub const RENDBIND_DEFAULTUSERNAME: u32 = 4u32;
pub const Rendezvous: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf1029e5b_cb5b_11d0_8d59_00c04fd91ac0);
pub const RequestMakeCall: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xac48ffe0_f8c4_11d1_a030_00c04fb6809f);
pub const STRINGFORMAT_ASCII: u32 = 1u32;
pub const STRINGFORMAT_BINARY: u32 = 4u32;
pub const STRINGFORMAT_DBCS: u32 = 2u32;
pub const STRINGFORMAT_UNICODE: u32 = 3u32;
pub const STRM_CONFIGURED: u32 = 2u32;
pub const STRM_INITIAL: u32 = 0u32;
pub const STRM_PAUSED: u32 = 8u32;
pub const STRM_RUNNING: u32 = 4u32;
pub const STRM_STOPPED: u32 = 16u32;
pub const STRM_TERMINALSELECTED: u32 = 1u32;
pub const TAPI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x21d6d48e_a88b_11d0_83dd_00aa003ccabd);
pub const TAPIERR_CONNECTED: i32 = 0i32;
pub const TAPIERR_DESTBUSY: i32 = -11i32;
pub const TAPIERR_DESTNOANSWER: i32 = -12i32;
pub const TAPIERR_DESTUNAVAIL: i32 = -13i32;
pub const TAPIERR_DEVICECLASSUNAVAIL: i32 = -8i32;
pub const TAPIERR_DEVICEIDUNAVAIL: i32 = -9i32;
pub const TAPIERR_DEVICEINUSE: i32 = -10i32;
pub const TAPIERR_DROPPED: i32 = -1i32;
pub const TAPIERR_INVALDESTADDRESS: i32 = -4i32;
pub const TAPIERR_INVALDEVICECLASS: i32 = -6i32;
pub const TAPIERR_INVALDEVICEID: i32 = -7i32;
pub const TAPIERR_INVALPOINTER: i32 = -18i32;
pub const TAPIERR_INVALWINDOWHANDLE: i32 = -5i32;
pub const TAPIERR_MMCWRITELOCKED: i32 = -20i32;
pub const TAPIERR_NOREQUESTRECIPIENT: i32 = -2i32;
pub const TAPIERR_NOTADMIN: i32 = -19i32;
pub const TAPIERR_PROVIDERALREADYINSTALLED: i32 = -21i32;
pub const TAPIERR_REQUESTCANCELLED: i32 = -17i32;
pub const TAPIERR_REQUESTFAILED: i32 = -16i32;
pub const TAPIERR_REQUESTQUEUEFULL: i32 = -3i32;
pub const TAPIERR_SCP_ALREADY_EXISTS: i32 = -22i32;
pub const TAPIERR_SCP_DOES_NOT_EXIST: i32 = -23i32;
pub const TAPIERR_UNKNOWNREQUESTID: i32 = -15i32;
pub const TAPIERR_UNKNOWNWINHANDLE: i32 = -14i32;
pub const TAPIMAXAPPNAMESIZE: i32 = 40i32;
pub const TAPIMAXCALLEDPARTYSIZE: i32 = 40i32;
pub const TAPIMAXCOMMENTSIZE: i32 = 80i32;
pub const TAPIMAXDESTADDRESSSIZE: i32 = 80i32;
pub const TAPIMAXDEVICECLASSSIZE: i32 = 40i32;
pub const TAPIMAXDEVICEIDSIZE: i32 = 40i32;
pub const TAPIMEDIATYPE_AUDIO: u32 = 8u32;
pub const TAPIMEDIATYPE_DATAMODEM: u32 = 16u32;
pub const TAPIMEDIATYPE_G3FAX: u32 = 32u32;
pub const TAPIMEDIATYPE_MULTITRACK: u32 = 65536u32;
pub const TAPIMEDIATYPE_VIDEO: u32 = 32768u32;
pub const TAPI_CURRENT_VERSION: u32 = 131074u32;
pub const TAPI_E_ADDRESSBLOCKED: ::windows_sys::core::HRESULT = -2147221462i32;
pub const TAPI_E_ALLOCATED: ::windows_sys::core::HRESULT = -2147221498i32;
pub const TAPI_E_BILLINGREJECTED: ::windows_sys::core::HRESULT = -2147221461i32;
pub const TAPI_E_CALLCENTER_GROUP_REMOVED: ::windows_sys::core::HRESULT = -2147221435i32;
pub const TAPI_E_CALLCENTER_INVALAGENTACTIVITY: ::windows_sys::core::HRESULT = -2147221428i32;
pub const TAPI_E_CALLCENTER_INVALAGENTGROUP: ::windows_sys::core::HRESULT = -2147221431i32;
pub const TAPI_E_CALLCENTER_INVALAGENTID: ::windows_sys::core::HRESULT = -2147221432i32;
pub const TAPI_E_CALLCENTER_INVALAGENTSTATE: ::windows_sys::core::HRESULT = -2147221429i32;
pub const TAPI_E_CALLCENTER_INVALPASSWORD: ::windows_sys::core::HRESULT = -2147221430i32;
pub const TAPI_E_CALLCENTER_NO_AGENT_ID: ::windows_sys::core::HRESULT = -2147221433i32;
pub const TAPI_E_CALLCENTER_QUEUE_REMOVED: ::windows_sys::core::HRESULT = -2147221434i32;
pub const TAPI_E_CALLNOTSELECTED: ::windows_sys::core::HRESULT = -2147221420i32;
pub const TAPI_E_CALLUNAVAIL: ::windows_sys::core::HRESULT = -2147221497i32;
pub const TAPI_E_COMPLETIONOVERRUN: ::windows_sys::core::HRESULT = -2147221496i32;
pub const TAPI_E_CONFERENCEFULL: ::windows_sys::core::HRESULT = -2147221495i32;
pub const TAPI_E_DESTBUSY: ::windows_sys::core::HRESULT = -2147221452i32;
pub const TAPI_E_DESTNOANSWER: ::windows_sys::core::HRESULT = -2147221451i32;
pub const TAPI_E_DESTUNAVAIL: ::windows_sys::core::HRESULT = -2147221450i32;
pub const TAPI_E_DIALMODIFIERNOTSUPPORTED: ::windows_sys::core::HRESULT = -2147221494i32;
pub const TAPI_E_DROPPED: ::windows_sys::core::HRESULT = -2147221455i32;
pub const TAPI_E_INUSE: ::windows_sys::core::HRESULT = -2147221493i32;
pub const TAPI_E_INVALADDRESS: ::windows_sys::core::HRESULT = -2147221492i32;
pub const TAPI_E_INVALADDRESSSTATE: ::windows_sys::core::HRESULT = -2147221491i32;
pub const TAPI_E_INVALADDRESSTYPE: ::windows_sys::core::HRESULT = -2147221423i32;
pub const TAPI_E_INVALBUTTONLAMPID: ::windows_sys::core::HRESULT = -2147221459i32;
pub const TAPI_E_INVALBUTTONSTATE: ::windows_sys::core::HRESULT = -2147221458i32;
pub const TAPI_E_INVALCALLPARAMS: ::windows_sys::core::HRESULT = -2147221490i32;
pub const TAPI_E_INVALCALLPRIVILEGE: ::windows_sys::core::HRESULT = -2147221489i32;
pub const TAPI_E_INVALCALLSTATE: ::windows_sys::core::HRESULT = -2147221488i32;
pub const TAPI_E_INVALCARD: ::windows_sys::core::HRESULT = -2147221487i32;
pub const TAPI_E_INVALCOMPLETIONID: ::windows_sys::core::HRESULT = -2147221486i32;
pub const TAPI_E_INVALCOUNTRYCODE: ::windows_sys::core::HRESULT = -2147221485i32;
pub const TAPI_E_INVALDATAID: ::windows_sys::core::HRESULT = -2147221457i32;
pub const TAPI_E_INVALDEVICECLASS: ::windows_sys::core::HRESULT = -2147221484i32;
pub const TAPI_E_INVALDIALPARAMS: ::windows_sys::core::HRESULT = -2147221483i32;
pub const TAPI_E_INVALDIGITS: ::windows_sys::core::HRESULT = -2147221482i32;
pub const TAPI_E_INVALFEATURE: ::windows_sys::core::HRESULT = -2147221460i32;
pub const TAPI_E_INVALGROUPID: ::windows_sys::core::HRESULT = -2147221481i32;
pub const TAPI_E_INVALHOOKSWITCHDEV: ::windows_sys::core::HRESULT = -2147221456i32;
pub const TAPI_E_INVALIDDIRECTION: ::windows_sys::core::HRESULT = -2147221446i32;
pub const TAPI_E_INVALIDMEDIATYPE: ::windows_sys::core::HRESULT = -2147221500i32;
pub const TAPI_E_INVALIDSTREAM: ::windows_sys::core::HRESULT = -2147221437i32;
pub const TAPI_E_INVALIDSTREAMSTATE: ::windows_sys::core::HRESULT = -2147221417i32;
pub const TAPI_E_INVALIDTERMINAL: ::windows_sys::core::HRESULT = -2147221445i32;
pub const TAPI_E_INVALIDTERMINALCLASS: ::windows_sys::core::HRESULT = -2147221444i32;
pub const TAPI_E_INVALLIST: ::windows_sys::core::HRESULT = -2147221474i32;
pub const TAPI_E_INVALLOCATION: ::windows_sys::core::HRESULT = -2147221480i32;
pub const TAPI_E_INVALMESSAGEID: ::windows_sys::core::HRESULT = -2147221479i32;
pub const TAPI_E_INVALMODE: ::windows_sys::core::HRESULT = -2147221473i32;
pub const TAPI_E_INVALPARKID: ::windows_sys::core::HRESULT = -2147221478i32;
pub const TAPI_E_INVALPRIVILEGE: ::windows_sys::core::HRESULT = -2147221447i32;
pub const TAPI_E_INVALRATE: ::windows_sys::core::HRESULT = -2147221477i32;
pub const TAPI_E_INVALTIMEOUT: ::windows_sys::core::HRESULT = -2147221476i32;
pub const TAPI_E_INVALTONE: ::windows_sys::core::HRESULT = -2147221475i32;
pub const TAPI_E_MAXSTREAMS: ::windows_sys::core::HRESULT = -2147221442i32;
pub const TAPI_E_MAXTERMINALS: ::windows_sys::core::HRESULT = -2147221438i32;
pub const TAPI_E_NOCONFERENCE: ::windows_sys::core::HRESULT = -2147221472i32;
pub const TAPI_E_NODEVICE: ::windows_sys::core::HRESULT = -2147221471i32;
pub const TAPI_E_NODRIVER: ::windows_sys::core::HRESULT = -2147221443i32;
pub const TAPI_E_NOEVENT: ::windows_sys::core::HRESULT = -2147221424i32;
pub const TAPI_E_NOFORMAT: ::windows_sys::core::HRESULT = -2147221418i32;
pub const TAPI_E_NOITEMS: ::windows_sys::core::HRESULT = -2147221502i32;
pub const TAPI_E_NOREQUEST: ::windows_sys::core::HRESULT = -2147221470i32;
pub const TAPI_E_NOREQUESTRECIPIENT: ::windows_sys::core::HRESULT = -2147221454i32;
pub const TAPI_E_NOTENOUGHMEMORY: ::windows_sys::core::HRESULT = -2147221503i32;
pub const TAPI_E_NOTERMINALSELECTED: ::windows_sys::core::HRESULT = -2147221441i32;
pub const TAPI_E_NOTOWNER: ::windows_sys::core::HRESULT = -2147221469i32;
pub const TAPI_E_NOTREGISTERED: ::windows_sys::core::HRESULT = -2147221468i32;
pub const TAPI_E_NOTSTOPPED: ::windows_sys::core::HRESULT = -2147221439i32;
pub const TAPI_E_NOTSUPPORTED: ::windows_sys::core::HRESULT = -2147221501i32;
pub const TAPI_E_NOT_INITIALIZED: ::windows_sys::core::HRESULT = -2147221415i32;
pub const TAPI_E_OPERATIONFAILED: ::windows_sys::core::HRESULT = -2147221499i32;
pub const TAPI_E_PEER_NOT_SET: ::windows_sys::core::HRESULT = -2147221425i32;
pub const TAPI_E_PHONENOTOPEN: ::windows_sys::core::HRESULT = -2147221421i32;
pub const TAPI_E_REGISTRY_SETTING_CORRUPT: ::windows_sys::core::HRESULT = -2147221427i32;
pub const TAPI_E_REINIT: ::windows_sys::core::HRESULT = -2147221463i32;
pub const TAPI_E_REQUESTCANCELLED: ::windows_sys::core::HRESULT = -2147221448i32;
pub const TAPI_E_REQUESTFAILED: ::windows_sys::core::HRESULT = -2147221449i32;
pub const TAPI_E_REQUESTOVERRUN: ::windows_sys::core::HRESULT = -2147221467i32;
pub const TAPI_E_REQUESTQUEUEFULL: ::windows_sys::core::HRESULT = -2147221453i32;
pub const TAPI_E_RESOURCEUNAVAIL: ::windows_sys::core::HRESULT = -2147221422i32;
pub const TAPI_E_SERVICE_NOT_RUNNING: ::windows_sys::core::HRESULT = -2147221414i32;
pub const TAPI_E_TARGETNOTFOUND: ::windows_sys::core::HRESULT = -2147221466i32;
pub const TAPI_E_TARGETSELF: ::windows_sys::core::HRESULT = -2147221465i32;
pub const TAPI_E_TERMINALINUSE: ::windows_sys::core::HRESULT = -2147221440i32;
pub const TAPI_E_TERMINAL_PEER: ::windows_sys::core::HRESULT = -2147221426i32;
pub const TAPI_E_TIMEOUT: ::windows_sys::core::HRESULT = -2147221436i32;
pub const TAPI_E_USERUSERINFOTOOBIG: ::windows_sys::core::HRESULT = -2147221464i32;
pub const TAPI_E_WRONGEVENT: ::windows_sys::core::HRESULT = -2147221419i32;
pub const TAPI_E_WRONG_STATE: ::windows_sys::core::HRESULT = -2147221416i32;
pub const TAPI_REPLY: u32 = 1123u32;
pub const TD_BIDIRECTIONAL: TERMINAL_DIRECTION = 2i32;
pub const TD_CAPTURE: TERMINAL_DIRECTION = 0i32;
pub const TD_MULTITRACK_MIXED: TERMINAL_DIRECTION = 3i32;
pub const TD_NONE: TERMINAL_DIRECTION = 4i32;
pub const TD_RENDER: TERMINAL_DIRECTION = 1i32;
pub const TE_ACDGROUP: TAPI_EVENT = 8192i32;
pub const TE_ADDRESS: TAPI_EVENT = 2i32;
pub const TE_ADDRESSCLOSE: TAPIOBJECT_EVENT = 4i32;
pub const TE_ADDRESSCREATE: TAPIOBJECT_EVENT = 0i32;
pub const TE_ADDRESSDEVSPECIFIC: TAPI_EVENT = 16777216i32;
pub const TE_ADDRESSREMOVE: TAPIOBJECT_EVENT = 1i32;
pub const TE_AGENT: TAPI_EVENT = 512i32;
pub const TE_AGENTHANDLER: TAPI_EVENT = 4096i32;
pub const TE_AGENTSESSION: TAPI_EVENT = 1024i32;
pub const TE_ASRTERMINAL: TAPI_EVENT = 131072i32;
pub const TE_CALLHUB: TAPI_EVENT = 32i32;
pub const TE_CALLINFOCHANGE: TAPI_EVENT = 64i32;
pub const TE_CALLMEDIA: TAPI_EVENT = 16i32;
pub const TE_CALLNOTIFICATION: TAPI_EVENT = 4i32;
pub const TE_CALLSTATE: TAPI_EVENT = 8i32;
pub const TE_DIGITEVENT: TAPI_EVENT = 32768i32;
pub const TE_FILETERMINAL: TAPI_EVENT = 524288i32;
pub const TE_GATHERDIGITS: TAPI_EVENT = 8388608i32;
pub const TE_GENERATEEVENT: TAPI_EVENT = 65536i32;
pub const TE_PHONECREATE: TAPIOBJECT_EVENT = 5i32;
pub const TE_PHONEDEVSPECIFIC: TAPI_EVENT = 33554432i32;
pub const TE_PHONEEVENT: TAPI_EVENT = 2097152i32;
pub const TE_PHONEREMOVE: TAPIOBJECT_EVENT = 6i32;
pub const TE_PRIVATE: TAPI_EVENT = 128i32;
pub const TE_QOSEVENT: TAPI_EVENT = 2048i32;
pub const TE_QUEUE: TAPI_EVENT = 16384i32;
pub const TE_REINIT: TAPIOBJECT_EVENT = 2i32;
pub const TE_REQUEST: TAPI_EVENT = 256i32;
pub const TE_TAPIOBJECT: TAPI_EVENT = 1i32;
pub const TE_TONEEVENT: TAPI_EVENT = 4194304i32;
pub const TE_TONETERMINAL: TAPI_EVENT = 1048576i32;
pub const TE_TRANSLATECHANGE: TAPIOBJECT_EVENT = 3i32;
pub const TE_TTSTERMINAL: TAPI_EVENT = 262144i32;
pub const TGT_BUFFERFULL: TAPI_GATHERTERM = 1i32;
pub const TGT_CANCEL: TAPI_GATHERTERM = 16i32;
pub const TGT_FIRSTTIMEOUT: TAPI_GATHERTERM = 4i32;
pub const TGT_INTERTIMEOUT: TAPI_GATHERTERM = 8i32;
pub const TGT_TERMDIGIT: TAPI_GATHERTERM = 2i32;
pub const TMS_ACTIVE: TERMINAL_MEDIA_STATE = 1i32;
pub const TMS_IDLE: TERMINAL_MEDIA_STATE = 0i32;
pub const TMS_LASTITEM: TERMINAL_MEDIA_STATE = 2i32;
pub const TMS_PAUSED: TERMINAL_MEDIA_STATE = 2i32;
pub const TOT_ADDRESS: TAPI_OBJECT_TYPE = 2i32;
pub const TOT_CALL: TAPI_OBJECT_TYPE = 4i32;
pub const TOT_CALLHUB: TAPI_OBJECT_TYPE = 5i32;
pub const TOT_NONE: TAPI_OBJECT_TYPE = 0i32;
pub const TOT_PHONE: TAPI_OBJECT_TYPE = 6i32;
pub const TOT_TAPI: TAPI_OBJECT_TYPE = 1i32;
pub const TOT_TERMINAL: TAPI_OBJECT_TYPE = 3i32;
pub const TSPI_LINEACCEPT: u32 = 500u32;
pub const TSPI_LINEADDTOCONFERENCE: u32 = 501u32;
pub const TSPI_LINEANSWER: u32 = 502u32;
pub const TSPI_LINEBLINDTRANSFER: u32 = 503u32;
pub const TSPI_LINECLOSE: u32 = 504u32;
pub const TSPI_LINECLOSECALL: u32 = 505u32;
pub const TSPI_LINECLOSEMSPINSTANCE: u32 = 609u32;
pub const TSPI_LINECOMPLETECALL: u32 = 506u32;
pub const TSPI_LINECOMPLETETRANSFER: u32 = 507u32;
pub const TSPI_LINECONDITIONALMEDIADETECTION: u32 = 508u32;
pub const TSPI_LINECONFIGDIALOG: u32 = 509u32;
pub const TSPI_LINECONFIGDIALOGEDIT: u32 = 601u32;
pub const TSPI_LINECREATEMSPINSTANCE: u32 = 608u32;
pub const TSPI_LINEDEVSPECIFIC: u32 = 510u32;
pub const TSPI_LINEDEVSPECIFICFEATURE: u32 = 511u32;
pub const TSPI_LINEDIAL: u32 = 512u32;
pub const TSPI_LINEDROP: u32 = 513u32;
pub const TSPI_LINEDROPNOOWNER: u32 = 597u32;
pub const TSPI_LINEDROPONCLOSE: u32 = 596u32;
pub const TSPI_LINEFORWARD: u32 = 514u32;
pub const TSPI_LINEGATHERDIGITS: u32 = 515u32;
pub const TSPI_LINEGENERATEDIGITS: u32 = 516u32;
pub const TSPI_LINEGENERATETONE: u32 = 517u32;
pub const TSPI_LINEGETADDRESSCAPS: u32 = 518u32;
pub const TSPI_LINEGETADDRESSID: u32 = 519u32;
pub const TSPI_LINEGETADDRESSSTATUS: u32 = 520u32;
pub const TSPI_LINEGETCALLADDRESSID: u32 = 521u32;
pub const TSPI_LINEGETCALLHUBTRACKING: u32 = 604u32;
pub const TSPI_LINEGETCALLID: u32 = 603u32;
pub const TSPI_LINEGETCALLINFO: u32 = 522u32;
pub const TSPI_LINEGETCALLSTATUS: u32 = 523u32;
pub const TSPI_LINEGETDEVCAPS: u32 = 524u32;
pub const TSPI_LINEGETDEVCONFIG: u32 = 525u32;
pub const TSPI_LINEGETEXTENSIONID: u32 = 526u32;
pub const TSPI_LINEGETICON: u32 = 527u32;
pub const TSPI_LINEGETID: u32 = 528u32;
pub const TSPI_LINEGETLINEDEVSTATUS: u32 = 529u32;
pub const TSPI_LINEGETNUMADDRESSIDS: u32 = 530u32;
pub const TSPI_LINEHOLD: u32 = 531u32;
pub const TSPI_LINEMAKECALL: u32 = 532u32;
pub const TSPI_LINEMONITORDIGITS: u32 = 533u32;
pub const TSPI_LINEMONITORMEDIA: u32 = 534u32;
pub const TSPI_LINEMONITORTONES: u32 = 535u32;
pub const TSPI_LINEMSPIDENTIFY: u32 = 607u32;
pub const TSPI_LINENEGOTIATEEXTVERSION: u32 = 536u32;
pub const TSPI_LINENEGOTIATETSPIVERSION: u32 = 537u32;
pub const TSPI_LINEOPEN: u32 = 538u32;
pub const TSPI_LINEPARK: u32 = 539u32;
pub const TSPI_LINEPICKUP: u32 = 540u32;
pub const TSPI_LINEPREPAREADDTOCONFERENCE: u32 = 541u32;
pub const TSPI_LINERECEIVEMSPDATA: u32 = 606u32;
pub const TSPI_LINEREDIRECT: u32 = 542u32;
pub const TSPI_LINERELEASEUSERUSERINFO: u32 = 602u32;
pub const TSPI_LINEREMOVEFROMCONFERENCE: u32 = 543u32;
pub const TSPI_LINESECURECALL: u32 = 544u32;
pub const TSPI_LINESELECTEXTVERSION: u32 = 545u32;
pub const TSPI_LINESENDUSERUSERINFO: u32 = 546u32;
pub const TSPI_LINESETAPPSPECIFIC: u32 = 547u32;
pub const TSPI_LINESETCALLHUBTRACKING: u32 = 605u32;
pub const TSPI_LINESETCALLPARAMS: u32 = 548u32;
pub const TSPI_LINESETCURRENTLOCATION: u32 = 600u32;
pub const TSPI_LINESETDEFAULTMEDIADETECTION: u32 = 549u32;
pub const TSPI_LINESETDEVCONFIG: u32 = 550u32;
pub const TSPI_LINESETMEDIACONTROL: u32 = 551u32;
pub const TSPI_LINESETMEDIAMODE: u32 = 552u32;
pub const TSPI_LINESETSTATUSMESSAGES: u32 = 553u32;
pub const TSPI_LINESETTERMINAL: u32 = 554u32;
pub const TSPI_LINESETUPCONFERENCE: u32 = 555u32;
pub const TSPI_LINESETUPTRANSFER: u32 = 556u32;
pub const TSPI_LINESWAPHOLD: u32 = 557u32;
pub const TSPI_LINEUNCOMPLETECALL: u32 = 558u32;
pub const TSPI_LINEUNHOLD: u32 = 559u32;
pub const TSPI_LINEUNPARK: u32 = 560u32;
pub const TSPI_MESSAGE_BASE: u32 = 500u32;
pub const TSPI_PHONECLOSE: u32 = 561u32;
pub const TSPI_PHONECONFIGDIALOG: u32 = 562u32;
pub const TSPI_PHONEDEVSPECIFIC: u32 = 563u32;
pub const TSPI_PHONEGETBUTTONINFO: u32 = 564u32;
pub const TSPI_PHONEGETDATA: u32 = 565u32;
pub const TSPI_PHONEGETDEVCAPS: u32 = 566u32;
pub const TSPI_PHONEGETDISPLAY: u32 = 567u32;
pub const TSPI_PHONEGETEXTENSIONID: u32 = 568u32;
pub const TSPI_PHONEGETGAIN: u32 = 569u32;
pub const TSPI_PHONEGETHOOKSWITCH: u32 = 570u32;
pub const TSPI_PHONEGETICON: u32 = 571u32;
pub const TSPI_PHONEGETID: u32 = 572u32;
pub const TSPI_PHONEGETLAMP: u32 = 573u32;
pub const TSPI_PHONEGETRING: u32 = 574u32;
pub const TSPI_PHONEGETSTATUS: u32 = 575u32;
pub const TSPI_PHONEGETVOLUME: u32 = 576u32;
pub const TSPI_PHONENEGOTIATEEXTVERSION: u32 = 577u32;
pub const TSPI_PHONENEGOTIATETSPIVERSION: u32 = 578u32;
pub const TSPI_PHONEOPEN: u32 = 579u32;
pub const TSPI_PHONESELECTEXTVERSION: u32 = 580u32;
pub const TSPI_PHONESETBUTTONINFO: u32 = 581u32;
pub const TSPI_PHONESETDATA: u32 = 582u32;
pub const TSPI_PHONESETDISPLAY: u32 = 583u32;
pub const TSPI_PHONESETGAIN: u32 = 584u32;
pub const TSPI_PHONESETHOOKSWITCH: u32 = 585u32;
pub const TSPI_PHONESETLAMP: u32 = 586u32;
pub const TSPI_PHONESETRING: u32 = 587u32;
pub const TSPI_PHONESETSTATUSMESSAGES: u32 = 588u32;
pub const TSPI_PHONESETVOLUME: u32 = 589u32;
pub const TSPI_PROC_BASE: u32 = 500u32;
pub const TSPI_PROVIDERCONFIG: u32 = 590u32;
pub const TSPI_PROVIDERCREATELINEDEVICE: u32 = 598u32;
pub const TSPI_PROVIDERCREATEPHONEDEVICE: u32 = 599u32;
pub const TSPI_PROVIDERENUMDEVICES: u32 = 595u32;
pub const TSPI_PROVIDERINIT: u32 = 591u32;
pub const TSPI_PROVIDERINSTALL: u32 = 592u32;
pub const TSPI_PROVIDERREMOVE: u32 = 593u32;
pub const TSPI_PROVIDERSHUTDOWN: u32 = 594u32;
pub const TS_INUSE: TERMINAL_STATE = 0i32;
pub const TS_NOTINUSE: TERMINAL_STATE = 1i32;
pub const TTM_BEEP: TAPI_TONEMODE = 8i32;
pub const TTM_BILLING: TAPI_TONEMODE = 16i32;
pub const TTM_BUSY: TAPI_TONEMODE = 4i32;
pub const TTM_RINGBACK: TAPI_TONEMODE = 2i32;
pub const TT_DYNAMIC: TERMINAL_TYPE = 1i32;
pub const TT_STATIC: TERMINAL_TYPE = 0i32;
pub const TUISPIDLL_OBJECT_DIALOGINSTANCE: i32 = 4i32;
pub const TUISPIDLL_OBJECT_LINEID: i32 = 1i32;
pub const TUISPIDLL_OBJECT_PHONEID: i32 = 2i32;
pub const TUISPIDLL_OBJECT_PROVIDERID: i32 = 3i32;
pub const atypFile: i32 = 1i32;
pub const atypMax: i32 = 4i32;
pub const atypNull: i32 = 0i32;
pub const atypOle: i32 = 2i32;
pub const atypPicture: i32 = 3i32;
pub const cbDisplayName: u32 = 41u32;
pub const cbEmailName: u32 = 11u32;
pub const cbMaxIdData: u32 = 200u32;
pub const cbSeverName: u32 = 12u32;
pub const cbTYPE: u32 = 16u32;
pub const prioHigh: u32 = 1u32;
pub const prioLow: u32 = 3u32;
pub const prioNorm: u32 = 2u32;
pub type ACDGROUP_EVENT = i32;
pub type ACDQUEUE_EVENT = i32;
pub type ADDRESS_CAPABILITY = i32;
pub type ADDRESS_CAPABILITY_STRING = i32;
pub type ADDRESS_EVENT = i32;
pub type ADDRESS_STATE = i32;
pub type AGENTHANDLER_EVENT = i32;
pub type AGENT_EVENT = i32;
pub type AGENT_SESSION_EVENT = i32;
pub type AGENT_SESSION_STATE = i32;
pub type AGENT_STATE = i32;
pub type CALLHUB_EVENT = i32;
pub type CALLHUB_STATE = i32;
pub type CALLINFOCHANGE_CAUSE = i32;
pub type CALLINFO_BUFFER = i32;
pub type CALLINFO_LONG = i32;
pub type CALLINFO_STRING = i32;
pub type CALL_MEDIA_EVENT = i32;
pub type CALL_MEDIA_EVENT_CAUSE = i32;
pub type CALL_NOTIFICATION_EVENT = i32;
pub type CALL_PRIVILEGE = i32;
pub type CALL_STATE = i32;
pub type CALL_STATE_EVENT_CAUSE = i32;
pub type DIRECTORY_OBJECT_TYPE = i32;
pub type DIRECTORY_TYPE = i32;
pub type DISCONNECT_CODE = i32;
pub type FINISH_MODE = i32;
pub type FT_STATE_EVENT_CAUSE = i32;
pub type FULLDUPLEX_SUPPORT = i32;
pub type MSP_ADDRESS_EVENT = i32;
pub type MSP_CALL_EVENT = i32;
pub type MSP_CALL_EVENT_CAUSE = i32;
pub type MSP_EVENT = i32;
pub type PHONECAPS_BUFFER = i32;
pub type PHONECAPS_LONG = i32;
pub type PHONECAPS_STRING = i32;
pub type PHONE_BUTTON_FUNCTION = i32;
pub type PHONE_BUTTON_MODE = i32;
pub type PHONE_BUTTON_STATE = i32;
pub type PHONE_EVENT = i32;
pub type PHONE_HOOK_SWITCH_DEVICE = i32;
pub type PHONE_HOOK_SWITCH_STATE = i32;
pub type PHONE_LAMP_MODE = i32;
pub type PHONE_PRIVILEGE = i32;
pub type PHONE_TONE = i32;
pub type QOS_EVENT = i32;
pub type QOS_SERVICE_LEVEL = i32;
pub type RND_ADVERTISING_SCOPE = i32;
pub type TAPIOBJECT_EVENT = i32;
pub type TAPI_EVENT = i32;
pub type TAPI_GATHERTERM = i32;
pub type TAPI_OBJECT_TYPE = i32;
pub type TAPI_TONEMODE = i32;
pub type TERMINAL_DIRECTION = i32;
pub type TERMINAL_MEDIA_STATE = i32;
pub type TERMINAL_STATE = i32;
pub type TERMINAL_TYPE = i32;
#[repr(C)]
pub struct ADDRALIAS {
    pub rgchName: [u8; 41],
    pub rgchEName: [u8; 11],
    pub rgchSrvr: [u8; 12],
    pub dibDetail: u32,
    pub r#type: u16,
}
impl ::core::marker::Copy for ADDRALIAS {}
impl ::core::clone::Clone for ADDRALIAS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DTR {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wDayOfWeek: u16,
}
impl ::core::marker::Copy for DTR {}
impl ::core::clone::Clone for DTR {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HDRVCALL = isize;
pub type HDRVDIALOGINSTANCE = isize;
pub type HDRVLINE = isize;
pub type HDRVMSPLINE = isize;
pub type HDRVPHONE = isize;
pub type HPROVIDER = isize;
pub type HTAPICALL = isize;
pub type HTAPILINE = isize;
pub type HTAPIPHONE = isize;
#[repr(C, packed(1))]
pub struct LINEADDRESSCAPS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwLineDeviceID: u32,
    pub dwAddressSize: u32,
    pub dwAddressOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwAddressSharing: u32,
    pub dwAddressStates: u32,
    pub dwCallInfoStates: u32,
    pub dwCallerIDFlags: u32,
    pub dwCalledIDFlags: u32,
    pub dwConnectedIDFlags: u32,
    pub dwRedirectionIDFlags: u32,
    pub dwRedirectingIDFlags: u32,
    pub dwCallStates: u32,
    pub dwDialToneModes: u32,
    pub dwBusyModes: u32,
    pub dwSpecialInfo: u32,
    pub dwDisconnectModes: u32,
    pub dwMaxNumActiveCalls: u32,
    pub dwMaxNumOnHoldCalls: u32,
    pub dwMaxNumOnHoldPendingCalls: u32,
    pub dwMaxNumConference: u32,
    pub dwMaxNumTransConf: u32,
    pub dwAddrCapFlags: u32,
    pub dwCallFeatures: u32,
    pub dwRemoveFromConfCaps: u32,
    pub dwRemoveFromConfState: u32,
    pub dwTransferModes: u32,
    pub dwParkModes: u32,
    pub dwForwardModes: u32,
    pub dwMaxForwardEntries: u32,
    pub dwMaxSpecificEntries: u32,
    pub dwMinFwdNumRings: u32,
    pub dwMaxFwdNumRings: u32,
    pub dwMaxCallCompletions: u32,
    pub dwCallCompletionConds: u32,
    pub dwCallCompletionModes: u32,
    pub dwNumCompletionMessages: u32,
    pub dwCompletionMsgTextEntrySize: u32,
    pub dwCompletionMsgTextSize: u32,
    pub dwCompletionMsgTextOffset: u32,
    pub dwAddressFeatures: u32,
    pub dwPredictiveAutoTransferStates: u32,
    pub dwNumCallTreatments: u32,
    pub dwCallTreatmentListSize: u32,
    pub dwCallTreatmentListOffset: u32,
    pub dwDeviceClassesSize: u32,
    pub dwDeviceClassesOffset: u32,
    pub dwMaxCallDataSize: u32,
    pub dwCallFeatures2: u32,
    pub dwMaxNoAnswerTimeout: u32,
    pub dwConnectedModes: u32,
    pub dwOfferingModes: u32,
    pub dwAvailableMediaModes: u32,
}
impl ::core::marker::Copy for LINEADDRESSCAPS {}
impl ::core::clone::Clone for LINEADDRESSCAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEADDRESSSTATUS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumInUse: u32,
    pub dwNumActiveCalls: u32,
    pub dwNumOnHoldCalls: u32,
    pub dwNumOnHoldPendCalls: u32,
    pub dwAddressFeatures: u32,
    pub dwNumRingsNoAnswer: u32,
    pub dwForwardNumEntries: u32,
    pub dwForwardSize: u32,
    pub dwForwardOffset: u32,
    pub dwTerminalModesSize: u32,
    pub dwTerminalModesOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
}
impl ::core::marker::Copy for LINEADDRESSSTATUS {}
impl ::core::clone::Clone for LINEADDRESSSTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTACTIVITYENTRY {
    pub dwID: u32,
    pub dwNameSize: u32,
    pub dwNameOffset: u32,
}
impl ::core::marker::Copy for LINEAGENTACTIVITYENTRY {}
impl ::core::clone::Clone for LINEAGENTACTIVITYENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTACTIVITYLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumEntries: u32,
    pub dwListSize: u32,
    pub dwListOffset: u32,
}
impl ::core::marker::Copy for LINEAGENTACTIVITYLIST {}
impl ::core::clone::Clone for LINEAGENTACTIVITYLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTCAPS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwAgentHandlerInfoSize: u32,
    pub dwAgentHandlerInfoOffset: u32,
    pub dwCapsVersion: u32,
    pub dwFeatures: u32,
    pub dwStates: u32,
    pub dwNextStates: u32,
    pub dwMaxNumGroupEntries: u32,
    pub dwAgentStatusMessages: u32,
    pub dwNumAgentExtensionIDs: u32,
    pub dwAgentExtensionIDListSize: u32,
    pub dwAgentExtensionIDListOffset: u32,
    pub ProxyGUID: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for LINEAGENTCAPS {}
impl ::core::clone::Clone for LINEAGENTCAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTENTRY {
    pub hAgent: u32,
    pub dwNameSize: u32,
    pub dwNameOffset: u32,
    pub dwIDSize: u32,
    pub dwIDOffset: u32,
    pub dwPINSize: u32,
    pub dwPINOffset: u32,
}
impl ::core::marker::Copy for LINEAGENTENTRY {}
impl ::core::clone::Clone for LINEAGENTENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTGROUPENTRY {
    pub GroupID: LINEAGENTGROUPENTRY_0,
    pub dwNameSize: u32,
    pub dwNameOffset: u32,
}
impl ::core::marker::Copy for LINEAGENTGROUPENTRY {}
impl ::core::clone::Clone for LINEAGENTGROUPENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTGROUPENTRY_0 {
    pub dwGroupID1: u32,
    pub dwGroupID2: u32,
    pub dwGroupID3: u32,
    pub dwGroupID4: u32,
}
impl ::core::marker::Copy for LINEAGENTGROUPENTRY_0 {}
impl ::core::clone::Clone for LINEAGENTGROUPENTRY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTGROUPLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumEntries: u32,
    pub dwListSize: u32,
    pub dwListOffset: u32,
}
impl ::core::marker::Copy for LINEAGENTGROUPLIST {}
impl ::core::clone::Clone for LINEAGENTGROUPLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEAGENTINFO {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwAgentState: u32,
    pub dwNextAgentState: u32,
    pub dwMeasurementPeriod: u32,
    pub cyOverallCallRate: super::super::System::Com::CY,
    pub dwNumberOfACDCalls: u32,
    pub dwNumberOfIncomingCalls: u32,
    pub dwNumberOfOutgoingCalls: u32,
    pub dwTotalACDTalkTime: u32,
    pub dwTotalACDCallTime: u32,
    pub dwTotalACDWrapUpTime: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEAGENTINFO {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEAGENTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumEntries: u32,
    pub dwListSize: u32,
    pub dwListOffset: u32,
}
impl ::core::marker::Copy for LINEAGENTLIST {}
impl ::core::clone::Clone for LINEAGENTLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTSESSIONENTRY {
    pub hAgentSession: u32,
    pub hAgent: u32,
    pub GroupID: ::windows_sys::core::GUID,
    pub dwWorkingAddressID: u32,
}
impl ::core::marker::Copy for LINEAGENTSESSIONENTRY {}
impl ::core::clone::Clone for LINEAGENTSESSIONENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEAGENTSESSIONINFO {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwAgentSessionState: u32,
    pub dwNextAgentSessionState: u32,
    pub dateSessionStartTime: f64,
    pub dwSessionDuration: u32,
    pub dwNumberOfCalls: u32,
    pub dwTotalTalkTime: u32,
    pub dwAverageTalkTime: u32,
    pub dwTotalCallTime: u32,
    pub dwAverageCallTime: u32,
    pub dwTotalWrapUpTime: u32,
    pub dwAverageWrapUpTime: u32,
    pub cyACDCallRate: super::super::System::Com::CY,
    pub dwLongestTimeToAnswer: u32,
    pub dwAverageTimeToAnswer: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEAGENTSESSIONINFO {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEAGENTSESSIONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTSESSIONLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumEntries: u32,
    pub dwListSize: u32,
    pub dwListOffset: u32,
}
impl ::core::marker::Copy for LINEAGENTSESSIONLIST {}
impl ::core::clone::Clone for LINEAGENTSESSIONLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAGENTSTATUS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumEntries: u32,
    pub dwGroupListSize: u32,
    pub dwGroupListOffset: u32,
    pub dwState: u32,
    pub dwNextState: u32,
    pub dwActivityID: u32,
    pub dwActivitySize: u32,
    pub dwActivityOffset: u32,
    pub dwAgentFeatures: u32,
    pub dwValidStates: u32,
    pub dwValidNextStates: u32,
}
impl ::core::marker::Copy for LINEAGENTSTATUS {}
impl ::core::clone::Clone for LINEAGENTSTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEAPPINFO {
    pub dwMachineNameSize: u32,
    pub dwMachineNameOffset: u32,
    pub dwUserNameSize: u32,
    pub dwUserNameOffset: u32,
    pub dwModuleFilenameSize: u32,
    pub dwModuleFilenameOffset: u32,
    pub dwFriendlyNameSize: u32,
    pub dwFriendlyNameOffset: u32,
    pub dwMediaModes: u32,
    pub dwAddressID: u32,
}
impl ::core::marker::Copy for LINEAPPINFO {}
impl ::core::clone::Clone for LINEAPPINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINECALLINFO {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub hLine: u32,
    pub dwLineDeviceID: u32,
    pub dwAddressID: u32,
    pub dwBearerMode: u32,
    pub dwRate: u32,
    pub dwMediaMode: u32,
    pub dwAppSpecific: u32,
    pub dwCallID: u32,
    pub dwRelatedCallID: u32,
    pub dwCallParamFlags: u32,
    pub dwCallStates: u32,
    pub dwMonitorDigitModes: u32,
    pub dwMonitorMediaModes: u32,
    pub DialParams: LINEDIALPARAMS,
    pub dwOrigin: u32,
    pub dwReason: u32,
    pub dwCompletionID: u32,
    pub dwNumOwners: u32,
    pub dwNumMonitors: u32,
    pub dwCountryCode: u32,
    pub dwTrunk: u32,
    pub dwCallerIDFlags: u32,
    pub dwCallerIDSize: u32,
    pub dwCallerIDOffset: u32,
    pub dwCallerIDNameSize: u32,
    pub dwCallerIDNameOffset: u32,
    pub dwCalledIDFlags: u32,
    pub dwCalledIDSize: u32,
    pub dwCalledIDOffset: u32,
    pub dwCalledIDNameSize: u32,
    pub dwCalledIDNameOffset: u32,
    pub dwConnectedIDFlags: u32,
    pub dwConnectedIDSize: u32,
    pub dwConnectedIDOffset: u32,
    pub dwConnectedIDNameSize: u32,
    pub dwConnectedIDNameOffset: u32,
    pub dwRedirectionIDFlags: u32,
    pub dwRedirectionIDSize: u32,
    pub dwRedirectionIDOffset: u32,
    pub dwRedirectionIDNameSize: u32,
    pub dwRedirectionIDNameOffset: u32,
    pub dwRedirectingIDFlags: u32,
    pub dwRedirectingIDSize: u32,
    pub dwRedirectingIDOffset: u32,
    pub dwRedirectingIDNameSize: u32,
    pub dwRedirectingIDNameOffset: u32,
    pub dwAppNameSize: u32,
    pub dwAppNameOffset: u32,
    pub dwDisplayableAddressSize: u32,
    pub dwDisplayableAddressOffset: u32,
    pub dwCalledPartySize: u32,
    pub dwCalledPartyOffset: u32,
    pub dwCommentSize: u32,
    pub dwCommentOffset: u32,
    pub dwDisplaySize: u32,
    pub dwDisplayOffset: u32,
    pub dwUserUserInfoSize: u32,
    pub dwUserUserInfoOffset: u32,
    pub dwHighLevelCompSize: u32,
    pub dwHighLevelCompOffset: u32,
    pub dwLowLevelCompSize: u32,
    pub dwLowLevelCompOffset: u32,
    pub dwChargingInfoSize: u32,
    pub dwChargingInfoOffset: u32,
    pub dwTerminalModesSize: u32,
    pub dwTerminalModesOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwCallTreatment: u32,
    pub dwCallDataSize: u32,
    pub dwCallDataOffset: u32,
    pub dwSendingFlowspecSize: u32,
    pub dwSendingFlowspecOffset: u32,
    pub dwReceivingFlowspecSize: u32,
    pub dwReceivingFlowspecOffset: u32,
}
impl ::core::marker::Copy for LINECALLINFO {}
impl ::core::clone::Clone for LINECALLINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINECALLLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwCallsNumEntries: u32,
    pub dwCallsSize: u32,
    pub dwCallsOffset: u32,
}
impl ::core::marker::Copy for LINECALLLIST {}
impl ::core::clone::Clone for LINECALLLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINECALLPARAMS {
    pub dwTotalSize: u32,
    pub dwBearerMode: u32,
    pub dwMinRate: u32,
    pub dwMaxRate: u32,
    pub dwMediaMode: u32,
    pub dwCallParamFlags: u32,
    pub dwAddressMode: u32,
    pub dwAddressID: u32,
    pub DialParams: LINEDIALPARAMS,
    pub dwOrigAddressSize: u32,
    pub dwOrigAddressOffset: u32,
    pub dwDisplayableAddressSize: u32,
    pub dwDisplayableAddressOffset: u32,
    pub dwCalledPartySize: u32,
    pub dwCalledPartyOffset: u32,
    pub dwCommentSize: u32,
    pub dwCommentOffset: u32,
    pub dwUserUserInfoSize: u32,
    pub dwUserUserInfoOffset: u32,
    pub dwHighLevelCompSize: u32,
    pub dwHighLevelCompOffset: u32,
    pub dwLowLevelCompSize: u32,
    pub dwLowLevelCompOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwPredictiveAutoTransferStates: u32,
    pub dwTargetAddressSize: u32,
    pub dwTargetAddressOffset: u32,
    pub dwSendingFlowspecSize: u32,
    pub dwSendingFlowspecOffset: u32,
    pub dwReceivingFlowspecSize: u32,
    pub dwReceivingFlowspecOffset: u32,
    pub dwDeviceClassSize: u32,
    pub dwDeviceClassOffset: u32,
    pub dwDeviceConfigSize: u32,
    pub dwDeviceConfigOffset: u32,
    pub dwCallDataSize: u32,
    pub dwCallDataOffset: u32,
    pub dwNoAnswerTimeout: u32,
    pub dwCallingPartyIDSize: u32,
    pub dwCallingPartyIDOffset: u32,
}
impl ::core::marker::Copy for LINECALLPARAMS {}
impl ::core::clone::Clone for LINECALLPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LINECALLSTATUS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwCallState: u32,
    pub dwCallStateMode: u32,
    pub dwCallPrivilege: u32,
    pub dwCallFeatures: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwCallFeatures2: u32,
    pub tStateEntryTime: super::super::Foundation::SYSTEMTIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LINECALLSTATUS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LINECALLSTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINECALLTREATMENTENTRY {
    pub dwCallTreatmentID: u32,
    pub dwCallTreatmentNameSize: u32,
    pub dwCallTreatmentNameOffset: u32,
}
impl ::core::marker::Copy for LINECALLTREATMENTENTRY {}
impl ::core::clone::Clone for LINECALLTREATMENTENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINECARDENTRY {
    pub dwPermanentCardID: u32,
    pub dwCardNameSize: u32,
    pub dwCardNameOffset: u32,
    pub dwCardNumberDigits: u32,
    pub dwSameAreaRuleSize: u32,
    pub dwSameAreaRuleOffset: u32,
    pub dwLongDistanceRuleSize: u32,
    pub dwLongDistanceRuleOffset: u32,
    pub dwInternationalRuleSize: u32,
    pub dwInternationalRuleOffset: u32,
    pub dwOptions: u32,
}
impl ::core::marker::Copy for LINECARDENTRY {}
impl ::core::clone::Clone for LINECARDENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINECOUNTRYENTRY {
    pub dwCountryID: u32,
    pub dwCountryCode: u32,
    pub dwNextCountryID: u32,
    pub dwCountryNameSize: u32,
    pub dwCountryNameOffset: u32,
    pub dwSameAreaRuleSize: u32,
    pub dwSameAreaRuleOffset: u32,
    pub dwLongDistanceRuleSize: u32,
    pub dwLongDistanceRuleOffset: u32,
    pub dwInternationalRuleSize: u32,
    pub dwInternationalRuleOffset: u32,
}
impl ::core::marker::Copy for LINECOUNTRYENTRY {}
impl ::core::clone::Clone for LINECOUNTRYENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINECOUNTRYLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumCountries: u32,
    pub dwCountryListSize: u32,
    pub dwCountryListOffset: u32,
}
impl ::core::marker::Copy for LINECOUNTRYLIST {}
impl ::core::clone::Clone for LINECOUNTRYLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEDEVCAPS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwProviderInfoSize: u32,
    pub dwProviderInfoOffset: u32,
    pub dwSwitchInfoSize: u32,
    pub dwSwitchInfoOffset: u32,
    pub dwPermanentLineID: u32,
    pub dwLineNameSize: u32,
    pub dwLineNameOffset: u32,
    pub dwStringFormat: u32,
    pub dwAddressModes: u32,
    pub dwNumAddresses: u32,
    pub dwBearerModes: u32,
    pub dwMaxRate: u32,
    pub dwMediaModes: u32,
    pub dwGenerateToneModes: u32,
    pub dwGenerateToneMaxNumFreq: u32,
    pub dwGenerateDigitModes: u32,
    pub dwMonitorToneMaxNumFreq: u32,
    pub dwMonitorToneMaxNumEntries: u32,
    pub dwMonitorDigitModes: u32,
    pub dwGatherDigitsMinTimeout: u32,
    pub dwGatherDigitsMaxTimeout: u32,
    pub dwMedCtlDigitMaxListSize: u32,
    pub dwMedCtlMediaMaxListSize: u32,
    pub dwMedCtlToneMaxListSize: u32,
    pub dwMedCtlCallStateMaxListSize: u32,
    pub dwDevCapFlags: u32,
    pub dwMaxNumActiveCalls: u32,
    pub dwAnswerMode: u32,
    pub dwRingModes: u32,
    pub dwLineStates: u32,
    pub dwUUIAcceptSize: u32,
    pub dwUUIAnswerSize: u32,
    pub dwUUIMakeCallSize: u32,
    pub dwUUIDropSize: u32,
    pub dwUUISendUserUserInfoSize: u32,
    pub dwUUICallInfoSize: u32,
    pub MinDialParams: LINEDIALPARAMS,
    pub MaxDialParams: LINEDIALPARAMS,
    pub DefaultDialParams: LINEDIALPARAMS,
    pub dwNumTerminals: u32,
    pub dwTerminalCapsSize: u32,
    pub dwTerminalCapsOffset: u32,
    pub dwTerminalTextEntrySize: u32,
    pub dwTerminalTextSize: u32,
    pub dwTerminalTextOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwLineFeatures: u32,
    pub dwSettableDevStatus: u32,
    pub dwDeviceClassesSize: u32,
    pub dwDeviceClassesOffset: u32,
    pub PermanentLineGuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for LINEDEVCAPS {}
impl ::core::clone::Clone for LINEDEVCAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEDEVSTATUS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumOpens: u32,
    pub dwOpenMediaModes: u32,
    pub dwNumActiveCalls: u32,
    pub dwNumOnHoldCalls: u32,
    pub dwNumOnHoldPendCalls: u32,
    pub dwLineFeatures: u32,
    pub dwNumCallCompletions: u32,
    pub dwRingMode: u32,
    pub dwSignalLevel: u32,
    pub dwBatteryLevel: u32,
    pub dwRoamMode: u32,
    pub dwDevStatusFlags: u32,
    pub dwTerminalModesSize: u32,
    pub dwTerminalModesOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwAvailableMediaModes: u32,
    pub dwAppInfoSize: u32,
    pub dwAppInfoOffset: u32,
}
impl ::core::marker::Copy for LINEDEVSTATUS {}
impl ::core::clone::Clone for LINEDEVSTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEDIALPARAMS {
    pub dwDialPause: u32,
    pub dwDialSpeed: u32,
    pub dwDigitDuration: u32,
    pub dwWaitForDialtone: u32,
}
impl ::core::marker::Copy for LINEDIALPARAMS {}
impl ::core::clone::Clone for LINEDIALPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEEXTENSIONID {
    pub dwExtensionID0: u32,
    pub dwExtensionID1: u32,
    pub dwExtensionID2: u32,
    pub dwExtensionID3: u32,
}
impl ::core::marker::Copy for LINEEXTENSIONID {}
impl ::core::clone::Clone for LINEEXTENSIONID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEFORWARD {
    pub dwForwardMode: u32,
    pub dwCallerAddressSize: u32,
    pub dwCallerAddressOffset: u32,
    pub dwDestCountryCode: u32,
    pub dwDestAddressSize: u32,
    pub dwDestAddressOffset: u32,
}
impl ::core::marker::Copy for LINEFORWARD {}
impl ::core::clone::Clone for LINEFORWARD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEFORWARDLIST {
    pub dwTotalSize: u32,
    pub dwNumEntries: u32,
    pub ForwardList: [LINEFORWARD; 1],
}
impl ::core::marker::Copy for LINEFORWARDLIST {}
impl ::core::clone::Clone for LINEFORWARDLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEGENERATETONE {
    pub dwFrequency: u32,
    pub dwCadenceOn: u32,
    pub dwCadenceOff: u32,
    pub dwVolume: u32,
}
impl ::core::marker::Copy for LINEGENERATETONE {}
impl ::core::clone::Clone for LINEGENERATETONE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LINEINITIALIZEEXPARAMS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwOptions: u32,
    pub Handles: LINEINITIALIZEEXPARAMS_0,
    pub dwCompletionKey: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LINEINITIALIZEEXPARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LINEINITIALIZEEXPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union LINEINITIALIZEEXPARAMS_0 {
    pub hEvent: super::super::Foundation::HANDLE,
    pub hCompletionPort: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LINEINITIALIZEEXPARAMS_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LINEINITIALIZEEXPARAMS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINELOCATIONENTRY {
    pub dwPermanentLocationID: u32,
    pub dwLocationNameSize: u32,
    pub dwLocationNameOffset: u32,
    pub dwCountryCode: u32,
    pub dwCityCodeSize: u32,
    pub dwCityCodeOffset: u32,
    pub dwPreferredCardID: u32,
    pub dwLocalAccessCodeSize: u32,
    pub dwLocalAccessCodeOffset: u32,
    pub dwLongDistanceAccessCodeSize: u32,
    pub dwLongDistanceAccessCodeOffset: u32,
    pub dwTollPrefixListSize: u32,
    pub dwTollPrefixListOffset: u32,
    pub dwCountryID: u32,
    pub dwOptions: u32,
    pub dwCancelCallWaitingSize: u32,
    pub dwCancelCallWaitingOffset: u32,
}
impl ::core::marker::Copy for LINELOCATIONENTRY {}
impl ::core::clone::Clone for LINELOCATIONENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEMEDIACONTROLCALLSTATE {
    pub dwCallStates: u32,
    pub dwMediaControl: u32,
}
impl ::core::marker::Copy for LINEMEDIACONTROLCALLSTATE {}
impl ::core::clone::Clone for LINEMEDIACONTROLCALLSTATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEMEDIACONTROLDIGIT {
    pub dwDigit: u32,
    pub dwDigitModes: u32,
    pub dwMediaControl: u32,
}
impl ::core::marker::Copy for LINEMEDIACONTROLDIGIT {}
impl ::core::clone::Clone for LINEMEDIACONTROLDIGIT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEMEDIACONTROLMEDIA {
    pub dwMediaModes: u32,
    pub dwDuration: u32,
    pub dwMediaControl: u32,
}
impl ::core::marker::Copy for LINEMEDIACONTROLMEDIA {}
impl ::core::clone::Clone for LINEMEDIACONTROLMEDIA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEMEDIACONTROLTONE {
    pub dwAppSpecific: u32,
    pub dwDuration: u32,
    pub dwFrequency1: u32,
    pub dwFrequency2: u32,
    pub dwFrequency3: u32,
    pub dwMediaControl: u32,
}
impl ::core::marker::Copy for LINEMEDIACONTROLTONE {}
impl ::core::clone::Clone for LINEMEDIACONTROLTONE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEMESSAGE {
    pub hDevice: u32,
    pub dwMessageID: u32,
    pub dwCallbackInstance: usize,
    pub dwParam1: usize,
    pub dwParam2: usize,
    pub dwParam3: usize,
}
impl ::core::marker::Copy for LINEMESSAGE {}
impl ::core::clone::Clone for LINEMESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEMONITORTONE {
    pub dwAppSpecific: u32,
    pub dwDuration: u32,
    pub dwFrequency1: u32,
    pub dwFrequency2: u32,
    pub dwFrequency3: u32,
}
impl ::core::marker::Copy for LINEMONITORTONE {}
impl ::core::clone::Clone for LINEMONITORTONE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEPROVIDERENTRY {
    pub dwPermanentProviderID: u32,
    pub dwProviderFilenameSize: u32,
    pub dwProviderFilenameOffset: u32,
}
impl ::core::marker::Copy for LINEPROVIDERENTRY {}
impl ::core::clone::Clone for LINEPROVIDERENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEPROVIDERLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumProviders: u32,
    pub dwProviderListSize: u32,
    pub dwProviderListOffset: u32,
}
impl ::core::marker::Copy for LINEPROVIDERLIST {}
impl ::core::clone::Clone for LINEPROVIDERLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST {
    pub dwSize: u32,
    pub dwClientMachineNameSize: u32,
    pub dwClientMachineNameOffset: u32,
    pub dwClientUserNameSize: u32,
    pub dwClientUserNameOffset: u32,
    pub dwClientAppAPIVersion: u32,
    pub dwRequestType: u32,
    pub Anonymous: LINEPROXYREQUEST_0,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union LINEPROXYREQUEST_0 {
    pub SetAgentGroup: LINEPROXYREQUEST_0_14,
    pub SetAgentState: LINEPROXYREQUEST_0_18,
    pub SetAgentActivity: LINEPROXYREQUEST_0_13,
    pub GetAgentCaps: LINEPROXYREQUEST_0_4,
    pub GetAgentStatus: LINEPROXYREQUEST_0_9,
    pub AgentSpecific: LINEPROXYREQUEST_0_0,
    pub GetAgentActivityList: LINEPROXYREQUEST_0_3,
    pub GetAgentGroupList: LINEPROXYREQUEST_0_5,
    pub CreateAgent: LINEPROXYREQUEST_0_2,
    pub SetAgentStateEx: LINEPROXYREQUEST_0_17,
    pub SetAgentMeasurementPeriod: LINEPROXYREQUEST_0_15,
    pub GetAgentInfo: LINEPROXYREQUEST_0_6,
    pub CreateAgentSession: LINEPROXYREQUEST_0_1,
    pub GetAgentSessionList: LINEPROXYREQUEST_0_8,
    pub GetAgentSessionInfo: LINEPROXYREQUEST_0_7,
    pub SetAgentSessionState: LINEPROXYREQUEST_0_16,
    pub GetQueueList: LINEPROXYREQUEST_0_12,
    pub SetQueueMeasurementPeriod: LINEPROXYREQUEST_0_19,
    pub GetQueueInfo: LINEPROXYREQUEST_0_11,
    pub GetGroupList: LINEPROXYREQUEST_0_10,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_0 {
    pub dwAddressID: u32,
    pub dwAgentExtensionIDIndex: u32,
    pub dwSize: u32,
    pub Params: [u8; 1],
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_1 {
    pub hAgentSession: u32,
    pub dwAgentPINSize: u32,
    pub dwAgentPINOffset: u32,
    pub hAgent: u32,
    pub GroupID: ::windows_sys::core::GUID,
    pub dwWorkingAddressID: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_1 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_2 {
    pub hAgent: u32,
    pub dwAgentIDSize: u32,
    pub dwAgentIDOffset: u32,
    pub dwAgentPINSize: u32,
    pub dwAgentPINOffset: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_2 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_3 {
    pub dwAddressID: u32,
    pub ActivityList: LINEAGENTACTIVITYLIST,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_3 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_4 {
    pub dwAddressID: u32,
    pub AgentCaps: LINEAGENTCAPS,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_4 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_5 {
    pub dwAddressID: u32,
    pub GroupList: LINEAGENTGROUPLIST,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_5 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_6 {
    pub hAgent: u32,
    pub AgentInfo: LINEAGENTINFO,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_6 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_7 {
    pub hAgentSession: u32,
    pub SessionInfo: LINEAGENTSESSIONINFO,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_7 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_8 {
    pub hAgent: u32,
    pub SessionList: LINEAGENTSESSIONLIST,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_8 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_8 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_9 {
    pub dwAddressID: u32,
    pub AgentStatus: LINEAGENTSTATUS,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_9 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_9 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_10 {
    pub GroupList: LINEAGENTGROUPLIST,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_10 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_10 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_11 {
    pub dwQueueID: u32,
    pub QueueInfo: LINEQUEUEINFO,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_11 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_11 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_12 {
    pub GroupID: ::windows_sys::core::GUID,
    pub QueueList: LINEQUEUELIST,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_12 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_12 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_13 {
    pub dwAddressID: u32,
    pub dwActivityID: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_13 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_13 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_14 {
    pub dwAddressID: u32,
    pub GroupList: LINEAGENTGROUPLIST,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_14 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_14 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_15 {
    pub hAgent: u32,
    pub dwMeasurementPeriod: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_15 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_15 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_16 {
    pub hAgentSession: u32,
    pub dwAgentSessionState: u32,
    pub dwNextAgentSessionState: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_16 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_16 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_17 {
    pub hAgent: u32,
    pub dwAgentState: u32,
    pub dwNextAgentState: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_17 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_17 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_18 {
    pub dwAddressID: u32,
    pub dwAgentState: u32,
    pub dwNextAgentState: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_18 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_18 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct LINEPROXYREQUEST_0_19 {
    pub dwQueueID: u32,
    pub dwMeasurementPeriod: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for LINEPROXYREQUEST_0_19 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for LINEPROXYREQUEST_0_19 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEPROXYREQUESTLIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumEntries: u32,
    pub dwListSize: u32,
    pub dwListOffset: u32,
}
impl ::core::marker::Copy for LINEPROXYREQUESTLIST {}
impl ::core::clone::Clone for LINEPROXYREQUESTLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEQUEUEENTRY {
    pub dwQueueID: u32,
    pub dwNameSize: u32,
    pub dwNameOffset: u32,
}
impl ::core::marker::Copy for LINEQUEUEENTRY {}
impl ::core::clone::Clone for LINEQUEUEENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEQUEUEINFO {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwMeasurementPeriod: u32,
    pub dwTotalCallsQueued: u32,
    pub dwCurrentCallsQueued: u32,
    pub dwTotalCallsAbandoned: u32,
    pub dwTotalCallsFlowedIn: u32,
    pub dwTotalCallsFlowedOut: u32,
    pub dwLongestEverWaitTime: u32,
    pub dwCurrentLongestWaitTime: u32,
    pub dwAverageWaitTime: u32,
    pub dwFinalDisposition: u32,
}
impl ::core::marker::Copy for LINEQUEUEINFO {}
impl ::core::clone::Clone for LINEQUEUEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEQUEUELIST {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumEntries: u32,
    pub dwListSize: u32,
    pub dwListOffset: u32,
}
impl ::core::marker::Copy for LINEQUEUELIST {}
impl ::core::clone::Clone for LINEQUEUELIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LINEREQMAKECALL {
    pub szDestAddress: [u8; 80],
    pub szAppName: [u8; 40],
    pub szCalledParty: [u8; 40],
    pub szComment: [u8; 80],
}
impl ::core::marker::Copy for LINEREQMAKECALL {}
impl ::core::clone::Clone for LINEREQMAKECALL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINEREQMAKECALLW {
    pub szDestAddress: [u16; 80],
    pub szAppName: [u16; 40],
    pub szCalledParty: [u16; 40],
    pub szComment: [u16; 80],
}
impl ::core::marker::Copy for LINEREQMAKECALLW {}
impl ::core::clone::Clone for LINEREQMAKECALLW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LINEREQMEDIACALL {
    pub hWnd: super::super::Foundation::HWND,
    pub wRequestID: super::super::Foundation::WPARAM,
    pub szDeviceClass: [u8; 40],
    pub ucDeviceID: [u8; 40],
    pub dwSize: u32,
    pub dwSecure: u32,
    pub szDestAddress: [u8; 80],
    pub szAppName: [u8; 40],
    pub szCalledParty: [u8; 40],
    pub szComment: [u8; 80],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LINEREQMEDIACALL {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LINEREQMEDIACALL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LINEREQMEDIACALLW {
    pub hWnd: super::super::Foundation::HWND,
    pub wRequestID: super::super::Foundation::WPARAM,
    pub szDeviceClass: [u16; 40],
    pub ucDeviceID: [u8; 40],
    pub dwSize: u32,
    pub dwSecure: u32,
    pub szDestAddress: [u16; 80],
    pub szAppName: [u16; 40],
    pub szCalledParty: [u16; 40],
    pub szComment: [u16; 80],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LINEREQMEDIACALLW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LINEREQMEDIACALLW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINETERMCAPS {
    pub dwTermDev: u32,
    pub dwTermModes: u32,
    pub dwTermSharing: u32,
}
impl ::core::marker::Copy for LINETERMCAPS {}
impl ::core::clone::Clone for LINETERMCAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINETRANSLATECAPS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwNumLocations: u32,
    pub dwLocationListSize: u32,
    pub dwLocationListOffset: u32,
    pub dwCurrentLocationID: u32,
    pub dwNumCards: u32,
    pub dwCardListSize: u32,
    pub dwCardListOffset: u32,
    pub dwCurrentPreferredCardID: u32,
}
impl ::core::marker::Copy for LINETRANSLATECAPS {}
impl ::core::clone::Clone for LINETRANSLATECAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct LINETRANSLATEOUTPUT {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwDialableStringSize: u32,
    pub dwDialableStringOffset: u32,
    pub dwDisplayableStringSize: u32,
    pub dwDisplayableStringOffset: u32,
    pub dwCurrentCountry: u32,
    pub dwDestCountry: u32,
    pub dwTranslateResults: u32,
}
impl ::core::marker::Copy for LINETRANSLATEOUTPUT {}
impl ::core::clone::Clone for LINETRANSLATEOUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO {
    pub dwSize: u32,
    pub Event: MSP_EVENT,
    pub hCall: *mut i32,
    pub Anonymous: MSP_EVENT_INFO_0,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union MSP_EVENT_INFO_0 {
    pub MSP_ADDRESS_EVENT_INFO: MSP_EVENT_INFO_0_0,
    pub MSP_CALL_EVENT_INFO: MSP_EVENT_INFO_0_2,
    pub MSP_TSP_DATA: MSP_EVENT_INFO_0_6,
    pub MSP_PRIVATE_EVENT_INFO: MSP_EVENT_INFO_0_4,
    pub MSP_FILE_TERMINAL_EVENT_INFO: MSP_EVENT_INFO_0_3,
    pub MSP_ASR_TERMINAL_EVENT_INFO: MSP_EVENT_INFO_0_1,
    pub MSP_TTS_TERMINAL_EVENT_INFO: MSP_EVENT_INFO_0_7,
    pub MSP_TONE_TERMINAL_EVENT_INFO: MSP_EVENT_INFO_0_5,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_0 {
    pub Type: MSP_ADDRESS_EVENT,
    pub pTerminal: ITTerminal,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_1 {
    pub pASRTerminal: ITTerminal,
    pub hrErrorCode: ::windows_sys::core::HRESULT,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_1 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_2 {
    pub Type: MSP_CALL_EVENT,
    pub Cause: MSP_CALL_EVENT_CAUSE,
    pub pStream: ITStream,
    pub pTerminal: ITTerminal,
    pub hrError: ::windows_sys::core::HRESULT,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_2 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_3 {
    pub pParentFileTerminal: ITTerminal,
    pub pFileTrack: ITFileTrack,
    pub TerminalMediaState: TERMINAL_MEDIA_STATE,
    pub ftecEventCause: FT_STATE_EVENT_CAUSE,
    pub hrErrorCode: ::windows_sys::core::HRESULT,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_3 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_4 {
    pub pEvent: super::super::System::Com::IDispatch,
    pub lEventCode: i32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_4 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_5 {
    pub pToneTerminal: ITTerminal,
    pub hrErrorCode: ::windows_sys::core::HRESULT,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_5 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_6 {
    pub dwBufferSize: u32,
    pub pBuffer: [u8; 1],
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_6 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MSP_EVENT_INFO_0_7 {
    pub pTTSTerminal: ITTerminal,
    pub hrErrorCode: ::windows_sys::core::HRESULT,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MSP_EVENT_INFO_0_7 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MSP_EVENT_INFO_0_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NSID {
    pub dwSize: u32,
    pub uchType: [u8; 16],
    pub xtype: u32,
    pub lTime: i32,
    pub address: NSID_0,
}
impl ::core::marker::Copy for NSID {}
impl ::core::clone::Clone for NSID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union NSID_0 {
    pub alias: ADDRALIAS,
    pub rgchInterNet: [u8; 1],
}
impl ::core::marker::Copy for NSID_0 {}
impl ::core::clone::Clone for NSID_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PHONEBUTTONINFO {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwButtonMode: u32,
    pub dwButtonFunction: u32,
    pub dwButtonTextSize: u32,
    pub dwButtonTextOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwButtonState: u32,
}
impl ::core::marker::Copy for PHONEBUTTONINFO {}
impl ::core::clone::Clone for PHONEBUTTONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PHONECAPS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwProviderInfoSize: u32,
    pub dwProviderInfoOffset: u32,
    pub dwPhoneInfoSize: u32,
    pub dwPhoneInfoOffset: u32,
    pub dwPermanentPhoneID: u32,
    pub dwPhoneNameSize: u32,
    pub dwPhoneNameOffset: u32,
    pub dwStringFormat: u32,
    pub dwPhoneStates: u32,
    pub dwHookSwitchDevs: u32,
    pub dwHandsetHookSwitchModes: u32,
    pub dwSpeakerHookSwitchModes: u32,
    pub dwHeadsetHookSwitchModes: u32,
    pub dwVolumeFlags: u32,
    pub dwGainFlags: u32,
    pub dwDisplayNumRows: u32,
    pub dwDisplayNumColumns: u32,
    pub dwNumRingModes: u32,
    pub dwNumButtonLamps: u32,
    pub dwButtonModesSize: u32,
    pub dwButtonModesOffset: u32,
    pub dwButtonFunctionsSize: u32,
    pub dwButtonFunctionsOffset: u32,
    pub dwLampModesSize: u32,
    pub dwLampModesOffset: u32,
    pub dwNumSetData: u32,
    pub dwSetDataSize: u32,
    pub dwSetDataOffset: u32,
    pub dwNumGetData: u32,
    pub dwGetDataSize: u32,
    pub dwGetDataOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwDeviceClassesSize: u32,
    pub dwDeviceClassesOffset: u32,
    pub dwPhoneFeatures: u32,
    pub dwSettableHandsetHookSwitchModes: u32,
    pub dwSettableSpeakerHookSwitchModes: u32,
    pub dwSettableHeadsetHookSwitchModes: u32,
    pub dwMonitoredHandsetHookSwitchModes: u32,
    pub dwMonitoredSpeakerHookSwitchModes: u32,
    pub dwMonitoredHeadsetHookSwitchModes: u32,
    pub PermanentPhoneGuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for PHONECAPS {}
impl ::core::clone::Clone for PHONECAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PHONEEXTENSIONID {
    pub dwExtensionID0: u32,
    pub dwExtensionID1: u32,
    pub dwExtensionID2: u32,
    pub dwExtensionID3: u32,
}
impl ::core::marker::Copy for PHONEEXTENSIONID {}
impl ::core::clone::Clone for PHONEEXTENSIONID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PHONEINITIALIZEEXPARAMS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwOptions: u32,
    pub Handles: PHONEINITIALIZEEXPARAMS_0,
    pub dwCompletionKey: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PHONEINITIALIZEEXPARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PHONEINITIALIZEEXPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union PHONEINITIALIZEEXPARAMS_0 {
    pub hEvent: super::super::Foundation::HANDLE,
    pub hCompletionPort: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PHONEINITIALIZEEXPARAMS_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PHONEINITIALIZEEXPARAMS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PHONEMESSAGE {
    pub hDevice: u32,
    pub dwMessageID: u32,
    pub dwCallbackInstance: usize,
    pub dwParam1: usize,
    pub dwParam2: usize,
    pub dwParam3: usize,
}
impl ::core::marker::Copy for PHONEMESSAGE {}
impl ::core::clone::Clone for PHONEMESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PHONESTATUS {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwStatusFlags: u32,
    pub dwNumOwners: u32,
    pub dwNumMonitors: u32,
    pub dwRingMode: u32,
    pub dwRingVolume: u32,
    pub dwHandsetHookSwitchMode: u32,
    pub dwHandsetVolume: u32,
    pub dwHandsetGain: u32,
    pub dwSpeakerHookSwitchMode: u32,
    pub dwSpeakerVolume: u32,
    pub dwSpeakerGain: u32,
    pub dwHeadsetHookSwitchMode: u32,
    pub dwHeadsetVolume: u32,
    pub dwHeadsetGain: u32,
    pub dwDisplaySize: u32,
    pub dwDisplayOffset: u32,
    pub dwLampModesSize: u32,
    pub dwLampModesOffset: u32,
    pub dwOwnerNameSize: u32,
    pub dwOwnerNameOffset: u32,
    pub dwDevSpecificSize: u32,
    pub dwDevSpecificOffset: u32,
    pub dwPhoneFeatures: u32,
}
impl ::core::marker::Copy for PHONESTATUS {}
impl ::core::clone::Clone for PHONESTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct RENDDATA {
    pub atyp: u16,
    pub ulPosition: u32,
    pub dxWidth: u16,
    pub dyHeight: u16,
    pub dwFlags: u32,
}
impl ::core::marker::Copy for RENDDATA {}
impl ::core::clone::Clone for RENDDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STnefProblem {
    pub ulComponent: u32,
    pub ulAttribute: u32,
    pub ulPropTag: u32,
    pub scode: i32,
}
impl ::core::marker::Copy for STnefProblem {}
impl ::core::clone::Clone for STnefProblem {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct STnefProblemArray {
    pub cProblem: u32,
    pub aProblem: [STnefProblem; 1],
}
impl ::core::marker::Copy for STnefProblemArray {}
impl ::core::clone::Clone for STnefProblemArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TAPI_CUSTOMTONE {
    pub dwFrequency: u32,
    pub dwCadenceOn: u32,
    pub dwCadenceOff: u32,
    pub dwVolume: u32,
}
impl ::core::marker::Copy for TAPI_CUSTOMTONE {}
impl ::core::clone::Clone for TAPI_CUSTOMTONE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TAPI_DETECTTONE {
    pub dwAppSpecific: u32,
    pub dwDuration: u32,
    pub dwFrequency1: u32,
    pub dwFrequency2: u32,
    pub dwFrequency3: u32,
}
impl ::core::marker::Copy for TAPI_DETECTTONE {}
impl ::core::clone::Clone for TAPI_DETECTTONE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRP {
    pub trpid: u16,
    pub cbgrtrp: u16,
    pub cch: u16,
    pub cbRgb: u16,
}
impl ::core::marker::Copy for TRP {}
impl ::core::clone::Clone for TRP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TUISPICREATEDIALOGINSTANCEPARAMS {
    pub dwRequestID: u32,
    pub hdDlgInst: HDRVDIALOGINSTANCE,
    pub htDlgInst: u32,
    pub lpszUIDLLName: ::windows_sys::core::PCWSTR,
    pub lpParams: *mut ::core::ffi::c_void,
    pub dwSize: u32,
}
impl ::core::marker::Copy for TUISPICREATEDIALOGINSTANCEPARAMS {}
impl ::core::clone::Clone for TUISPICREATEDIALOGINSTANCEPARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct VARSTRING {
    pub dwTotalSize: u32,
    pub dwNeededSize: u32,
    pub dwUsedSize: u32,
    pub dwStringFormat: u32,
    pub dwStringSize: u32,
    pub dwStringOffset: u32,
}
impl ::core::marker::Copy for VARSTRING {}
impl ::core::clone::Clone for VARSTRING {
    fn clone(&self) -> Self {
        *self
    }
}
pub type ASYNC_COMPLETION = ::core::option::Option<unsafe extern "system" fn(dwrequestid: u32, lresult: i32) -> ()>;
pub type LINECALLBACK = ::core::option::Option<unsafe extern "system" fn(hdevice: u32, dwmessage: u32, dwinstance: usize, dwparam1: usize, dwparam2: usize, dwparam3: usize) -> ()>;
pub type LINEEVENT = ::core::option::Option<unsafe extern "system" fn(htline: HTAPILINE, htcall: HTAPICALL, dwmsg: u32, dwparam1: usize, dwparam2: usize, dwparam3: usize) -> ()>;
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub type LPGETTNEFSTREAMCODEPAGE = ::core::option::Option<unsafe extern "system" fn(lpstream: super::super::System::Com::IStream, lpulcodepage: *mut u32, lpulsubcodepage: *mut u32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_System_AddressBook\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_System_AddressBook", feature = "Win32_System_Com"))]
pub type LPOPENTNEFSTREAM = ::core::option::Option<unsafe extern "system" fn(lpvsupport: *mut ::core::ffi::c_void, lpstream: super::super::System::Com::IStream, lpszstreamname: *const i8, ulflags: u32, lpmessage: super::super::System::AddressBook::IMessage, wkeyval: u16, lpptnef: *mut ITnef) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_System_AddressBook\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_System_AddressBook", feature = "Win32_System_Com"))]
pub type LPOPENTNEFSTREAMEX = ::core::option::Option<unsafe extern "system" fn(lpvsupport: *mut ::core::ffi::c_void, lpstream: super::super::System::Com::IStream, lpszstreamname: *const i8, ulflags: u32, lpmessage: super::super::System::AddressBook::IMessage, wkeyval: u16, lpadressbook: super::super::System::AddressBook::IAddrBook, lpptnef: *mut ITnef) -> ::windows_sys::core::HRESULT>;
pub type PHONECALLBACK = ::core::option::Option<unsafe extern "system" fn(hdevice: u32, dwmessage: u32, dwinstance: usize, dwparam1: usize, dwparam2: usize, dwparam3: usize) -> ()>;
pub type PHONEEVENT = ::core::option::Option<unsafe extern "system" fn(htphone: HTAPIPHONE, dwmsg: u32, dwparam1: usize, dwparam2: usize, dwparam3: usize) -> ()>;
pub type TUISPIDLLCALLBACK = ::core::option::Option<unsafe extern "system" fn(dwobjectid: usize, dwobjecttype: u32, lpparams: *mut ::core::ffi::c_void, dwsize: u32) -> i32>;
