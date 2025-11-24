// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Sync` X11 extension.

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

pub use x11rb_protocol::protocol::sync::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn initialize<Conn>(conn: &Conn, desired_major_version: u8, desired_minor_version: u8) -> Result<Cookie<'_, Conn, InitializeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = InitializeRequest {
        desired_major_version,
        desired_minor_version,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_system_counters<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, ListSystemCountersReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListSystemCountersRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn create_counter<Conn>(conn: &Conn, id: Counter, initial_value: Int64) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateCounterRequest {
        id,
        initial_value,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn destroy_counter<Conn>(conn: &Conn, counter: Counter) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DestroyCounterRequest {
        counter,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_counter<Conn>(conn: &Conn, counter: Counter) -> Result<Cookie<'_, Conn, QueryCounterReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryCounterRequest {
        counter,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn await_<'c, 'input, Conn>(conn: &'c Conn, wait_list: &'input [Waitcondition]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AwaitRequest {
        wait_list: Cow::Borrowed(wait_list),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn change_counter<Conn>(conn: &Conn, counter: Counter, amount: Int64) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeCounterRequest {
        counter,
        amount,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_counter<Conn>(conn: &Conn, counter: Counter, value: Int64) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetCounterRequest {
        counter,
        value,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_alarm<'c, 'input, Conn>(conn: &'c Conn, id: Alarm, value_list: &'input CreateAlarmAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateAlarmRequest {
        id,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn change_alarm<'c, 'input, Conn>(conn: &'c Conn, id: Alarm, value_list: &'input ChangeAlarmAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeAlarmRequest {
        id,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn destroy_alarm<Conn>(conn: &Conn, alarm: Alarm) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DestroyAlarmRequest {
        alarm,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_alarm<Conn>(conn: &Conn, alarm: Alarm) -> Result<Cookie<'_, Conn, QueryAlarmReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryAlarmRequest {
        alarm,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_priority<Conn>(conn: &Conn, id: u32, priority: i32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetPriorityRequest {
        id,
        priority,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_priority<Conn>(conn: &Conn, id: u32) -> Result<Cookie<'_, Conn, GetPriorityReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetPriorityRequest {
        id,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn create_fence<Conn>(conn: &Conn, drawable: xproto::Drawable, fence: Fence, initially_triggered: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateFenceRequest {
        drawable,
        fence,
        initially_triggered,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn trigger_fence<Conn>(conn: &Conn, fence: Fence) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = TriggerFenceRequest {
        fence,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn reset_fence<Conn>(conn: &Conn, fence: Fence) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ResetFenceRequest {
        fence,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn destroy_fence<Conn>(conn: &Conn, fence: Fence) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DestroyFenceRequest {
        fence,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_fence<Conn>(conn: &Conn, fence: Fence) -> Result<Cookie<'_, Conn, QueryFenceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryFenceRequest {
        fence,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn await_fence<'c, 'input, Conn>(conn: &'c Conn, fence_list: &'input [Fence]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AwaitFenceRequest {
        fence_list: Cow::Borrowed(fence_list),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn sync_initialize(&self, desired_major_version: u8, desired_minor_version: u8) -> Result<Cookie<'_, Self, InitializeReply>, ConnectionError>
    {
        initialize(self, desired_major_version, desired_minor_version)
    }
    fn sync_list_system_counters(&self) -> Result<Cookie<'_, Self, ListSystemCountersReply>, ConnectionError>
    {
        list_system_counters(self)
    }
    fn sync_create_counter(&self, id: Counter, initial_value: Int64) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_counter(self, id, initial_value)
    }
    fn sync_destroy_counter(&self, counter: Counter) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        destroy_counter(self, counter)
    }
    fn sync_query_counter(&self, counter: Counter) -> Result<Cookie<'_, Self, QueryCounterReply>, ConnectionError>
    {
        query_counter(self, counter)
    }
    fn sync_await_<'c, 'input>(&'c self, wait_list: &'input [Waitcondition]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        await_(self, wait_list)
    }
    fn sync_change_counter(&self, counter: Counter, amount: Int64) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        change_counter(self, counter, amount)
    }
    fn sync_set_counter(&self, counter: Counter, value: Int64) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_counter(self, counter, value)
    }
    fn sync_create_alarm<'c, 'input>(&'c self, id: Alarm, value_list: &'input CreateAlarmAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_alarm(self, id, value_list)
    }
    fn sync_change_alarm<'c, 'input>(&'c self, id: Alarm, value_list: &'input ChangeAlarmAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_alarm(self, id, value_list)
    }
    fn sync_destroy_alarm(&self, alarm: Alarm) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        destroy_alarm(self, alarm)
    }
    fn sync_query_alarm(&self, alarm: Alarm) -> Result<Cookie<'_, Self, QueryAlarmReply>, ConnectionError>
    {
        query_alarm(self, alarm)
    }
    fn sync_set_priority(&self, id: u32, priority: i32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_priority(self, id, priority)
    }
    fn sync_get_priority(&self, id: u32) -> Result<Cookie<'_, Self, GetPriorityReply>, ConnectionError>
    {
        get_priority(self, id)
    }
    fn sync_create_fence(&self, drawable: xproto::Drawable, fence: Fence, initially_triggered: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_fence(self, drawable, fence, initially_triggered)
    }
    fn sync_trigger_fence(&self, fence: Fence) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        trigger_fence(self, fence)
    }
    fn sync_reset_fence(&self, fence: Fence) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        reset_fence(self, fence)
    }
    fn sync_destroy_fence(&self, fence: Fence) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        destroy_fence(self, fence)
    }
    fn sync_query_fence(&self, fence: Fence) -> Result<Cookie<'_, Self, QueryFenceReply>, ConnectionError>
    {
        query_fence(self, fence)
    }
    fn sync_await_fence<'c, 'input>(&'c self, fence_list: &'input [Fence]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        await_fence(self, fence_list)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}

/// A RAII-like wrapper around a [Counter].
///
/// Instances of this struct represent a Counter that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct CounterWrapper<C: RequestConnection>(C, Counter);

impl<C: RequestConnection> CounterWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_counter(conn: C, id: Counter) -> Self {
        CounterWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn counter(&self) -> Counter {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_counter(self) -> Counter {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> CounterWrapper<&'c C>
{
    /// Create a new Counter and return a Counter wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_counter] that allocates an id for the Counter.
    /// This function returns the resulting `CounterWrapper` that owns the created Counter and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_counter].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_counter].
    pub fn create_counter_and_get_cookie(conn: &'c C, initial_value: Int64) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let id = conn.generate_id()?;
        let cookie = create_counter(conn, id, initial_value)?;
        Ok((Self::for_counter(conn, id), cookie))
    }
}
impl<C: X11Connection> CounterWrapper<C>
{
    /// Create a new Counter and return a Counter wrapper
    ///
    /// This is a thin wrapper around [create_counter] that allocates an id for the Counter.
    /// This function returns the resulting `CounterWrapper` that owns the created Counter and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_counter].
    pub fn create_counter(conn: C, initial_value: Int64) -> Result<Self, ReplyOrIdError>
    {
        let id = conn.generate_id()?;
        let _ = create_counter(&conn, id, initial_value)?;
        Ok(Self::for_counter(conn, id))
    }
}

impl<C: RequestConnection> From<&CounterWrapper<C>> for Counter {
    fn from(from: &CounterWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for CounterWrapper<C> {
    fn drop(&mut self) {
        let _ = destroy_counter(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Alarm].
///
/// Instances of this struct represent a Alarm that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct AlarmWrapper<C: RequestConnection>(C, Alarm);

impl<C: RequestConnection> AlarmWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_alarm(conn: C, id: Alarm) -> Self {
        AlarmWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn alarm(&self) -> Alarm {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_alarm(self) -> Alarm {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> AlarmWrapper<&'c C>
{
    /// Create a new Alarm and return a Alarm wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_alarm] that allocates an id for the Alarm.
    /// This function returns the resulting `AlarmWrapper` that owns the created Alarm and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_alarm].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_alarm].
    pub fn create_alarm_and_get_cookie(conn: &'c C, value_list: &CreateAlarmAux) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let id = conn.generate_id()?;
        let cookie = create_alarm(conn, id, value_list)?;
        Ok((Self::for_alarm(conn, id), cookie))
    }
}
impl<C: X11Connection> AlarmWrapper<C>
{
    /// Create a new Alarm and return a Alarm wrapper
    ///
    /// This is a thin wrapper around [create_alarm] that allocates an id for the Alarm.
    /// This function returns the resulting `AlarmWrapper` that owns the created Alarm and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_alarm].
    pub fn create_alarm(conn: C, value_list: &CreateAlarmAux) -> Result<Self, ReplyOrIdError>
    {
        let id = conn.generate_id()?;
        let _ = create_alarm(&conn, id, value_list)?;
        Ok(Self::for_alarm(conn, id))
    }
}

impl<C: RequestConnection> From<&AlarmWrapper<C>> for Alarm {
    fn from(from: &AlarmWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for AlarmWrapper<C> {
    fn drop(&mut self) {
        let _ = destroy_alarm(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Fence].
///
/// Instances of this struct represent a Fence that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct FenceWrapper<C: RequestConnection>(C, Fence);

impl<C: RequestConnection> FenceWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_fence(conn: C, id: Fence) -> Self {
        FenceWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn fence(&self) -> Fence {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_fence(self) -> Fence {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> FenceWrapper<&'c C>
{
    /// Create a new Fence and return a Fence wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_fence] that allocates an id for the Fence.
    /// This function returns the resulting `FenceWrapper` that owns the created Fence and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_fence].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_fence].
    pub fn create_fence_and_get_cookie(conn: &'c C, drawable: xproto::Drawable, initially_triggered: bool) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let fence = conn.generate_id()?;
        let cookie = create_fence(conn, drawable, fence, initially_triggered)?;
        Ok((Self::for_fence(conn, fence), cookie))
    }
}
impl<C: X11Connection> FenceWrapper<C>
{
    /// Create a new Fence and return a Fence wrapper
    ///
    /// This is a thin wrapper around [create_fence] that allocates an id for the Fence.
    /// This function returns the resulting `FenceWrapper` that owns the created Fence and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_fence].
    pub fn create_fence(conn: C, drawable: xproto::Drawable, initially_triggered: bool) -> Result<Self, ReplyOrIdError>
    {
        let fence = conn.generate_id()?;
        let _ = create_fence(&conn, drawable, fence, initially_triggered)?;
        Ok(Self::for_fence(conn, fence))
    }
}

impl<'c, C: X11Connection> FenceWrapper<&'c C>
{
    /// Create a new Fence and return a Fence wrapper and a cookie.
    ///
    /// This is a thin wrapper around [super::dri3::fence_from_fd] that allocates an id for the Fence.
    /// This function returns the resulting `FenceWrapper` that owns the created Fence and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [super::dri3::fence_from_fd].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [super::dri3::fence_from_fd].
    #[cfg(feature = "dri3")]
    pub fn dri3_fence_from_fd_and_get_cookie<A>(conn: &'c C, drawable: xproto::Drawable, initially_triggered: bool, fence_fd: A) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    where
        A: Into<RawFdContainer>,
    {
        let fence = conn.generate_id()?;
        let cookie = super::dri3::fence_from_fd(conn, drawable, fence, initially_triggered, fence_fd)?;
        Ok((Self::for_fence(conn, fence), cookie))
    }
}
impl<C: X11Connection> FenceWrapper<C>
{
    /// Create a new Fence and return a Fence wrapper
    ///
    /// This is a thin wrapper around [super::dri3::fence_from_fd] that allocates an id for the Fence.
    /// This function returns the resulting `FenceWrapper` that owns the created Fence and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [super::dri3::fence_from_fd].
    #[cfg(feature = "dri3")]
    pub fn dri3_fence_from_fd<A>(conn: C, drawable: xproto::Drawable, initially_triggered: bool, fence_fd: A) -> Result<Self, ReplyOrIdError>
    where
        A: Into<RawFdContainer>,
    {
        let fence = conn.generate_id()?;
        let _ = super::dri3::fence_from_fd(&conn, drawable, fence, initially_triggered, fence_fd)?;
        Ok(Self::for_fence(conn, fence))
    }
}

#[cfg(feature = "dri3")]
#[allow(unused_imports)]
use super::dri3;
impl<C: RequestConnection> From<&FenceWrapper<C>> for Fence {
    fn from(from: &FenceWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for FenceWrapper<C> {
    fn drop(&mut self) {
        let _ = destroy_fence(&self.0, self.1);
    }
}
