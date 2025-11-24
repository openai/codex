// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `xkb` X11 extension.

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
use super::xproto;

pub use x11rb_protocol::protocol::xkb::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn use_extension<Conn>(conn: &Conn, wanted_major: u16, wanted_minor: u16) -> Result<Cookie<'_, Conn, UseExtensionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UseExtensionRequest {
        wanted_major,
        wanted_minor,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn select_events<'c, 'input, Conn>(conn: &'c Conn, device_spec: DeviceSpec, clear: EventType, select_all: EventType, affect_map: MapPart, map: MapPart, details: &'input SelectEventsAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectEventsRequest {
        device_spec,
        clear,
        select_all,
        affect_map,
        map,
        details: Cow::Borrowed(details),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn bell<Conn>(conn: &Conn, device_spec: DeviceSpec, bell_class: BellClassSpec, bell_id: IDSpec, percent: i8, force_sound: bool, event_only: bool, pitch: i16, duration: i16, name: xproto::Atom, window: xproto::Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = BellRequest {
        device_spec,
        bell_class,
        bell_id,
        percent,
        force_sound,
        event_only,
        pitch,
        duration,
        name,
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_state<Conn>(conn: &Conn, device_spec: DeviceSpec) -> Result<Cookie<'_, Conn, GetStateReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetStateRequest {
        device_spec,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn latch_lock_state<Conn>(conn: &Conn, device_spec: DeviceSpec, affect_mod_locks: xproto::ModMask, mod_locks: xproto::ModMask, lock_group: bool, group_lock: Group, affect_mod_latches: xproto::ModMask, latch_group: bool, group_latch: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = LatchLockStateRequest {
        device_spec,
        affect_mod_locks,
        mod_locks,
        lock_group,
        group_lock,
        affect_mod_latches,
        latch_group,
        group_latch,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_controls<Conn>(conn: &Conn, device_spec: DeviceSpec) -> Result<Cookie<'_, Conn, GetControlsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetControlsRequest {
        device_spec,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_controls<'c, 'input, Conn>(conn: &'c Conn, device_spec: DeviceSpec, affect_internal_real_mods: xproto::ModMask, internal_real_mods: xproto::ModMask, affect_ignore_lock_real_mods: xproto::ModMask, ignore_lock_real_mods: xproto::ModMask, affect_internal_virtual_mods: VMod, internal_virtual_mods: VMod, affect_ignore_lock_virtual_mods: VMod, ignore_lock_virtual_mods: VMod, mouse_keys_dflt_btn: u8, groups_wrap: u8, access_x_options: AXOption, affect_enabled_controls: BoolCtrl, enabled_controls: BoolCtrl, change_controls: Control, repeat_delay: u16, repeat_interval: u16, slow_keys_delay: u16, debounce_delay: u16, mouse_keys_delay: u16, mouse_keys_interval: u16, mouse_keys_time_to_max: u16, mouse_keys_max_speed: u16, mouse_keys_curve: i16, access_x_timeout: u16, access_x_timeout_mask: BoolCtrl, access_x_timeout_values: BoolCtrl, access_x_timeout_options_mask: AXOption, access_x_timeout_options_values: AXOption, per_key_repeat: &'input [u8; 32]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetControlsRequest {
        device_spec,
        affect_internal_real_mods,
        internal_real_mods,
        affect_ignore_lock_real_mods,
        ignore_lock_real_mods,
        affect_internal_virtual_mods,
        internal_virtual_mods,
        affect_ignore_lock_virtual_mods,
        ignore_lock_virtual_mods,
        mouse_keys_dflt_btn,
        groups_wrap,
        access_x_options,
        affect_enabled_controls,
        enabled_controls,
        change_controls,
        repeat_delay,
        repeat_interval,
        slow_keys_delay,
        debounce_delay,
        mouse_keys_delay,
        mouse_keys_interval,
        mouse_keys_time_to_max,
        mouse_keys_max_speed,
        mouse_keys_curve,
        access_x_timeout,
        access_x_timeout_mask,
        access_x_timeout_values,
        access_x_timeout_options_mask,
        access_x_timeout_options_values,
        per_key_repeat: Cow::Borrowed(per_key_repeat),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_map<Conn>(conn: &Conn, device_spec: DeviceSpec, full: MapPart, partial: MapPart, first_type: u8, n_types: u8, first_key_sym: xproto::Keycode, n_key_syms: u8, first_key_action: xproto::Keycode, n_key_actions: u8, first_key_behavior: xproto::Keycode, n_key_behaviors: u8, virtual_mods: VMod, first_key_explicit: xproto::Keycode, n_key_explicit: u8, first_mod_map_key: xproto::Keycode, n_mod_map_keys: u8, first_v_mod_map_key: xproto::Keycode, n_v_mod_map_keys: u8) -> Result<Cookie<'_, Conn, GetMapReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetMapRequest {
        device_spec,
        full,
        partial,
        first_type,
        n_types,
        first_key_sym,
        n_key_syms,
        first_key_action,
        n_key_actions,
        first_key_behavior,
        n_key_behaviors,
        virtual_mods,
        first_key_explicit,
        n_key_explicit,
        first_mod_map_key,
        n_mod_map_keys,
        first_v_mod_map_key,
        n_v_mod_map_keys,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_map<'c, 'input, Conn>(conn: &'c Conn, device_spec: DeviceSpec, flags: SetMapFlags, min_key_code: xproto::Keycode, max_key_code: xproto::Keycode, first_type: u8, n_types: u8, first_key_sym: xproto::Keycode, n_key_syms: u8, total_syms: u16, first_key_action: xproto::Keycode, n_key_actions: u8, total_actions: u16, first_key_behavior: xproto::Keycode, n_key_behaviors: u8, total_key_behaviors: u8, first_key_explicit: xproto::Keycode, n_key_explicit: u8, total_key_explicit: u8, first_mod_map_key: xproto::Keycode, n_mod_map_keys: u8, total_mod_map_keys: u8, first_v_mod_map_key: xproto::Keycode, n_v_mod_map_keys: u8, total_v_mod_map_keys: u8, virtual_mods: VMod, values: &'input SetMapAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetMapRequest {
        device_spec,
        flags,
        min_key_code,
        max_key_code,
        first_type,
        n_types,
        first_key_sym,
        n_key_syms,
        total_syms,
        first_key_action,
        n_key_actions,
        total_actions,
        first_key_behavior,
        n_key_behaviors,
        total_key_behaviors,
        first_key_explicit,
        n_key_explicit,
        total_key_explicit,
        first_mod_map_key,
        n_mod_map_keys,
        total_mod_map_keys,
        first_v_mod_map_key,
        n_v_mod_map_keys,
        total_v_mod_map_keys,
        virtual_mods,
        values: Cow::Borrowed(values),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_compat_map<Conn>(conn: &Conn, device_spec: DeviceSpec, groups: SetOfGroup, get_all_si: bool, first_si: u16, n_si: u16) -> Result<Cookie<'_, Conn, GetCompatMapReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetCompatMapRequest {
        device_spec,
        groups,
        get_all_si,
        first_si,
        n_si,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_compat_map<'c, 'input, Conn>(conn: &'c Conn, device_spec: DeviceSpec, recompute_actions: bool, truncate_si: bool, groups: SetOfGroup, first_si: u16, si: &'input [SymInterpret], group_maps: &'input [ModDef]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetCompatMapRequest {
        device_spec,
        recompute_actions,
        truncate_si,
        groups,
        first_si,
        si: Cow::Borrowed(si),
        group_maps: Cow::Borrowed(group_maps),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_indicator_state<Conn>(conn: &Conn, device_spec: DeviceSpec) -> Result<Cookie<'_, Conn, GetIndicatorStateReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetIndicatorStateRequest {
        device_spec,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_indicator_map<Conn>(conn: &Conn, device_spec: DeviceSpec, which: u32) -> Result<Cookie<'_, Conn, GetIndicatorMapReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetIndicatorMapRequest {
        device_spec,
        which,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_indicator_map<'c, 'input, Conn>(conn: &'c Conn, device_spec: DeviceSpec, which: u32, maps: &'input [IndicatorMap]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetIndicatorMapRequest {
        device_spec,
        which,
        maps: Cow::Borrowed(maps),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_named_indicator<Conn, A>(conn: &Conn, device_spec: DeviceSpec, led_class: LedClass, led_id: A, indicator: xproto::Atom) -> Result<Cookie<'_, Conn, GetNamedIndicatorReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<IDSpec>,
{
    let led_id: IDSpec = led_id.into();
    let request0 = GetNamedIndicatorRequest {
        device_spec,
        led_class,
        led_id,
        indicator,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_named_indicator<Conn, A>(conn: &Conn, device_spec: DeviceSpec, led_class: LedClass, led_id: A, indicator: xproto::Atom, set_state: bool, on: bool, set_map: bool, create_map: bool, map_flags: IMFlag, map_which_groups: IMGroupsWhich, map_groups: SetOfGroups, map_which_mods: IMModsWhich, map_real_mods: xproto::ModMask, map_vmods: VMod, map_ctrls: BoolCtrl) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<IDSpec>,
{
    let led_id: IDSpec = led_id.into();
    let request0 = SetNamedIndicatorRequest {
        device_spec,
        led_class,
        led_id,
        indicator,
        set_state,
        on,
        set_map,
        create_map,
        map_flags,
        map_which_groups,
        map_groups,
        map_which_mods,
        map_real_mods,
        map_vmods,
        map_ctrls,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_names<Conn>(conn: &Conn, device_spec: DeviceSpec, which: NameDetail) -> Result<Cookie<'_, Conn, GetNamesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetNamesRequest {
        device_spec,
        which,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_names<'c, 'input, Conn>(conn: &'c Conn, device_spec: DeviceSpec, virtual_mods: VMod, first_type: u8, n_types: u8, first_kt_levelt: u8, n_kt_levels: u8, indicators: u32, group_names: SetOfGroup, n_radio_groups: u8, first_key: xproto::Keycode, n_keys: u8, n_key_aliases: u8, total_kt_level_names: u16, values: &'input SetNamesAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetNamesRequest {
        device_spec,
        virtual_mods,
        first_type,
        n_types,
        first_kt_levelt,
        n_kt_levels,
        indicators,
        group_names,
        n_radio_groups,
        first_key,
        n_keys,
        n_key_aliases,
        total_kt_level_names,
        values: Cow::Borrowed(values),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn per_client_flags<Conn>(conn: &Conn, device_spec: DeviceSpec, change: PerClientFlag, value: PerClientFlag, ctrls_to_change: BoolCtrl, auto_ctrls: BoolCtrl, auto_ctrls_values: BoolCtrl) -> Result<Cookie<'_, Conn, PerClientFlagsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PerClientFlagsRequest {
        device_spec,
        change,
        value,
        ctrls_to_change,
        auto_ctrls,
        auto_ctrls_values,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_components<Conn>(conn: &Conn, device_spec: DeviceSpec, max_names: u16) -> Result<Cookie<'_, Conn, ListComponentsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListComponentsRequest {
        device_spec,
        max_names,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_kbd_by_name<Conn>(conn: &Conn, device_spec: DeviceSpec, need: GBNDetail, want: GBNDetail, load: bool) -> Result<Cookie<'_, Conn, GetKbdByNameReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetKbdByNameRequest {
        device_spec,
        need,
        want,
        load,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_device_info<Conn, A>(conn: &Conn, device_spec: DeviceSpec, wanted: XIFeature, all_buttons: bool, first_button: u8, n_buttons: u8, led_class: LedClass, led_id: A) -> Result<Cookie<'_, Conn, GetDeviceInfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<IDSpec>,
{
    let led_id: IDSpec = led_id.into();
    let request0 = GetDeviceInfoRequest {
        device_spec,
        wanted,
        all_buttons,
        first_button,
        n_buttons,
        led_class,
        led_id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_device_info<'c, 'input, Conn>(conn: &'c Conn, device_spec: DeviceSpec, first_btn: u8, change: XIFeature, btn_actions: &'input [Action], leds: &'input [DeviceLedInfo]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDeviceInfoRequest {
        device_spec,
        first_btn,
        change,
        btn_actions: Cow::Borrowed(btn_actions),
        leds: Cow::Borrowed(leds),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_debugging_flags<'c, 'input, Conn>(conn: &'c Conn, affect_flags: u32, flags: u32, affect_ctrls: u32, ctrls: u32, message: &'input [String8]) -> Result<Cookie<'c, Conn, SetDebuggingFlagsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDebuggingFlagsRequest {
        affect_flags,
        flags,
        affect_ctrls,
        ctrls,
        message: Cow::Borrowed(message),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn xkb_use_extension(&self, wanted_major: u16, wanted_minor: u16) -> Result<Cookie<'_, Self, UseExtensionReply>, ConnectionError>
    {
        use_extension(self, wanted_major, wanted_minor)
    }
    fn xkb_select_events<'c, 'input>(&'c self, device_spec: DeviceSpec, clear: EventType, select_all: EventType, affect_map: MapPart, map: MapPart, details: &'input SelectEventsAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        select_events(self, device_spec, clear, select_all, affect_map, map, details)
    }
    fn xkb_bell(&self, device_spec: DeviceSpec, bell_class: BellClassSpec, bell_id: IDSpec, percent: i8, force_sound: bool, event_only: bool, pitch: i16, duration: i16, name: xproto::Atom, window: xproto::Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        bell(self, device_spec, bell_class, bell_id, percent, force_sound, event_only, pitch, duration, name, window)
    }
    fn xkb_get_state(&self, device_spec: DeviceSpec) -> Result<Cookie<'_, Self, GetStateReply>, ConnectionError>
    {
        get_state(self, device_spec)
    }
    fn xkb_latch_lock_state(&self, device_spec: DeviceSpec, affect_mod_locks: xproto::ModMask, mod_locks: xproto::ModMask, lock_group: bool, group_lock: Group, affect_mod_latches: xproto::ModMask, latch_group: bool, group_latch: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        latch_lock_state(self, device_spec, affect_mod_locks, mod_locks, lock_group, group_lock, affect_mod_latches, latch_group, group_latch)
    }
    fn xkb_get_controls(&self, device_spec: DeviceSpec) -> Result<Cookie<'_, Self, GetControlsReply>, ConnectionError>
    {
        get_controls(self, device_spec)
    }
    fn xkb_set_controls<'c, 'input>(&'c self, device_spec: DeviceSpec, affect_internal_real_mods: xproto::ModMask, internal_real_mods: xproto::ModMask, affect_ignore_lock_real_mods: xproto::ModMask, ignore_lock_real_mods: xproto::ModMask, affect_internal_virtual_mods: VMod, internal_virtual_mods: VMod, affect_ignore_lock_virtual_mods: VMod, ignore_lock_virtual_mods: VMod, mouse_keys_dflt_btn: u8, groups_wrap: u8, access_x_options: AXOption, affect_enabled_controls: BoolCtrl, enabled_controls: BoolCtrl, change_controls: Control, repeat_delay: u16, repeat_interval: u16, slow_keys_delay: u16, debounce_delay: u16, mouse_keys_delay: u16, mouse_keys_interval: u16, mouse_keys_time_to_max: u16, mouse_keys_max_speed: u16, mouse_keys_curve: i16, access_x_timeout: u16, access_x_timeout_mask: BoolCtrl, access_x_timeout_values: BoolCtrl, access_x_timeout_options_mask: AXOption, access_x_timeout_options_values: AXOption, per_key_repeat: &'input [u8; 32]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_controls(self, device_spec, affect_internal_real_mods, internal_real_mods, affect_ignore_lock_real_mods, ignore_lock_real_mods, affect_internal_virtual_mods, internal_virtual_mods, affect_ignore_lock_virtual_mods, ignore_lock_virtual_mods, mouse_keys_dflt_btn, groups_wrap, access_x_options, affect_enabled_controls, enabled_controls, change_controls, repeat_delay, repeat_interval, slow_keys_delay, debounce_delay, mouse_keys_delay, mouse_keys_interval, mouse_keys_time_to_max, mouse_keys_max_speed, mouse_keys_curve, access_x_timeout, access_x_timeout_mask, access_x_timeout_values, access_x_timeout_options_mask, access_x_timeout_options_values, per_key_repeat)
    }
    fn xkb_get_map(&self, device_spec: DeviceSpec, full: MapPart, partial: MapPart, first_type: u8, n_types: u8, first_key_sym: xproto::Keycode, n_key_syms: u8, first_key_action: xproto::Keycode, n_key_actions: u8, first_key_behavior: xproto::Keycode, n_key_behaviors: u8, virtual_mods: VMod, first_key_explicit: xproto::Keycode, n_key_explicit: u8, first_mod_map_key: xproto::Keycode, n_mod_map_keys: u8, first_v_mod_map_key: xproto::Keycode, n_v_mod_map_keys: u8) -> Result<Cookie<'_, Self, GetMapReply>, ConnectionError>
    {
        get_map(self, device_spec, full, partial, first_type, n_types, first_key_sym, n_key_syms, first_key_action, n_key_actions, first_key_behavior, n_key_behaviors, virtual_mods, first_key_explicit, n_key_explicit, first_mod_map_key, n_mod_map_keys, first_v_mod_map_key, n_v_mod_map_keys)
    }
    fn xkb_set_map<'c, 'input>(&'c self, device_spec: DeviceSpec, flags: SetMapFlags, min_key_code: xproto::Keycode, max_key_code: xproto::Keycode, first_type: u8, n_types: u8, first_key_sym: xproto::Keycode, n_key_syms: u8, total_syms: u16, first_key_action: xproto::Keycode, n_key_actions: u8, total_actions: u16, first_key_behavior: xproto::Keycode, n_key_behaviors: u8, total_key_behaviors: u8, first_key_explicit: xproto::Keycode, n_key_explicit: u8, total_key_explicit: u8, first_mod_map_key: xproto::Keycode, n_mod_map_keys: u8, total_mod_map_keys: u8, first_v_mod_map_key: xproto::Keycode, n_v_mod_map_keys: u8, total_v_mod_map_keys: u8, virtual_mods: VMod, values: &'input SetMapAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_map(self, device_spec, flags, min_key_code, max_key_code, first_type, n_types, first_key_sym, n_key_syms, total_syms, first_key_action, n_key_actions, total_actions, first_key_behavior, n_key_behaviors, total_key_behaviors, first_key_explicit, n_key_explicit, total_key_explicit, first_mod_map_key, n_mod_map_keys, total_mod_map_keys, first_v_mod_map_key, n_v_mod_map_keys, total_v_mod_map_keys, virtual_mods, values)
    }
    fn xkb_get_compat_map(&self, device_spec: DeviceSpec, groups: SetOfGroup, get_all_si: bool, first_si: u16, n_si: u16) -> Result<Cookie<'_, Self, GetCompatMapReply>, ConnectionError>
    {
        get_compat_map(self, device_spec, groups, get_all_si, first_si, n_si)
    }
    fn xkb_set_compat_map<'c, 'input>(&'c self, device_spec: DeviceSpec, recompute_actions: bool, truncate_si: bool, groups: SetOfGroup, first_si: u16, si: &'input [SymInterpret], group_maps: &'input [ModDef]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_compat_map(self, device_spec, recompute_actions, truncate_si, groups, first_si, si, group_maps)
    }
    fn xkb_get_indicator_state(&self, device_spec: DeviceSpec) -> Result<Cookie<'_, Self, GetIndicatorStateReply>, ConnectionError>
    {
        get_indicator_state(self, device_spec)
    }
    fn xkb_get_indicator_map(&self, device_spec: DeviceSpec, which: u32) -> Result<Cookie<'_, Self, GetIndicatorMapReply>, ConnectionError>
    {
        get_indicator_map(self, device_spec, which)
    }
    fn xkb_set_indicator_map<'c, 'input>(&'c self, device_spec: DeviceSpec, which: u32, maps: &'input [IndicatorMap]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_indicator_map(self, device_spec, which, maps)
    }
    fn xkb_get_named_indicator<A>(&self, device_spec: DeviceSpec, led_class: LedClass, led_id: A, indicator: xproto::Atom) -> Result<Cookie<'_, Self, GetNamedIndicatorReply>, ConnectionError>
    where
        A: Into<IDSpec>,
    {
        get_named_indicator(self, device_spec, led_class, led_id, indicator)
    }
    fn xkb_set_named_indicator<A>(&self, device_spec: DeviceSpec, led_class: LedClass, led_id: A, indicator: xproto::Atom, set_state: bool, on: bool, set_map: bool, create_map: bool, map_flags: IMFlag, map_which_groups: IMGroupsWhich, map_groups: SetOfGroups, map_which_mods: IMModsWhich, map_real_mods: xproto::ModMask, map_vmods: VMod, map_ctrls: BoolCtrl) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<IDSpec>,
    {
        set_named_indicator(self, device_spec, led_class, led_id, indicator, set_state, on, set_map, create_map, map_flags, map_which_groups, map_groups, map_which_mods, map_real_mods, map_vmods, map_ctrls)
    }
    fn xkb_get_names(&self, device_spec: DeviceSpec, which: NameDetail) -> Result<Cookie<'_, Self, GetNamesReply>, ConnectionError>
    {
        get_names(self, device_spec, which)
    }
    fn xkb_set_names<'c, 'input>(&'c self, device_spec: DeviceSpec, virtual_mods: VMod, first_type: u8, n_types: u8, first_kt_levelt: u8, n_kt_levels: u8, indicators: u32, group_names: SetOfGroup, n_radio_groups: u8, first_key: xproto::Keycode, n_keys: u8, n_key_aliases: u8, total_kt_level_names: u16, values: &'input SetNamesAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_names(self, device_spec, virtual_mods, first_type, n_types, first_kt_levelt, n_kt_levels, indicators, group_names, n_radio_groups, first_key, n_keys, n_key_aliases, total_kt_level_names, values)
    }
    fn xkb_per_client_flags(&self, device_spec: DeviceSpec, change: PerClientFlag, value: PerClientFlag, ctrls_to_change: BoolCtrl, auto_ctrls: BoolCtrl, auto_ctrls_values: BoolCtrl) -> Result<Cookie<'_, Self, PerClientFlagsReply>, ConnectionError>
    {
        per_client_flags(self, device_spec, change, value, ctrls_to_change, auto_ctrls, auto_ctrls_values)
    }
    fn xkb_list_components(&self, device_spec: DeviceSpec, max_names: u16) -> Result<Cookie<'_, Self, ListComponentsReply>, ConnectionError>
    {
        list_components(self, device_spec, max_names)
    }
    fn xkb_get_kbd_by_name(&self, device_spec: DeviceSpec, need: GBNDetail, want: GBNDetail, load: bool) -> Result<Cookie<'_, Self, GetKbdByNameReply>, ConnectionError>
    {
        get_kbd_by_name(self, device_spec, need, want, load)
    }
    fn xkb_get_device_info<A>(&self, device_spec: DeviceSpec, wanted: XIFeature, all_buttons: bool, first_button: u8, n_buttons: u8, led_class: LedClass, led_id: A) -> Result<Cookie<'_, Self, GetDeviceInfoReply>, ConnectionError>
    where
        A: Into<IDSpec>,
    {
        get_device_info(self, device_spec, wanted, all_buttons, first_button, n_buttons, led_class, led_id)
    }
    fn xkb_set_device_info<'c, 'input>(&'c self, device_spec: DeviceSpec, first_btn: u8, change: XIFeature, btn_actions: &'input [Action], leds: &'input [DeviceLedInfo]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_device_info(self, device_spec, first_btn, change, btn_actions, leds)
    }
    fn xkb_set_debugging_flags<'c, 'input>(&'c self, affect_flags: u32, flags: u32, affect_ctrls: u32, ctrls: u32, message: &'input [String8]) -> Result<Cookie<'c, Self, SetDebuggingFlagsReply>, ConnectionError>
    {
        set_debugging_flags(self, affect_flags, flags, affect_ctrls, ctrls, message)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
