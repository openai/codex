//! XWayland related protocols

#![cfg_attr(rustfmt, rustfmt_skip)]

#[cfg(feature = "staging")]
pub mod shell {
    //! This protocol adds a xwayland_surface role which allows an Xwayland
    //! server to associate an X11 window to a wl_surface.
    //!
    //! Before this protocol, this would be done via the Xwayland server
    //! providing the wl_surface's resource id via the a client message with
    //! the WL_SURFACE_ID atom on the X window.
    //! This was problematic as a race could occur if the wl_surface
    //! associated with a WL_SURFACE_ID for a window was destroyed before the
    //! client message was processed by the compositor and another surface
    //! (or other object) had taken its id due to recycling.
    //!
    //! This protocol solves the problem by moving the X11 window to wl_surface
    //! association step to the Wayland side, which means that the association
    //! cannot happen out-of-sync with the resource lifetime of the wl_surface.
    //!
    //! This protocol avoids duplicating the race on the other side by adding a
    //! non-zero monotonic serial number which is entirely unique that is set on
    //! both the wl_surface (via. xwayland_surface_v1's set_serial method) and
    //! the X11 window (via. the `WL_SURFACE_SERIAL` client message) that can be
    //! used to associate them, and synchronize the two timelines.
    //!
    //! The key words "must", "must not", "required", "shall", "shall not",
    //! "should", "should not", "recommended",  "may", and "optional" in this
    //! document are to be interpreted as described in IETF RFC 2119.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./protocols/staging/xwayland-shell/xwayland-shell-v1.xml",
            []
        );
    }
}

#[cfg(feature = "unstable")]
pub mod keyboard_grab {
    //! Protocol for grabbing the keyboard from Xwayland
    //!
    //! This protocol is application-specific to meet the needs of the X11
    //! protocol through Xwayland. It provides a way for Xwayland to request
    //! all keyboard events to be forwarded to a surface even when the
    //! surface does not have keyboard focus.
    //!
    //! In the X11 protocol, a client may request an "active grab" on the
    //! keyboard. On success, all key events are reported only to the
    //! grabbing X11 client. For details, see XGrabKeyboard(3).
    //!
    //! The core Wayland protocol does not have a notion of an active
    //! keyboard grab. When running in Xwayland, X11 applications may
    //! acquire an active grab inside Xwayland but that cannot be translated
    //! to the Wayland compositor who may set the input focus to some other
    //! surface. In doing so, it breaks the X11 client assumption that all
    //! key events are reported to the grabbing client.
    //!
    //! This protocol specifies a way for Xwayland to request all keyboard
    //! be directed to the given surface. The protocol does not guarantee
    //! that the compositor will honor this request and it does not
    //! prescribe user interfaces on how to handle the respond. For example,
    //! a compositor may inform the user that all key events are now
    //! forwarded to the given client surface, or it may ask the user for
    //! permission to do so.
    //!
    //! Compositors are required to restrict access to this application
    //! specific protocol to Xwayland alone.

    /// Unstable version 1
    pub mod zv1 {
        wayland_protocol!(
            "./protocols/unstable/xwayland-keyboard-grab/xwayland-keyboard-grab-unstable-v1.xml",
            []
        );
    }
}
