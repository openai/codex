#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("htmlhelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HtmlHelpA(hwndcaller : super::super::Foundation:: HWND, pszfile : ::windows_sys::core::PCSTR, ucommand : u32, dwdata : usize) -> super::super::Foundation:: HWND);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("htmlhelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn HtmlHelpW(hwndcaller : super::super::Foundation:: HWND, pszfile : ::windows_sys::core::PCWSTR, ucommand : u32, dwdata : usize) -> super::super::Foundation:: HWND);
pub type IITDatabase = *mut ::core::ffi::c_void;
pub type IITPropList = *mut ::core::ffi::c_void;
pub type IITResultSet = *mut ::core::ffi::c_void;
pub type IStemSink = *mut ::core::ffi::c_void;
pub type IStemmerConfig = *mut ::core::ffi::c_void;
pub type IWordBreakerConfig = *mut ::core::ffi::c_void;
pub const CLSID_IITCmdInt: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daa2_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_IITDatabase: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x66673452_8c23_11d0_a84e_00aa006c7d01);
pub const CLSID_IITDatabaseLocal: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daa9_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_IITGroupUpdate: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daa4_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_IITIndexBuild: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8fa0d5aa_dedf_11d0_9a61_00c04fb68bf7);
pub const CLSID_IITPropList: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daae_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_IITResultSet: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daa7_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_IITSvMgr: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daa3_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_IITWWFilterBuild: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8fa0d5ab_dedf_11d0_9a61_00c04fb68bf7);
pub const CLSID_IITWordWheel: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd73725c2_8c12_11d0_a84e_00aa006c7d01);
pub const CLSID_IITWordWheelLocal: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daa8_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_IITWordWheelUpdate: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daa5_d393_11d0_9a56_00c04fb68bf7);
pub const CLSID_ITEngStemmer: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8fa0d5a8_dedf_11d0_9a61_00c04fb68bf7);
pub const CLSID_ITStdBreaker: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4662daaf_d393_11d0_9a56_00c04fb68bf7);
pub const E_ALL_WILD: ::windows_sys::core::HRESULT = -2147479467i32;
pub const E_ALREADYINIT: ::windows_sys::core::HRESULT = -2147479421i32;
pub const E_ALREADYOPEN: ::windows_sys::core::HRESULT = -2147479533i32;
pub const E_ASSERT: ::windows_sys::core::HRESULT = -2147479546i32;
pub const E_BADBREAKER: ::windows_sys::core::HRESULT = -2147479469i32;
pub const E_BADFILE: ::windows_sys::core::HRESULT = -2147479549i32;
pub const E_BADFILTERSIZE: ::windows_sys::core::HRESULT = -2147479528i32;
pub const E_BADFORMAT: ::windows_sys::core::HRESULT = -2147479548i32;
pub const E_BADINDEXFLAGS: ::windows_sys::core::HRESULT = -2147479456i32;
pub const E_BADPARAM: ::windows_sys::core::HRESULT = -2147479535i32;
pub const E_BADRANGEOP: ::windows_sys::core::HRESULT = -2147479459i32;
pub const E_BADVALUE: ::windows_sys::core::HRESULT = -2147479468i32;
pub const E_BADVERSION: ::windows_sys::core::HRESULT = -2147479550i32;
pub const E_CANTFINDDLL: ::windows_sys::core::HRESULT = -2147479538i32;
pub const E_DISKFULL: ::windows_sys::core::HRESULT = -2147479496i32;
pub const E_DUPLICATE: ::windows_sys::core::HRESULT = -2147479551i32;
pub const E_EXPECTEDTERM: ::windows_sys::core::HRESULT = -2147479465i32;
pub const E_FILECLOSE: ::windows_sys::core::HRESULT = -2147479503i32;
pub const E_FILECREATE: ::windows_sys::core::HRESULT = -2147479504i32;
pub const E_FILEDELETE: ::windows_sys::core::HRESULT = -2147479499i32;
pub const E_FILEINVALID: ::windows_sys::core::HRESULT = -2147479498i32;
pub const E_FILENOTFOUND: ::windows_sys::core::HRESULT = -2147479497i32;
pub const E_FILEREAD: ::windows_sys::core::HRESULT = -2147479502i32;
pub const E_FILESEEK: ::windows_sys::core::HRESULT = -2147479501i32;
pub const E_FILEWRITE: ::windows_sys::core::HRESULT = -2147479500i32;
pub const E_GETLASTERROR: ::windows_sys::core::HRESULT = -2147479536i32;
pub const E_GROUPIDTOOBIG: ::windows_sys::core::HRESULT = -2147479542i32;
pub const E_INTERRUPT: ::windows_sys::core::HRESULT = -2147479545i32;
pub const E_INVALIDSTATE: ::windows_sys::core::HRESULT = -2147479534i32;
pub const E_MISSINGPROP: ::windows_sys::core::HRESULT = -2147479424i32;
pub const E_MISSLPAREN: ::windows_sys::core::HRESULT = -2147479464i32;
pub const E_MISSQUOTE: ::windows_sys::core::HRESULT = -2147479462i32;
pub const E_MISSRPAREN: ::windows_sys::core::HRESULT = -2147479463i32;
pub const E_NAMETOOLONG: ::windows_sys::core::HRESULT = -2147479520i32;
pub const E_NOHANDLE: ::windows_sys::core::HRESULT = -2147479537i32;
pub const E_NOKEYPROP: ::windows_sys::core::HRESULT = -2147479417i32;
pub const E_NOMERGEDDATA: ::windows_sys::core::HRESULT = -2147479540i32;
pub const E_NOPERMISSION: ::windows_sys::core::HRESULT = -2147479547i32;
pub const E_NOSTEMMER: ::windows_sys::core::HRESULT = -2147479454i32;
pub const E_NOTEXIST: ::windows_sys::core::HRESULT = -2147479552i32;
pub const E_NOTFOUND: ::windows_sys::core::HRESULT = -2147479539i32;
pub const E_NOTINIT: ::windows_sys::core::HRESULT = -2147479420i32;
pub const E_NOTOPEN: ::windows_sys::core::HRESULT = -2147479533i32;
pub const E_NOTSUPPORTED: ::windows_sys::core::HRESULT = -2147479544i32;
pub const E_NULLQUERY: ::windows_sys::core::HRESULT = -2147479461i32;
pub const E_OUTOFRANGE: ::windows_sys::core::HRESULT = -2147479543i32;
pub const E_PROPLISTEMPTY: ::windows_sys::core::HRESULT = -2147479422i32;
pub const E_PROPLISTNOTEMPTY: ::windows_sys::core::HRESULT = -2147479423i32;
pub const E_RESULTSETEMPTY: ::windows_sys::core::HRESULT = -2147479419i32;
pub const E_STOPWORD: ::windows_sys::core::HRESULT = -2147479460i32;
pub const E_TOODEEP: ::windows_sys::core::HRESULT = -2147479466i32;
pub const E_TOOMANYCOLUMNS: ::windows_sys::core::HRESULT = -2147479418i32;
pub const E_TOOMANYDUPS: ::windows_sys::core::HRESULT = -2147479471i32;
pub const E_TOOMANYOBJECTS: ::windows_sys::core::HRESULT = -2147479527i32;
pub const E_TOOMANYTITLES: ::windows_sys::core::HRESULT = -2147479541i32;
pub const E_TOOMANYTOPICS: ::windows_sys::core::HRESULT = -2147479472i32;
pub const E_TREETOOBIG: ::windows_sys::core::HRESULT = -2147479470i32;
pub const E_UNKNOWN_TRANSPORT: ::windows_sys::core::HRESULT = -2147479530i32;
pub const E_UNMATCHEDTYPE: ::windows_sys::core::HRESULT = -2147479458i32;
pub const E_UNSUPPORTED_TRANSPORT: ::windows_sys::core::HRESULT = -2147479529i32;
pub const E_WILD_IN_DTYPE: ::windows_sys::core::HRESULT = -2147479455i32;
pub const E_WORDTOOLONG: ::windows_sys::core::HRESULT = -2147479457i32;
pub const HHACT_BACK: i32 = 7i32;
pub const HHACT_CONTRACT: i32 = 6i32;
pub const HHACT_CUSTOMIZE: i32 = 16i32;
pub const HHACT_EXPAND: i32 = 5i32;
pub const HHACT_FORWARD: i32 = 8i32;
pub const HHACT_HIGHLIGHT: i32 = 15i32;
pub const HHACT_HOME: i32 = 11i32;
pub const HHACT_JUMP1: i32 = 17i32;
pub const HHACT_JUMP2: i32 = 18i32;
pub const HHACT_LAST_ENUM: i32 = 23i32;
pub const HHACT_NOTES: i32 = 22i32;
pub const HHACT_OPTIONS: i32 = 13i32;
pub const HHACT_PRINT: i32 = 14i32;
pub const HHACT_REFRESH: i32 = 10i32;
pub const HHACT_STOP: i32 = 9i32;
pub const HHACT_SYNC: i32 = 12i32;
pub const HHACT_TAB_CONTENTS: i32 = 0i32;
pub const HHACT_TAB_FAVORITES: i32 = 4i32;
pub const HHACT_TAB_HISTORY: i32 = 3i32;
pub const HHACT_TAB_INDEX: i32 = 1i32;
pub const HHACT_TAB_SEARCH: i32 = 2i32;
pub const HHACT_TOC_NEXT: i32 = 20i32;
pub const HHACT_TOC_PREV: i32 = 21i32;
pub const HHACT_ZOOM: i32 = 19i32;
pub const HHN_FIRST: u32 = 4294966436u32;
pub const HHN_LAST: u32 = 4294966417u32;
pub const HHN_NAVCOMPLETE: u32 = 4294966436u32;
pub const HHN_TRACK: u32 = 4294966435u32;
pub const HHN_WINDOW_CREATE: u32 = 4294966434u32;
pub const HHWIN_BUTTON_BACK: u32 = 4u32;
pub const HHWIN_BUTTON_BROWSE_BCK: u32 = 256u32;
pub const HHWIN_BUTTON_BROWSE_FWD: u32 = 128u32;
pub const HHWIN_BUTTON_CONTENTS: u32 = 1024u32;
pub const HHWIN_BUTTON_EXPAND: u32 = 2u32;
pub const HHWIN_BUTTON_FAVORITES: u32 = 131072u32;
pub const HHWIN_BUTTON_FORWARD: u32 = 8u32;
pub const HHWIN_BUTTON_HISTORY: u32 = 65536u32;
pub const HHWIN_BUTTON_HOME: u32 = 64u32;
pub const HHWIN_BUTTON_INDEX: u32 = 16384u32;
pub const HHWIN_BUTTON_JUMP1: u32 = 262144u32;
pub const HHWIN_BUTTON_JUMP2: u32 = 524288u32;
pub const HHWIN_BUTTON_NOTES: u32 = 512u32;
pub const HHWIN_BUTTON_OPTIONS: u32 = 4096u32;
pub const HHWIN_BUTTON_PRINT: u32 = 8192u32;
pub const HHWIN_BUTTON_REFRESH: u32 = 32u32;
pub const HHWIN_BUTTON_SEARCH: u32 = 32768u32;
pub const HHWIN_BUTTON_STOP: u32 = 16u32;
pub const HHWIN_BUTTON_SYNC: u32 = 2048u32;
pub const HHWIN_BUTTON_TOC_NEXT: u32 = 2097152u32;
pub const HHWIN_BUTTON_TOC_PREV: u32 = 4194304u32;
pub const HHWIN_BUTTON_ZOOM: u32 = 1048576u32;
pub const HHWIN_NAVTAB_BOTTOM: i32 = 2i32;
pub const HHWIN_NAVTAB_LEFT: i32 = 1i32;
pub const HHWIN_NAVTAB_TOP: i32 = 0i32;
pub const HHWIN_NAVTYPE_AUTHOR: i32 = 5i32;
pub const HHWIN_NAVTYPE_CUSTOM_FIRST: i32 = 11i32;
pub const HHWIN_NAVTYPE_FAVORITES: i32 = 3i32;
pub const HHWIN_NAVTYPE_HISTORY: i32 = 4i32;
pub const HHWIN_NAVTYPE_INDEX: i32 = 1i32;
pub const HHWIN_NAVTYPE_SEARCH: i32 = 2i32;
pub const HHWIN_NAVTYPE_TOC: i32 = 0i32;
pub const HHWIN_PARAM_CUR_TAB: u32 = 8192u32;
pub const HHWIN_PARAM_EXPANSION: u32 = 512u32;
pub const HHWIN_PARAM_EXSTYLES: u32 = 8u32;
pub const HHWIN_PARAM_HISTORY_COUNT: u32 = 4096u32;
pub const HHWIN_PARAM_INFOTYPES: u32 = 128u32;
pub const HHWIN_PARAM_NAV_WIDTH: u32 = 32u32;
pub const HHWIN_PARAM_PROPERTIES: u32 = 2u32;
pub const HHWIN_PARAM_RECT: u32 = 16u32;
pub const HHWIN_PARAM_SHOWSTATE: u32 = 64u32;
pub const HHWIN_PARAM_STYLES: u32 = 4u32;
pub const HHWIN_PARAM_TABORDER: u32 = 2048u32;
pub const HHWIN_PARAM_TABPOS: u32 = 1024u32;
pub const HHWIN_PARAM_TB_FLAGS: u32 = 256u32;
pub const HHWIN_PROP_AUTO_SYNC: u32 = 256u32;
pub const HHWIN_PROP_CHANGE_TITLE: u32 = 8192u32;
pub const HHWIN_PROP_MENU: u32 = 65536u32;
pub const HHWIN_PROP_NAV_ONLY_WIN: u32 = 16384u32;
pub const HHWIN_PROP_NODEF_EXSTYLES: u32 = 16u32;
pub const HHWIN_PROP_NODEF_STYLES: u32 = 8u32;
pub const HHWIN_PROP_NOTB_TEXT: u32 = 64u32;
pub const HHWIN_PROP_NOTITLEBAR: u32 = 4u32;
pub const HHWIN_PROP_NO_TOOLBAR: u32 = 32768u32;
pub const HHWIN_PROP_ONTOP: u32 = 2u32;
pub const HHWIN_PROP_POST_QUIT: u32 = 128u32;
pub const HHWIN_PROP_TAB_ADVSEARCH: u32 = 131072u32;
pub const HHWIN_PROP_TAB_AUTOHIDESHOW: u32 = 1u32;
pub const HHWIN_PROP_TAB_CUSTOM1: u32 = 524288u32;
pub const HHWIN_PROP_TAB_CUSTOM2: u32 = 1048576u32;
pub const HHWIN_PROP_TAB_CUSTOM3: u32 = 2097152u32;
pub const HHWIN_PROP_TAB_CUSTOM4: u32 = 4194304u32;
pub const HHWIN_PROP_TAB_CUSTOM5: u32 = 8388608u32;
pub const HHWIN_PROP_TAB_CUSTOM6: u32 = 16777216u32;
pub const HHWIN_PROP_TAB_CUSTOM7: u32 = 33554432u32;
pub const HHWIN_PROP_TAB_CUSTOM8: u32 = 67108864u32;
pub const HHWIN_PROP_TAB_CUSTOM9: u32 = 134217728u32;
pub const HHWIN_PROP_TAB_FAVORITES: u32 = 4096u32;
pub const HHWIN_PROP_TAB_HISTORY: u32 = 2048u32;
pub const HHWIN_PROP_TAB_SEARCH: u32 = 1024u32;
pub const HHWIN_PROP_TRACKING: u32 = 512u32;
pub const HHWIN_PROP_TRI_PANE: u32 = 32u32;
pub const HHWIN_PROP_USER_POS: u32 = 262144u32;
pub const HHWIN_TB_MARGIN: u32 = 268435456u32;
pub const HH_ALINK_LOOKUP: HTML_HELP_COMMAND = 19i32;
pub const HH_CLOSE_ALL: HTML_HELP_COMMAND = 18i32;
pub const HH_DISPLAY_INDEX: HTML_HELP_COMMAND = 2i32;
pub const HH_DISPLAY_SEARCH: HTML_HELP_COMMAND = 3i32;
pub const HH_DISPLAY_TEXT_POPUP: HTML_HELP_COMMAND = 14i32;
pub const HH_DISPLAY_TOC: HTML_HELP_COMMAND = 1i32;
pub const HH_DISPLAY_TOPIC: HTML_HELP_COMMAND = 0i32;
pub const HH_ENUM_CATEGORY: HTML_HELP_COMMAND = 21i32;
pub const HH_ENUM_CATEGORY_IT: HTML_HELP_COMMAND = 22i32;
pub const HH_ENUM_INFO_TYPE: HTML_HELP_COMMAND = 7i32;
pub const HH_FTS_DEFAULT_PROXIMITY: HTML_HELP_COMMAND = -1i32;
pub const HH_GET_LAST_ERROR: HTML_HELP_COMMAND = 20i32;
pub const HH_GET_WIN_HANDLE: HTML_HELP_COMMAND = 6i32;
pub const HH_GET_WIN_TYPE: HTML_HELP_COMMAND = 5i32;
pub const HH_GPROPID_CONTENT_LANGUAGE: HH_GPROPID = 5i32;
pub const HH_GPROPID_CURRENT_SUBSET: HH_GPROPID = 4i32;
pub const HH_GPROPID_SINGLETHREAD: HH_GPROPID = 1i32;
pub const HH_GPROPID_TOOLBAR_MARGIN: HH_GPROPID = 2i32;
pub const HH_GPROPID_UI_LANGUAGE: HH_GPROPID = 3i32;
pub const HH_HELP_CONTEXT: HTML_HELP_COMMAND = 15i32;
pub const HH_HELP_FINDER: HTML_HELP_COMMAND = 0i32;
pub const HH_INITIALIZE: HTML_HELP_COMMAND = 28i32;
pub const HH_KEYWORD_LOOKUP: HTML_HELP_COMMAND = 13i32;
pub const HH_MAX_TABS: HTML_HELP_COMMAND = 19i32;
pub const HH_MAX_TABS_CUSTOM: HTML_HELP_COMMAND = 9i32;
pub const HH_PRETRANSLATEMESSAGE: HTML_HELP_COMMAND = 253i32;
pub const HH_RESERVED1: HTML_HELP_COMMAND = 10i32;
pub const HH_RESERVED2: HTML_HELP_COMMAND = 11i32;
pub const HH_RESERVED3: HTML_HELP_COMMAND = 12i32;
pub const HH_RESET_IT_FILTER: HTML_HELP_COMMAND = 23i32;
pub const HH_SAFE_DISPLAY_TOPIC: HTML_HELP_COMMAND = 32i32;
pub const HH_SET_EXCLUSIVE_FILTER: HTML_HELP_COMMAND = 25i32;
pub const HH_SET_GLOBAL_PROPERTY: HTML_HELP_COMMAND = 252i32;
pub const HH_SET_INCLUSIVE_FILTER: HTML_HELP_COMMAND = 24i32;
pub const HH_SET_INFO_TYPE: HTML_HELP_COMMAND = 8i32;
pub const HH_SET_QUERYSERVICE: HTML_HELP_COMMAND = 30i32;
pub const HH_SET_WIN_TYPE: HTML_HELP_COMMAND = 4i32;
pub const HH_SYNC: HTML_HELP_COMMAND = 9i32;
pub const HH_TAB_AUTHOR: i32 = 5i32;
pub const HH_TAB_CONTENTS: i32 = 0i32;
pub const HH_TAB_CUSTOM_FIRST: i32 = 11i32;
pub const HH_TAB_CUSTOM_LAST: i32 = 19i32;
pub const HH_TAB_FAVORITES: i32 = 3i32;
pub const HH_TAB_HISTORY: i32 = 4i32;
pub const HH_TAB_INDEX: i32 = 1i32;
pub const HH_TAB_SEARCH: i32 = 2i32;
pub const HH_TP_HELP_CONTEXTMENU: HTML_HELP_COMMAND = 16i32;
pub const HH_TP_HELP_WM_HELP: HTML_HELP_COMMAND = 17i32;
pub const HH_UNINITIALIZE: HTML_HELP_COMMAND = 29i32;
pub const IDTB_BACK: u32 = 204u32;
pub const IDTB_BROWSE_BACK: u32 = 212u32;
pub const IDTB_BROWSE_FWD: u32 = 211u32;
pub const IDTB_CONTENTS: u32 = 213u32;
pub const IDTB_CONTRACT: u32 = 201u32;
pub const IDTB_CUSTOMIZE: u32 = 221u32;
pub const IDTB_EXPAND: u32 = 200u32;
pub const IDTB_FAVORITES: u32 = 217u32;
pub const IDTB_FORWARD: u32 = 209u32;
pub const IDTB_HISTORY: u32 = 216u32;
pub const IDTB_HOME: u32 = 205u32;
pub const IDTB_INDEX: u32 = 214u32;
pub const IDTB_JUMP1: u32 = 218u32;
pub const IDTB_JUMP2: u32 = 219u32;
pub const IDTB_NOTES: u32 = 210u32;
pub const IDTB_OPTIONS: u32 = 208u32;
pub const IDTB_PRINT: u32 = 207u32;
pub const IDTB_REFRESH: u32 = 203u32;
pub const IDTB_SEARCH: u32 = 215u32;
pub const IDTB_STOP: u32 = 202u32;
pub const IDTB_SYNC: u32 = 206u32;
pub const IDTB_TOC_NEXT: u32 = 223u32;
pub const IDTB_TOC_PREV: u32 = 224u32;
pub const IDTB_ZOOM: u32 = 222u32;
pub const IITWBC_BREAK_ACCEPT_WILDCARDS: u32 = 1u32;
pub const IITWBC_BREAK_AND_STEM: u32 = 2u32;
pub const ITWW_CBKEY_MAX: u32 = 1024u32;
pub const ITWW_OPEN_NOCONNECT: u32 = 1u32;
pub const IT_EXCLUSIVE: i32 = 1i32;
pub const IT_HIDDEN: i32 = 2i32;
pub const IT_INCLUSIVE: i32 = 0i32;
pub const MAX_COLUMNS: u32 = 256u32;
pub const PRIORITY_HIGH: PRIORITY = 2i32;
pub const PRIORITY_LOW: PRIORITY = 0i32;
pub const PRIORITY_NORMAL: PRIORITY = 1i32;
pub const PROP_ADD: u32 = 0u32;
pub const PROP_DELETE: u32 = 1u32;
pub const PROP_UPDATE: u32 = 2u32;
pub const STDPROP_DISPLAYKEY: u32 = 101u32;
pub const STDPROP_INDEX_BREAK: u32 = 204u32;
pub const STDPROP_INDEX_DTYPE: u32 = 202u32;
pub const STDPROP_INDEX_LENGTH: u32 = 203u32;
pub const STDPROP_INDEX_TERM: u32 = 210u32;
pub const STDPROP_INDEX_TERM_RAW_LENGTH: u32 = 211u32;
pub const STDPROP_INDEX_TEXT: u32 = 200u32;
pub const STDPROP_INDEX_VFLD: u32 = 201u32;
pub const STDPROP_KEY: u32 = 4u32;
pub const STDPROP_SORTKEY: u32 = 100u32;
pub const STDPROP_SORTORDINAL: u32 = 102u32;
pub const STDPROP_TITLE: u32 = 2u32;
pub const STDPROP_UID: u32 = 1u32;
pub const STDPROP_USERDATA: u32 = 3u32;
pub const STDPROP_USERPROP_BASE: u32 = 65536u32;
pub const STDPROP_USERPROP_MAX: u32 = 2147483647u32;
pub const SZ_WWDEST_GLOBAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GLOBAL");
pub const SZ_WWDEST_KEY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("KEY");
pub const SZ_WWDEST_OCC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OCC");
pub const TYPE_POINTER: u32 = 1u32;
pub const TYPE_STRING: u32 = 2u32;
pub const TYPE_VALUE: u32 = 0u32;
pub type HH_GPROPID = i32;
pub type HTML_HELP_COMMAND = i32;
pub type PRIORITY = i32;
#[repr(C)]
pub struct COLUMNSTATUS {
    pub cPropCount: i32,
    pub cPropsLoaded: i32,
}
impl ::core::marker::Copy for COLUMNSTATUS {}
impl ::core::clone::Clone for COLUMNSTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CProperty {
    pub dwPropID: u32,
    pub cbData: u32,
    pub dwType: u32,
    pub Anonymous: CProperty_0,
    pub fPersist: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CProperty {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CProperty {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union CProperty_0 {
    pub lpszwData: ::windows_sys::core::PWSTR,
    pub lpvData: *mut ::core::ffi::c_void,
    pub dwValue: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CProperty_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CProperty_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
pub struct HHNTRACK {
    pub hdr: super::super::UI::Controls::NMHDR,
    pub pszCurUrl: ::windows_sys::core::PCSTR,
    pub idAction: i32,
    pub phhWinType: *mut HH_WINTYPE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::marker::Copy for HHNTRACK {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::clone::Clone for HHNTRACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
pub struct HHN_NOTIFY {
    pub hdr: super::super::UI::Controls::NMHDR,
    pub pszUrl: ::windows_sys::core::PCSTR,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::marker::Copy for HHN_NOTIFY {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::clone::Clone for HHN_NOTIFY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HH_AKLINK {
    pub cbStruct: i32,
    pub fReserved: super::super::Foundation::BOOL,
    pub pszKeywords: *mut i8,
    pub pszUrl: *mut i8,
    pub pszMsgText: *mut i8,
    pub pszMsgTitle: *mut i8,
    pub pszWindow: *mut i8,
    pub fIndexOnFail: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HH_AKLINK {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HH_AKLINK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HH_ENUM_CAT {
    pub cbStruct: i32,
    pub pszCatName: ::windows_sys::core::PCSTR,
    pub pszCatDescription: ::windows_sys::core::PCSTR,
}
impl ::core::marker::Copy for HH_ENUM_CAT {}
impl ::core::clone::Clone for HH_ENUM_CAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HH_ENUM_IT {
    pub cbStruct: i32,
    pub iType: i32,
    pub pszCatName: ::windows_sys::core::PCSTR,
    pub pszITName: ::windows_sys::core::PCSTR,
    pub pszITDescription: ::windows_sys::core::PCSTR,
}
impl ::core::marker::Copy for HH_ENUM_IT {}
impl ::core::clone::Clone for HH_ENUM_IT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HH_FTS_QUERY {
    pub cbStruct: i32,
    pub fUniCodeStrings: super::super::Foundation::BOOL,
    pub pszSearchQuery: *mut i8,
    pub iProximity: i32,
    pub fStemmedSearch: super::super::Foundation::BOOL,
    pub fTitleOnly: super::super::Foundation::BOOL,
    pub fExecute: super::super::Foundation::BOOL,
    pub pszWindow: *mut i8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HH_FTS_QUERY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HH_FTS_QUERY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct HH_GLOBAL_PROPERTY {
    pub id: HH_GPROPID,
    pub var: super::super::System::Variant::VARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for HH_GLOBAL_PROPERTY {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for HH_GLOBAL_PROPERTY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HH_POPUP {
    pub cbStruct: i32,
    pub hinst: super::super::Foundation::HINSTANCE,
    pub idString: u32,
    pub pszText: *mut i8,
    pub pt: super::super::Foundation::POINT,
    pub clrForeground: super::super::Foundation::COLORREF,
    pub clrBackground: super::super::Foundation::COLORREF,
    pub rcMargins: super::super::Foundation::RECT,
    pub pszFont: *mut i8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HH_POPUP {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HH_POPUP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct HH_SET_INFOTYPE {
    pub cbStruct: i32,
    pub pszCatName: ::windows_sys::core::PCSTR,
    pub pszInfoTypeName: ::windows_sys::core::PCSTR,
}
impl ::core::marker::Copy for HH_SET_INFOTYPE {}
impl ::core::clone::Clone for HH_SET_INFOTYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct HH_WINTYPE {
    pub cbStruct: i32,
    pub fUniCodeStrings: super::super::Foundation::BOOL,
    pub pszType: *mut i8,
    pub fsValidMembers: u32,
    pub fsWinProperties: u32,
    pub pszCaption: *mut i8,
    pub dwStyles: u32,
    pub dwExStyles: u32,
    pub rcWindowPos: super::super::Foundation::RECT,
    pub nShowState: i32,
    pub hwndHelp: super::super::Foundation::HWND,
    pub hwndCaller: super::super::Foundation::HWND,
    pub paInfoTypes: *mut u32,
    pub hwndToolBar: super::super::Foundation::HWND,
    pub hwndNavigation: super::super::Foundation::HWND,
    pub hwndHTML: super::super::Foundation::HWND,
    pub iNavWidth: i32,
    pub rcHTML: super::super::Foundation::RECT,
    pub pszToc: *mut i8,
    pub pszIndex: *mut i8,
    pub pszFile: *mut i8,
    pub pszHome: *mut i8,
    pub fsToolBarFlags: u32,
    pub fNotExpanded: super::super::Foundation::BOOL,
    pub curNavType: i32,
    pub tabpos: i32,
    pub idNotify: i32,
    pub tabOrder: [u8; 20],
    pub cHistory: i32,
    pub pszJump1: *mut i8,
    pub pszJump2: *mut i8,
    pub pszUrlJump1: *mut i8,
    pub pszUrlJump2: *mut i8,
    pub rcMinSize: super::super::Foundation::RECT,
    pub cbInfoTypes: i32,
    pub pszCustomTabs: *mut i8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for HH_WINTYPE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for HH_WINTYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ROWSTATUS {
    pub lRowFirst: i32,
    pub cRows: i32,
    pub cProperties: i32,
    pub cRowsTotal: i32,
}
impl ::core::marker::Copy for ROWSTATUS {}
impl ::core::clone::Clone for ROWSTATUS {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PFNCOLHEAPFREE = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void) -> i32>;
