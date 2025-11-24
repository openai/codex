//! Bindings to the EGL library `libwayland-egl.so`
//!
//! This lib allows to create EGL surfaces out of wayland surfaces.
//!
//! The created handle is named `wayland_egl_handle()`.

use crate::client::wl_proxy;
#[cfg(feature = "dlopen")]
use once_cell::sync::Lazy;
use std::os::raw::c_int;

pub enum wl_egl_window {}

external_library!(WaylandEgl, "wayland-egl",
    functions:
        fn wl_egl_window_create(*mut wl_proxy, c_int, c_int) -> *mut wl_egl_window,
        fn wl_egl_window_destroy(*mut wl_egl_window) -> (),
        fn wl_egl_window_resize(*mut wl_egl_window, c_int, c_int, c_int, c_int) -> (),
        fn wl_egl_window_get_attached_size(*mut wl_egl_window, *mut c_int, *mut c_int) -> (),
);

#[cfg(feature = "dlopen")]
pub fn wayland_egl_option() -> Option<&'static WaylandEgl> {
    static WAYLAND_EGL_OPTION: Lazy<Option<WaylandEgl>> = Lazy::new(|| {
        let versions = ["libwayland-egl.so.1", "libwayland-egl.so"];

        for ver in &versions {
            match unsafe { WaylandEgl::open(ver) } {
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

    WAYLAND_EGL_OPTION.as_ref()
}

#[cfg(feature = "dlopen")]
pub fn wayland_egl_handle() -> &'static WaylandEgl {
    static WAYLAND_EGL_HANDLE: Lazy<&'static WaylandEgl> =
        Lazy::new(|| wayland_egl_option().expect("Library libwayland-egl.so could not be loaded."));

    &WAYLAND_EGL_HANDLE
}

#[cfg(not(feature = "dlopen"))]
pub fn is_lib_available() -> bool {
    true
}
#[cfg(feature = "dlopen")]
pub fn is_lib_available() -> bool {
    wayland_egl_option().is_some()
}
