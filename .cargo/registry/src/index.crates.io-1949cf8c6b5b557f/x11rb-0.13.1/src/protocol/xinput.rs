// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Input` X11 extension.

#![allow(clippy::too_many_arguments)]

#[allow(unused_imports)]
use std::borrow::Cow;
#[allow(unused_imports)]
use std::convert::TryInto;
#[allow(unused_imports)]
use crate::utils::RawFdContainer;
#[allow(unused_imports)]
use crate::x11_utils::{Request, RequestHeader, Serialize, TryParse, TryParseFd};
use std::io::IoSlice;
use crate::connection::RequestConnection;
#[allow(unused_imports)]
use crate::connection::Connection as X11Connection;
#[allow(unused_imports)]
use crate::cookie::{Cookie, CookieWithFds, VoidCookie};
use crate::errors::ConnectionError;
#[allow(unused_imports)]
use crate::errors::ReplyOrIdError;
#[allow(unused_imports)]
use super::xfixes;
#[allow(unused_imports)]
use super::xproto;

pub use x11rb_protocol::protocol::xinput::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn get_extension_version<'c, 'input, Conn>(conn: &'c Conn, name: &'input [u8]) -> Result<Cookie<'c, Conn, GetExtensionVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetExtensionVersionRequest {
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_input_devices<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, ListInputDevicesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListInputDevicesRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn open_device<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, OpenDeviceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = OpenDeviceRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn close_device<Conn>(conn: &Conn, device_id: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CloseDeviceRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_device_mode<Conn>(conn: &Conn, device_id: u8, mode: ValuatorMode) -> Result<Cookie<'_, Conn, SetDeviceModeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDeviceModeRequest {
        device_id,
        mode,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn select_extension_event<'c, 'input, Conn>(conn: &'c Conn, window: xproto::Window, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectExtensionEventRequest {
        window,
        classes: Cow::Borrowed(classes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_selected_extension_events<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetSelectedExtensionEventsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetSelectedExtensionEventsRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_device_dont_propagate_list<'c, 'input, Conn>(conn: &'c Conn, window: xproto::Window, mode: PropagateMode, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeDeviceDontPropagateListRequest {
        window,
        mode,
        classes: Cow::Borrowed(classes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_device_dont_propagate_list<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetDeviceDontPropagateListReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetDeviceDontPropagateListRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_device_motion_events<Conn, A>(conn: &Conn, start: xproto::Timestamp, stop: A, device_id: u8) -> Result<Cookie<'_, Conn, GetDeviceMotionEventsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
{
    let stop: xproto::Timestamp = stop.into();
    let request0 = GetDeviceMotionEventsRequest {
        start,
        stop,
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_keyboard_device<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, ChangeKeyboardDeviceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeKeyboardDeviceRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_pointer_device<Conn>(conn: &Conn, x_axis: u8, y_axis: u8, device_id: u8) -> Result<Cookie<'_, Conn, ChangePointerDeviceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangePointerDeviceRequest {
        x_axis,
        y_axis,
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn grab_device<'c, 'input, Conn, A>(conn: &'c Conn, grab_window: xproto::Window, time: A, this_device_mode: xproto::GrabMode, other_device_mode: xproto::GrabMode, owner_events: bool, device_id: u8, classes: &'input [EventClass]) -> Result<Cookie<'c, Conn, GrabDeviceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
{
    let time: xproto::Timestamp = time.into();
    let request0 = GrabDeviceRequest {
        grab_window,
        time,
        this_device_mode,
        other_device_mode,
        owner_events,
        device_id,
        classes: Cow::Borrowed(classes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn ungrab_device<Conn, A>(conn: &Conn, time: A, device_id: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
{
    let time: xproto::Timestamp = time.into();
    let request0 = UngrabDeviceRequest {
        time,
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn grab_device_key<'c, 'input, Conn, A, B>(conn: &'c Conn, grab_window: xproto::Window, modifiers: xproto::ModMask, modifier_device: A, grabbed_device: u8, key: B, this_device_mode: xproto::GrabMode, other_device_mode: xproto::GrabMode, owner_events: bool, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<u8>,
    B: Into<u8>,
{
    let modifier_device: u8 = modifier_device.into();
    let key: u8 = key.into();
    let request0 = GrabDeviceKeyRequest {
        grab_window,
        modifiers,
        modifier_device,
        grabbed_device,
        key,
        this_device_mode,
        other_device_mode,
        owner_events,
        classes: Cow::Borrowed(classes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn ungrab_device_key<Conn, A, B>(conn: &Conn, grab_window: xproto::Window, modifiers: xproto::ModMask, modifier_device: A, key: B, grabbed_device: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<u8>,
    B: Into<u8>,
{
    let modifier_device: u8 = modifier_device.into();
    let key: u8 = key.into();
    let request0 = UngrabDeviceKeyRequest {
        grab_window,
        modifiers,
        modifier_device,
        key,
        grabbed_device,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn grab_device_button<'c, 'input, Conn, A, B>(conn: &'c Conn, grab_window: xproto::Window, grabbed_device: u8, modifier_device: A, modifiers: xproto::ModMask, this_device_mode: xproto::GrabMode, other_device_mode: xproto::GrabMode, button: B, owner_events: bool, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<u8>,
    B: Into<u8>,
{
    let modifier_device: u8 = modifier_device.into();
    let button: u8 = button.into();
    let request0 = GrabDeviceButtonRequest {
        grab_window,
        grabbed_device,
        modifier_device,
        modifiers,
        this_device_mode,
        other_device_mode,
        button,
        owner_events,
        classes: Cow::Borrowed(classes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn ungrab_device_button<Conn, A, B>(conn: &Conn, grab_window: xproto::Window, modifiers: xproto::ModMask, modifier_device: A, button: B, grabbed_device: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<u8>,
    B: Into<u8>,
{
    let modifier_device: u8 = modifier_device.into();
    let button: u8 = button.into();
    let request0 = UngrabDeviceButtonRequest {
        grab_window,
        modifiers,
        modifier_device,
        button,
        grabbed_device,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn allow_device_events<Conn, A>(conn: &Conn, time: A, mode: DeviceInputMode, device_id: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
{
    let time: xproto::Timestamp = time.into();
    let request0 = AllowDeviceEventsRequest {
        time,
        mode,
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_device_focus<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, GetDeviceFocusReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetDeviceFocusRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_device_focus<Conn, A, B>(conn: &Conn, focus: A, time: B, revert_to: xproto::InputFocus, device_id: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Window>,
    B: Into<xproto::Timestamp>,
{
    let focus: xproto::Window = focus.into();
    let time: xproto::Timestamp = time.into();
    let request0 = SetDeviceFocusRequest {
        focus,
        time,
        revert_to,
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_feedback_control<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, GetFeedbackControlReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetFeedbackControlRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_feedback_control<Conn>(conn: &Conn, mask: ChangeFeedbackControlMask, device_id: u8, feedback_id: u8, feedback: FeedbackCtl) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeFeedbackControlRequest {
        mask,
        device_id,
        feedback_id,
        feedback,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_device_key_mapping<Conn>(conn: &Conn, device_id: u8, first_keycode: KeyCode, count: u8) -> Result<Cookie<'_, Conn, GetDeviceKeyMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetDeviceKeyMappingRequest {
        device_id,
        first_keycode,
        count,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_device_key_mapping<'c, 'input, Conn>(conn: &'c Conn, device_id: u8, first_keycode: KeyCode, keysyms_per_keycode: u8, keycode_count: u8, keysyms: &'input [xproto::Keysym]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeDeviceKeyMappingRequest {
        device_id,
        first_keycode,
        keysyms_per_keycode,
        keycode_count,
        keysyms: Cow::Borrowed(keysyms),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_device_modifier_mapping<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, GetDeviceModifierMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetDeviceModifierMappingRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_device_modifier_mapping<'c, 'input, Conn>(conn: &'c Conn, device_id: u8, keymaps: &'input [u8]) -> Result<Cookie<'c, Conn, SetDeviceModifierMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDeviceModifierMappingRequest {
        device_id,
        keymaps: Cow::Borrowed(keymaps),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_device_button_mapping<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, GetDeviceButtonMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetDeviceButtonMappingRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_device_button_mapping<'c, 'input, Conn>(conn: &'c Conn, device_id: u8, map: &'input [u8]) -> Result<Cookie<'c, Conn, SetDeviceButtonMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDeviceButtonMappingRequest {
        device_id,
        map: Cow::Borrowed(map),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_device_state<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, QueryDeviceStateReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryDeviceStateRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn device_bell<Conn>(conn: &Conn, device_id: u8, feedback_id: u8, feedback_class: u8, percent: i8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeviceBellRequest {
        device_id,
        feedback_id,
        feedback_class,
        percent,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_device_valuators<'c, 'input, Conn>(conn: &'c Conn, device_id: u8, first_valuator: u8, valuators: &'input [i32]) -> Result<Cookie<'c, Conn, SetDeviceValuatorsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDeviceValuatorsRequest {
        device_id,
        first_valuator,
        valuators: Cow::Borrowed(valuators),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_device_control<Conn>(conn: &Conn, control_id: DeviceControl, device_id: u8) -> Result<Cookie<'_, Conn, GetDeviceControlReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetDeviceControlRequest {
        control_id,
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_device_control<Conn>(conn: &Conn, control_id: DeviceControl, device_id: u8, control: DeviceCtl) -> Result<Cookie<'_, Conn, ChangeDeviceControlReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeDeviceControlRequest {
        control_id,
        device_id,
        control,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_device_properties<Conn>(conn: &Conn, device_id: u8) -> Result<Cookie<'_, Conn, ListDevicePropertiesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListDevicePropertiesRequest {
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_device_property<'c, 'input, Conn>(conn: &'c Conn, property: xproto::Atom, type_: xproto::Atom, device_id: u8, mode: xproto::PropMode, num_items: u32, items: &'input ChangeDevicePropertyAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeDevicePropertyRequest {
        property,
        type_,
        device_id,
        mode,
        num_items,
        items: Cow::Borrowed(items),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn delete_device_property<Conn>(conn: &Conn, property: xproto::Atom, device_id: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeleteDevicePropertyRequest {
        property,
        device_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_device_property<Conn>(conn: &Conn, property: xproto::Atom, type_: xproto::Atom, offset: u32, len: u32, device_id: u8, delete: bool) -> Result<Cookie<'_, Conn, GetDevicePropertyReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetDevicePropertyRequest {
        property,
        type_,
        offset,
        len,
        device_id,
        delete,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_query_pointer<Conn, A>(conn: &Conn, window: xproto::Window, deviceid: A) -> Result<Cookie<'_, Conn, XIQueryPointerReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIQueryPointerRequest {
        window,
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_warp_pointer<Conn, A>(conn: &Conn, src_win: xproto::Window, dst_win: xproto::Window, src_x: Fp1616, src_y: Fp1616, src_width: u16, src_height: u16, dst_x: Fp1616, dst_y: Fp1616, deviceid: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIWarpPointerRequest {
        src_win,
        dst_win,
        src_x,
        src_y,
        src_width,
        src_height,
        dst_x,
        dst_y,
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_change_cursor<Conn, A>(conn: &Conn, window: xproto::Window, cursor: xproto::Cursor, deviceid: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIChangeCursorRequest {
        window,
        cursor,
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_change_hierarchy<'c, 'input, Conn>(conn: &'c Conn, changes: &'input [HierarchyChange]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = XIChangeHierarchyRequest {
        changes: Cow::Borrowed(changes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_set_client_pointer<Conn, A>(conn: &Conn, window: xproto::Window, deviceid: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XISetClientPointerRequest {
        window,
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_get_client_pointer<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, XIGetClientPointerReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = XIGetClientPointerRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_select_events<'c, 'input, Conn>(conn: &'c Conn, window: xproto::Window, masks: &'input [EventMask]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = XISelectEventsRequest {
        window,
        masks: Cow::Borrowed(masks),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_query_version<Conn>(conn: &Conn, major_version: u16, minor_version: u16) -> Result<Cookie<'_, Conn, XIQueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = XIQueryVersionRequest {
        major_version,
        minor_version,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_query_device<Conn, A>(conn: &Conn, deviceid: A) -> Result<Cookie<'_, Conn, XIQueryDeviceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIQueryDeviceRequest {
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_set_focus<Conn, A, B>(conn: &Conn, window: xproto::Window, time: A, deviceid: B) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
    B: Into<DeviceId>,
{
    let time: xproto::Timestamp = time.into();
    let deviceid: DeviceId = deviceid.into();
    let request0 = XISetFocusRequest {
        window,
        time,
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_get_focus<Conn, A>(conn: &Conn, deviceid: A) -> Result<Cookie<'_, Conn, XIGetFocusReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIGetFocusRequest {
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_grab_device<'c, 'input, Conn, A, B>(conn: &'c Conn, window: xproto::Window, time: A, cursor: xproto::Cursor, deviceid: B, mode: xproto::GrabMode, paired_device_mode: xproto::GrabMode, owner_events: GrabOwner, mask: &'input [u32]) -> Result<Cookie<'c, Conn, XIGrabDeviceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
    B: Into<DeviceId>,
{
    let time: xproto::Timestamp = time.into();
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIGrabDeviceRequest {
        window,
        time,
        cursor,
        deviceid,
        mode,
        paired_device_mode,
        owner_events,
        mask: Cow::Borrowed(mask),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_ungrab_device<Conn, A, B>(conn: &Conn, time: A, deviceid: B) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
    B: Into<DeviceId>,
{
    let time: xproto::Timestamp = time.into();
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIUngrabDeviceRequest {
        time,
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_allow_events<Conn, A, B>(conn: &Conn, time: A, deviceid: B, event_mode: EventMode, touchid: u32, grab_window: xproto::Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
    B: Into<DeviceId>,
{
    let time: xproto::Timestamp = time.into();
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIAllowEventsRequest {
        time,
        deviceid,
        event_mode,
        touchid,
        grab_window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_passive_grab_device<'c, 'input, Conn, A, B>(conn: &'c Conn, time: A, grab_window: xproto::Window, cursor: xproto::Cursor, detail: u32, deviceid: B, grab_type: GrabType, grab_mode: GrabMode22, paired_device_mode: xproto::GrabMode, owner_events: GrabOwner, mask: &'input [u32], modifiers: &'input [u32]) -> Result<Cookie<'c, Conn, XIPassiveGrabDeviceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
    B: Into<DeviceId>,
{
    let time: xproto::Timestamp = time.into();
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIPassiveGrabDeviceRequest {
        time,
        grab_window,
        cursor,
        detail,
        deviceid,
        grab_type,
        grab_mode,
        paired_device_mode,
        owner_events,
        mask: Cow::Borrowed(mask),
        modifiers: Cow::Borrowed(modifiers),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_passive_ungrab_device<'c, 'input, Conn, A>(conn: &'c Conn, grab_window: xproto::Window, detail: u32, deviceid: A, grab_type: GrabType, modifiers: &'input [u32]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIPassiveUngrabDeviceRequest {
        grab_window,
        detail,
        deviceid,
        grab_type,
        modifiers: Cow::Borrowed(modifiers),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_list_properties<Conn, A>(conn: &Conn, deviceid: A) -> Result<Cookie<'_, Conn, XIListPropertiesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIListPropertiesRequest {
        deviceid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_change_property<'c, 'input, Conn, A>(conn: &'c Conn, deviceid: A, mode: xproto::PropMode, property: xproto::Atom, type_: xproto::Atom, num_items: u32, items: &'input XIChangePropertyAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIChangePropertyRequest {
        deviceid,
        mode,
        property,
        type_,
        num_items,
        items: Cow::Borrowed(items),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_delete_property<Conn, A>(conn: &Conn, deviceid: A, property: xproto::Atom) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIDeletePropertyRequest {
        deviceid,
        property,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn xi_get_property<Conn, A>(conn: &Conn, deviceid: A, delete: bool, property: xproto::Atom, type_: xproto::Atom, offset: u32, len: u32) -> Result<Cookie<'_, Conn, XIGetPropertyReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<DeviceId>,
{
    let deviceid: DeviceId = deviceid.into();
    let request0 = XIGetPropertyRequest {
        deviceid,
        delete,
        property,
        type_,
        offset,
        len,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_get_selected_events<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, XIGetSelectedEventsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = XIGetSelectedEventsRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn xi_barrier_release_pointer<'c, 'input, Conn>(conn: &'c Conn, barriers: &'input [BarrierReleasePointerInfo]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = XIBarrierReleasePointerRequest {
        barriers: Cow::Borrowed(barriers),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn send_extension_event<'c, 'input, Conn>(conn: &'c Conn, destination: xproto::Window, device_id: u8, propagate: bool, events: &'input [EventForSend], classes: &'input [EventClass]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SendExtensionEventRequest {
        destination,
        device_id,
        propagate,
        events: Cow::Borrowed(events),
        classes: Cow::Borrowed(classes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn xinput_get_extension_version<'c, 'input>(&'c self, name: &'input [u8]) -> Result<Cookie<'c, Self, GetExtensionVersionReply>, ConnectionError>
    {
        get_extension_version(self, name)
    }
    fn xinput_list_input_devices(&self) -> Result<Cookie<'_, Self, ListInputDevicesReply>, ConnectionError>
    {
        list_input_devices(self)
    }
    fn xinput_open_device(&self, device_id: u8) -> Result<Cookie<'_, Self, OpenDeviceReply>, ConnectionError>
    {
        open_device(self, device_id)
    }
    fn xinput_close_device(&self, device_id: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        close_device(self, device_id)
    }
    fn xinput_set_device_mode(&self, device_id: u8, mode: ValuatorMode) -> Result<Cookie<'_, Self, SetDeviceModeReply>, ConnectionError>
    {
        set_device_mode(self, device_id, mode)
    }
    fn xinput_select_extension_event<'c, 'input>(&'c self, window: xproto::Window, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        select_extension_event(self, window, classes)
    }
    fn xinput_get_selected_extension_events(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetSelectedExtensionEventsReply>, ConnectionError>
    {
        get_selected_extension_events(self, window)
    }
    fn xinput_change_device_dont_propagate_list<'c, 'input>(&'c self, window: xproto::Window, mode: PropagateMode, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_device_dont_propagate_list(self, window, mode, classes)
    }
    fn xinput_get_device_dont_propagate_list(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetDeviceDontPropagateListReply>, ConnectionError>
    {
        get_device_dont_propagate_list(self, window)
    }
    fn xinput_get_device_motion_events<A>(&self, start: xproto::Timestamp, stop: A, device_id: u8) -> Result<Cookie<'_, Self, GetDeviceMotionEventsReply>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
    {
        get_device_motion_events(self, start, stop, device_id)
    }
    fn xinput_change_keyboard_device(&self, device_id: u8) -> Result<Cookie<'_, Self, ChangeKeyboardDeviceReply>, ConnectionError>
    {
        change_keyboard_device(self, device_id)
    }
    fn xinput_change_pointer_device(&self, x_axis: u8, y_axis: u8, device_id: u8) -> Result<Cookie<'_, Self, ChangePointerDeviceReply>, ConnectionError>
    {
        change_pointer_device(self, x_axis, y_axis, device_id)
    }
    fn xinput_grab_device<'c, 'input, A>(&'c self, grab_window: xproto::Window, time: A, this_device_mode: xproto::GrabMode, other_device_mode: xproto::GrabMode, owner_events: bool, device_id: u8, classes: &'input [EventClass]) -> Result<Cookie<'c, Self, GrabDeviceReply>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
    {
        grab_device(self, grab_window, time, this_device_mode, other_device_mode, owner_events, device_id, classes)
    }
    fn xinput_ungrab_device<A>(&self, time: A, device_id: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
    {
        ungrab_device(self, time, device_id)
    }
    fn xinput_grab_device_key<'c, 'input, A, B>(&'c self, grab_window: xproto::Window, modifiers: xproto::ModMask, modifier_device: A, grabbed_device: u8, key: B, this_device_mode: xproto::GrabMode, other_device_mode: xproto::GrabMode, owner_events: bool, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    where
        A: Into<u8>,
        B: Into<u8>,
    {
        grab_device_key(self, grab_window, modifiers, modifier_device, grabbed_device, key, this_device_mode, other_device_mode, owner_events, classes)
    }
    fn xinput_ungrab_device_key<A, B>(&self, grab_window: xproto::Window, modifiers: xproto::ModMask, modifier_device: A, key: B, grabbed_device: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<u8>,
        B: Into<u8>,
    {
        ungrab_device_key(self, grab_window, modifiers, modifier_device, key, grabbed_device)
    }
    fn xinput_grab_device_button<'c, 'input, A, B>(&'c self, grab_window: xproto::Window, grabbed_device: u8, modifier_device: A, modifiers: xproto::ModMask, this_device_mode: xproto::GrabMode, other_device_mode: xproto::GrabMode, button: B, owner_events: bool, classes: &'input [EventClass]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    where
        A: Into<u8>,
        B: Into<u8>,
    {
        grab_device_button(self, grab_window, grabbed_device, modifier_device, modifiers, this_device_mode, other_device_mode, button, owner_events, classes)
    }
    fn xinput_ungrab_device_button<A, B>(&self, grab_window: xproto::Window, modifiers: xproto::ModMask, modifier_device: A, button: B, grabbed_device: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<u8>,
        B: Into<u8>,
    {
        ungrab_device_button(self, grab_window, modifiers, modifier_device, button, grabbed_device)
    }
    fn xinput_allow_device_events<A>(&self, time: A, mode: DeviceInputMode, device_id: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
    {
        allow_device_events(self, time, mode, device_id)
    }
    fn xinput_get_device_focus(&self, device_id: u8) -> Result<Cookie<'_, Self, GetDeviceFocusReply>, ConnectionError>
    {
        get_device_focus(self, device_id)
    }
    fn xinput_set_device_focus<A, B>(&self, focus: A, time: B, revert_to: xproto::InputFocus, device_id: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<xproto::Window>,
        B: Into<xproto::Timestamp>,
    {
        set_device_focus(self, focus, time, revert_to, device_id)
    }
    fn xinput_get_feedback_control(&self, device_id: u8) -> Result<Cookie<'_, Self, GetFeedbackControlReply>, ConnectionError>
    {
        get_feedback_control(self, device_id)
    }
    fn xinput_change_feedback_control(&self, mask: ChangeFeedbackControlMask, device_id: u8, feedback_id: u8, feedback: FeedbackCtl) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        change_feedback_control(self, mask, device_id, feedback_id, feedback)
    }
    fn xinput_get_device_key_mapping(&self, device_id: u8, first_keycode: KeyCode, count: u8) -> Result<Cookie<'_, Self, GetDeviceKeyMappingReply>, ConnectionError>
    {
        get_device_key_mapping(self, device_id, first_keycode, count)
    }
    fn xinput_change_device_key_mapping<'c, 'input>(&'c self, device_id: u8, first_keycode: KeyCode, keysyms_per_keycode: u8, keycode_count: u8, keysyms: &'input [xproto::Keysym]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_device_key_mapping(self, device_id, first_keycode, keysyms_per_keycode, keycode_count, keysyms)
    }
    fn xinput_get_device_modifier_mapping(&self, device_id: u8) -> Result<Cookie<'_, Self, GetDeviceModifierMappingReply>, ConnectionError>
    {
        get_device_modifier_mapping(self, device_id)
    }
    fn xinput_set_device_modifier_mapping<'c, 'input>(&'c self, device_id: u8, keymaps: &'input [u8]) -> Result<Cookie<'c, Self, SetDeviceModifierMappingReply>, ConnectionError>
    {
        set_device_modifier_mapping(self, device_id, keymaps)
    }
    fn xinput_get_device_button_mapping(&self, device_id: u8) -> Result<Cookie<'_, Self, GetDeviceButtonMappingReply>, ConnectionError>
    {
        get_device_button_mapping(self, device_id)
    }
    fn xinput_set_device_button_mapping<'c, 'input>(&'c self, device_id: u8, map: &'input [u8]) -> Result<Cookie<'c, Self, SetDeviceButtonMappingReply>, ConnectionError>
    {
        set_device_button_mapping(self, device_id, map)
    }
    fn xinput_query_device_state(&self, device_id: u8) -> Result<Cookie<'_, Self, QueryDeviceStateReply>, ConnectionError>
    {
        query_device_state(self, device_id)
    }
    fn xinput_device_bell(&self, device_id: u8, feedback_id: u8, feedback_class: u8, percent: i8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        device_bell(self, device_id, feedback_id, feedback_class, percent)
    }
    fn xinput_set_device_valuators<'c, 'input>(&'c self, device_id: u8, first_valuator: u8, valuators: &'input [i32]) -> Result<Cookie<'c, Self, SetDeviceValuatorsReply>, ConnectionError>
    {
        set_device_valuators(self, device_id, first_valuator, valuators)
    }
    fn xinput_get_device_control(&self, control_id: DeviceControl, device_id: u8) -> Result<Cookie<'_, Self, GetDeviceControlReply>, ConnectionError>
    {
        get_device_control(self, control_id, device_id)
    }
    fn xinput_change_device_control(&self, control_id: DeviceControl, device_id: u8, control: DeviceCtl) -> Result<Cookie<'_, Self, ChangeDeviceControlReply>, ConnectionError>
    {
        change_device_control(self, control_id, device_id, control)
    }
    fn xinput_list_device_properties(&self, device_id: u8) -> Result<Cookie<'_, Self, ListDevicePropertiesReply>, ConnectionError>
    {
        list_device_properties(self, device_id)
    }
    fn xinput_change_device_property<'c, 'input>(&'c self, property: xproto::Atom, type_: xproto::Atom, device_id: u8, mode: xproto::PropMode, num_items: u32, items: &'input ChangeDevicePropertyAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_device_property(self, property, type_, device_id, mode, num_items, items)
    }
    fn xinput_delete_device_property(&self, property: xproto::Atom, device_id: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        delete_device_property(self, property, device_id)
    }
    fn xinput_get_device_property(&self, property: xproto::Atom, type_: xproto::Atom, offset: u32, len: u32, device_id: u8, delete: bool) -> Result<Cookie<'_, Self, GetDevicePropertyReply>, ConnectionError>
    {
        get_device_property(self, property, type_, offset, len, device_id, delete)
    }
    fn xinput_xi_query_pointer<A>(&self, window: xproto::Window, deviceid: A) -> Result<Cookie<'_, Self, XIQueryPointerReply>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_query_pointer(self, window, deviceid)
    }
    fn xinput_xi_warp_pointer<A>(&self, src_win: xproto::Window, dst_win: xproto::Window, src_x: Fp1616, src_y: Fp1616, src_width: u16, src_height: u16, dst_x: Fp1616, dst_y: Fp1616, deviceid: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_warp_pointer(self, src_win, dst_win, src_x, src_y, src_width, src_height, dst_x, dst_y, deviceid)
    }
    fn xinput_xi_change_cursor<A>(&self, window: xproto::Window, cursor: xproto::Cursor, deviceid: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_change_cursor(self, window, cursor, deviceid)
    }
    fn xinput_xi_change_hierarchy<'c, 'input>(&'c self, changes: &'input [HierarchyChange]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        xi_change_hierarchy(self, changes)
    }
    fn xinput_xi_set_client_pointer<A>(&self, window: xproto::Window, deviceid: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_set_client_pointer(self, window, deviceid)
    }
    fn xinput_xi_get_client_pointer(&self, window: xproto::Window) -> Result<Cookie<'_, Self, XIGetClientPointerReply>, ConnectionError>
    {
        xi_get_client_pointer(self, window)
    }
    fn xinput_xi_select_events<'c, 'input>(&'c self, window: xproto::Window, masks: &'input [EventMask]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        xi_select_events(self, window, masks)
    }
    fn xinput_xi_query_version(&self, major_version: u16, minor_version: u16) -> Result<Cookie<'_, Self, XIQueryVersionReply>, ConnectionError>
    {
        xi_query_version(self, major_version, minor_version)
    }
    fn xinput_xi_query_device<A>(&self, deviceid: A) -> Result<Cookie<'_, Self, XIQueryDeviceReply>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_query_device(self, deviceid)
    }
    fn xinput_xi_set_focus<A, B>(&self, window: xproto::Window, time: A, deviceid: B) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
        B: Into<DeviceId>,
    {
        xi_set_focus(self, window, time, deviceid)
    }
    fn xinput_xi_get_focus<A>(&self, deviceid: A) -> Result<Cookie<'_, Self, XIGetFocusReply>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_get_focus(self, deviceid)
    }
    fn xinput_xi_grab_device<'c, 'input, A, B>(&'c self, window: xproto::Window, time: A, cursor: xproto::Cursor, deviceid: B, mode: xproto::GrabMode, paired_device_mode: xproto::GrabMode, owner_events: GrabOwner, mask: &'input [u32]) -> Result<Cookie<'c, Self, XIGrabDeviceReply>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
        B: Into<DeviceId>,
    {
        xi_grab_device(self, window, time, cursor, deviceid, mode, paired_device_mode, owner_events, mask)
    }
    fn xinput_xi_ungrab_device<A, B>(&self, time: A, deviceid: B) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
        B: Into<DeviceId>,
    {
        xi_ungrab_device(self, time, deviceid)
    }
    fn xinput_xi_allow_events<A, B>(&self, time: A, deviceid: B, event_mode: EventMode, touchid: u32, grab_window: xproto::Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
        B: Into<DeviceId>,
    {
        xi_allow_events(self, time, deviceid, event_mode, touchid, grab_window)
    }
    fn xinput_xi_passive_grab_device<'c, 'input, A, B>(&'c self, time: A, grab_window: xproto::Window, cursor: xproto::Cursor, detail: u32, deviceid: B, grab_type: GrabType, grab_mode: GrabMode22, paired_device_mode: xproto::GrabMode, owner_events: GrabOwner, mask: &'input [u32], modifiers: &'input [u32]) -> Result<Cookie<'c, Self, XIPassiveGrabDeviceReply>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
        B: Into<DeviceId>,
    {
        xi_passive_grab_device(self, time, grab_window, cursor, detail, deviceid, grab_type, grab_mode, paired_device_mode, owner_events, mask, modifiers)
    }
    fn xinput_xi_passive_ungrab_device<'c, 'input, A>(&'c self, grab_window: xproto::Window, detail: u32, deviceid: A, grab_type: GrabType, modifiers: &'input [u32]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_passive_ungrab_device(self, grab_window, detail, deviceid, grab_type, modifiers)
    }
    fn xinput_xi_list_properties<A>(&self, deviceid: A) -> Result<Cookie<'_, Self, XIListPropertiesReply>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_list_properties(self, deviceid)
    }
    fn xinput_xi_change_property<'c, 'input, A>(&'c self, deviceid: A, mode: xproto::PropMode, property: xproto::Atom, type_: xproto::Atom, num_items: u32, items: &'input XIChangePropertyAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_change_property(self, deviceid, mode, property, type_, num_items, items)
    }
    fn xinput_xi_delete_property<A>(&self, deviceid: A, property: xproto::Atom) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_delete_property(self, deviceid, property)
    }
    fn xinput_xi_get_property<A>(&self, deviceid: A, delete: bool, property: xproto::Atom, type_: xproto::Atom, offset: u32, len: u32) -> Result<Cookie<'_, Self, XIGetPropertyReply>, ConnectionError>
    where
        A: Into<DeviceId>,
    {
        xi_get_property(self, deviceid, delete, property, type_, offset, len)
    }
    fn xinput_xi_get_selected_events(&self, window: xproto::Window) -> Result<Cookie<'_, Self, XIGetSelectedEventsReply>, ConnectionError>
    {
        xi_get_selected_events(self, window)
    }
    fn xinput_xi_barrier_release_pointer<'c, 'input>(&'c self, barriers: &'input [BarrierReleasePointerInfo]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        xi_barrier_release_pointer(self, barriers)
    }
    fn xinput_send_extension_event<'c, 'input>(&'c self, destination: xproto::Window, device_id: u8, propagate: bool, events: &'input [EventForSend], classes: &'input [EventClass]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        send_extension_event(self, destination, device_id, propagate, events, classes)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
