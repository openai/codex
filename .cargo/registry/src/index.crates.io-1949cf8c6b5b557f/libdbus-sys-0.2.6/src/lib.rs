use std::os::raw::{c_void, c_char, c_uint, c_int, c_long};

pub type DBusConnection = c_void;
pub type DBusMessage = c_void;
pub type DBusWatch = c_void;
pub type DBusPendingCall = c_void;
pub type DBusTimeout = c_void;

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
/// System or Session bus
pub enum DBusBusType {
    Session = 0,
    System = 1,
    Starter = 2,
}

pub const DBUS_TYPE_ARRAY: c_int = 'a' as c_int;
pub const DBUS_TYPE_VARIANT: c_int = 'v' as c_int;
pub const DBUS_TYPE_BOOLEAN: c_int = 'b' as c_int;
pub const DBUS_TYPE_INVALID: c_int = 0;
pub const DBUS_TYPE_STRING: c_int = 's' as c_int;
pub const DBUS_TYPE_DICT_ENTRY: c_int = 'e' as c_int;
pub const DBUS_TYPE_BYTE: c_int = 'y' as c_int;
pub const DBUS_TYPE_INT16: c_int = 'n' as c_int;
pub const DBUS_TYPE_UINT16: c_int = 'q' as c_int;
pub const DBUS_TYPE_INT32: c_int = 'i' as c_int;
pub const DBUS_TYPE_UINT32: c_int = 'u' as c_int;
pub const DBUS_TYPE_INT64: c_int = 'x' as c_int;
pub const DBUS_TYPE_UINT64: c_int = 't' as c_int;
pub const DBUS_TYPE_DOUBLE: c_int = 'd' as c_int;
pub const DBUS_TYPE_UNIX_FD: c_int = 'h' as c_int;
pub const DBUS_TYPE_STRUCT: c_int = 'r' as c_int;
pub const DBUS_TYPE_OBJECT_PATH: c_int = 'o' as c_int;
pub const DBUS_TYPE_SIGNATURE: c_int = 'g' as c_int;

pub const DBUS_TIMEOUT_USE_DEFAULT: c_int = -1;
pub const DBUS_TIMEOUT_INFINITE: c_int = 0x7fffffff;

pub const DBUS_NAME_FLAG_ALLOW_REPLACEMENT: c_int = 1;
pub const DBUS_NAME_FLAG_REPLACE_EXISTING: c_int = 2;
pub const DBUS_NAME_FLAG_DO_NOT_QUEUE: c_int = 4;

