::windows_targets::link!("websocket.dll" "system" fn WebSocketAbortHandle(hwebsocket : WEB_SOCKET_HANDLE) -> ());
::windows_targets::link!("websocket.dll" "system" fn WebSocketBeginClientHandshake(hwebsocket : WEB_SOCKET_HANDLE, pszsubprotocols : *const ::windows_sys::core::PCSTR, ulsubprotocolcount : u32, pszextensions : *const ::windows_sys::core::PCSTR, ulextensioncount : u32, pinitialheaders : *const WEB_SOCKET_HTTP_HEADER, ulinitialheadercount : u32, padditionalheaders : *mut *mut WEB_SOCKET_HTTP_HEADER, puladditionalheadercount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketBeginServerHandshake(hwebsocket : WEB_SOCKET_HANDLE, pszsubprotocolselected : ::windows_sys::core::PCSTR, pszextensionselected : *const ::windows_sys::core::PCSTR, ulextensionselectedcount : u32, prequestheaders : *const WEB_SOCKET_HTTP_HEADER, ulrequestheadercount : u32, presponseheaders : *mut *mut WEB_SOCKET_HTTP_HEADER, pulresponseheadercount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketCompleteAction(hwebsocket : WEB_SOCKET_HANDLE, pvactioncontext : *const ::core::ffi::c_void, ulbytestransferred : u32) -> ());
::windows_targets::link!("websocket.dll" "system" fn WebSocketCreateClientHandle(pproperties : *const WEB_SOCKET_PROPERTY, ulpropertycount : u32, phwebsocket : *mut WEB_SOCKET_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketCreateServerHandle(pproperties : *const WEB_SOCKET_PROPERTY, ulpropertycount : u32, phwebsocket : *mut WEB_SOCKET_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketDeleteHandle(hwebsocket : WEB_SOCKET_HANDLE) -> ());
::windows_targets::link!("websocket.dll" "system" fn WebSocketEndClientHandshake(hwebsocket : WEB_SOCKET_HANDLE, presponseheaders : *const WEB_SOCKET_HTTP_HEADER, ulreponseheadercount : u32, pulselectedextensions : *mut u32, pulselectedextensioncount : *mut u32, pulselectedsubprotocol : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketEndServerHandshake(hwebsocket : WEB_SOCKET_HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketGetAction(hwebsocket : WEB_SOCKET_HANDLE, eactionqueue : WEB_SOCKET_ACTION_QUEUE, pdatabuffers : *mut WEB_SOCKET_BUFFER, puldatabuffercount : *mut u32, paction : *mut WEB_SOCKET_ACTION, pbuffertype : *mut WEB_SOCKET_BUFFER_TYPE, pvapplicationcontext : *mut *mut ::core::ffi::c_void, pvactioncontext : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketGetGlobalProperty(etype : WEB_SOCKET_PROPERTY_TYPE, pvvalue : *mut ::core::ffi::c_void, ulsize : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketReceive(hwebsocket : WEB_SOCKET_HANDLE, pbuffer : *const WEB_SOCKET_BUFFER, pvcontext : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("websocket.dll" "system" fn WebSocketSend(hwebsocket : WEB_SOCKET_HANDLE, buffertype : WEB_SOCKET_BUFFER_TYPE, pbuffer : *const WEB_SOCKET_BUFFER, context : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
pub const WEB_SOCKET_ABORTED_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1006i32;
pub const WEB_SOCKET_ALLOCATED_BUFFER_PROPERTY_TYPE: WEB_SOCKET_PROPERTY_TYPE = 3i32;
pub const WEB_SOCKET_ALL_ACTION_QUEUE: WEB_SOCKET_ACTION_QUEUE = 3i32;
pub const WEB_SOCKET_BINARY_FRAGMENT_BUFFER_TYPE: WEB_SOCKET_BUFFER_TYPE = -2147483645i32;
pub const WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE: WEB_SOCKET_BUFFER_TYPE = -2147483646i32;
pub const WEB_SOCKET_CLOSE_BUFFER_TYPE: WEB_SOCKET_BUFFER_TYPE = -2147483644i32;
pub const WEB_SOCKET_DISABLE_MASKING_PROPERTY_TYPE: WEB_SOCKET_PROPERTY_TYPE = 2i32;
pub const WEB_SOCKET_DISABLE_UTF8_VERIFICATION_PROPERTY_TYPE: WEB_SOCKET_PROPERTY_TYPE = 4i32;
pub const WEB_SOCKET_EMPTY_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1005i32;
pub const WEB_SOCKET_ENDPOINT_UNAVAILABLE_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1001i32;
pub const WEB_SOCKET_INDICATE_RECEIVE_COMPLETE_ACTION: WEB_SOCKET_ACTION = 4i32;
pub const WEB_SOCKET_INDICATE_SEND_COMPLETE_ACTION: WEB_SOCKET_ACTION = 2i32;
pub const WEB_SOCKET_INVALID_DATA_TYPE_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1003i32;
pub const WEB_SOCKET_INVALID_PAYLOAD_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1007i32;
pub const WEB_SOCKET_KEEPALIVE_INTERVAL_PROPERTY_TYPE: WEB_SOCKET_PROPERTY_TYPE = 5i32;
pub const WEB_SOCKET_MAX_CLOSE_REASON_LENGTH: u32 = 123u32;
pub const WEB_SOCKET_MESSAGE_TOO_BIG_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1009i32;
pub const WEB_SOCKET_NO_ACTION: WEB_SOCKET_ACTION = 0i32;
pub const WEB_SOCKET_PING_PONG_BUFFER_TYPE: WEB_SOCKET_BUFFER_TYPE = -2147483643i32;
pub const WEB_SOCKET_POLICY_VIOLATION_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1008i32;
pub const WEB_SOCKET_PROTOCOL_ERROR_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1002i32;
pub const WEB_SOCKET_RECEIVE_ACTION_QUEUE: WEB_SOCKET_ACTION_QUEUE = 2i32;
pub const WEB_SOCKET_RECEIVE_BUFFER_SIZE_PROPERTY_TYPE: WEB_SOCKET_PROPERTY_TYPE = 0i32;
pub const WEB_SOCKET_RECEIVE_FROM_NETWORK_ACTION: WEB_SOCKET_ACTION = 3i32;
pub const WEB_SOCKET_SECURE_HANDSHAKE_ERROR_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1015i32;
pub const WEB_SOCKET_SEND_ACTION_QUEUE: WEB_SOCKET_ACTION_QUEUE = 1i32;
pub const WEB_SOCKET_SEND_BUFFER_SIZE_PROPERTY_TYPE: WEB_SOCKET_PROPERTY_TYPE = 1i32;
pub const WEB_SOCKET_SEND_TO_NETWORK_ACTION: WEB_SOCKET_ACTION = 1i32;
pub const WEB_SOCKET_SERVER_ERROR_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1011i32;
pub const WEB_SOCKET_SUCCESS_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1000i32;
pub const WEB_SOCKET_SUPPORTED_VERSIONS_PROPERTY_TYPE: WEB_SOCKET_PROPERTY_TYPE = 6i32;
pub const WEB_SOCKET_UNSOLICITED_PONG_BUFFER_TYPE: WEB_SOCKET_BUFFER_TYPE = -2147483642i32;
pub const WEB_SOCKET_UNSUPPORTED_EXTENSIONS_CLOSE_STATUS: WEB_SOCKET_CLOSE_STATUS = 1010i32;
pub const WEB_SOCKET_UTF8_FRAGMENT_BUFFER_TYPE: WEB_SOCKET_BUFFER_TYPE = -2147483647i32;
pub const WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE: WEB_SOCKET_BUFFER_TYPE = -2147483648i32;
pub type WEB_SOCKET_ACTION = i32;
pub type WEB_SOCKET_ACTION_QUEUE = i32;
pub type WEB_SOCKET_BUFFER_TYPE = i32;
pub type WEB_SOCKET_CLOSE_STATUS = i32;
pub type WEB_SOCKET_PROPERTY_TYPE = i32;
#[repr(C)]
pub union WEB_SOCKET_BUFFER {
    pub Data: WEB_SOCKET_BUFFER_1,
    pub CloseStatus: WEB_SOCKET_BUFFER_0,
}
impl ::core::marker::Copy for WEB_SOCKET_BUFFER {}
impl ::core::clone::Clone for WEB_SOCKET_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WEB_SOCKET_BUFFER_0 {
    pub pbReason: *mut u8,
    pub ulReasonLength: u32,
    pub usStatus: u16,
}
impl ::core::marker::Copy for WEB_SOCKET_BUFFER_0 {}
impl ::core::clone::Clone for WEB_SOCKET_BUFFER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WEB_SOCKET_BUFFER_1 {
    pub pbBuffer: *mut u8,
    pub ulBufferLength: u32,
}
impl ::core::marker::Copy for WEB_SOCKET_BUFFER_1 {}
impl ::core::clone::Clone for WEB_SOCKET_BUFFER_1 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type WEB_SOCKET_HANDLE = isize;
#[repr(C)]
pub struct WEB_SOCKET_HTTP_HEADER {
    pub pcName: ::windows_sys::core::PSTR,
    pub ulNameLength: u32,
    pub pcValue: ::windows_sys::core::PSTR,
    pub ulValueLength: u32,
}
impl ::core::marker::Copy for WEB_SOCKET_HTTP_HEADER {}
impl ::core::clone::Clone for WEB_SOCKET_HTTP_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WEB_SOCKET_PROPERTY {
    pub Type: WEB_SOCKET_PROPERTY_TYPE,
    pub pvValue: *mut ::core::ffi::c_void,
    pub ulValueSize: u32,
}
impl ::core::marker::Copy for WEB_SOCKET_PROPERTY {}
impl ::core::clone::Clone for WEB_SOCKET_PROPERTY {
    fn clone(&self) -> Self {
        *self
    }
}
