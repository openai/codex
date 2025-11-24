// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `RandR` X11 extension.

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
use super::render;
#[allow(unused_imports)]
use super::xproto;

pub use x11rb_protocol::protocol::randr::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_version<Conn>(conn: &Conn, major_version: u32, minor_version: u32) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryVersionRequest {
        major_version,
        minor_version,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_screen_config<Conn>(conn: &Conn, window: xproto::Window, timestamp: xproto::Timestamp, config_timestamp: xproto::Timestamp, size_id: u16, rotation: Rotation, rate: u16) -> Result<Cookie<'_, Conn, SetScreenConfigReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetScreenConfigRequest {
        window,
        timestamp,
        config_timestamp,
        size_id,
        rotation,
        rate,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn select_input<Conn>(conn: &Conn, window: xproto::Window, enable: NotifyMask) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectInputRequest {
        window,
        enable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_screen_info<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetScreenInfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetScreenInfoRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_screen_size_range<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetScreenSizeRangeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetScreenSizeRangeRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_screen_size<Conn>(conn: &Conn, window: xproto::Window, width: u16, height: u16, mm_width: u32, mm_height: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetScreenSizeRequest {
        window,
        width,
        height,
        mm_width,
        mm_height,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_screen_resources<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetScreenResourcesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetScreenResourcesRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_output_info<Conn>(conn: &Conn, output: Output, config_timestamp: xproto::Timestamp) -> Result<Cookie<'_, Conn, GetOutputInfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetOutputInfoRequest {
        output,
        config_timestamp,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_output_properties<Conn>(conn: &Conn, output: Output) -> Result<Cookie<'_, Conn, ListOutputPropertiesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListOutputPropertiesRequest {
        output,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_output_property<Conn>(conn: &Conn, output: Output, property: xproto::Atom) -> Result<Cookie<'_, Conn, QueryOutputPropertyReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryOutputPropertyRequest {
        output,
        property,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn configure_output_property<'c, 'input, Conn>(conn: &'c Conn, output: Output, property: xproto::Atom, pending: bool, range: bool, values: &'input [i32]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ConfigureOutputPropertyRequest {
        output,
        property,
        pending,
        range,
        values: Cow::Borrowed(values),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn change_output_property<'c, 'input, Conn>(conn: &'c Conn, output: Output, property: xproto::Atom, type_: xproto::Atom, format: u8, mode: xproto::PropMode, num_units: u32, data: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeOutputPropertyRequest {
        output,
        property,
        type_,
        format,
        mode,
        num_units,
        data: Cow::Borrowed(data),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn delete_output_property<Conn>(conn: &Conn, output: Output, property: xproto::Atom) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeleteOutputPropertyRequest {
        output,
        property,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_output_property<Conn, A>(conn: &Conn, output: Output, property: xproto::Atom, type_: A, long_offset: u32, long_length: u32, delete: bool, pending: bool) -> Result<Cookie<'_, Conn, GetOutputPropertyReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Atom>,
{
    let type_: xproto::Atom = type_.into();
    let request0 = GetOutputPropertyRequest {
        output,
        property,
        type_,
        long_offset,
        long_length,
        delete,
        pending,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn create_mode<'c, 'input, Conn>(conn: &'c Conn, window: xproto::Window, mode_info: ModeInfo, name: &'input [u8]) -> Result<Cookie<'c, Conn, CreateModeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateModeRequest {
        window,
        mode_info,
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn destroy_mode<Conn>(conn: &Conn, mode: Mode) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DestroyModeRequest {
        mode,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn add_output_mode<Conn>(conn: &Conn, output: Output, mode: Mode) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AddOutputModeRequest {
        output,
        mode,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn delete_output_mode<Conn>(conn: &Conn, output: Output, mode: Mode) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeleteOutputModeRequest {
        output,
        mode,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_crtc_info<Conn>(conn: &Conn, crtc: Crtc, config_timestamp: xproto::Timestamp) -> Result<Cookie<'_, Conn, GetCrtcInfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetCrtcInfoRequest {
        crtc,
        config_timestamp,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_crtc_config<'c, 'input, Conn>(conn: &'c Conn, crtc: Crtc, timestamp: xproto::Timestamp, config_timestamp: xproto::Timestamp, x: i16, y: i16, mode: Mode, rotation: Rotation, outputs: &'input [Output]) -> Result<Cookie<'c, Conn, SetCrtcConfigReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetCrtcConfigRequest {
        crtc,
        timestamp,
        config_timestamp,
        x,
        y,
        mode,
        rotation,
        outputs: Cow::Borrowed(outputs),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_crtc_gamma_size<Conn>(conn: &Conn, crtc: Crtc) -> Result<Cookie<'_, Conn, GetCrtcGammaSizeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetCrtcGammaSizeRequest {
        crtc,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_crtc_gamma<Conn>(conn: &Conn, crtc: Crtc) -> Result<Cookie<'_, Conn, GetCrtcGammaReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetCrtcGammaRequest {
        crtc,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_crtc_gamma<'c, 'input, Conn>(conn: &'c Conn, crtc: Crtc, red: &'input [u16], green: &'input [u16], blue: &'input [u16]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetCrtcGammaRequest {
        crtc,
        red: Cow::Borrowed(red),
        green: Cow::Borrowed(green),
        blue: Cow::Borrowed(blue),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3]), IoSlice::new(&bytes[4])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_screen_resources_current<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetScreenResourcesCurrentReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetScreenResourcesCurrentRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_crtc_transform<'c, 'input, Conn>(conn: &'c Conn, crtc: Crtc, transform: render::Transform, filter_name: &'input [u8], filter_params: &'input [render::Fixed]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetCrtcTransformRequest {
        crtc,
        transform,
        filter_name: Cow::Borrowed(filter_name),
        filter_params: Cow::Borrowed(filter_params),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3]), IoSlice::new(&bytes[4])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_crtc_transform<Conn>(conn: &Conn, crtc: Crtc) -> Result<Cookie<'_, Conn, GetCrtcTransformReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetCrtcTransformRequest {
        crtc,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_panning<Conn>(conn: &Conn, crtc: Crtc) -> Result<Cookie<'_, Conn, GetPanningReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetPanningRequest {
        crtc,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_panning<Conn>(conn: &Conn, crtc: Crtc, timestamp: xproto::Timestamp, left: u16, top: u16, width: u16, height: u16, track_left: u16, track_top: u16, track_width: u16, track_height: u16, border_left: i16, border_top: i16, border_right: i16, border_bottom: i16) -> Result<Cookie<'_, Conn, SetPanningReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetPanningRequest {
        crtc,
        timestamp,
        left,
        top,
        width,
        height,
        track_left,
        track_top,
        track_width,
        track_height,
        border_left,
        border_top,
        border_right,
        border_bottom,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_output_primary<Conn>(conn: &Conn, window: xproto::Window, output: Output) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetOutputPrimaryRequest {
        window,
        output,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_output_primary<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetOutputPrimaryReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetOutputPrimaryRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_providers<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetProvidersReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetProvidersRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_provider_info<Conn>(conn: &Conn, provider: Provider, config_timestamp: xproto::Timestamp) -> Result<Cookie<'_, Conn, GetProviderInfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetProviderInfoRequest {
        provider,
        config_timestamp,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_provider_offload_sink<Conn>(conn: &Conn, provider: Provider, sink_provider: Provider, config_timestamp: xproto::Timestamp) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetProviderOffloadSinkRequest {
        provider,
        sink_provider,
        config_timestamp,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_provider_output_source<Conn>(conn: &Conn, provider: Provider, source_provider: Provider, config_timestamp: xproto::Timestamp) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetProviderOutputSourceRequest {
        provider,
        source_provider,
        config_timestamp,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn list_provider_properties<Conn>(conn: &Conn, provider: Provider) -> Result<Cookie<'_, Conn, ListProviderPropertiesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListProviderPropertiesRequest {
        provider,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_provider_property<Conn>(conn: &Conn, provider: Provider, property: xproto::Atom) -> Result<Cookie<'_, Conn, QueryProviderPropertyReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryProviderPropertyRequest {
        provider,
        property,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn configure_provider_property<'c, 'input, Conn>(conn: &'c Conn, provider: Provider, property: xproto::Atom, pending: bool, range: bool, values: &'input [i32]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ConfigureProviderPropertyRequest {
        provider,
        property,
        pending,
        range,
        values: Cow::Borrowed(values),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn change_provider_property<'c, 'input, Conn>(conn: &'c Conn, provider: Provider, property: xproto::Atom, type_: xproto::Atom, format: u8, mode: u8, num_items: u32, data: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeProviderPropertyRequest {
        provider,
        property,
        type_,
        format,
        mode,
        num_items,
        data: Cow::Borrowed(data),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn delete_provider_property<Conn>(conn: &Conn, provider: Provider, property: xproto::Atom) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeleteProviderPropertyRequest {
        provider,
        property,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_provider_property<Conn>(conn: &Conn, provider: Provider, property: xproto::Atom, type_: xproto::Atom, long_offset: u32, long_length: u32, delete: bool, pending: bool) -> Result<Cookie<'_, Conn, GetProviderPropertyReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetProviderPropertyRequest {
        provider,
        property,
        type_,
        long_offset,
        long_length,
        delete,
        pending,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_monitors<Conn>(conn: &Conn, window: xproto::Window, get_active: bool) -> Result<Cookie<'_, Conn, GetMonitorsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetMonitorsRequest {
        window,
        get_active,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_monitor<Conn>(conn: &Conn, window: xproto::Window, monitorinfo: MonitorInfo) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetMonitorRequest {
        window,
        monitorinfo,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn delete_monitor<Conn>(conn: &Conn, window: xproto::Window, name: xproto::Atom) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeleteMonitorRequest {
        window,
        name,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_lease<'c, 'input, Conn>(conn: &'c Conn, window: xproto::Window, lid: Lease, crtcs: &'input [Crtc], outputs: &'input [Output]) -> Result<CookieWithFds<'c, Conn, CreateLeaseReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateLeaseRequest {
        window,
        lid,
        crtcs: Cow::Borrowed(crtcs),
        outputs: Cow::Borrowed(outputs),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply_with_fds(&slices, fds)
}

pub fn free_lease<Conn>(conn: &Conn, lid: Lease, terminate: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeLeaseRequest {
        lid,
        terminate,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn randr_query_version(&self, major_version: u32, minor_version: u32) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, major_version, minor_version)
    }
    fn randr_set_screen_config(&self, window: xproto::Window, timestamp: xproto::Timestamp, config_timestamp: xproto::Timestamp, size_id: u16, rotation: Rotation, rate: u16) -> Result<Cookie<'_, Self, SetScreenConfigReply>, ConnectionError>
    {
        set_screen_config(self, window, timestamp, config_timestamp, size_id, rotation, rate)
    }
    fn randr_select_input(&self, window: xproto::Window, enable: NotifyMask) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        select_input(self, window, enable)
    }
    fn randr_get_screen_info(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetScreenInfoReply>, ConnectionError>
    {
        get_screen_info(self, window)
    }
    fn randr_get_screen_size_range(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetScreenSizeRangeReply>, ConnectionError>
    {
        get_screen_size_range(self, window)
    }
    fn randr_set_screen_size(&self, window: xproto::Window, width: u16, height: u16, mm_width: u32, mm_height: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_screen_size(self, window, width, height, mm_width, mm_height)
    }
    fn randr_get_screen_resources(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetScreenResourcesReply>, ConnectionError>
    {
        get_screen_resources(self, window)
    }
    fn randr_get_output_info(&self, output: Output, config_timestamp: xproto::Timestamp) -> Result<Cookie<'_, Self, GetOutputInfoReply>, ConnectionError>
    {
        get_output_info(self, output, config_timestamp)
    }
    fn randr_list_output_properties(&self, output: Output) -> Result<Cookie<'_, Self, ListOutputPropertiesReply>, ConnectionError>
    {
        list_output_properties(self, output)
    }
    fn randr_query_output_property(&self, output: Output, property: xproto::Atom) -> Result<Cookie<'_, Self, QueryOutputPropertyReply>, ConnectionError>
    {
        query_output_property(self, output, property)
    }
    fn randr_configure_output_property<'c, 'input>(&'c self, output: Output, property: xproto::Atom, pending: bool, range: bool, values: &'input [i32]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        configure_output_property(self, output, property, pending, range, values)
    }
    fn randr_change_output_property<'c, 'input>(&'c self, output: Output, property: xproto::Atom, type_: xproto::Atom, format: u8, mode: xproto::PropMode, num_units: u32, data: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_output_property(self, output, property, type_, format, mode, num_units, data)
    }
    fn randr_delete_output_property(&self, output: Output, property: xproto::Atom) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        delete_output_property(self, output, property)
    }
    fn randr_get_output_property<A>(&self, output: Output, property: xproto::Atom, type_: A, long_offset: u32, long_length: u32, delete: bool, pending: bool) -> Result<Cookie<'_, Self, GetOutputPropertyReply>, ConnectionError>
    where
        A: Into<xproto::Atom>,
    {
        get_output_property(self, output, property, type_, long_offset, long_length, delete, pending)
    }
    fn randr_create_mode<'c, 'input>(&'c self, window: xproto::Window, mode_info: ModeInfo, name: &'input [u8]) -> Result<Cookie<'c, Self, CreateModeReply>, ConnectionError>
    {
        create_mode(self, window, mode_info, name)
    }
    fn randr_destroy_mode(&self, mode: Mode) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        destroy_mode(self, mode)
    }
    fn randr_add_output_mode(&self, output: Output, mode: Mode) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        add_output_mode(self, output, mode)
    }
    fn randr_delete_output_mode(&self, output: Output, mode: Mode) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        delete_output_mode(self, output, mode)
    }
    fn randr_get_crtc_info(&self, crtc: Crtc, config_timestamp: xproto::Timestamp) -> Result<Cookie<'_, Self, GetCrtcInfoReply>, ConnectionError>
    {
        get_crtc_info(self, crtc, config_timestamp)
    }
    fn randr_set_crtc_config<'c, 'input>(&'c self, crtc: Crtc, timestamp: xproto::Timestamp, config_timestamp: xproto::Timestamp, x: i16, y: i16, mode: Mode, rotation: Rotation, outputs: &'input [Output]) -> Result<Cookie<'c, Self, SetCrtcConfigReply>, ConnectionError>
    {
        set_crtc_config(self, crtc, timestamp, config_timestamp, x, y, mode, rotation, outputs)
    }
    fn randr_get_crtc_gamma_size(&self, crtc: Crtc) -> Result<Cookie<'_, Self, GetCrtcGammaSizeReply>, ConnectionError>
    {
        get_crtc_gamma_size(self, crtc)
    }
    fn randr_get_crtc_gamma(&self, crtc: Crtc) -> Result<Cookie<'_, Self, GetCrtcGammaReply>, ConnectionError>
    {
        get_crtc_gamma(self, crtc)
    }
    fn randr_set_crtc_gamma<'c, 'input>(&'c self, crtc: Crtc, red: &'input [u16], green: &'input [u16], blue: &'input [u16]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_crtc_gamma(self, crtc, red, green, blue)
    }
    fn randr_get_screen_resources_current(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetScreenResourcesCurrentReply>, ConnectionError>
    {
        get_screen_resources_current(self, window)
    }
    fn randr_set_crtc_transform<'c, 'input>(&'c self, crtc: Crtc, transform: render::Transform, filter_name: &'input [u8], filter_params: &'input [render::Fixed]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_crtc_transform(self, crtc, transform, filter_name, filter_params)
    }
    fn randr_get_crtc_transform(&self, crtc: Crtc) -> Result<Cookie<'_, Self, GetCrtcTransformReply>, ConnectionError>
    {
        get_crtc_transform(self, crtc)
    }
    fn randr_get_panning(&self, crtc: Crtc) -> Result<Cookie<'_, Self, GetPanningReply>, ConnectionError>
    {
        get_panning(self, crtc)
    }
    fn randr_set_panning(&self, crtc: Crtc, timestamp: xproto::Timestamp, left: u16, top: u16, width: u16, height: u16, track_left: u16, track_top: u16, track_width: u16, track_height: u16, border_left: i16, border_top: i16, border_right: i16, border_bottom: i16) -> Result<Cookie<'_, Self, SetPanningReply>, ConnectionError>
    {
        set_panning(self, crtc, timestamp, left, top, width, height, track_left, track_top, track_width, track_height, border_left, border_top, border_right, border_bottom)
    }
    fn randr_set_output_primary(&self, window: xproto::Window, output: Output) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_output_primary(self, window, output)
    }
    fn randr_get_output_primary(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetOutputPrimaryReply>, ConnectionError>
    {
        get_output_primary(self, window)
    }
    fn randr_get_providers(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetProvidersReply>, ConnectionError>
    {
        get_providers(self, window)
    }
    fn randr_get_provider_info(&self, provider: Provider, config_timestamp: xproto::Timestamp) -> Result<Cookie<'_, Self, GetProviderInfoReply>, ConnectionError>
    {
        get_provider_info(self, provider, config_timestamp)
    }
    fn randr_set_provider_offload_sink(&self, provider: Provider, sink_provider: Provider, config_timestamp: xproto::Timestamp) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_provider_offload_sink(self, provider, sink_provider, config_timestamp)
    }
    fn randr_set_provider_output_source(&self, provider: Provider, source_provider: Provider, config_timestamp: xproto::Timestamp) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_provider_output_source(self, provider, source_provider, config_timestamp)
    }
    fn randr_list_provider_properties(&self, provider: Provider) -> Result<Cookie<'_, Self, ListProviderPropertiesReply>, ConnectionError>
    {
        list_provider_properties(self, provider)
    }
    fn randr_query_provider_property(&self, provider: Provider, property: xproto::Atom) -> Result<Cookie<'_, Self, QueryProviderPropertyReply>, ConnectionError>
    {
        query_provider_property(self, provider, property)
    }
    fn randr_configure_provider_property<'c, 'input>(&'c self, provider: Provider, property: xproto::Atom, pending: bool, range: bool, values: &'input [i32]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        configure_provider_property(self, provider, property, pending, range, values)
    }
    fn randr_change_provider_property<'c, 'input>(&'c self, provider: Provider, property: xproto::Atom, type_: xproto::Atom, format: u8, mode: u8, num_items: u32, data: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_provider_property(self, provider, property, type_, format, mode, num_items, data)
    }
    fn randr_delete_provider_property(&self, provider: Provider, property: xproto::Atom) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        delete_provider_property(self, provider, property)
    }
    fn randr_get_provider_property(&self, provider: Provider, property: xproto::Atom, type_: xproto::Atom, long_offset: u32, long_length: u32, delete: bool, pending: bool) -> Result<Cookie<'_, Self, GetProviderPropertyReply>, ConnectionError>
    {
        get_provider_property(self, provider, property, type_, long_offset, long_length, delete, pending)
    }
    fn randr_get_monitors(&self, window: xproto::Window, get_active: bool) -> Result<Cookie<'_, Self, GetMonitorsReply>, ConnectionError>
    {
        get_monitors(self, window, get_active)
    }
    fn randr_set_monitor(&self, window: xproto::Window, monitorinfo: MonitorInfo) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_monitor(self, window, monitorinfo)
    }
    fn randr_delete_monitor(&self, window: xproto::Window, name: xproto::Atom) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        delete_monitor(self, window, name)
    }
    fn randr_create_lease<'c, 'input>(&'c self, window: xproto::Window, lid: Lease, crtcs: &'input [Crtc], outputs: &'input [Output]) -> Result<CookieWithFds<'c, Self, CreateLeaseReply>, ConnectionError>
    {
        create_lease(self, window, lid, crtcs, outputs)
    }
    fn randr_free_lease(&self, lid: Lease, terminate: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_lease(self, lid, terminate)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
