//! Bindings to the `wayland-cursor.so` library
//!
//! The created handle is named `wayland_cursor_handle()`.

use crate::client::wl_proxy;
#[cfg(feature = "dlopen")]
use once_cell::sync::Lazy;
use std::os::raw::{c_char, c_int, c_uint};

pub enum wl_cursor_theme {}

#[repr(C)]
pub struct wl_cursor_image {
    /// actual width
    pub width: u32,
    /// actual height
    pub height: u32,
    /// hot spot x (must be inside image)
    pub hotspot_x: u32,
    /// hot spot y (must be inside image)
    pub hotspot_y: u32,
    /// animation delay to next frame
    pub delay: u32,
}

#[repr(C)]
pub struct wl_cursor {
    pub image_count: c_uint,
    pub images: *mut *mut wl_cursor_image,
    pub name: *mut c_char,
}

external_library!(WaylandCursor, "wayland-cursor",
    functions:
        fn wl_cursor_theme_load(*const c_char, c_int, *mut wl_proxy) -> *mut wl_cursor_theme,
        fn wl_cursor_theme_destroy(*mut wl_cursor_theme) -> (),
        fn wl_cursor_theme_get_cursor(*mut wl_cursor_theme, *const c_char) -> *mut wl_cursor,
        fn wl_cursor_image_get_buffer(*mut wl_cursor_image) -> *mut wl_proxy,
        fn wl_cursor_frame(*mut wl_cursor, u32) -> c_int,
        fn wl_cursor_frame_and_duration(*mut wl_cursor, u32, *mut u32) -> c_int,
);

#[cfg(feature = "dlopen")]
pub fn wayland_cursor_option() -> Option<&'static WaylandCursor> {
    static WAYLAND_CURSOR_OPTION: Lazy<Option<WaylandCursor>> = Lazy::new(|| {
        let versions = ["libwayland-cursor.so.0", "libwayland-cursor.so"];

        for ver in &versions {
            match unsafe { WaylandCursor::open(ver) } {
                Ok(h) => return Some(h),
                Err(::dlib::DlError::CantOpen(_)) => continue,
                Err(::dlib::DlError::MissingSymbol(s)) => {
                    log::error!("Found library {ver} cannot be used: symbol {s} is missing.");
                    return None;
                }
            }
        }
        None
    });

    WAYLAND_CURSOR_OPTION.as_ref()
}

#[cfg(feature = "dlopen")]
pub fn wayland_cursor_handle() -> &'static WaylandCursor {
    static WAYLAND_CURSOR_HANDLE: Lazy<&'static WaylandCursor> = Lazy::new(|| {
        wayland_cursor_option().expect("Library libwayland-cursor.so could not be loaded.")
    });

    &WAYLAND_CURSOR_HANDLE
}

#[cfg(not(feature = "dlopen"))]
pub fn is_lib_available() -> bool {
    true
}
#[cfg(feature = "dlopen")]
pub fn is_lib_available() -> bool {
    wayland_cursor_option().is_some()
}
