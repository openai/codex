::windows_targets::link!("mapi32.dll" "system" fn MAPIFreeBuffer(pv : *mut ::core::ffi::c_void) -> u32);
pub const MAPI_AB_NOMODIFY: u32 = 1024u32;
pub const MAPI_BCC: u32 = 3u32;
pub const MAPI_BODY_AS_FILE: u32 = 512u32;
pub const MAPI_CC: u32 = 2u32;
pub const MAPI_DIALOG: u32 = 8u32;
pub const MAPI_ENVELOPE_ONLY: u32 = 64u32;
pub const MAPI_EXTENDED: u32 = 32u32;
pub const MAPI_E_ACCESS_DENIED: u32 = 6u32;
pub const MAPI_E_AMBIGUOUS_RECIPIENT: u32 = 21u32;
pub const MAPI_E_AMBIG_RECIP: u32 = 21u32;
pub const MAPI_E_ATTACHMENT_NOT_FOUND: u32 = 11u32;
pub const MAPI_E_ATTACHMENT_OPEN_FAILURE: u32 = 12u32;
pub const MAPI_E_ATTACHMENT_TOO_LARGE: u32 = 28u32;
pub const MAPI_E_ATTACHMENT_WRITE_FAILURE: u32 = 13u32;
pub const MAPI_E_BAD_RECIPTYPE: u32 = 15u32;
pub const MAPI_E_DISK_FULL: u32 = 4u32;
pub const MAPI_E_FAILURE: u32 = 2u32;
pub const MAPI_E_INSUFFICIENT_MEMORY: u32 = 5u32;
pub const MAPI_E_INVALID_EDITFIELDS: u32 = 24u32;
pub const MAPI_E_INVALID_MESSAGE: u32 = 17u32;
pub const MAPI_E_INVALID_RECIPS: u32 = 25u32;
pub const MAPI_E_INVALID_SESSION: u32 = 19u32;
pub const MAPI_E_LOGIN_FAILURE: u32 = 3u32;
pub const MAPI_E_LOGON_FAILURE: u32 = 3u32;
pub const MAPI_E_MESSAGE_IN_USE: u32 = 22u32;
pub const MAPI_E_NETWORK_FAILURE: u32 = 23u32;
pub const MAPI_E_NOT_SUPPORTED: u32 = 26u32;
pub const MAPI_E_NO_MESSAGES: u32 = 16u32;
pub const MAPI_E_TEXT_TOO_LARGE: u32 = 18u32;
pub const MAPI_E_TOO_MANY_FILES: u32 = 9u32;
pub const MAPI_E_TOO_MANY_RECIPIENTS: u32 = 10u32;
pub const MAPI_E_TOO_MANY_SESSIONS: u32 = 8u32;
pub const MAPI_E_TYPE_NOT_SUPPORTED: u32 = 20u32;
pub const MAPI_E_UNICODE_NOT_SUPPORTED: u32 = 27u32;
pub const MAPI_E_UNKNOWN_RECIPIENT: u32 = 14u32;
pub const MAPI_E_USER_ABORT: u32 = 1u32;
pub const MAPI_FORCE_DOWNLOAD: u32 = 4096u32;
pub const MAPI_FORCE_UNICODE: u32 = 262144u32;
pub const MAPI_GUARANTEE_FIFO: u32 = 256u32;
pub const MAPI_LOGON_UI: u32 = 1u32;
pub const MAPI_LONG_MSGID: u32 = 16384u32;
pub const MAPI_NEW_SESSION: u32 = 2u32;
pub const MAPI_OLE: u32 = 1u32;
pub const MAPI_OLE_STATIC: u32 = 2u32;
pub const MAPI_ORIG: u32 = 0u32;
pub const MAPI_PASSWORD_UI: u32 = 131072u32;
pub const MAPI_PEEK: u32 = 128u32;
pub const MAPI_RECEIPT_REQUESTED: u32 = 2u32;
pub const MAPI_SENT: u32 = 4u32;
pub const MAPI_SUPPRESS_ATTACH: u32 = 2048u32;
pub const MAPI_TO: u32 = 1u32;
pub const MAPI_UNREAD: u32 = 1u32;
pub const MAPI_UNREAD_ONLY: u32 = 32u32;
pub const MAPI_USER_ABORT: u32 = 1u32;
pub const SUCCESS_SUCCESS: u32 = 0u32;
#[repr(C)]
pub struct MapiFileDesc {
    pub ulReserved: u32,
    pub flFlags: u32,
    pub nPosition: u32,
    pub lpszPathName: ::windows_sys::core::PSTR,
    pub lpszFileName: ::windows_sys::core::PSTR,
    pub lpFileType: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for MapiFileDesc {}
impl ::core::clone::Clone for MapiFileDesc {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MapiFileDescW {
    pub ulReserved: u32,
    pub flFlags: u32,
    pub nPosition: u32,
    pub lpszPathName: ::windows_sys::core::PWSTR,
    pub lpszFileName: ::windows_sys::core::PWSTR,
    pub lpFileType: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for MapiFileDescW {}
impl ::core::clone::Clone for MapiFileDescW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MapiFileTagExt {
    pub ulReserved: u32,
    pub cbTag: u32,
    pub lpTag: *mut u8,
    pub cbEncoding: u32,
    pub lpEncoding: *mut u8,
}
impl ::core::marker::Copy for MapiFileTagExt {}
impl ::core::clone::Clone for MapiFileTagExt {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MapiMessage {
    pub ulReserved: u32,
    pub lpszSubject: ::windows_sys::core::PSTR,
    pub lpszNoteText: ::windows_sys::core::PSTR,
    pub lpszMessageType: ::windows_sys::core::PSTR,
    pub lpszDateReceived: ::windows_sys::core::PSTR,
    pub lpszConversationID: ::windows_sys::core::PSTR,
    pub flFlags: u32,
    pub lpOriginator: *mut MapiRecipDesc,
    pub nRecipCount: u32,
    pub lpRecips: *mut MapiRecipDesc,
    pub nFileCount: u32,
    pub lpFiles: *mut MapiFileDesc,
}
impl ::core::marker::Copy for MapiMessage {}
impl ::core::clone::Clone for MapiMessage {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MapiMessageW {
    pub ulReserved: u32,
    pub lpszSubject: ::windows_sys::core::PWSTR,
    pub lpszNoteText: ::windows_sys::core::PWSTR,
    pub lpszMessageType: ::windows_sys::core::PWSTR,
    pub lpszDateReceived: ::windows_sys::core::PWSTR,
    pub lpszConversationID: ::windows_sys::core::PWSTR,
    pub flFlags: u32,
    pub lpOriginator: *mut MapiRecipDescW,
    pub nRecipCount: u32,
    pub lpRecips: *mut MapiRecipDescW,
    pub nFileCount: u32,
    pub lpFiles: *mut MapiFileDescW,
}
impl ::core::marker::Copy for MapiMessageW {}
impl ::core::clone::Clone for MapiMessageW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MapiRecipDesc {
    pub ulReserved: u32,
    pub ulRecipClass: u32,
    pub lpszName: ::windows_sys::core::PSTR,
    pub lpszAddress: ::windows_sys::core::PSTR,
    pub ulEIDSize: u32,
    pub lpEntryID: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for MapiRecipDesc {}
impl ::core::clone::Clone for MapiRecipDesc {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MapiRecipDescW {
    pub ulReserved: u32,
    pub ulRecipClass: u32,
    pub lpszName: ::windows_sys::core::PWSTR,
    pub lpszAddress: ::windows_sys::core::PWSTR,
    pub ulEIDSize: u32,
    pub lpEntryID: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for MapiRecipDescW {}
impl ::core::clone::Clone for MapiRecipDescW {
    fn clone(&self) -> Self {
        *self
    }
}
pub type LPMAPIADDRESS = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpszcaption: ::windows_sys::core::PCSTR, neditfields: u32, lpszlabels: ::windows_sys::core::PCSTR, nrecips: u32, lprecips: *mut MapiRecipDesc, flflags: u32, ulreserved: u32, lpnnewrecips: *mut u32, lppnewrecips: *mut *mut MapiRecipDesc) -> u32>;
pub type LPMAPIDELETEMAIL = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpszmessageid: ::windows_sys::core::PCSTR, flflags: u32, ulreserved: u32) -> u32>;
pub type LPMAPIDETAILS = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lprecip: *mut MapiRecipDesc, flflags: u32, ulreserved: u32) -> u32>;
pub type LPMAPIFINDNEXT = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpszmessagetype: ::windows_sys::core::PCSTR, lpszseedmessageid: ::windows_sys::core::PCSTR, flflags: u32, ulreserved: u32, lpszmessageid: ::windows_sys::core::PCSTR) -> u32>;
pub type LPMAPIFREEBUFFER = ::core::option::Option<unsafe extern "system" fn(pv: *mut ::core::ffi::c_void) -> u32>;
pub type LPMAPILOGOFF = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, flflags: u32, ulreserved: u32) -> u32>;
pub type LPMAPILOGON = ::core::option::Option<unsafe extern "system" fn(uluiparam: usize, lpszprofilename: ::windows_sys::core::PCSTR, lpszpassword: ::windows_sys::core::PCSTR, flflags: u32, ulreserved: u32, lplhsession: *mut usize) -> u32>;
pub type LPMAPIREADMAIL = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpszmessageid: ::windows_sys::core::PCSTR, flflags: u32, ulreserved: u32, lppmessage: *mut *mut MapiMessage) -> u32>;
pub type LPMAPIRESOLVENAME = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpszname: ::windows_sys::core::PCSTR, flflags: u32, ulreserved: u32, lpprecip: *mut *mut MapiRecipDesc) -> u32>;
pub type LPMAPISAVEMAIL = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpmessage: *mut MapiMessage, flflags: u32, ulreserved: u32, lpszmessageid: ::windows_sys::core::PCSTR) -> u32>;
pub type LPMAPISENDDOCUMENTS = ::core::option::Option<unsafe extern "system" fn(uluiparam: usize, lpszdelimchar: ::windows_sys::core::PCSTR, lpszfilepaths: ::windows_sys::core::PCSTR, lpszfilenames: ::windows_sys::core::PCSTR, ulreserved: u32) -> u32>;
pub type LPMAPISENDMAIL = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpmessage: *mut MapiMessage, flflags: u32, ulreserved: u32) -> u32>;
pub type LPMAPISENDMAILW = ::core::option::Option<unsafe extern "system" fn(lhsession: usize, uluiparam: usize, lpmessage: *const MapiMessageW, flflags: u32, ulreserved: u32) -> u32>;