pub const DBUS_WATCH_READABLE: c_int = 1;
pub const DBUS_WATCH_WRITABLE: c_int = 2;
pub const DBUS_WATCH_ERROR: c_int = 4;
pub const DBUS_WATCH_HANGUP: c_int = 8;

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusRequestNameReply {
    PrimaryOwner = 1,
    InQueue = 2,
    Exists = 3,
    AlreadyOwner = 4,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusReleaseNameReply {
    Released = 1,
    NonExistent = 2,
    NotOwner = 3,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusHandlerResult {
    Handled = 0,
    NotYetHandled = 1,
    NeedMemory = 2,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
/// One of the four different D-Bus message types.
pub enum DBusMessageType {
    /// This is not a valid message type (rarely used)
    Invalid = 0,
    /// This is a method call D-Bus message
    MethodCall = 1,
    /// This is a method return Ok D-Bus message, used when the method call message was successfully processed
    MethodReturn = 2,
    /// This is a method return with error D-Bus message, used when the method call message could not be handled 
    Error = 3,
    /// This is a signal, usually sent to whoever wants to listen
    Signal = 4,
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DBusDispatchStatus {
    DataRemains = 0,
    Complete = 1,
    NeedMemory = 2,
}

#[repr(C)]
pub struct DBusError {
    pub name: *const c_char,
    pub message: *const c_char,
    pub dummy: c_uint,
    pub padding1: *const c_void
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DBusMessageIter {
    pub dummy1: *mut c_void,
    pub dummy2: *mut c_void,
    pub dummy3: u32,
    pub dummy4: c_int,
    pub dummy5: c_int,
    pub dummy6: c_int,
    pub dummy7: c_int,
    pub dummy8: c_int,
    pub dummy9: c_int,
    pub dummy10: c_int,
    pub dummy11: c_int,
    pub pad1: c_int,
    pub pad2: c_int,
    // Here would have been padding; because pad3 is a 8 byte aligned pointer (on amd64).
    // Rust reserves the right not to copy the padding when cloning a struct,
    // but the D-Bus library uses this part of the struct too.
    // Hence, add a field as big as the padding to ensure Rust copies all data.
    pub pad2_added_by_rust: c_int,
    pub pad3: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DBusSignatureIter {
    pub dummy1: *mut c_void,
    pub dummy2: *mut c_void,
    pub dummy8: u32,
    pub dummy12: c_int,
    pub dummy17: c_int,
}

pub type DBusHandleMessageFunction = Option<extern "C" fn(conn: *mut DBusConnection, msg: *mut DBusMessage, user_data: *mut c_void) -> DBusHandlerResult>;

pub type DBusAddWatchFunction = Option<extern "C" fn(watch: *mut DBusWatch, user_data: *mut c_void) -> u32>;
pub type DBusRemoveWatchFunction = Option<extern "C" fn(watch: *mut DBusWatch, user_data: *mut c_void)>;
pub type DBusWatchToggledFunction = Option<extern "C" fn(watch: *mut DBusWatch, user_data: *mut c_void)>;

pub type DBusAddTimeoutFunction = Option<extern "C" fn(timeout: *mut DBusTimeout, user_data: *mut c_void) -> u32>;
pub type DBusTimeoutToggledFunction = Option<extern "C" fn(timeout: *mut DBusTimeout, user_data: *mut c_void)>;
pub type DBusRemoveTimeoutFunction = Option<extern "C" fn(timeout: *mut DBusTimeout, user_data: *mut c_void)>;

pub type DBusDispatchStatusFunction = Option<extern "C" fn(conn: *mut DBusConnection, new_status: DBusDispatchStatus, user_data: *mut c_void)>;

pub type DBusWakeupMainFunction = Option<extern "C" fn(conn: *mut DBusConnection, user_data: *mut c_void)>;

pub type DBusPendingCallNotifyFunction = Option<extern "C" fn(pending: *mut DBusPendingCall, user_data: *mut c_void)>;

pub type DBusFreeFunction = Option<extern "C" fn(memory: *mut c_void)>;

#[repr(C)]
pub struct DBusObjectPathVTable {
    pub unregister_function: Option<extern "C" fn(conn: *mut DBusConnection, user_data: *mut c_void)>,
    pub message_function: DBusHandleMessageFunction,
    pub dbus_internal_pad1: Option<extern "C" fn()>,
    pub dbus_internal_pad2: Option<extern "C" fn()>,
    pub dbus_internal_pad3: Option<extern "C" fn()>,
    pub dbus_internal_pad4: Option<extern "C" fn()>,
}

extern "C" {
    pub fn dbus_bus_get_private(t: DBusBusType, error: *mut DBusError) -> *mut DBusConnection;
    pub fn dbus_bus_get_unique_name(conn: *mut DBusConnection) -> *const c_char;
    pub fn dbus_bus_request_name(conn: *mut DBusConnection, name: *const c_char,
        flags: c_uint, error: *mut DBusError) -> c_int;
    pub fn dbus_bus_release_name(conn: *mut DBusConnection, name: *const c_char,
        error: *mut DBusError) -> c_int;
    pub fn dbus_bus_add_match(conn: *mut DBusConnection, rule: *const c_char,
        error: *mut DBusError);
    pub fn dbus_bus_remove_match(conn: *mut DBusConnection, rule: *const c_char,
        error: *mut DBusError);
    pub fn dbus_bus_register(conn: *mut DBusConnection, error: *mut DBusError) -> u32;

    pub fn dbus_connection_close(conn: *mut DBusConnection);
    pub fn dbus_connection_dispatch(conn: *mut DBusConnection) -> DBusDispatchStatus;
    pub fn dbus_connection_flush(conn: *mut DBusConnection);
    pub fn dbus_connection_open_private(address: *const c_char, error: *mut DBusError) -> *mut DBusConnection;
    pub fn dbus_connection_unref(conn: *mut DBusConnection);
    pub fn dbus_connection_get_is_connected(conn: *mut DBusConnection) -> u32;
    pub fn dbus_connection_set_exit_on_disconnect(conn: *mut DBusConnection, enable: u32);
    pub fn dbus_connection_send_with_reply_and_block(conn: *mut DBusConnection,
        message: *mut DBusMessage, timeout_milliseconds: c_int, error: *mut DBusError) -> *mut DBusMessage;
    pub fn dbus_connection_send_with_reply(conn: *mut DBusConnection,
        message: *mut DBusMessage, pending_return: *mut *mut DBusPendingCall, timeout_milliseconds: c_int) -> u32;
    pub fn dbus_connection_send(conn: *mut DBusConnection,
        message: *mut DBusMessage, serial: *mut u32) -> u32;
    pub fn dbus_connection_read_write_dispatch(conn: *mut DBusConnection,
        timeout_milliseconds: c_int) -> u32;
    pub fn dbus_connection_read_write(conn: *mut DBusConnection, timeout_milliseconds: c_int) -> u32;
    pub fn dbus_connection_try_register_object_path(conn: *mut DBusConnection,
        path: *const c_char, vtable: *const DBusObjectPathVTable, user_data: *mut c_void,
        error: *mut DBusError) -> u32;
    pub fn dbus_connection_unregister_object_path(conn: *mut DBusConnection,
        path: *const c_char) -> u32;
    pub fn dbus_connection_list_registered(conn: *mut DBusConnection,
        parent_path: *const c_char, child_entries: *mut *mut *mut c_char) -> u32;
    pub fn dbus_connection_add_filter(conn: *mut DBusConnection, function: DBusHandleMessageFunction,
        user_data: *mut c_void, free_data_function: DBusFreeFunction) -> u32;
    pub fn dbus_connection_remove_filter(conn: *mut DBusConnection, function: DBusHandleMessageFunction,
        user_data: *mut c_void) -> u32;
    pub fn dbus_connection_set_watch_functions(conn: *mut DBusConnection, add_function: DBusAddWatchFunction,
        remove_function: DBusRemoveWatchFunction, toggled_function: DBusWatchToggledFunction,
        data: *mut c_void, free_data_function: DBusFreeFunction) -> u32;
    pub fn dbus_connection_set_timeout_functions(conn: *mut DBusConnection, add_function: DBusAddTimeoutFunction,
        remove_function: DBusRemoveTimeoutFunction, toggled_function: DBusTimeoutToggledFunction,
        data: *mut c_void, free_data_function: DBusFreeFunction) -> u32;
    pub fn dbus_connection_set_dispatch_status_function(conn: *mut DBusConnection,
        dispatch_function: DBusDispatchStatusFunction, data: *mut c_void, free_data_function: DBusFreeFunction);
    pub fn dbus_connection_set_wakeup_main_function(conn: *mut DBusConnection,
        wakeup_function: DBusWakeupMainFunction, data: *mut c_void, free_data_function: DBusFreeFunction);
    pub fn dbus_connection_pop_message(conn: *mut DBusConnection) -> *mut DBusMessage;
    pub fn dbus_connection_get_dispatch_status(conn: *mut DBusConnection) -> DBusDispatchStatus;

    pub fn dbus_error_init(error: *mut DBusError);
    pub fn dbus_error_free(error: *mut DBusError);
    pub fn dbus_set_error(error: *mut DBusError, name: *const c_char, message: *const c_char, ...);
    pub fn dbus_set_error_from_message(error: *mut DBusError, message: *mut DBusMessage) -> u32;

    pub fn dbus_message_new_method_call(destination: *const c_char, path: *const c_char,
        iface: *const c_char, method: *const c_char) -> *mut DBusMessage;
    pub fn dbus_message_new_method_return(message: *mut DBusMessage) -> *mut DBusMessage;
    pub fn dbus_message_new_error(message: *mut DBusMessage,
        error_name: *const c_char, error_message: *const c_char) -> *mut DBusMessage;
    pub fn dbus_message_new_signal(path: *const c_char,
        iface: *const c_char, name: *const c_char) -> *mut DBusMessage;
    pub fn dbus_message_ref(message: *mut DBusMessage) -> *mut DBusMessage;
    pub fn dbus_message_unref(message: *mut DBusMessage);
    pub fn dbus_message_get_type(message: *mut DBusMessage) -> c_int;
    pub fn dbus_message_is_method_call(message: *mut DBusMessage, iface: *const c_char, method: *const c_char) -> u32;
    pub fn dbus_message_is_signal(message: *mut DBusMessage, iface: *const c_char, signal_name: *const c_char) -> u32;
    pub fn dbus_message_get_reply_serial(message: *mut DBusMessage) -> u32;
    pub fn dbus_message_get_serial(message: *mut DBusMessage) -> u32;
    pub fn dbus_message_get_path(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_get_interface(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_get_destination(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_get_member(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_get_sender(message: *mut DBusMessage) -> *const c_char;
    pub fn dbus_message_set_serial(message: *mut DBusMessage, serial: u32);
    pub fn dbus_message_set_sender(message: *mut DBusMessage, sender: *const c_char) -> u32;
    pub fn dbus_message_set_destination(message: *mut DBusMessage, destination: *const c_char) -> u32;
    pub fn dbus_message_get_no_reply(message: *mut DBusMessage) -> u32;
    pub fn dbus_message_set_no_reply(message: *mut DBusMessage, no_reply: u32);
    pub fn dbus_message_get_auto_start(message: *mut DBusMessage) -> u32;
    pub fn dbus_message_set_auto_start(message: *mut DBusMessage, no_reply: u32);
    pub fn dbus_message_copy(message: *const DBusMessage) -> *mut DBusMessage;

    pub fn dbus_message_iter_append_basic(iter: *mut DBusMessageIter, t: c_int, value: *const c_void) -> u32;
    pub fn dbus_message_iter_append_fixed_array(iter: *mut DBusMessageIter, element_type: c_int,
        value: *const c_void, n_elements: c_int) -> u32;
    pub fn dbus_message_iter_init(message: *mut DBusMessage, iter: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_init_append(message: *mut DBusMessage, iter: *mut DBusMessageIter);
    pub fn dbus_message_iter_get_arg_type(iter: *mut DBusMessageIter) -> c_int;
    pub fn dbus_message_iter_get_basic(iter: *mut DBusMessageIter, value: *mut c_void);
    pub fn dbus_message_iter_get_element_type(iter: *mut DBusMessageIter) -> c_int;
    pub fn dbus_message_iter_get_fixed_array(iter: *mut DBusMessageIter,
        value: *mut c_void, n_elements: *mut c_int) -> u32;
    pub fn dbus_message_iter_get_signature(iter: *mut DBusMessageIter) -> *mut c_char;
    pub fn dbus_message_iter_next(iter: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_recurse(iter: *mut DBusMessageIter, subiter: *mut DBusMessageIter);
    pub fn dbus_message_iter_open_container(iter: *mut DBusMessageIter, _type: c_int,
        contained_signature: *const c_char, sub: *mut DBusMessageIter) -> u32;
    pub fn dbus_message_iter_close_container(iter: *mut DBusMessageIter, sub: *mut DBusMessageIter) -> u32;

    pub fn dbus_signature_iter_init(iter: *mut DBusSignatureIter, signature: *const c_char);
    pub fn dbus_signature_iter_get_current_type(iter: *const DBusSignatureIter) -> c_int;
    pub fn dbus_signature_iter_get_signature(iter: *const DBusSignatureIter) -> *mut c_char;
    pub fn dbus_signature_iter_get_element_type(iter: *const DBusSignatureIter) -> c_int;
    pub fn dbus_signature_iter_next(iter: *mut DBusSignatureIter) -> bool;
    pub fn dbus_signature_iter_recurse(iter: *const DBusSignatureIter, subiter: *mut DBusSignatureIter);

    pub fn dbus_free(memory: *mut c_void);
    pub fn dbus_free_string_array(str_array: *mut *mut c_char) -> c_void;

    pub fn dbus_signature_validate_single(signature: *const c_char, error: *mut DBusError) -> u32;

    pub fn dbus_threads_init_default() -> c_int;

    pub fn dbus_validate_bus_name(busname: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_error_name(errorname: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_interface(interface: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_member(member: *const c_char, error: *mut DBusError) -> u32;
    pub fn dbus_validate_path(path: *const c_char, error: *mut DBusError) -> u32;

    pub fn dbus_watch_get_enabled(watch: *mut DBusWatch) -> u32;
    pub fn dbus_watch_get_flags(watch: *mut DBusWatch) -> c_uint;
    pub fn dbus_watch_get_unix_fd(watch: *mut DBusWatch) -> c_int;
    pub fn dbus_watch_get_socket(watch: *mut DBusWatch) -> c_int;
    pub fn dbus_watch_handle(watch: *mut DBusWatch, flags: c_uint) -> u32;
    pub fn dbus_watch_get_data(watch: *mut DBusWatch) -> *mut c_void;
    pub fn dbus_watch_set_data(watch: *mut DBusWatch, user_data: *mut c_void,
        free_data_function: DBusFreeFunction);

    pub fn dbus_timeout_get_enabled(timeout: *mut DBusTimeout) -> u32;
    pub fn dbus_timeout_get_interval(timeout: *mut DBusTimeout) -> c_int;
    pub fn dbus_timeout_handle(timeout: *mut DBusTimeout) -> u32;
    pub fn dbus_timeout_get_data(timeout: *mut DBusTimeout) -> *mut c_void;
    pub fn dbus_timeout_set_data(timeout: *mut DBusTimeout, user_data: *mut c_void,
        free_data_function: DBusFreeFunction);

    pub fn dbus_pending_call_ref(pending: *mut DBusPendingCall) -> *mut DBusPendingCall;
    pub fn dbus_pending_call_unref(pending: *mut DBusPendingCall);
    pub fn dbus_pending_call_set_notify(pending: *mut DBusPendingCall, n: DBusPendingCallNotifyFunction,
        user_data: *mut c_void, free_user_data: DBusFreeFunction) -> u32;
    pub fn dbus_pending_call_steal_reply(pending: *mut DBusPendingCall) -> *mut DBusMessage;

    pub fn dbus_message_marshal(msg: *mut DBusMessage, marshalled_data_p: *mut *mut c_char, len_p: *mut c_int) -> u32;
    pub fn dbus_message_demarshal(s: *const c_char, len: c_int, error: *mut DBusError) -> *mut DBusMessage;
    pub fn dbus_message_demarshal_bytes_needed(buf: *const c_char, len: c_int) -> c_int;

    pub fn dbus_connection_set_max_message_size(conn: *mut DBusConnection, size: c_long);
    pub fn dbus_connection_get_max_message_size(conn: *mut DBusConnection) -> c_long;
    pub fn dbus_connection_set_max_message_unix_fds(conn: *mut DBusConnection, n: c_long);
    pub fn dbus_connection_get_max_message_unix_fds(conn: *mut DBusConnection) -> c_long;

    pub fn dbus_connection_set_max_received_size(conn: *mut DBusConnection, size: c_long);
    pub fn dbus_connection_get_max_received_size(conn: *mut DBusConnection) -> c_long;
    pub fn dbus_connection_set_max_received_unix_fds(conn: *mut DBusConnection, n: c_long);
    pub fn dbus_connection_get_max_received_unix_fds(conn: *mut DBusConnection) -> c_long;

    pub fn dbus_connection_get_outgoing_size(conn: *mut DBusConnection) -> c_long;
    pub fn dbus_connection_get_outgoing_unix_fds(conn: *mut DBusConnection) -> c_long;
    pub fn dbus_connection_has_messages_to_send(conn: *mut DBusConnection) -> u32;

    pub fn dbus_try_get_local_machine_id(error: *mut DBusError) -> *mut c_char;
    pub fn dbus_get_local_machine_id() -> *mut c_char;
}
