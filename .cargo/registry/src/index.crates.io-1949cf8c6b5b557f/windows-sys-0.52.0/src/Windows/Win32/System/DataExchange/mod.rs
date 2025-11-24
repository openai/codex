::windows_targets::link!("kernel32.dll" "system" fn AddAtomA(lpstring : ::windows_sys::core::PCSTR) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn AddAtomW(lpstring : ::windows_sys::core::PCWSTR) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AddClipboardFormatListener(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ChangeClipboardChain(hwndremove : super::super::Foundation:: HWND, hwndnewnext : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClipboard() -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn CountClipboardFormats() -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeAbandonTransaction(idinst : u32, hconv : HCONV, idtransaction : u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn DdeAccessData(hdata : HDDEDATA, pcbdatasize : *mut u32) -> *mut u8);
::windows_targets::link!("user32.dll" "system" fn DdeAddData(hdata : HDDEDATA, psrc : *const u8, cb : u32, cboff : u32) -> HDDEDATA);
::windows_targets::link!("user32.dll" "system" fn DdeClientTransaction(pdata : *const u8, cbdata : u32, hconv : HCONV, hszitem : HSZ, wfmt : u32, wtype : DDE_CLIENT_TRANSACTION_TYPE, dwtimeout : u32, pdwresult : *mut u32) -> HDDEDATA);
::windows_targets::link!("user32.dll" "system" fn DdeCmpStringHandles(hsz1 : HSZ, hsz2 : HSZ) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn DdeConnect(idinst : u32, hszservice : HSZ, hsztopic : HSZ, pcc : *const CONVCONTEXT) -> HCONV);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn DdeConnectList(idinst : u32, hszservice : HSZ, hsztopic : HSZ, hconvlist : HCONVLIST, pcc : *const CONVCONTEXT) -> HCONVLIST);
::windows_targets::link!("user32.dll" "system" fn DdeCreateDataHandle(idinst : u32, psrc : *const u8, cb : u32, cboff : u32, hszitem : HSZ, wfmt : u32, afcmd : u32) -> HDDEDATA);
::windows_targets::link!("user32.dll" "system" fn DdeCreateStringHandleA(idinst : u32, psz : ::windows_sys::core::PCSTR, icodepage : i32) -> HSZ);
::windows_targets::link!("user32.dll" "system" fn DdeCreateStringHandleW(idinst : u32, psz : ::windows_sys::core::PCWSTR, icodepage : i32) -> HSZ);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeDisconnect(hconv : HCONV) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeDisconnectList(hconvlist : HCONVLIST) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeEnableCallback(idinst : u32, hconv : HCONV, wcmd : DDE_ENABLE_CALLBACK_CMD) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeFreeDataHandle(hdata : HDDEDATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeFreeStringHandle(idinst : u32, hsz : HSZ) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn DdeGetData(hdata : HDDEDATA, pdst : *mut u8, cbmax : u32, cboff : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn DdeGetLastError(idinst : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeImpersonateClient(hconv : HCONV) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn DdeInitializeA(pidinst : *mut u32, pfncallback : PFNCALLBACK, afcmd : DDE_INITIALIZE_COMMAND, ulres : u32) -> u32);
::windows_targets::link!("user32.dll" "system" fn DdeInitializeW(pidinst : *mut u32, pfncallback : PFNCALLBACK, afcmd : DDE_INITIALIZE_COMMAND, ulres : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeKeepStringHandle(idinst : u32, hsz : HSZ) -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn DdeNameService(idinst : u32, hsz1 : HSZ, hsz2 : HSZ, afcmd : DDE_NAME_SERVICE_CMD) -> HDDEDATA);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdePostAdvise(idinst : u32, hsztopic : HSZ, hszitem : HSZ) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn DdeQueryConvInfo(hconv : HCONV, idtransaction : u32, pconvinfo : *mut CONVINFO) -> u32);
::windows_targets::link!("user32.dll" "system" fn DdeQueryNextServer(hconvlist : HCONVLIST, hconvprev : HCONV) -> HCONV);
::windows_targets::link!("user32.dll" "system" fn DdeQueryStringA(idinst : u32, hsz : HSZ, psz : ::windows_sys::core::PSTR, cchmax : u32, icodepage : i32) -> u32);
::windows_targets::link!("user32.dll" "system" fn DdeQueryStringW(idinst : u32, hsz : HSZ, psz : ::windows_sys::core::PWSTR, cchmax : u32, icodepage : i32) -> u32);
::windows_targets::link!("user32.dll" "system" fn DdeReconnect(hconv : HCONV) -> HCONV);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn DdeSetQualityOfService(hwndclient : super::super::Foundation:: HWND, pqosnew : *const super::super::Security:: SECURITY_QUALITY_OF_SERVICE, pqosprev : *mut super::super::Security:: SECURITY_QUALITY_OF_SERVICE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeSetUserHandle(hconv : HCONV, id : u32, huser : usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeUnaccessData(hdata : HDDEDATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DdeUninitialize(idinst : u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn DeleteAtom(natom : u16) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EmptyClipboard() -> super::super::Foundation:: BOOL);
::windows_targets::link!("user32.dll" "system" fn EnumClipboardFormats(format : u32) -> u32);
::windows_targets::link!("kernel32.dll" "system" fn FindAtomA(lpstring : ::windows_sys::core::PCSTR) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn FindAtomW(lpstring : ::windows_sys::core::PCWSTR) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FreeDDElParam(msg : u32, lparam : super::super::Foundation:: LPARAM) -> super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetAtomNameA(natom : u16, lpbuffer : ::windows_sys::core::PSTR, nsize : i32) -> u32);
::windows_targets::link!("kernel32.dll" "system" fn GetAtomNameW(natom : u16, lpbuffer : ::windows_sys::core::PWSTR, nsize : i32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClipboardData(uformat : u32) -> super::super::Foundation:: HANDLE);
::windows_targets::link!("user32.dll" "system" fn GetClipboardFormatNameA(format : u32, lpszformatname : ::windows_sys::core::PSTR, cchmaxcount : i32) -> i32);
::windows_targets::link!("user32.dll" "system" fn GetClipboardFormatNameW(format : u32, lpszformatname : ::windows_sys::core::PWSTR, cchmaxcount : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClipboardOwner() -> super::super::Foundation:: HWND);
::windows_targets::link!("user32.dll" "system" fn GetClipboardSequenceNumber() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClipboardViewer() -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetOpenClipboardWindow() -> super::super::Foundation:: HWND);
::windows_targets::link!("user32.dll" "system" fn GetPriorityClipboardFormat(paformatprioritylist : *const u32, cformats : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetUpdatedClipboardFormats(lpuiformats : *mut u32, cformats : u32, pcformatsout : *mut u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GlobalAddAtomA(lpstring : ::windows_sys::core::PCSTR) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GlobalAddAtomExA(lpstring : ::windows_sys::core::PCSTR, flags : u32) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GlobalAddAtomExW(lpstring : ::windows_sys::core::PCWSTR, flags : u32) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GlobalAddAtomW(lpstring : ::windows_sys::core::PCWSTR) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GlobalDeleteAtom(natom : u16) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GlobalFindAtomA(lpstring : ::windows_sys::core::PCSTR) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GlobalFindAtomW(lpstring : ::windows_sys::core::PCWSTR) -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GlobalGetAtomNameA(natom : u16, lpbuffer : ::windows_sys::core::PSTR, nsize : i32) -> u32);
::windows_targets::link!("kernel32.dll" "system" fn GlobalGetAtomNameW(natom : u16, lpbuffer : ::windows_sys::core::PWSTR, nsize : i32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImpersonateDdeClientWindow(hwndclient : super::super::Foundation:: HWND, hwndserver : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InitAtomTable(nsize : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsClipboardFormatAvailable(format : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenClipboard(hwndnewowner : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PackDDElParam(msg : u32, uilo : usize, uihi : usize) -> super::super::Foundation:: LPARAM);
::windows_targets::link!("user32.dll" "system" fn RegisterClipboardFormatA(lpszformat : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("user32.dll" "system" fn RegisterClipboardFormatW(lpszformat : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RemoveClipboardFormatListener(hwnd : super::super::Foundation:: HWND) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReuseDDElParam(lparam : super::super::Foundation:: LPARAM, msgin : u32, msgout : u32, uilo : usize, uihi : usize) -> super::super::Foundation:: LPARAM);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClipboardData(uformat : u32, hmem : super::super::Foundation:: HANDLE) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClipboardViewer(hwndnewviewer : super::super::Foundation:: HWND) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn SetWinMetaFileBits(nsize : u32, lpmeta16data : *const u8, hdcref : super::super::Graphics::Gdi:: HDC, lpmfp : *const METAFILEPICT) -> super::super::Graphics::Gdi:: HENHMETAFILE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UnpackDDElParam(msg : u32, lparam : super::super::Foundation:: LPARAM, puilo : *mut usize, puihi : *mut usize) -> super::super::Foundation:: BOOL);
pub const APPCLASS_MASK: i32 = 15i32;
pub const APPCLASS_MONITOR: DDE_INITIALIZE_COMMAND = 1u32;
pub const APPCLASS_STANDARD: DDE_INITIALIZE_COMMAND = 0u32;
pub const APPCMD_CLIENTONLY: DDE_INITIALIZE_COMMAND = 16u32;
pub const APPCMD_FILTERINITS: DDE_INITIALIZE_COMMAND = 32u32;
pub const APPCMD_MASK: i32 = 4080i32;
pub const CADV_LATEACK: u32 = 65535u32;
pub const CBF_FAIL_ADVISES: DDE_INITIALIZE_COMMAND = 16384u32;
pub const CBF_FAIL_ALLSVRXACTIONS: DDE_INITIALIZE_COMMAND = 258048u32;
pub const CBF_FAIL_CONNECTIONS: DDE_INITIALIZE_COMMAND = 8192u32;
pub const CBF_FAIL_EXECUTES: DDE_INITIALIZE_COMMAND = 32768u32;
pub const CBF_FAIL_POKES: DDE_INITIALIZE_COMMAND = 65536u32;
pub const CBF_FAIL_REQUESTS: DDE_INITIALIZE_COMMAND = 131072u32;
pub const CBF_FAIL_SELFCONNECTIONS: DDE_INITIALIZE_COMMAND = 4096u32;
pub const CBF_SKIP_ALLNOTIFICATIONS: DDE_INITIALIZE_COMMAND = 3932160u32;
pub const CBF_SKIP_CONNECT_CONFIRMS: DDE_INITIALIZE_COMMAND = 262144u32;
pub const CBF_SKIP_DISCONNECTS: DDE_INITIALIZE_COMMAND = 2097152u32;
pub const CBF_SKIP_REGISTRATIONS: DDE_INITIALIZE_COMMAND = 524288u32;
pub const CBF_SKIP_UNREGISTRATIONS: DDE_INITIALIZE_COMMAND = 1048576u32;
pub const CP_WINANSI: i32 = 1004i32;
pub const CP_WINNEUTRAL: i32 = 1200i32;
pub const CP_WINUNICODE: i32 = 1200i32;
pub const DDE_FACK: u32 = 32768u32;
pub const DDE_FACKREQ: u32 = 32768u32;
pub const DDE_FAPPSTATUS: u32 = 255u32;
pub const DDE_FBUSY: u32 = 16384u32;
pub const DDE_FDEFERUPD: u32 = 16384u32;
pub const DDE_FNOTPROCESSED: u32 = 0u32;
pub const DDE_FRELEASE: u32 = 8192u32;
pub const DDE_FREQUESTED: u32 = 4096u32;
pub const DMLERR_ADVACKTIMEOUT: u32 = 16384u32;
pub const DMLERR_BUSY: u32 = 16385u32;
pub const DMLERR_DATAACKTIMEOUT: u32 = 16386u32;
pub const DMLERR_DLL_NOT_INITIALIZED: u32 = 16387u32;
pub const DMLERR_DLL_USAGE: u32 = 16388u32;
pub const DMLERR_EXECACKTIMEOUT: u32 = 16389u32;
pub const DMLERR_FIRST: u32 = 16384u32;
pub const DMLERR_INVALIDPARAMETER: u32 = 16390u32;
pub const DMLERR_LAST: u32 = 16401u32;
pub const DMLERR_LOW_MEMORY: u32 = 16391u32;
pub const DMLERR_MEMORY_ERROR: u32 = 16392u32;
pub const DMLERR_NOTPROCESSED: u32 = 16393u32;
pub const DMLERR_NO_CONV_ESTABLISHED: u32 = 16394u32;
pub const DMLERR_NO_ERROR: u32 = 0u32;
pub const DMLERR_POKEACKTIMEOUT: u32 = 16395u32;
pub const DMLERR_POSTMSG_FAILED: u32 = 16396u32;
pub const DMLERR_REENTRANCY: u32 = 16397u32;
pub const DMLERR_SERVER_DIED: u32 = 16398u32;
pub const DMLERR_SYS_ERROR: u32 = 16399u32;
pub const DMLERR_UNADVACKTIMEOUT: u32 = 16400u32;
pub const DMLERR_UNFOUND_QUEUE_ID: u32 = 16401u32;
pub const DNS_FILTEROFF: DDE_NAME_SERVICE_CMD = 8u32;
pub const DNS_FILTERON: DDE_NAME_SERVICE_CMD = 4u32;
pub const DNS_REGISTER: DDE_NAME_SERVICE_CMD = 1u32;
pub const DNS_UNREGISTER: DDE_NAME_SERVICE_CMD = 2u32;
pub const EC_DISABLE: DDE_ENABLE_CALLBACK_CMD = 8u32;
pub const EC_ENABLEALL: DDE_ENABLE_CALLBACK_CMD = 0u32;
pub const EC_ENABLEONE: DDE_ENABLE_CALLBACK_CMD = 128u32;
pub const EC_QUERYWAITING: DDE_ENABLE_CALLBACK_CMD = 2u32;
pub const HDATA_APPOWNED: u32 = 1u32;
pub const MAX_MONITORS: u32 = 4u32;
pub const MF_CALLBACKS: DDE_INITIALIZE_COMMAND = 134217728u32;
pub const MF_CONV: DDE_INITIALIZE_COMMAND = 1073741824u32;
pub const MF_ERRORS: DDE_INITIALIZE_COMMAND = 268435456u32;
pub const MF_HSZ_INFO: DDE_INITIALIZE_COMMAND = 16777216u32;
pub const MF_LINKS: DDE_INITIALIZE_COMMAND = 536870912u32;
pub const MF_MASK: u32 = 4278190080u32;
pub const MF_POSTMSGS: DDE_INITIALIZE_COMMAND = 67108864u32;
pub const MF_SENDMSGS: DDE_INITIALIZE_COMMAND = 33554432u32;
pub const MH_CLEANUP: u32 = 4u32;
pub const MH_CREATE: u32 = 1u32;
pub const MH_DELETE: u32 = 3u32;
pub const MH_KEEP: u32 = 2u32;
pub const MSGF_DDEMGR: u32 = 32769u32;
pub const QID_SYNC: u32 = 4294967295u32;
pub const ST_ADVISE: CONVINFO_STATUS = 2u32;
pub const ST_BLOCKED: CONVINFO_STATUS = 8u32;
pub const ST_BLOCKNEXT: CONVINFO_STATUS = 128u32;
pub const ST_CLIENT: CONVINFO_STATUS = 16u32;
pub const ST_CONNECTED: CONVINFO_STATUS = 1u32;
pub const ST_INLIST: CONVINFO_STATUS = 64u32;
pub const ST_ISLOCAL: CONVINFO_STATUS = 4u32;
pub const ST_ISSELF: CONVINFO_STATUS = 256u32;
pub const ST_TERMINATED: CONVINFO_STATUS = 32u32;
pub const SZDDESYS_ITEM_FORMATS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Formats");
pub const SZDDESYS_ITEM_HELP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Help");
pub const SZDDESYS_ITEM_RTNMSG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ReturnMessage");
pub const SZDDESYS_ITEM_STATUS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Status");
pub const SZDDESYS_ITEM_SYSITEMS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SysItems");
pub const SZDDESYS_ITEM_TOPICS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Topics");
pub const SZDDESYS_TOPIC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("System");
pub const SZDDE_ITEM_ITEMLIST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TopicItemList");
pub const TIMEOUT_ASYNC: u32 = 4294967295u32;
pub const WM_DDE_ACK: u32 = 996u32;
pub const WM_DDE_ADVISE: u32 = 994u32;
pub const WM_DDE_DATA: u32 = 997u32;
pub const WM_DDE_EXECUTE: u32 = 1000u32;
pub const WM_DDE_FIRST: u32 = 992u32;
pub const WM_DDE_INITIATE: u32 = 992u32;
pub const WM_DDE_LAST: u32 = 1000u32;
pub const WM_DDE_POKE: u32 = 999u32;
pub const WM_DDE_REQUEST: u32 = 998u32;
pub const WM_DDE_TERMINATE: u32 = 993u32;
pub const WM_DDE_UNADVISE: u32 = 995u32;
pub const XCLASS_BOOL: u32 = 4096u32;
pub const XCLASS_DATA: u32 = 8192u32;
pub const XCLASS_FLAGS: u32 = 16384u32;
pub const XCLASS_MASK: u32 = 64512u32;
pub const XCLASS_NOTIFICATION: u32 = 32768u32;
pub const XST_ADVACKRCVD: CONVINFO_CONVERSATION_STATE = 13u32;
pub const XST_ADVDATAACKRCVD: CONVINFO_CONVERSATION_STATE = 16u32;
pub const XST_ADVDATASENT: CONVINFO_CONVERSATION_STATE = 15u32;
pub const XST_ADVSENT: CONVINFO_CONVERSATION_STATE = 11u32;
pub const XST_CONNECTED: CONVINFO_CONVERSATION_STATE = 2u32;
pub const XST_DATARCVD: CONVINFO_CONVERSATION_STATE = 6u32;
pub const XST_EXECACKRCVD: CONVINFO_CONVERSATION_STATE = 10u32;
pub const XST_EXECSENT: CONVINFO_CONVERSATION_STATE = 9u32;
pub const XST_INCOMPLETE: CONVINFO_CONVERSATION_STATE = 1u32;
pub const XST_INIT1: CONVINFO_CONVERSATION_STATE = 3u32;
pub const XST_INIT2: CONVINFO_CONVERSATION_STATE = 4u32;
pub const XST_NULL: CONVINFO_CONVERSATION_STATE = 0u32;
pub const XST_POKEACKRCVD: CONVINFO_CONVERSATION_STATE = 8u32;
pub const XST_POKESENT: CONVINFO_CONVERSATION_STATE = 7u32;
pub const XST_REQSENT: CONVINFO_CONVERSATION_STATE = 5u32;
pub const XST_UNADVACKRCVD: CONVINFO_CONVERSATION_STATE = 14u32;
pub const XST_UNADVSENT: CONVINFO_CONVERSATION_STATE = 12u32;
pub const XTYPF_ACKREQ: u32 = 8u32;
pub const XTYPF_NOBLOCK: u32 = 2u32;
pub const XTYPF_NODATA: u32 = 4u32;
pub const XTYP_ADVDATA: DDE_CLIENT_TRANSACTION_TYPE = 16400u32;
pub const XTYP_ADVREQ: DDE_CLIENT_TRANSACTION_TYPE = 8226u32;
pub const XTYP_ADVSTART: DDE_CLIENT_TRANSACTION_TYPE = 4144u32;
pub const XTYP_ADVSTOP: DDE_CLIENT_TRANSACTION_TYPE = 32832u32;
pub const XTYP_CONNECT: DDE_CLIENT_TRANSACTION_TYPE = 4194u32;
pub const XTYP_CONNECT_CONFIRM: DDE_CLIENT_TRANSACTION_TYPE = 32882u32;
pub const XTYP_DISCONNECT: DDE_CLIENT_TRANSACTION_TYPE = 32962u32;
pub const XTYP_EXECUTE: DDE_CLIENT_TRANSACTION_TYPE = 16464u32;
pub const XTYP_MASK: u32 = 240u32;
pub const XTYP_MONITOR: DDE_CLIENT_TRANSACTION_TYPE = 33010u32;
pub const XTYP_POKE: DDE_CLIENT_TRANSACTION_TYPE = 16528u32;
pub const XTYP_REGISTER: DDE_CLIENT_TRANSACTION_TYPE = 32930u32;
pub const XTYP_REQUEST: DDE_CLIENT_TRANSACTION_TYPE = 8368u32;
pub const XTYP_SHIFT: u32 = 4u32;
pub const XTYP_UNREGISTER: DDE_CLIENT_TRANSACTION_TYPE = 32978u32;
pub const XTYP_WILDCONNECT: DDE_CLIENT_TRANSACTION_TYPE = 8418u32;
pub const XTYP_XACT_COMPLETE: DDE_CLIENT_TRANSACTION_TYPE = 32896u32;
pub type CONVINFO_CONVERSATION_STATE = u32;
pub type CONVINFO_STATUS = u32;
pub type DDE_CLIENT_TRANSACTION_TYPE = u32;
pub type DDE_ENABLE_CALLBACK_CMD = u32;
pub type DDE_INITIALIZE_COMMAND = u32;
pub type DDE_NAME_SERVICE_CMD = u32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct CONVCONTEXT {
    pub cb: u32,
    pub wFlags: u32,
    pub wCountryID: u32,
    pub iCodePage: i32,
    pub dwLangID: u32,
    pub dwSecurity: u32,
    pub qos: super::super::Security::SECURITY_QUALITY_OF_SERVICE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for CONVCONTEXT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for CONVCONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct CONVINFO {
    pub cb: u32,
    pub hUser: usize,
    pub hConvPartner: HCONV,
    pub hszSvcPartner: HSZ,
    pub hszServiceReq: HSZ,
    pub hszTopic: HSZ,
    pub hszItem: HSZ,
    pub wFmt: u32,
    pub wType: DDE_CLIENT_TRANSACTION_TYPE,
    pub wStatus: CONVINFO_STATUS,
    pub wConvst: CONVINFO_CONVERSATION_STATE,
    pub wLastError: u32,
    pub hConvList: HCONVLIST,
    pub ConvCtxt: CONVCONTEXT,
    pub hwnd: super::super::Foundation::HWND,
    pub hwndPartner: super::super::Foundation::HWND,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for CONVINFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for CONVINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COPYDATASTRUCT {
    pub dwData: usize,
    pub cbData: u32,
    pub lpData: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for COPYDATASTRUCT {}
impl ::core::clone::Clone for COPYDATASTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DDEACK {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for DDEACK {}
impl ::core::clone::Clone for DDEACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DDEADVISE {
    pub _bitfield: u16,
    pub cfFormat: i16,
}
impl ::core::marker::Copy for DDEADVISE {}
impl ::core::clone::Clone for DDEADVISE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DDEDATA {
    pub _bitfield: u16,
    pub cfFormat: i16,
    pub Value: [u8; 1],
}
impl ::core::marker::Copy for DDEDATA {}
impl ::core::clone::Clone for DDEDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DDELN {
    pub _bitfield: u16,
    pub cfFormat: i16,
}
impl ::core::marker::Copy for DDELN {}
impl ::core::clone::Clone for DDELN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DDEML_MSG_HOOK_DATA {
    pub uiLo: usize,
    pub uiHi: usize,
    pub cbData: u32,
    pub Data: [u32; 8],
}
impl ::core::marker::Copy for DDEML_MSG_HOOK_DATA {}
impl ::core::clone::Clone for DDEML_MSG_HOOK_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DDEPOKE {
    pub _bitfield: u16,
    pub cfFormat: i16,
    pub Value: [u8; 1],
}
impl ::core::marker::Copy for DDEPOKE {}
impl ::core::clone::Clone for DDEPOKE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DDEUP {
    pub _bitfield: u16,
    pub cfFormat: i16,
    pub rgb: [u8; 1],
}
impl ::core::marker::Copy for DDEUP {}
impl ::core::clone::Clone for DDEUP {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HCONV = isize;
pub type HCONVLIST = isize;
pub type HDDEDATA = isize;
pub type HSZ = isize;
#[repr(C)]
pub struct HSZPAIR {
    pub hszSvc: HSZ,
    pub hszTopic: HSZ,
}
impl ::core::marker::Copy for HSZPAIR {}
impl ::core::clone::Clone for HSZPAIR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct METAFILEPICT {
    pub mm: i32,
    pub xExt: i32,
    pub yExt: i32,
    pub hMF: super::super::Graphics::Gdi::HMETAFILE,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for METAFILEPICT {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for METAFILEPICT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub struct MONCBSTRUCT {
    pub cb: u32,
    pub dwTime: u32,
    pub hTask: super::super::Foundation::HANDLE,
    pub dwRet: u32,
    pub wType: u32,
    pub wFmt: u32,
    pub hConv: HCONV,
    pub hsz1: HSZ,
    pub hsz2: HSZ,
    pub hData: HDDEDATA,
    pub dwData1: usize,
    pub dwData2: usize,
    pub cc: CONVCONTEXT,
    pub cbData: u32,
    pub Data: [u32; 8],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for MONCBSTRUCT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for MONCBSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MONCONVSTRUCT {
    pub cb: u32,
    pub fConnect: super::super::Foundation::BOOL,
    pub dwTime: u32,
    pub hTask: super::super::Foundation::HANDLE,
    pub hszSvc: HSZ,
    pub hszTopic: HSZ,
    pub hConvClient: HCONV,
    pub hConvServer: HCONV,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MONCONVSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MONCONVSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MONERRSTRUCT {
    pub cb: u32,
    pub wLastError: u32,
    pub dwTime: u32,
    pub hTask: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MONERRSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MONERRSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MONHSZSTRUCTA {
    pub cb: u32,
    pub fsAction: super::super::Foundation::BOOL,
    pub dwTime: u32,
    pub hsz: HSZ,
    pub hTask: super::super::Foundation::HANDLE,
    pub str: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MONHSZSTRUCTA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MONHSZSTRUCTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MONHSZSTRUCTW {
    pub cb: u32,
    pub fsAction: super::super::Foundation::BOOL,
    pub dwTime: u32,
    pub hsz: HSZ,
    pub hTask: super::super::Foundation::HANDLE,
    pub str: [u16; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MONHSZSTRUCTW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MONHSZSTRUCTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MONLINKSTRUCT {
    pub cb: u32,
    pub dwTime: u32,
    pub hTask: super::super::Foundation::HANDLE,
    pub fEstablished: super::super::Foundation::BOOL,
    pub fNoData: super::super::Foundation::BOOL,
    pub hszSvc: HSZ,
    pub hszTopic: HSZ,
    pub hszItem: HSZ,
    pub wFmt: u32,
    pub fServer: super::super::Foundation::BOOL,
    pub hConvServer: HCONV,
    pub hConvClient: HCONV,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MONLINKSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MONLINKSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MONMSGSTRUCT {
    pub cb: u32,
    pub hwndTo: super::super::Foundation::HWND,
    pub dwTime: u32,
    pub hTask: super::super::Foundation::HANDLE,
    pub wMsg: u32,
    pub wParam: super::super::Foundation::WPARAM,
    pub lParam: super::super::Foundation::LPARAM,
    pub dmhd: DDEML_MSG_HOOK_DATA,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MONMSGSTRUCT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MONMSGSTRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PFNCALLBACK = ::core::option::Option<unsafe extern "system" fn(wtype: u32, wfmt: u32, hconv: HCONV, hsz1: HSZ, hsz2: HSZ, hdata: HDDEDATA, dwdata1: usize, dwdata2: usize) -> HDDEDATA>;
