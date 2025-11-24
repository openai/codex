#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn BuildDisplayTable(lpallocatebuffer : LPALLOCATEBUFFER, lpallocatemore : LPALLOCATEMORE, lpfreebuffer : LPFREEBUFFER, lpmalloc : super::Com:: IMalloc, hinstance : super::super::Foundation:: HINSTANCE, cpages : u32, lppage : *mut DTPAGE, ulflags : u32, lpptable : *mut IMAPITable, lpptbldata : *mut ITableData) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChangeIdleRoutine(ftg : *mut ::core::ffi::c_void, lpfnidle : PFNIDLE, lpvidleparam : *mut ::core::ffi::c_void, priidle : i16, csecidle : u32, iroidle : u16, ircidle : u16) -> ());
::windows_targets::link!("mapi32.dll" "system" fn CreateIProp(lpinterface : *mut ::windows_sys::core::GUID, lpallocatebuffer : LPALLOCATEBUFFER, lpallocatemore : LPALLOCATEMORE, lpfreebuffer : LPFREEBUFFER, lpvreserved : *mut ::core::ffi::c_void, lpppropdata : *mut IPropData) -> i32);
::windows_targets::link!("rtm.dll" "system" fn CreateTable(lpinterface : *mut ::windows_sys::core::GUID, lpallocatebuffer : LPALLOCATEBUFFER, lpallocatemore : LPALLOCATEMORE, lpfreebuffer : LPFREEBUFFER, lpvreserved : *mut ::core::ffi::c_void, ultabletype : u32, ulproptagindexcolumn : u32, lpsproptagarraycolumns : *mut SPropTagArray, lpptabledata : *mut ITableData) -> i32);
::windows_targets::link!("mapi32.dll" "system" fn DeinitMapiUtil() -> ());
::windows_targets::link!("mapi32.dll" "system" fn DeregisterIdleRoutine(ftg : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnableIdleRoutine(ftg : *mut ::core::ffi::c_void, fenable : super::super::Foundation:: BOOL) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FEqualNames(lpname1 : *mut MAPINAMEID, lpname2 : *mut MAPINAMEID) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn FPropCompareProp(lpspropvalue1 : *mut SPropValue, ulrelop : u32, lpspropvalue2 : *mut SPropValue) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn FPropContainsProp(lpspropvaluedst : *mut SPropValue, lpspropvaluesrc : *mut SPropValue, ulfuzzylevel : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FPropExists(lpmapiprop : IMAPIProp, ulproptag : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn FreePadrlist(lpadrlist : *mut ADRLIST) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn FreeProws(lprows : *mut SRowSet) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FtAddFt(ftaddend1 : super::super::Foundation:: FILETIME, ftaddend2 : super::super::Foundation:: FILETIME) -> super::super::Foundation:: FILETIME);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FtMulDw(ftmultiplier : u32, ftmultiplicand : super::super::Foundation:: FILETIME) -> super::super::Foundation:: FILETIME);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FtMulDwDw(ftmultiplicand : u32, ftmultiplier : u32) -> super::super::Foundation:: FILETIME);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FtNegFt(ft : super::super::Foundation:: FILETIME) -> super::super::Foundation:: FILETIME);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FtSubFt(ftminuend : super::super::Foundation:: FILETIME, ftsubtrahend : super::super::Foundation:: FILETIME) -> super::super::Foundation:: FILETIME);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FtgRegisterIdleRoutine(lpfnidle : PFNIDLE, lpvidleparam : *mut ::core::ffi::c_void, priidle : i16, csecidle : u32, iroidle : u16) -> *mut ::core::ffi::c_void);
::windows_targets::link!("mapi32.dll" "system" fn HrAddColumns(lptbl : IMAPITable, lpproptagcolumnsnew : *mut SPropTagArray, lpallocatebuffer : LPALLOCATEBUFFER, lpfreebuffer : LPFREEBUFFER) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mapi32.dll" "system" fn HrAddColumnsEx(lptbl : IMAPITable, lpproptagcolumnsnew : *mut SPropTagArray, lpallocatebuffer : LPALLOCATEBUFFER, lpfreebuffer : LPFREEBUFFER, lpfnfiltercolumns : isize) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn HrAllocAdviseSink(lpfncallback : LPNOTIFCALLBACK, lpvcontext : *mut ::core::ffi::c_void, lppadvisesink : *mut IMAPIAdviseSink) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mapi32.dll" "system" fn HrDispatchNotifications(ulflags : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn HrGetOneProp(lpmapiprop : IMAPIProp, ulproptag : u32, lppprop : *mut *mut SPropValue) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn HrIStorageFromStream(lpunkin : ::windows_sys::core::IUnknown, lpinterface : *mut ::windows_sys::core::GUID, ulflags : u32, lppstorageout : *mut super::Com::StructuredStorage:: IStorage) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn HrQueryAllRows(lptable : IMAPITable, lpproptags : *mut SPropTagArray, lprestriction : *mut SRestriction, lpsortorderset : *mut SSortOrderSet, crowsmax : i32, lpprows : *mut *mut SRowSet) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn HrSetOneProp(lpmapiprop : IMAPIProp, lpprop : *mut SPropValue) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mapi32.dll" "system" fn HrThisThreadAdviseSink(lpadvisesink : IMAPIAdviseSink, lppadvisesink : *mut IMAPIAdviseSink) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn LPropCompareProp(lpspropvaluea : *mut SPropValue, lpspropvalueb : *mut SPropValue) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn LpValFindProp(ulproptag : u32, cvalues : u32, lpproparray : *mut SPropValue) -> *mut SPropValue);
::windows_targets::link!("mapi32.dll" "system" fn MAPIDeinitIdle() -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn MAPIGetDefaultMalloc() -> super::Com:: IMalloc);
::windows_targets::link!("mapi32.dll" "system" fn MAPIInitIdle(lpvreserved : *mut ::core::ffi::c_void) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn OpenStreamOnFile(lpallocatebuffer : LPALLOCATEBUFFER, lpfreebuffer : LPFREEBUFFER, ulflags : u32, lpszfilename : *const i8, lpszprefix : *const i8, lppstream : *mut super::Com:: IStream) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn PpropFindProp(lpproparray : *mut SPropValue, cvalues : u32, ulproptag : u32) -> *mut SPropValue);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn PropCopyMore(lpspropvaluedest : *mut SPropValue, lpspropvaluesrc : *mut SPropValue, lpfallocmore : LPALLOCATEMORE, lpvobject : *mut ::core::ffi::c_void) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RTFSync(lpmessage : IMessage, ulflags : u32, lpfmessageupdated : *mut super::super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ScCopyNotifications(cnotification : i32, lpnotifications : *mut NOTIFICATION, lpvdst : *mut ::core::ffi::c_void, lpcb : *mut u32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ScCopyProps(cvalues : i32, lpproparray : *mut SPropValue, lpvdst : *mut ::core::ffi::c_void, lpcb : *mut u32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ScCountNotifications(cnotifications : i32, lpnotifications : *mut NOTIFICATION, lpcb : *mut u32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ScCountProps(cvalues : i32, lpproparray : *mut SPropValue, lpcb : *mut u32) -> i32);
::windows_targets::link!("mapi32.dll" "system" fn ScCreateConversationIndex(cbparent : u32, lpbparent : *mut u8, lpcbconvindex : *mut u32, lppbconvindex : *mut *mut u8) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ScDupPropset(cvalues : i32, lpproparray : *mut SPropValue, lpallocatebuffer : LPALLOCATEBUFFER, lppproparray : *mut *mut SPropValue) -> i32);
::windows_targets::link!("mapi32.dll" "system" fn ScInitMapiUtil(ulflags : u32) -> i32);
::windows_targets::link!("mapi32.dll" "system" fn ScLocalPathFromUNC(lpszunc : ::windows_sys::core::PCSTR, lpszlocal : ::windows_sys::core::PCSTR, cchlocal : u32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ScRelocNotifications(cnotification : i32, lpnotifications : *mut NOTIFICATION, lpvbaseold : *mut ::core::ffi::c_void, lpvbasenew : *mut ::core::ffi::c_void, lpcb : *mut u32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn ScRelocProps(cvalues : i32, lpproparray : *mut SPropValue, lpvbaseold : *mut ::core::ffi::c_void, lpvbasenew : *mut ::core::ffi::c_void, lpcb : *mut u32) -> i32);
::windows_targets::link!("mapi32.dll" "system" fn ScUNCFromLocalPath(lpszlocal : ::windows_sys::core::PCSTR, lpszunc : ::windows_sys::core::PCSTR, cchunc : u32) -> i32);
::windows_targets::link!("mapi32.dll" "system" fn SzFindCh(lpsz : *mut i8, ch : u16) -> *mut i8);
::windows_targets::link!("mapi32.dll" "system" fn SzFindLastCh(lpsz : *mut i8, ch : u16) -> *mut i8);
::windows_targets::link!("mapi32.dll" "system" fn SzFindSz(lpsz : *mut i8, lpszkey : *mut i8) -> *mut i8);
::windows_targets::link!("mapi32.dll" "system" fn UFromSz(lpsz : *mut i8) -> u32);
::windows_targets::link!("mapi32.dll" "system" fn UlAddRef(lpunk : *mut ::core::ffi::c_void) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"] fn UlPropSize(lpspropvalue : *mut SPropValue) -> u32);
::windows_targets::link!("mapi32.dll" "system" fn UlRelease(lpunk : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("mapi32.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn WrapCompressedRTFStream(lpcompressedrtfstream : super::Com:: IStream, ulflags : u32, lpuncompressedrtfstream : *mut super::Com:: IStream) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mapi32.dll" "system" fn WrapStoreEntryID(ulflags : u32, lpszdllname : *const i8, cborigentry : u32, lporigentry : *const ENTRYID, lpcbwrappedentry : *mut u32, lppwrappedentry : *mut *mut ENTRYID) -> ::windows_sys::core::HRESULT);
pub type IABContainer = *mut ::core::ffi::c_void;
pub type IAddrBook = *mut ::core::ffi::c_void;
pub type IAttach = *mut ::core::ffi::c_void;
pub type IDistList = *mut ::core::ffi::c_void;
pub type IMAPIAdviseSink = *mut ::core::ffi::c_void;
pub type IMAPIContainer = *mut ::core::ffi::c_void;
pub type IMAPIControl = *mut ::core::ffi::c_void;
pub type IMAPIFolder = *mut ::core::ffi::c_void;
pub type IMAPIProgress = *mut ::core::ffi::c_void;
pub type IMAPIProp = *mut ::core::ffi::c_void;
pub type IMAPIStatus = *mut ::core::ffi::c_void;
pub type IMAPITable = *mut ::core::ffi::c_void;
pub type IMailUser = *mut ::core::ffi::c_void;
pub type IMessage = *mut ::core::ffi::c_void;
pub type IMsgStore = *mut ::core::ffi::c_void;
pub type IProfSect = *mut ::core::ffi::c_void;
pub type IPropData = *mut ::core::ffi::c_void;
pub type IProviderAdmin = *mut ::core::ffi::c_void;
pub type ITableData = *mut ::core::ffi::c_void;
pub type IWABExtInit = *mut ::core::ffi::c_void;
pub type IWABObject = *mut ::core::ffi::c_void;
pub const E_IMAPI_BURN_VERIFICATION_FAILED: ::windows_sys::core::HRESULT = -1062600697i32;
pub const E_IMAPI_DF2DATA_CLIENT_NAME_IS_NOT_VALID: ::windows_sys::core::HRESULT = -1062599672i32;
pub const E_IMAPI_DF2DATA_INVALID_MEDIA_STATE: ::windows_sys::core::HRESULT = -1062599678i32;
pub const E_IMAPI_DF2DATA_MEDIA_IS_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599674i32;
pub const E_IMAPI_DF2DATA_MEDIA_NOT_BLANK: ::windows_sys::core::HRESULT = -1062599675i32;
pub const E_IMAPI_DF2DATA_RECORDER_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599673i32;
pub const E_IMAPI_DF2DATA_STREAM_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599677i32;
pub const E_IMAPI_DF2DATA_STREAM_TOO_LARGE_FOR_CURRENT_MEDIA: ::windows_sys::core::HRESULT = -1062599676i32;
pub const E_IMAPI_DF2DATA_WRITE_IN_PROGRESS: ::windows_sys::core::HRESULT = -1062599680i32;
pub const E_IMAPI_DF2DATA_WRITE_NOT_IN_PROGRESS: ::windows_sys::core::HRESULT = -1062599679i32;
pub const E_IMAPI_DF2RAW_CLIENT_NAME_IS_NOT_VALID: ::windows_sys::core::HRESULT = -1062599164i32;
pub const E_IMAPI_DF2RAW_DATA_BLOCK_TYPE_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599154i32;
pub const E_IMAPI_DF2RAW_MEDIA_IS_NOT_BLANK: ::windows_sys::core::HRESULT = -1062599162i32;
pub const E_IMAPI_DF2RAW_MEDIA_IS_NOT_PREPARED: ::windows_sys::core::HRESULT = -1062599166i32;
pub const E_IMAPI_DF2RAW_MEDIA_IS_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599161i32;
pub const E_IMAPI_DF2RAW_MEDIA_IS_PREPARED: ::windows_sys::core::HRESULT = -1062599165i32;
pub const E_IMAPI_DF2RAW_NOT_ENOUGH_SPACE: ::windows_sys::core::HRESULT = -1062599159i32;
pub const E_IMAPI_DF2RAW_NO_RECORDER_SPECIFIED: ::windows_sys::core::HRESULT = -1062599158i32;
pub const E_IMAPI_DF2RAW_RECORDER_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599152i32;
pub const E_IMAPI_DF2RAW_STREAM_LEADIN_TOO_SHORT: ::windows_sys::core::HRESULT = -1062599153i32;
pub const E_IMAPI_DF2RAW_STREAM_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599155i32;
pub const E_IMAPI_DF2RAW_WRITE_IN_PROGRESS: ::windows_sys::core::HRESULT = -1062599168i32;
pub const E_IMAPI_DF2RAW_WRITE_NOT_IN_PROGRESS: ::windows_sys::core::HRESULT = -1062599167i32;
pub const E_IMAPI_DF2TAO_CLIENT_NAME_IS_NOT_VALID: ::windows_sys::core::HRESULT = -1062599409i32;
pub const E_IMAPI_DF2TAO_INVALID_ISRC: ::windows_sys::core::HRESULT = -1062599413i32;
pub const E_IMAPI_DF2TAO_INVALID_MCN: ::windows_sys::core::HRESULT = -1062599412i32;
pub const E_IMAPI_DF2TAO_MEDIA_IS_NOT_BLANK: ::windows_sys::core::HRESULT = -1062599418i32;
pub const E_IMAPI_DF2TAO_MEDIA_IS_NOT_PREPARED: ::windows_sys::core::HRESULT = -1062599422i32;
pub const E_IMAPI_DF2TAO_MEDIA_IS_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599417i32;
pub const E_IMAPI_DF2TAO_MEDIA_IS_PREPARED: ::windows_sys::core::HRESULT = -1062599421i32;
pub const E_IMAPI_DF2TAO_NOT_ENOUGH_SPACE: ::windows_sys::core::HRESULT = -1062599415i32;
pub const E_IMAPI_DF2TAO_NO_RECORDER_SPECIFIED: ::windows_sys::core::HRESULT = -1062599414i32;
pub const E_IMAPI_DF2TAO_PROPERTY_FOR_BLANK_MEDIA_ONLY: ::windows_sys::core::HRESULT = -1062599420i32;
pub const E_IMAPI_DF2TAO_RECORDER_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599410i32;
pub const E_IMAPI_DF2TAO_STREAM_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062599411i32;
pub const E_IMAPI_DF2TAO_TABLE_OF_CONTENTS_EMPTY_DISC: ::windows_sys::core::HRESULT = -1062599419i32;
pub const E_IMAPI_DF2TAO_TRACK_LIMIT_REACHED: ::windows_sys::core::HRESULT = -1062599416i32;
pub const E_IMAPI_DF2TAO_WRITE_IN_PROGRESS: ::windows_sys::core::HRESULT = -1062599424i32;
pub const E_IMAPI_DF2TAO_WRITE_NOT_IN_PROGRESS: ::windows_sys::core::HRESULT = -1062599423i32;
pub const E_IMAPI_ERASE_CLIENT_NAME_IS_NOT_VALID: ::windows_sys::core::HRESULT = -1062598389i32;
pub const E_IMAPI_ERASE_DISC_INFORMATION_TOO_SMALL: ::windows_sys::core::HRESULT = -2136340222i32;
pub const E_IMAPI_ERASE_DRIVE_FAILED_ERASE_COMMAND: ::windows_sys::core::HRESULT = -2136340219i32;
pub const E_IMAPI_ERASE_DRIVE_FAILED_SPINUP_COMMAND: ::windows_sys::core::HRESULT = -2136340216i32;
pub const E_IMAPI_ERASE_MEDIA_IS_NOT_ERASABLE: ::windows_sys::core::HRESULT = -2136340220i32;
pub const E_IMAPI_ERASE_MEDIA_IS_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062598391i32;
pub const E_IMAPI_ERASE_MODE_PAGE_2A_TOO_SMALL: ::windows_sys::core::HRESULT = -2136340221i32;
pub const E_IMAPI_ERASE_ONLY_ONE_RECORDER_SUPPORTED: ::windows_sys::core::HRESULT = -2136340223i32;
pub const E_IMAPI_ERASE_RECORDER_IN_USE: ::windows_sys::core::HRESULT = -2136340224i32;
pub const E_IMAPI_ERASE_RECORDER_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062598390i32;
pub const E_IMAPI_ERASE_TOOK_LONGER_THAN_ONE_HOUR: ::windows_sys::core::HRESULT = -2136340218i32;
pub const E_IMAPI_ERASE_UNEXPECTED_DRIVE_RESPONSE_DURING_ERASE: ::windows_sys::core::HRESULT = -2136340217i32;
pub const E_IMAPI_LOSS_OF_STREAMING: ::windows_sys::core::HRESULT = -1062599936i32;
pub const E_IMAPI_RAW_IMAGE_INSUFFICIENT_SPACE: ::windows_sys::core::HRESULT = -2136339963i32;
pub const E_IMAPI_RAW_IMAGE_IS_READ_ONLY: ::windows_sys::core::HRESULT = -2136339968i32;
pub const E_IMAPI_RAW_IMAGE_NO_TRACKS: ::windows_sys::core::HRESULT = -2136339965i32;
pub const E_IMAPI_RAW_IMAGE_SECTOR_TYPE_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -2136339966i32;
pub const E_IMAPI_RAW_IMAGE_TOO_MANY_TRACKS: ::windows_sys::core::HRESULT = -2136339967i32;
pub const E_IMAPI_RAW_IMAGE_TOO_MANY_TRACK_INDEXES: ::windows_sys::core::HRESULT = -2136339962i32;
pub const E_IMAPI_RAW_IMAGE_TRACKS_ALREADY_ADDED: ::windows_sys::core::HRESULT = -2136339964i32;
pub const E_IMAPI_RAW_IMAGE_TRACK_INDEX_NOT_FOUND: ::windows_sys::core::HRESULT = -2136339961i32;
pub const E_IMAPI_RAW_IMAGE_TRACK_INDEX_OFFSET_ZERO_CANNOT_BE_CLEARED: ::windows_sys::core::HRESULT = -2136339959i32;
pub const E_IMAPI_RAW_IMAGE_TRACK_INDEX_TOO_CLOSE_TO_OTHER_INDEX: ::windows_sys::core::HRESULT = -2136339958i32;
pub const E_IMAPI_RECORDER_CLIENT_NAME_IS_NOT_VALID: ::windows_sys::core::HRESULT = -1062600175i32;
pub const E_IMAPI_RECORDER_COMMAND_TIMEOUT: ::windows_sys::core::HRESULT = -1062600179i32;
pub const E_IMAPI_RECORDER_DVD_STRUCTURE_NOT_PRESENT: ::windows_sys::core::HRESULT = -1062600178i32;
pub const E_IMAPI_RECORDER_FEATURE_IS_NOT_CURRENT: ::windows_sys::core::HRESULT = -1062600181i32;
pub const E_IMAPI_RECORDER_GET_CONFIGURATION_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062600180i32;
pub const E_IMAPI_RECORDER_INVALID_MODE_PARAMETERS: ::windows_sys::core::HRESULT = -1062600184i32;
pub const E_IMAPI_RECORDER_INVALID_RESPONSE_FROM_DEVICE: ::windows_sys::core::HRESULT = -1062599937i32;
pub const E_IMAPI_RECORDER_LOCKED: ::windows_sys::core::HRESULT = -1062600176i32;
pub const E_IMAPI_RECORDER_MEDIA_BECOMING_READY: ::windows_sys::core::HRESULT = -1062600187i32;
pub const E_IMAPI_RECORDER_MEDIA_BUSY: ::windows_sys::core::HRESULT = -1062600185i32;
pub const E_IMAPI_RECORDER_MEDIA_FORMAT_IN_PROGRESS: ::windows_sys::core::HRESULT = -1062600186i32;
pub const E_IMAPI_RECORDER_MEDIA_INCOMPATIBLE: ::windows_sys::core::HRESULT = -1062600189i32;
pub const E_IMAPI_RECORDER_MEDIA_NOT_FORMATTED: ::windows_sys::core::HRESULT = -1062600174i32;
pub const E_IMAPI_RECORDER_MEDIA_NO_MEDIA: ::windows_sys::core::HRESULT = -1062600190i32;
pub const E_IMAPI_RECORDER_MEDIA_SPEED_MISMATCH: ::windows_sys::core::HRESULT = -1062600177i32;
pub const E_IMAPI_RECORDER_MEDIA_UPSIDE_DOWN: ::windows_sys::core::HRESULT = -1062600188i32;
pub const E_IMAPI_RECORDER_MEDIA_WRITE_PROTECTED: ::windows_sys::core::HRESULT = -1062600183i32;
pub const E_IMAPI_RECORDER_NO_SUCH_FEATURE: ::windows_sys::core::HRESULT = -1062600182i32;
pub const E_IMAPI_RECORDER_NO_SUCH_MODE_PAGE: ::windows_sys::core::HRESULT = -1062600191i32;
pub const E_IMAPI_RECORDER_REQUIRED: ::windows_sys::core::HRESULT = -1062600701i32;
pub const E_IMAPI_REQUEST_CANCELLED: ::windows_sys::core::HRESULT = -1062600702i32;
pub const E_IMAPI_UNEXPECTED_RESPONSE_FROM_DEVICE: ::windows_sys::core::HRESULT = -1062599935i32;
pub const FACILITY_IMAPI2: u32 = 170u32;
pub const IMAPI_E_BAD_MULTISESSION_PARAMETER: ::windows_sys::core::HRESULT = -1062555294i32;
pub const IMAPI_E_BOOT_EMULATION_IMAGE_SIZE_MISMATCH: ::windows_sys::core::HRESULT = -1062555318i32;
pub const IMAPI_E_BOOT_IMAGE_DATA: ::windows_sys::core::HRESULT = -1062555320i32;
pub const IMAPI_E_BOOT_OBJECT_CONFLICT: ::windows_sys::core::HRESULT = -1062555319i32;
pub const IMAPI_E_DATA_STREAM_CREATE_FAILURE: ::windows_sys::core::HRESULT = -1062555350i32;
pub const IMAPI_E_DATA_STREAM_INCONSISTENCY: ::windows_sys::core::HRESULT = -1062555352i32;
pub const IMAPI_E_DATA_STREAM_READ_FAILURE: ::windows_sys::core::HRESULT = -1062555351i32;
pub const IMAPI_E_DATA_TOO_BIG: ::windows_sys::core::HRESULT = -1062555342i32;
pub const IMAPI_E_DIRECTORY_READ_FAILURE: ::windows_sys::core::HRESULT = -1062555349i32;
pub const IMAPI_E_DIR_NOT_EMPTY: ::windows_sys::core::HRESULT = -1062555382i32;
pub const IMAPI_E_DIR_NOT_FOUND: ::windows_sys::core::HRESULT = -1062555366i32;
pub const IMAPI_E_DISC_MISMATCH: ::windows_sys::core::HRESULT = -1062555304i32;
pub const IMAPI_E_DUP_NAME: ::windows_sys::core::HRESULT = -1062555374i32;
pub const IMAPI_E_EMPTY_DISC: ::windows_sys::core::HRESULT = -1062555312i32;
pub const IMAPI_E_FILE_NOT_FOUND: ::windows_sys::core::HRESULT = -1062555367i32;
pub const IMAPI_E_FILE_SYSTEM_CHANGE_NOT_ALLOWED: ::windows_sys::core::HRESULT = -1062555293i32;
pub const IMAPI_E_FILE_SYSTEM_FEATURE_NOT_SUPPORTED: ::windows_sys::core::HRESULT = -1062555308i32;
pub const IMAPI_E_FILE_SYSTEM_NOT_EMPTY: ::windows_sys::core::HRESULT = -1062555386i32;
pub const IMAPI_E_FILE_SYSTEM_NOT_FOUND: ::windows_sys::core::HRESULT = -1062555310i32;
pub const IMAPI_E_FILE_SYSTEM_READ_CONSISTENCY_ERROR: ::windows_sys::core::HRESULT = -1062555309i32;
pub const IMAPI_E_FSI_INTERNAL_ERROR: ::windows_sys::core::HRESULT = -1062555392i32;
pub const IMAPI_E_IMAGEMANAGER_IMAGE_NOT_ALIGNED: ::windows_sys::core::HRESULT = -1062555136i32;
pub const IMAPI_E_IMAGEMANAGER_IMAGE_TOO_BIG: ::windows_sys::core::HRESULT = -1062555133i32;
pub const IMAPI_E_IMAGEMANAGER_NO_IMAGE: ::windows_sys::core::HRESULT = -1062555134i32;
pub const IMAPI_E_IMAGEMANAGER_NO_VALID_VD_FOUND: ::windows_sys::core::HRESULT = -1062555135i32;
pub const IMAPI_E_IMAGE_SIZE_LIMIT: ::windows_sys::core::HRESULT = -1062555360i32;
pub const IMAPI_E_IMAGE_TOO_BIG: ::windows_sys::core::HRESULT = -1062555359i32;
pub const IMAPI_E_IMPORT_MEDIA_NOT_ALLOWED: ::windows_sys::core::HRESULT = -1062555303i32;
pub const IMAPI_E_IMPORT_READ_FAILURE: ::windows_sys::core::HRESULT = -1062555305i32;
pub const IMAPI_E_IMPORT_SEEK_FAILURE: ::windows_sys::core::HRESULT = -1062555306i32;
pub const IMAPI_E_IMPORT_TYPE_COLLISION_DIRECTORY_EXISTS_AS_FILE: ::windows_sys::core::HRESULT = -1062555298i32;
pub const IMAPI_E_IMPORT_TYPE_COLLISION_FILE_EXISTS_AS_DIRECTORY: ::windows_sys::core::HRESULT = -1062555307i32;
pub const IMAPI_E_INCOMPATIBLE_MULTISESSION_TYPE: ::windows_sys::core::HRESULT = -1062555301i32;
pub const IMAPI_E_INCOMPATIBLE_PREVIOUS_SESSION: ::windows_sys::core::HRESULT = -1062555341i32;
pub const IMAPI_E_INVALID_DATE: ::windows_sys::core::HRESULT = -1062555387i32;
pub const IMAPI_E_INVALID_PARAM: ::windows_sys::core::HRESULT = -1062555391i32;
pub const IMAPI_E_INVALID_PATH: ::windows_sys::core::HRESULT = -1062555376i32;
pub const IMAPI_E_INVALID_VOLUME_NAME: ::windows_sys::core::HRESULT = -1062555388i32;
pub const IMAPI_E_INVALID_WORKING_DIRECTORY: ::windows_sys::core::HRESULT = -1062555328i32;
pub const IMAPI_E_ISO9660_LEVELS: ::windows_sys::core::HRESULT = -1062555343i32;
pub const IMAPI_E_ITEM_NOT_FOUND: ::windows_sys::core::HRESULT = -1062555368i32;
pub const IMAPI_E_MULTISESSION_NOT_SET: ::windows_sys::core::HRESULT = -1062555299i32;
pub const IMAPI_E_NOT_DIR: ::windows_sys::core::HRESULT = -1062555383i32;
pub const IMAPI_E_NOT_FILE: ::windows_sys::core::HRESULT = -1062555384i32;
pub const IMAPI_E_NOT_IN_FILE_SYSTEM: ::windows_sys::core::HRESULT = -1062555381i32;
pub const IMAPI_E_NO_COMPATIBLE_MULTISESSION_TYPE: ::windows_sys::core::HRESULT = -1062555300i32;
pub const IMAPI_E_NO_OUTPUT: ::windows_sys::core::HRESULT = -1062555389i32;
pub const IMAPI_E_NO_SUPPORTED_FILE_SYSTEM: ::windows_sys::core::HRESULT = -1062555311i32;
pub const IMAPI_E_NO_UNIQUE_NAME: ::windows_sys::core::HRESULT = -1062555373i32;
pub const IMAPI_E_PROPERTY_NOT_ACCESSIBLE: ::windows_sys::core::HRESULT = -1062555296i32;
pub const IMAPI_E_READONLY: ::windows_sys::core::HRESULT = -1062555390i32;
pub const IMAPI_E_RESTRICTED_NAME_VIOLATION: ::windows_sys::core::HRESULT = -1062555375i32;
pub const IMAPI_E_STASHFILE_MOVE: ::windows_sys::core::HRESULT = -1062555326i32;
pub const IMAPI_E_STASHFILE_OPEN_FAILURE: ::windows_sys::core::HRESULT = -1062555336i32;
pub const IMAPI_E_STASHFILE_READ_FAILURE: ::windows_sys::core::HRESULT = -1062555333i32;
pub const IMAPI_E_STASHFILE_SEEK_FAILURE: ::windows_sys::core::HRESULT = -1062555335i32;
pub const IMAPI_E_STASHFILE_WRITE_FAILURE: ::windows_sys::core::HRESULT = -1062555334i32;
pub const IMAPI_E_TOO_MANY_DIRS: ::windows_sys::core::HRESULT = -1062555344i32;
pub const IMAPI_E_UDF_NOT_WRITE_COMPATIBLE: ::windows_sys::core::HRESULT = -1062555302i32;
pub const IMAPI_E_UDF_REVISION_CHANGE_NOT_ALLOWED: ::windows_sys::core::HRESULT = -1062555295i32;
pub const IMAPI_E_WORKING_DIRECTORY_SPACE: ::windows_sys::core::HRESULT = -1062555327i32;
pub const IMAPI_S_IMAGE_FEATURE_NOT_SUPPORTED: ::windows_sys::core::HRESULT = 11186527i32;
pub const MAPI_COMPOUND: u32 = 128u32;
pub const MAPI_DIM: u32 = 1u32;
pub const MAPI_ERROR_VERSION: i32 = 0i32;
pub const MAPI_E_CALL_FAILED: i32 = -2147467259i32;
pub const MAPI_E_INTERFACE_NOT_SUPPORTED: i32 = -2147467262i32;
pub const MAPI_E_INVALID_PARAMETER: i32 = -2147024809i32;
pub const MAPI_E_NOT_ENOUGH_MEMORY: i32 = -2147024882i32;
pub const MAPI_E_NO_ACCESS: i32 = -2147024891i32;
pub const MAPI_NOTRECIP: u32 = 64u32;
pub const MAPI_NOTRESERVED: u32 = 8u32;
pub const MAPI_NOW: u32 = 16u32;
pub const MAPI_ONE_OFF_NO_RICH_INFO: u32 = 1u32;
pub const MAPI_P1: u32 = 268435456u32;
pub const MAPI_SHORTTERM: u32 = 128u32;
pub const MAPI_SUBMITTED: u32 = 2147483648u32;
pub const MAPI_THISSESSION: u32 = 32u32;
pub const MAPI_USE_DEFAULT: u32 = 64u32;
pub const MNID_ID: u32 = 0u32;
pub const MNID_STRING: u32 = 1u32;
pub const MV_FLAG: u32 = 4096u32;
pub const MV_INSTANCE: u32 = 8192u32;
pub const OPENSTREAMONFILE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("OpenStreamOnFile");
pub const PRIHIGHEST: u32 = 32767u32;
pub const PRILOWEST: i32 = -32768i32;
pub const PRIUSER: u32 = 0u32;
pub const PROP_ID_INVALID: u32 = 65535u32;
pub const PROP_ID_NULL: u32 = 0u32;
pub const PROP_ID_SECURE_MAX: u32 = 26623u32;
pub const PROP_ID_SECURE_MIN: u32 = 26608u32;
pub const SERVICE_UI_ALLOWED: u32 = 16u32;
pub const SERVICE_UI_ALWAYS: u32 = 2u32;
pub const S_IMAPI_BOTHADJUSTED: ::windows_sys::core::HRESULT = 11141126i32;
pub const S_IMAPI_COMMAND_HAS_SENSE_DATA: ::windows_sys::core::HRESULT = 11141632i32;
pub const S_IMAPI_RAW_IMAGE_TRACK_INDEX_ALREADY_EXISTS: ::windows_sys::core::HRESULT = 11143688i32;
pub const S_IMAPI_ROTATIONADJUSTED: ::windows_sys::core::HRESULT = 11141125i32;
pub const S_IMAPI_SPEEDADJUSTED: ::windows_sys::core::HRESULT = 11141124i32;
pub const S_IMAPI_WRITE_NOT_IN_PROGRESS: ::windows_sys::core::HRESULT = 11141890i32;
pub const TABLE_CHANGED: u32 = 1u32;
pub const TABLE_ERROR: u32 = 2u32;
pub const TABLE_RELOAD: u32 = 9u32;
pub const TABLE_RESTRICT_DONE: u32 = 7u32;
pub const TABLE_ROW_ADDED: u32 = 3u32;
pub const TABLE_ROW_DELETED: u32 = 4u32;
pub const TABLE_ROW_MODIFIED: u32 = 5u32;
pub const TABLE_SETCOL_DONE: u32 = 8u32;
pub const TABLE_SORT_DONE: u32 = 6u32;
pub const TAD_ALL_ROWS: u32 = 1u32;
pub const UI_CURRENT_PROVIDER_FIRST: u32 = 4u32;
pub const UI_SERVICE: u32 = 2u32;
pub const WABOBJECT_LDAPURL_RETURN_MAILUSER: u32 = 1u32;
pub const WABOBJECT_ME_NEW: u32 = 1u32;
pub const WABOBJECT_ME_NOCREATE: u32 = 2u32;
pub const WAB_CONTEXT_ADRLIST: u32 = 2u32;
pub const WAB_DISPLAY_ISNTDS: u32 = 4u32;
pub const WAB_DISPLAY_LDAPURL: u32 = 1u32;
pub const WAB_DLL_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WAB32.DLL");
pub const WAB_DLL_PATH_KEY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Software\\Microsoft\\WAB\\DLLPath");
pub const WAB_ENABLE_PROFILES: u32 = 4194304u32;
pub const WAB_IGNORE_PROFILES: u32 = 8388608u32;
pub const WAB_LOCAL_CONTAINERS: u32 = 1048576u32;
pub const WAB_PROFILE_CONTENTS: u32 = 2097152u32;
pub const WAB_USE_OE_SENDMAIL: u32 = 1u32;
pub const WAB_VCARD_FILE: u32 = 0u32;
pub const WAB_VCARD_STREAM: u32 = 1u32;
pub const cchProfileNameMax: u32 = 64u32;
pub const cchProfilePassMax: u32 = 64u32;
pub const fMapiUnicode: u32 = 0u32;
pub const genderFemale: Gender = 1i32;
pub const genderMale: Gender = 2i32;
pub const genderUnspecified: Gender = 0i32;
pub const hrSuccess: u32 = 0u32;
pub const szHrDispatchNotifications: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("HrDispatchNotifications");
pub const szMAPINotificationMsg: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MAPI Notify window message");
pub const szScCreateConversationIndex: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ScCreateConversationIndex");
pub type Gender = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct ADRENTRY {
    pub ulReserved1: u32,
    pub cValues: u32,
    pub rgPropVals: *mut SPropValue,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for ADRENTRY {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for ADRENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct ADRLIST {
    pub cEntries: u32,
    pub aEntries: [ADRENTRY; 1],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for ADRLIST {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for ADRLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct ADRPARM {
    pub cbABContEntryID: u32,
    pub lpABContEntryID: *mut ENTRYID,
    pub ulFlags: u32,
    pub lpReserved: *mut ::core::ffi::c_void,
    pub ulHelpContext: u32,
    pub lpszHelpFileName: *mut i8,
    pub lpfnABSDI: LPFNABSDI,
    pub lpfnDismiss: LPFNDISMISS,
    pub lpvDismissContext: *mut ::core::ffi::c_void,
    pub lpszCaption: *mut i8,
    pub lpszNewEntryTitle: *mut i8,
    pub lpszDestWellsTitle: *mut i8,
    pub cDestFields: u32,
    pub nDestFieldFocus: u32,
    pub lppszDestTitles: *mut *mut i8,
    pub lpulDestComps: *mut u32,
    pub lpContRestriction: *mut SRestriction,
    pub lpHierRestriction: *mut SRestriction,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for ADRPARM {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for ADRPARM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLBUTTON {
    pub ulbLpszLabel: u32,
    pub ulFlags: u32,
    pub ulPRControl: u32,
}
impl ::core::marker::Copy for DTBLBUTTON {}
impl ::core::clone::Clone for DTBLBUTTON {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLCHECKBOX {
    pub ulbLpszLabel: u32,
    pub ulFlags: u32,
    pub ulPRPropertyName: u32,
}
impl ::core::marker::Copy for DTBLCHECKBOX {}
impl ::core::clone::Clone for DTBLCHECKBOX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLCOMBOBOX {
    pub ulbLpszCharsAllowed: u32,
    pub ulFlags: u32,
    pub ulNumCharsAllowed: u32,
    pub ulPRPropertyName: u32,
    pub ulPRTableName: u32,
}
impl ::core::marker::Copy for DTBLCOMBOBOX {}
impl ::core::clone::Clone for DTBLCOMBOBOX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLDDLBX {
    pub ulFlags: u32,
    pub ulPRDisplayProperty: u32,
    pub ulPRSetProperty: u32,
    pub ulPRTableName: u32,
}
impl ::core::marker::Copy for DTBLDDLBX {}
impl ::core::clone::Clone for DTBLDDLBX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLEDIT {
    pub ulbLpszCharsAllowed: u32,
    pub ulFlags: u32,
    pub ulNumCharsAllowed: u32,
    pub ulPropTag: u32,
}
impl ::core::marker::Copy for DTBLEDIT {}
impl ::core::clone::Clone for DTBLEDIT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLGROUPBOX {
    pub ulbLpszLabel: u32,
    pub ulFlags: u32,
}
impl ::core::marker::Copy for DTBLGROUPBOX {}
impl ::core::clone::Clone for DTBLGROUPBOX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLLABEL {
    pub ulbLpszLabelName: u32,
    pub ulFlags: u32,
}
impl ::core::marker::Copy for DTBLLABEL {}
impl ::core::clone::Clone for DTBLLABEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLLBX {
    pub ulFlags: u32,
    pub ulPRSetProperty: u32,
    pub ulPRTableName: u32,
}
impl ::core::marker::Copy for DTBLLBX {}
impl ::core::clone::Clone for DTBLLBX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLMVDDLBX {
    pub ulFlags: u32,
    pub ulMVPropTag: u32,
}
impl ::core::marker::Copy for DTBLMVDDLBX {}
impl ::core::clone::Clone for DTBLMVDDLBX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLMVLISTBOX {
    pub ulFlags: u32,
    pub ulMVPropTag: u32,
}
impl ::core::marker::Copy for DTBLMVLISTBOX {}
impl ::core::clone::Clone for DTBLMVLISTBOX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLPAGE {
    pub ulbLpszLabel: u32,
    pub ulFlags: u32,
    pub ulbLpszComponent: u32,
    pub ulContext: u32,
}
impl ::core::marker::Copy for DTBLPAGE {}
impl ::core::clone::Clone for DTBLPAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTBLRADIOBUTTON {
    pub ulbLpszLabel: u32,
    pub ulFlags: u32,
    pub ulcButtons: u32,
    pub ulPropTag: u32,
    pub lReturnValue: i32,
}
impl ::core::marker::Copy for DTBLRADIOBUTTON {}
impl ::core::clone::Clone for DTBLRADIOBUTTON {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTCTL {
    pub ulCtlType: u32,
    pub ulCtlFlags: u32,
    pub lpbNotif: *mut u8,
    pub cbNotif: u32,
    pub lpszFilter: *mut i8,
    pub ulItemID: u32,
    pub ctl: DTCTL_0,
}
impl ::core::marker::Copy for DTCTL {}
impl ::core::clone::Clone for DTCTL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union DTCTL_0 {
    pub lpv: *mut ::core::ffi::c_void,
    pub lplabel: *mut DTBLLABEL,
    pub lpedit: *mut DTBLEDIT,
    pub lplbx: *mut DTBLLBX,
    pub lpcombobox: *mut DTBLCOMBOBOX,
    pub lpddlbx: *mut DTBLDDLBX,
    pub lpcheckbox: *mut DTBLCHECKBOX,
    pub lpgroupbox: *mut DTBLGROUPBOX,
    pub lpbutton: *mut DTBLBUTTON,
    pub lpradiobutton: *mut DTBLRADIOBUTTON,
    pub lpmvlbx: *mut DTBLMVLISTBOX,
    pub lpmvddlbx: *mut DTBLMVDDLBX,
    pub lppage: *mut DTBLPAGE,
}
impl ::core::marker::Copy for DTCTL_0 {}
impl ::core::clone::Clone for DTCTL_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DTPAGE {
    pub cctl: u32,
    pub lpszResourceName: *mut i8,
    pub Anonymous: DTPAGE_0,
    pub lpctl: *mut DTCTL,
}
impl ::core::marker::Copy for DTPAGE {}
impl ::core::clone::Clone for DTPAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union DTPAGE_0 {
    pub lpszComponent: *mut i8,
    pub ulItemID: u32,
}
impl ::core::marker::Copy for DTPAGE_0 {}
impl ::core::clone::Clone for DTPAGE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ENTRYID {
    pub abFlags: [u8; 4],
    pub ab: [u8; 1],
}
impl ::core::marker::Copy for ENTRYID {}
impl ::core::clone::Clone for ENTRYID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ERROR_NOTIFICATION {
    pub cbEntryID: u32,
    pub lpEntryID: *mut ENTRYID,
    pub scode: i32,
    pub ulFlags: u32,
    pub lpMAPIError: *mut MAPIERROR,
}
impl ::core::marker::Copy for ERROR_NOTIFICATION {}
impl ::core::clone::Clone for ERROR_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXTENDED_NOTIFICATION {
    pub ulEvent: u32,
    pub cb: u32,
    pub pbEventParameters: *mut u8,
}
impl ::core::marker::Copy for EXTENDED_NOTIFICATION {}
impl ::core::clone::Clone for EXTENDED_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLATENTRY {
    pub cb: u32,
    pub abEntry: [u8; 1],
}
impl ::core::marker::Copy for FLATENTRY {}
impl ::core::clone::Clone for FLATENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLATENTRYLIST {
    pub cEntries: u32,
    pub cbEntries: u32,
    pub abEntries: [u8; 1],
}
impl ::core::marker::Copy for FLATENTRYLIST {}
impl ::core::clone::Clone for FLATENTRYLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FLATMTSIDLIST {
    pub cMTSIDs: u32,
    pub cbMTSIDs: u32,
    pub abMTSIDs: [u8; 1],
}
impl ::core::marker::Copy for FLATMTSIDLIST {}
impl ::core::clone::Clone for FLATMTSIDLIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FlagList {
    pub cFlags: u32,
    pub ulFlag: [u32; 1],
}
impl ::core::marker::Copy for FlagList {}
impl ::core::clone::Clone for FlagList {
    fn clone(&self) -> Self {
        *self
    }
}
pub type LPWABACTIONITEM = isize;
#[repr(C)]
pub struct MAPIERROR {
    pub ulVersion: u32,
    pub lpszError: *mut i8,
    pub lpszComponent: *mut i8,
    pub ulLowLevelError: u32,
    pub ulContext: u32,
}
impl ::core::marker::Copy for MAPIERROR {}
impl ::core::clone::Clone for MAPIERROR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAPINAMEID {
    pub lpguid: *mut ::windows_sys::core::GUID,
    pub ulKind: u32,
    pub Kind: MAPINAMEID_0,
}
impl ::core::marker::Copy for MAPINAMEID {}
impl ::core::clone::Clone for MAPINAMEID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union MAPINAMEID_0 {
    pub lID: i32,
    pub lpwstrName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for MAPINAMEID_0 {}
impl ::core::clone::Clone for MAPINAMEID_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAPIUID {
    pub ab: [u8; 16],
}
impl ::core::marker::Copy for MAPIUID {}
impl ::core::clone::Clone for MAPIUID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MTSID {
    pub cb: u32,
    pub ab: [u8; 1],
}
impl ::core::marker::Copy for MTSID {}
impl ::core::clone::Clone for MTSID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NEWMAIL_NOTIFICATION {
    pub cbEntryID: u32,
    pub lpEntryID: *mut ENTRYID,
    pub cbParentID: u32,
    pub lpParentID: *mut ENTRYID,
    pub ulFlags: u32,
    pub lpszMessageClass: *mut i8,
    pub ulMessageFlags: u32,
}
impl ::core::marker::Copy for NEWMAIL_NOTIFICATION {}
impl ::core::clone::Clone for NEWMAIL_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct NOTIFICATION {
    pub ulEventType: u32,
    pub ulAlignPad: u32,
    pub info: NOTIFICATION_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for NOTIFICATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub union NOTIFICATION_0 {
    pub err: ERROR_NOTIFICATION,
    pub newmail: NEWMAIL_NOTIFICATION,
    pub obj: OBJECT_NOTIFICATION,
    pub tab: TABLE_NOTIFICATION,
    pub ext: EXTENDED_NOTIFICATION,
    pub statobj: STATUS_OBJECT_NOTIFICATION,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for NOTIFICATION_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for NOTIFICATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NOTIFKEY {
    pub cb: u32,
    pub ab: [u8; 1],
}
impl ::core::marker::Copy for NOTIFKEY {}
impl ::core::clone::Clone for NOTIFKEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OBJECT_NOTIFICATION {
    pub cbEntryID: u32,
    pub lpEntryID: *mut ENTRYID,
    pub ulObjType: u32,
    pub cbParentID: u32,
    pub lpParentID: *mut ENTRYID,
    pub cbOldID: u32,
    pub lpOldID: *mut ENTRYID,
    pub cbOldParentID: u32,
    pub lpOldParentID: *mut ENTRYID,
    pub lpPropTagArray: *mut SPropTagArray,
}
impl ::core::marker::Copy for OBJECT_NOTIFICATION {}
impl ::core::clone::Clone for OBJECT_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SAndRestriction {
    pub cRes: u32,
    pub lpRes: *mut SRestriction,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SAndRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SAndRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SAppTimeArray {
    pub cValues: u32,
    pub lpat: *mut f64,
}
impl ::core::marker::Copy for SAppTimeArray {}
impl ::core::clone::Clone for SAppTimeArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SBinary {
    pub cb: u32,
    pub lpb: *mut u8,
}
impl ::core::marker::Copy for SBinary {}
impl ::core::clone::Clone for SBinary {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SBinaryArray {
    pub cValues: u32,
    pub lpbin: *mut SBinary,
}
impl ::core::marker::Copy for SBinaryArray {}
impl ::core::clone::Clone for SBinaryArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SBitMaskRestriction {
    pub relBMR: u32,
    pub ulPropTag: u32,
    pub ulMask: u32,
}
impl ::core::marker::Copy for SBitMaskRestriction {}
impl ::core::clone::Clone for SBitMaskRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SCommentRestriction {
    pub cValues: u32,
    pub lpRes: *mut SRestriction,
    pub lpProp: *mut SPropValue,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SCommentRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SCommentRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SComparePropsRestriction {
    pub relop: u32,
    pub ulPropTag1: u32,
    pub ulPropTag2: u32,
}
impl ::core::marker::Copy for SComparePropsRestriction {}
impl ::core::clone::Clone for SComparePropsRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SContentRestriction {
    pub ulFuzzyLevel: u32,
    pub ulPropTag: u32,
    pub lpProp: *mut SPropValue,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SContentRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SContentRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct SCurrencyArray {
    pub cValues: u32,
    pub lpcur: *mut super::Com::CY,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for SCurrencyArray {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for SCurrencyArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SDateTimeArray {
    pub cValues: u32,
    pub lpft: *mut super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SDateTimeArray {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SDateTimeArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SDoubleArray {
    pub cValues: u32,
    pub lpdbl: *mut f64,
}
impl ::core::marker::Copy for SDoubleArray {}
impl ::core::clone::Clone for SDoubleArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SExistRestriction {
    pub ulReserved1: u32,
    pub ulPropTag: u32,
    pub ulReserved2: u32,
}
impl ::core::marker::Copy for SExistRestriction {}
impl ::core::clone::Clone for SExistRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SGuidArray {
    pub cValues: u32,
    pub lpguid: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for SGuidArray {}
impl ::core::clone::Clone for SGuidArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SLPSTRArray {
    pub cValues: u32,
    pub lppszA: *mut ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for SLPSTRArray {}
impl ::core::clone::Clone for SLPSTRArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SLargeIntegerArray {
    pub cValues: u32,
    pub lpli: *mut i64,
}
impl ::core::marker::Copy for SLargeIntegerArray {}
impl ::core::clone::Clone for SLargeIntegerArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SLongArray {
    pub cValues: u32,
    pub lpl: *mut i32,
}
impl ::core::marker::Copy for SLongArray {}
impl ::core::clone::Clone for SLongArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SNotRestriction {
    pub ulReserved: u32,
    pub lpRes: *mut SRestriction,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SNotRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SNotRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SOrRestriction {
    pub cRes: u32,
    pub lpRes: *mut SRestriction,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SOrRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SOrRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SPropProblem {
    pub ulIndex: u32,
    pub ulPropTag: u32,
    pub scode: i32,
}
impl ::core::marker::Copy for SPropProblem {}
impl ::core::clone::Clone for SPropProblem {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SPropProblemArray {
    pub cProblem: u32,
    pub aProblem: [SPropProblem; 1],
}
impl ::core::marker::Copy for SPropProblemArray {}
impl ::core::clone::Clone for SPropProblemArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SPropTagArray {
    pub cValues: u32,
    pub aulPropTag: [u32; 1],
}
impl ::core::marker::Copy for SPropTagArray {}
impl ::core::clone::Clone for SPropTagArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SPropValue {
    pub ulPropTag: u32,
    pub dwAlignPad: u32,
    pub Value: __UPV,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SPropValue {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SPropValue {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SPropertyRestriction {
    pub relop: u32,
    pub ulPropTag: u32,
    pub lpProp: *mut SPropValue,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SPropertyRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SPropertyRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SRealArray {
    pub cValues: u32,
    pub lpflt: *mut f32,
}
impl ::core::marker::Copy for SRealArray {}
impl ::core::clone::Clone for SRealArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SRestriction {
    pub rt: u32,
    pub res: SRestriction_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub union SRestriction_0 {
    pub resCompareProps: SComparePropsRestriction,
    pub resAnd: SAndRestriction,
    pub resOr: SOrRestriction,
    pub resNot: SNotRestriction,
    pub resContent: SContentRestriction,
    pub resProperty: SPropertyRestriction,
    pub resBitMask: SBitMaskRestriction,
    pub resSize: SSizeRestriction,
    pub resExist: SExistRestriction,
    pub resSub: SSubRestriction,
    pub resComment: SCommentRestriction,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SRestriction_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SRestriction_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SRow {
    pub ulAdrEntryPad: u32,
    pub cValues: u32,
    pub lpProps: *mut SPropValue,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SRow {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SRow {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SRowSet {
    pub cRows: u32,
    pub aRow: [SRow; 1],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SRowSet {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SRowSet {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SShortArray {
    pub cValues: u32,
    pub lpi: *mut i16,
}
impl ::core::marker::Copy for SShortArray {}
impl ::core::clone::Clone for SShortArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SSizeRestriction {
    pub relop: u32,
    pub ulPropTag: u32,
    pub cb: u32,
}
impl ::core::marker::Copy for SSizeRestriction {}
impl ::core::clone::Clone for SSizeRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SSortOrder {
    pub ulPropTag: u32,
    pub ulOrder: u32,
}
impl ::core::marker::Copy for SSortOrder {}
impl ::core::clone::Clone for SSortOrder {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SSortOrderSet {
    pub cSorts: u32,
    pub cCategories: u32,
    pub cExpanded: u32,
    pub aSort: [SSortOrder; 1],
}
impl ::core::marker::Copy for SSortOrderSet {}
impl ::core::clone::Clone for SSortOrderSet {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct SSubRestriction {
    pub ulSubObject: u32,
    pub lpRes: *mut SRestriction,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for SSubRestriction {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for SSubRestriction {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct STATUS_OBJECT_NOTIFICATION {
    pub cbEntryID: u32,
    pub lpEntryID: *mut ENTRYID,
    pub cValues: u32,
    pub lpPropVals: *mut SPropValue,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for STATUS_OBJECT_NOTIFICATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for STATUS_OBJECT_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SWStringArray {
    pub cValues: u32,
    pub lppszW: *mut ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for SWStringArray {}
impl ::core::clone::Clone for SWStringArray {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub struct TABLE_NOTIFICATION {
    pub ulTableEvent: u32,
    pub hResult: ::windows_sys::core::HRESULT,
    pub propIndex: SPropValue,
    pub propPrior: SPropValue,
    pub row: SRow,
    pub ulPad: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for TABLE_NOTIFICATION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for TABLE_NOTIFICATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WABEXTDISPLAY {
    pub cbSize: u32,
    pub lpWABObject: IWABObject,
    pub lpAdrBook: IAddrBook,
    pub lpPropObj: IMAPIProp,
    pub fReadOnly: super::super::Foundation::BOOL,
    pub fDataChanged: super::super::Foundation::BOOL,
    pub ulFlags: u32,
    pub lpv: *mut ::core::ffi::c_void,
    pub lpsz: *mut i8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WABEXTDISPLAY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WABEXTDISPLAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WABIMPORTPARAM {
    pub cbSize: u32,
    pub lpAdrBook: IAddrBook,
    pub hWnd: super::super::Foundation::HWND,
    pub ulFlags: u32,
    pub lpszFileName: ::windows_sys::core::PSTR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WABIMPORTPARAM {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WABIMPORTPARAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WAB_PARAM {
    pub cbSize: u32,
    pub hwnd: super::super::Foundation::HWND,
    pub szFileName: ::windows_sys::core::PSTR,
    pub ulFlags: u32,
    pub guidPSExt: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WAB_PARAM {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WAB_PARAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub union __UPV {
    pub i: i16,
    pub l: i32,
    pub ul: u32,
    pub flt: f32,
    pub dbl: f64,
    pub b: u16,
    pub cur: super::Com::CY,
    pub at: f64,
    pub ft: super::super::Foundation::FILETIME,
    pub lpszA: ::windows_sys::core::PSTR,
    pub bin: SBinary,
    pub lpszW: ::windows_sys::core::PWSTR,
    pub lpguid: *mut ::windows_sys::core::GUID,
    pub li: i64,
    pub MVi: SShortArray,
    pub MVl: SLongArray,
    pub MVflt: SRealArray,
    pub MVdbl: SDoubleArray,
    pub MVcur: SCurrencyArray,
    pub MVat: SAppTimeArray,
    pub MVft: SDateTimeArray,
    pub MVbin: SBinaryArray,
    pub MVszA: SLPSTRArray,
    pub MVszW: SWStringArray,
    pub MVguid: SGuidArray,
    pub MVli: SLargeIntegerArray,
    pub err: i32,
    pub x: i32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::marker::Copy for __UPV {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
impl ::core::clone::Clone for __UPV {
    fn clone(&self) -> Self {
        *self
    }
}
pub type CALLERRELEASE = ::core::option::Option<unsafe extern "system" fn(ulcallerdata: u32, lptbldata: ITableData, lpvue: IMAPITable) -> ()>;
pub type LPALLOCATEBUFFER = ::core::option::Option<unsafe extern "system" fn(cbsize: u32, lppbuffer: *mut *mut ::core::ffi::c_void) -> i32>;
pub type LPALLOCATEMORE = ::core::option::Option<unsafe extern "system" fn(cbsize: u32, lpobject: *mut ::core::ffi::c_void, lppbuffer: *mut *mut ::core::ffi::c_void) -> i32>;
pub type LPCREATECONVERSATIONINDEX = ::core::option::Option<unsafe extern "system" fn(cbparent: u32, lpbparent: *mut u8, lpcbconvindex: *mut u32, lppbconvindex: *mut *mut u8) -> i32>;
pub type LPDISPATCHNOTIFICATIONS = ::core::option::Option<unsafe extern "system" fn(ulflags: u32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPFNABSDI = ::core::option::Option<unsafe extern "system" fn(uluiparam: usize, lpvmsg: *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
pub type LPFNBUTTON = ::core::option::Option<unsafe extern "system" fn(uluiparam: usize, lpvcontext: *mut ::core::ffi::c_void, cbentryid: u32, lpselection: *mut ENTRYID, ulflags: u32) -> i32>;
pub type LPFNDISMISS = ::core::option::Option<unsafe extern "system" fn(uluiparam: usize, lpvcontext: *mut ::core::ffi::c_void) -> ()>;
pub type LPFREEBUFFER = ::core::option::Option<unsafe extern "system" fn(lpbuffer: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com"))]
pub type LPNOTIFCALLBACK = ::core::option::Option<unsafe extern "system" fn(lpvcontext: *mut ::core::ffi::c_void, cnotification: u32, lpnotifications: *mut NOTIFICATION) -> i32>;
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub type LPOPENSTREAMONFILE = ::core::option::Option<unsafe extern "system" fn(lpallocatebuffer: LPALLOCATEBUFFER, lpfreebuffer: LPFREEBUFFER, ulflags: u32, lpszfilename: *const i8, lpszprefix: *const i8, lppstream: *mut super::Com::IStream) -> ::windows_sys::core::HRESULT>;
pub type LPWABALLOCATEBUFFER = ::core::option::Option<unsafe extern "system" fn(lpwabobject: IWABObject, cbsize: u32, lppbuffer: *mut *mut ::core::ffi::c_void) -> i32>;
pub type LPWABALLOCATEMORE = ::core::option::Option<unsafe extern "system" fn(lpwabobject: IWABObject, cbsize: u32, lpobject: *mut ::core::ffi::c_void, lppbuffer: *mut *mut ::core::ffi::c_void) -> i32>;
pub type LPWABFREEBUFFER = ::core::option::Option<unsafe extern "system" fn(lpwabobject: IWABObject, lpbuffer: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPWABOPEN = ::core::option::Option<unsafe extern "system" fn(lppadrbook: *mut IAddrBook, lppwabobject: *mut IWABObject, lpwp: *mut WAB_PARAM, reserved2: u32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LPWABOPENEX = ::core::option::Option<unsafe extern "system" fn(lppadrbook: *mut IAddrBook, lppwabobject: *mut IWABObject, lpwp: *mut WAB_PARAM, reserved: u32, fnallocatebuffer: LPALLOCATEBUFFER, fnallocatemore: LPALLOCATEMORE, fnfreebuffer: LPFREEBUFFER) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNIDLE = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
