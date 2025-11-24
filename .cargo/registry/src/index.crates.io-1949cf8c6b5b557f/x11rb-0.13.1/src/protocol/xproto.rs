// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the core X11 protocol.
//!
//! For more documentation on the X11 protocol, see the
//! [protocol reference manual](https://www.x.org/releases/X11R7.6/doc/xproto/x11protocol.html).
//! This is especially recommended for looking up the exact semantics of
//! specific errors, events, or requests.

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
use crate::cookie::ListFontsWithInfoCookie;
use crate::errors::ConnectionError;
#[allow(unused_imports)]
use crate::errors::ReplyOrIdError;

pub use x11rb_protocol::protocol::xproto::*;

/// Creates a window.
///
/// Creates an unmapped window as child of the specified `parent` window. A
/// CreateNotify event will be generated. The new window is placed on top in the
/// stacking order with respect to siblings.
///
/// The coordinate system has the X axis horizontal and the Y axis vertical with
/// the origin [0, 0] at the upper-left corner. Coordinates are integral, in terms
/// of pixels, and coincide with pixel centers. Each window and pixmap has its own
/// coordinate system. For a window, the origin is inside the border at the inside,
/// upper-left corner.
///
/// The created window is not yet displayed (mapped), call `xcb_map_window` to
/// display it.
///
/// The created window will initially use the same cursor as its parent.
///
/// # Fields
///
/// * `wid` - The ID with which you will refer to the new window, created by
/// `xcb_generate_id`.
/// * `depth` - Specifies the new window's depth (TODO: what unit?).
///
/// The special value `XCB_COPY_FROM_PARENT` means the depth is taken from the
/// `parent` window.
/// * `visual` - Specifies the id for the new window's visual.
///
/// The special value `XCB_COPY_FROM_PARENT` means the visual is taken from the
/// `parent` window.
/// * `class` -
/// * `parent` - The parent window of the new window.
/// * `border_width` - TODO:
///
/// Must be zero if the `class` is `InputOnly` or a `xcb_match_error_t` occurs.
/// * `x` - The X coordinate of the new window.
/// * `y` - The Y coordinate of the new window.
/// * `width` - The width of the new window.
/// * `height` - The height of the new window.
///
/// # Errors
///
/// * `Colormap` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Cursor` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Window` - TODO: reasons?
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # See
///
/// * `xcb_generate_id`: function
/// * `MapWindow`: request
/// * `CreateNotify`: event
pub fn create_window<'c, 'input, Conn>(conn: &'c Conn, depth: u8, wid: Window, parent: Window, x: i16, y: i16, width: u16, height: u16, border_width: u16, class: WindowClass, visual: Visualid, value_list: &'input CreateWindowAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateWindowRequest {
        depth,
        wid,
        parent,
        x,
        y,
        width,
        height,
        border_width,
        class,
        visual,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// change window attributes.
///
/// Changes the attributes specified by `value_mask` for the specified `window`.
///
/// # Fields
///
/// * `window` - The window to change.
/// * `value_mask` -
/// * `value_list` - Values for each of the attributes specified in the bitmask `value_mask`. The
/// order has to correspond to the order of possible `value_mask` bits. See the
/// example.
///
/// # Errors
///
/// * `Access` - TODO: reasons?
/// * `Colormap` - TODO: reasons?
/// * `Cursor` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
pub fn change_window_attributes<'c, 'input, Conn>(conn: &'c Conn, window: Window, value_list: &'input ChangeWindowAttributesAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeWindowAttributesRequest {
        window,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Gets window attributes.
///
/// Gets the current attributes for the specified `window`.
///
/// # Fields
///
/// * `window` - The window to get the attributes from.
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
/// * `Drawable` - TODO: reasons?
pub fn get_window_attributes<Conn>(conn: &Conn, window: Window) -> Result<Cookie<'_, Conn, GetWindowAttributesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetWindowAttributesRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Destroys a window.
///
/// Destroys the specified window and all of its subwindows. A DestroyNotify event
/// is generated for each destroyed window (a DestroyNotify event is first generated
/// for any given window's inferiors). If the window was mapped, it will be
/// automatically unmapped before destroying.
///
/// Calling DestroyWindow on the root window will do nothing.
///
/// # Fields
///
/// * `window` - The window to destroy.
///
/// # Errors
///
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `DestroyNotify`: event
/// * `MapWindow`: request
/// * `UnmapWindow`: request
pub fn destroy_window<Conn>(conn: &Conn, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DestroyWindowRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn destroy_subwindows<Conn>(conn: &Conn, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DestroySubwindowsRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Changes a client's save set.
///
/// TODO: explain what the save set is for.
///
/// This function either adds or removes the specified window to the client's (your
/// application's) save set.
///
/// # Fields
///
/// * `mode` - Insert to add the specified window to the save set or Delete to delete it from the save set.
/// * `window` - The window to add or delete to/from your save set.
///
/// # Errors
///
/// * `Match` - You created the specified window. This does not make sense, you can only add
/// windows created by other clients to your save set.
/// * `Value` - You specified an invalid mode.
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `ReparentWindow`: request
pub fn change_save_set<Conn>(conn: &Conn, mode: SetMode, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeSaveSetRequest {
        mode,
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Reparents a window.
///
/// Makes the specified window a child of the specified parent window. If the
/// window is mapped, it will automatically be unmapped before reparenting and
/// re-mapped after reparenting. The window is placed in the stacking order on top
/// with respect to sibling windows.
///
/// After reparenting, a ReparentNotify event is generated.
///
/// # Fields
///
/// * `window` - The window to reparent.
/// * `parent` - The new parent of the window.
/// * `x` - The X position of the window within its new parent.
/// * `y` - The Y position of the window within its new parent.
///
/// # Errors
///
/// * `Match` - The new parent window is not on the same screen as the old parent window.
/// 
/// The new parent window is the specified window or an inferior of the specified window.
/// 
/// The new parent is InputOnly and the window is not.
/// 
/// The specified window has a ParentRelative background and the new parent window is not the same depth as the specified window.
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `ReparentNotify`: event
/// * `MapWindow`: request
/// * `UnmapWindow`: request
pub fn reparent_window<Conn>(conn: &Conn, window: Window, parent: Window, x: i16, y: i16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ReparentWindowRequest {
        window,
        parent,
        x,
        y,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Makes a window visible.
///
/// Maps the specified window. This means making the window visible (as long as its
/// parent is visible).
///
/// This MapWindow request will be translated to a MapRequest request if a window
/// manager is running. The window manager then decides to either map the window or
/// not. Set the override-redirect window attribute to true if you want to bypass
/// this mechanism.
///
/// If the window manager decides to map the window (or if no window manager is
/// running), a MapNotify event is generated.
///
/// If the window becomes viewable and no earlier contents for it are remembered,
/// the X server tiles the window with its background. If the window's background
/// is undefined, the existing screen contents are not altered, and the X server
/// generates zero or more Expose events.
///
/// If the window type is InputOutput, an Expose event will be generated when the
/// window becomes visible. The normal response to an Expose event should be to
/// repaint the window.
///
/// # Fields
///
/// * `window` - The window to make visible.
///
/// # Errors
///
/// * `Match` - The specified window does not exist.
///
/// # See
///
/// * `MapNotify`: event
/// * `Expose`: event
/// * `UnmapWindow`: request
pub fn map_window<Conn>(conn: &Conn, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = MapWindowRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn map_subwindows<Conn>(conn: &Conn, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = MapSubwindowsRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Makes a window invisible.
///
/// Unmaps the specified window. This means making the window invisible (and all
/// its child windows).
///
/// Unmapping a window leads to the `UnmapNotify` event being generated. Also,
/// `Expose` events are generated for formerly obscured windows.
///
/// # Fields
///
/// * `window` - The window to make invisible.
///
/// # Errors
///
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `UnmapNotify`: event
/// * `Expose`: event
/// * `MapWindow`: request
pub fn unmap_window<Conn>(conn: &Conn, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UnmapWindowRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn unmap_subwindows<Conn>(conn: &Conn, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UnmapSubwindowsRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Configures window attributes.
///
/// Configures a window's size, position, border width and stacking order.
///
/// # Fields
///
/// * `window` - The window to configure.
/// * `value_mask` - Bitmask of attributes to change.
/// * `value_list` - New values, corresponding to the attributes in value_mask. The order has to
/// correspond to the order of possible `value_mask` bits. See the example.
///
/// # Errors
///
/// * `Match` - You specified a Sibling without also specifying StackMode or the window is not
/// actually a Sibling.
/// * `Window` - The specified window does not exist. TODO: any other reason?
/// * `Value` - TODO: reasons?
///
/// # See
///
/// * `MapNotify`: event
/// * `Expose`: event
///
/// # Example
///
/// ```text
/// /*
///  * Configures the given window to the left upper corner
///  * with a size of 1024x768 pixels.
///  *
///  */
/// void my_example(xcb_connection_t *c, xcb_window_t window) {
///     uint16_t mask = 0;
///
///     mask |= XCB_CONFIG_WINDOW_X;
///     mask |= XCB_CONFIG_WINDOW_Y;
///     mask |= XCB_CONFIG_WINDOW_WIDTH;
///     mask |= XCB_CONFIG_WINDOW_HEIGHT;
///
///     const uint32_t values[] = {
///         0,    /* x */
///         0,    /* y */
///         1024, /* width */
///         768   /* height */
///     };
///
///     xcb_configure_window(c, window, mask, values);
///     xcb_flush(c);
/// }
/// ```
pub fn configure_window<'c, 'input, Conn>(conn: &'c Conn, window: Window, value_list: &'input ConfigureWindowAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ConfigureWindowRequest {
        window,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Change window stacking order.
///
/// If `direction` is `XCB_CIRCULATE_RAISE_LOWEST`, the lowest mapped child (if
/// any) will be raised to the top of the stack.
///
/// If `direction` is `XCB_CIRCULATE_LOWER_HIGHEST`, the highest mapped child will
/// be lowered to the bottom of the stack.
///
/// # Fields
///
/// * `direction` -
/// * `window` - The window to raise/lower (depending on `direction`).
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
/// * `Value` - The specified `direction` is invalid.
pub fn circulate_window<Conn>(conn: &Conn, direction: Circulate, window: Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CirculateWindowRequest {
        direction,
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Get current window geometry.
///
/// Gets the current geometry of the specified drawable (either `Window` or `Pixmap`).
///
/// # Fields
///
/// * `drawable` - The drawable (`Window` or `Pixmap`) of which the geometry will be received.
///
/// # Errors
///
/// * `Drawable` - TODO: reasons?
/// * `Window` - TODO: reasons?
///
/// # See
///
/// * `xwininfo`: program
///
/// # Example
///
/// ```text
/// /*
///  * Displays the x and y position of the given window.
///  *
///  */
/// void my_example(xcb_connection_t *c, xcb_window_t window) {
///     xcb_get_geometry_cookie_t cookie;
///     xcb_get_geometry_reply_t *reply;
///
///     cookie = xcb_get_geometry(c, window);
///     /* ... do other work here if possible ... */
///     if ((reply = xcb_get_geometry_reply(c, cookie, NULL))) {
///         printf("This window is at %d, %d\\n", reply->x, reply->y);
///     }
///     free(reply);
/// }
/// ```
pub fn get_geometry<Conn>(conn: &Conn, drawable: Drawable) -> Result<Cookie<'_, Conn, GetGeometryReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetGeometryRequest {
        drawable,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// query the window tree.
///
/// Gets the root window ID, parent window ID and list of children windows for the
/// specified `window`. The children are listed in bottom-to-top stacking order.
///
/// # Fields
///
/// * `window` - The `window` to query.
///
/// # See
///
/// * `xwininfo`: program
///
/// # Example
///
/// ```text
/// /*
///  * Displays the root, parent and children of the specified window.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_window_t window) {
///     xcb_query_tree_cookie_t cookie;
///     xcb_query_tree_reply_t *reply;
///
///     cookie = xcb_query_tree(conn, window);
///     if ((reply = xcb_query_tree_reply(conn, cookie, NULL))) {
///         printf("root = 0x%08x\\n", reply->root);
///         printf("parent = 0x%08x\\n", reply->parent);
///
///         xcb_window_t *children = xcb_query_tree_children(reply);
///         for (int i = 0; i < xcb_query_tree_children_length(reply); i++)
///             printf("child window = 0x%08x\\n", children[i]);
///
///         free(reply);
///     }
/// }
/// ```
pub fn query_tree<Conn>(conn: &Conn, window: Window) -> Result<Cookie<'_, Conn, QueryTreeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryTreeRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Get atom identifier by name.
///
/// Retrieves the identifier (xcb_atom_t TODO) for the atom with the specified
/// name. Atoms are used in protocols like EWMH, for example to store window titles
/// (`_NET_WM_NAME` atom) as property of a window.
///
/// If `only_if_exists` is 0, the atom will be created if it does not already exist.
/// If `only_if_exists` is 1, `XCB_ATOM_NONE` will be returned if the atom does
/// not yet exist.
///
/// # Fields
///
/// * `name_len` - The length of the following `name`.
/// * `name` - The name of the atom.
/// * `only_if_exists` - Return a valid atom id only if the atom already exists.
///
/// # Errors
///
/// * `Alloc` - TODO: reasons?
/// * `Value` - A value other than 0 or 1 was specified for `only_if_exists`.
///
/// # See
///
/// * `xlsatoms`: program
/// * `GetAtomName`: request
///
/// # Example
///
/// ```text
/// /*
///  * Resolves the _NET_WM_NAME atom.
///  *
///  */
/// void my_example(xcb_connection_t *c) {
///     xcb_intern_atom_cookie_t cookie;
///     xcb_intern_atom_reply_t *reply;
///
///     cookie = xcb_intern_atom(c, 0, strlen("_NET_WM_NAME"), "_NET_WM_NAME");
///     /* ... do other work here if possible ... */
///     if ((reply = xcb_intern_atom_reply(c, cookie, NULL))) {
///         printf("The _NET_WM_NAME atom has ID %u\n", reply->atom);
///         free(reply);
///     }
/// }
/// ```
pub fn intern_atom<'c, 'input, Conn>(conn: &'c Conn, only_if_exists: bool, name: &'input [u8]) -> Result<Cookie<'c, Conn, InternAtomReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = InternAtomRequest {
        only_if_exists,
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_atom_name<Conn>(conn: &Conn, atom: Atom) -> Result<Cookie<'_, Conn, GetAtomNameReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetAtomNameRequest {
        atom,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Changes a window property.
///
/// Sets or updates a property on the specified `window`. Properties are for
/// example the window title (`WM_NAME`) or its minimum size (`WM_NORMAL_HINTS`).
/// Protocols such as EWMH also use properties - for example EWMH defines the
/// window title, encoded as UTF-8 string, in the `_NET_WM_NAME` property.
///
/// # Fields
///
/// * `window` - The window whose property you want to change.
/// * `mode` -
/// * `property` - The property you want to change (an atom).
/// * `type` - The type of the property you want to change (an atom).
/// * `format` - Specifies whether the data should be viewed as a list of 8-bit, 16-bit or
/// 32-bit quantities. Possible values are 8, 16 and 32. This information allows
/// the X server to correctly perform byte-swap operations as necessary.
/// * `data_len` - Specifies the number of elements (see `format`).
/// * `data` - The property data.
///
/// # Errors
///
/// * `Match` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
/// * `Atom` - `property` or `type` do not refer to a valid atom.
/// * `Alloc` - The X server could not store the property (no memory?).
///
/// # See
///
/// * `InternAtom`: request
/// * `xprop`: program
///
/// # Example
///
/// ```text
/// /*
///  * Sets the WM_NAME property of the window to "XCB Example".
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_window_t window) {
///     xcb_change_property(conn,
///         XCB_PROP_MODE_REPLACE,
///         window,
///         XCB_ATOM_WM_NAME,
///         XCB_ATOM_STRING,
///         8,
///         strlen("XCB Example"),
///         "XCB Example");
///     xcb_flush(conn);
/// }
/// ```
pub fn change_property<'c, 'input, Conn, A, B>(conn: &'c Conn, mode: PropMode, window: Window, property: A, type_: B, format: u8, data_len: u32, data: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Atom>,
    B: Into<Atom>,
{
    let property: Atom = property.into();
    let type_: Atom = type_.into();
    let request0 = ChangePropertyRequest {
        mode,
        window,
        property,
        type_,
        format,
        data_len,
        data: Cow::Borrowed(data),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn delete_property<Conn>(conn: &Conn, window: Window, property: Atom) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeletePropertyRequest {
        window,
        property,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Gets a window property.
///
/// Gets the specified `property` from the specified `window`. Properties are for
/// example the window title (`WM_NAME`) or its minimum size (`WM_NORMAL_HINTS`).
/// Protocols such as EWMH also use properties - for example EWMH defines the
/// window title, encoded as UTF-8 string, in the `_NET_WM_NAME` property.
///
/// TODO: talk about `type`
///
/// TODO: talk about `delete`
///
/// TODO: talk about the offset/length thing. what's a valid use case?
///
/// # Fields
///
/// * `window` - The window whose property you want to get.
/// * `delete` - Whether the property should actually be deleted. For deleting a property, the
/// specified `type` has to match the actual property type.
/// * `property` - The property you want to get (an atom).
/// * `type` - The type of the property you want to get (an atom).
/// * `long_offset` - Specifies the offset (in 32-bit multiples) in the specified property where the
/// data is to be retrieved.
/// * `long_length` - Specifies how many 32-bit multiples of data should be retrieved (e.g. if you
/// set `long_length` to 4, you will receive 16 bytes of data).
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
/// * `Atom` - `property` or `type` do not refer to a valid atom.
/// * `Value` - The specified `long_offset` is beyond the actual property length (e.g. the
/// property has a length of 3 bytes and you are setting `long_offset` to 1,
/// resulting in a byte offset of 4).
///
/// # See
///
/// * `InternAtom`: request
/// * `xprop`: program
///
/// # Example
///
/// ```text
/// /*
///  * Prints the WM_NAME property of the window.
///  *
///  */
/// void my_example(xcb_connection_t *c, xcb_window_t window) {
///     xcb_get_property_cookie_t cookie;
///     xcb_get_property_reply_t *reply;
///
///     /* These atoms are predefined in the X11 protocol. */
///     xcb_atom_t property = XCB_ATOM_WM_NAME;
///     xcb_atom_t type = XCB_ATOM_STRING;
///
///     // TODO: a reasonable long_length for WM_NAME?
///     cookie = xcb_get_property(c, 0, window, property, type, 0, 0);
///     if ((reply = xcb_get_property_reply(c, cookie, NULL))) {
///         int len = xcb_get_property_value_length(reply);
///         if (len == 0) {
///             printf("TODO\\n");
///             free(reply);
///             return;
///         }
///         printf("WM_NAME is %.*s\\n", len,
///                (char*)xcb_get_property_value(reply));
///     }
///     free(reply);
/// }
/// ```
pub fn get_property<Conn, A, B>(conn: &Conn, delete: bool, window: Window, property: A, type_: B, long_offset: u32, long_length: u32) -> Result<Cookie<'_, Conn, GetPropertyReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Atom>,
    B: Into<Atom>,
{
    let property: Atom = property.into();
    let type_: Atom = type_.into();
    let request0 = GetPropertyRequest {
        delete,
        window,
        property,
        type_,
        long_offset,
        long_length,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_properties<Conn>(conn: &Conn, window: Window) -> Result<Cookie<'_, Conn, ListPropertiesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListPropertiesRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Sets the owner of a selection.
///
/// Makes `window` the owner of the selection `selection` and updates the
/// last-change time of the specified selection.
///
/// TODO: briefly explain what a selection is.
///
/// # Fields
///
/// * `selection` - The selection.
/// * `owner` - The new owner of the selection.
///
/// The special value `XCB_NONE` means that the selection will have no owner.
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The selection will not be changed if `time` is earlier than the current
/// last-change time of the `selection` or is later than the current X server time.
/// Otherwise, the last-change time is set to the specified time.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
///
/// # Errors
///
/// * `Atom` - `selection` does not refer to a valid atom.
///
/// # See
///
/// * `SetSelectionOwner`: request
pub fn set_selection_owner<Conn, A, B>(conn: &Conn, owner: A, selection: Atom, time: B) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Window>,
    B: Into<Timestamp>,
{
    let owner: Window = owner.into();
    let time: Timestamp = time.into();
    let request0 = SetSelectionOwnerRequest {
        owner,
        selection,
        time,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Gets the owner of a selection.
///
/// Gets the owner of the specified selection.
///
/// TODO: briefly explain what a selection is.
///
/// # Fields
///
/// * `selection` - The selection.
///
/// # Errors
///
/// * `Atom` - `selection` does not refer to a valid atom.
///
/// # See
///
/// * `SetSelectionOwner`: request
pub fn get_selection_owner<Conn>(conn: &Conn, selection: Atom) -> Result<Cookie<'_, Conn, GetSelectionOwnerReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetSelectionOwnerRequest {
        selection,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn convert_selection<Conn, A, B>(conn: &Conn, requestor: Window, selection: Atom, target: Atom, property: A, time: B) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Atom>,
    B: Into<Timestamp>,
{
    let property: Atom = property.into();
    let time: Timestamp = time.into();
    let request0 = ConvertSelectionRequest {
        requestor,
        selection,
        target,
        property,
        time,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// send an event.
///
/// Identifies the `destination` window, determines which clients should receive
/// the specified event and ignores any active grabs.
///
/// The `event` must be one of the core events or an event defined by an extension,
/// so that the X server can correctly byte-swap the contents as necessary. The
/// contents of `event` are otherwise unaltered and unchecked except for the
/// `send_event` field which is forced to 'true'.
///
/// # Fields
///
/// * `destination` - The window to send this event to. Every client which selects any event within
/// `event_mask` on `destination` will get the event.
///
/// The special value `XCB_SEND_EVENT_DEST_POINTER_WINDOW` refers to the window
/// that contains the mouse pointer.
///
/// The special value `XCB_SEND_EVENT_DEST_ITEM_FOCUS` refers to the window which
/// has the keyboard focus.
/// * `event_mask` - Event_mask for determining which clients should receive the specified event.
/// See `destination` and `propagate`.
/// * `propagate` - If `propagate` is true and no clients have selected any event on `destination`,
/// the destination is replaced with the closest ancestor of `destination` for
/// which some client has selected a type in `event_mask` and for which no
/// intervening window has that type in its do-not-propagate-mask. If no such
/// window exists or if the window is an ancestor of the focus window and
/// `InputFocus` was originally specified as the destination, the event is not sent
/// to any clients. Otherwise, the event is reported to every client selecting on
/// the final destination any of the types specified in `event_mask`.
/// * `event` - The event to send to the specified `destination`.
///
/// # Errors
///
/// * `Window` - The specified `destination` window does not exist.
/// * `Value` - The given `event` is neither a core event nor an event defined by an extension.
///
/// # See
///
/// * `ConfigureNotify`: event
///
/// # Example
///
/// ```text
/// /*
///  * Tell the given window that it was configured to a size of 800x600 pixels.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_window_t window) {
///     /* Every X11 event is 32 bytes long. Therefore, XCB will copy 32 bytes.
///      * In order to properly initialize these bytes, we allocate 32 bytes even
///      * though we only need less for an xcb_configure_notify_event_t */
///     xcb_configure_notify_event_t *event = calloc(32, 1);
///
///     event->event = window;
///     event->window = window;
///     event->response_type = XCB_CONFIGURE_NOTIFY;
///
///     event->x = 0;
///     event->y = 0;
///     event->width = 800;
///     event->height = 600;
///
///     event->border_width = 0;
///     event->above_sibling = XCB_NONE;
///     event->override_redirect = false;
///
///     xcb_send_event(conn, false, window, XCB_EVENT_MASK_STRUCTURE_NOTIFY,
///                    (char*)event);
///     xcb_flush(conn);
///     free(event);
/// }
/// ```
pub fn send_event<Conn, A, B>(conn: &Conn, propagate: bool, destination: A, event_mask: EventMask, event: B) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Window>,
    B: Into<[u8; 32]>,
{
    let destination: Window = destination.into();
    let event = Cow::Owned(event.into());
    let request0 = SendEventRequest {
        propagate,
        destination,
        event_mask,
        event,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Grab the pointer.
///
/// Actively grabs control of the pointer. Further pointer events are reported only to the grabbing client. Overrides any active pointer grab by this client.
///
/// # Fields
///
/// * `event_mask` - Specifies which pointer events are reported to the client.
///
/// TODO: which values?
/// * `confine_to` - Specifies the window to confine the pointer in (the user will not be able to
/// move the pointer out of that window).
///
/// The special value `XCB_NONE` means don't confine the pointer.
/// * `cursor` - Specifies the cursor that should be displayed or `XCB_NONE` to not change the
/// cursor.
/// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the pointer should be grabbed.
/// * `time` - The time argument allows you to avoid certain circumstances that come up if
/// applications take a long time to respond or if there are long network delays.
/// Consider a situation where you have two applications, both of which normally
/// grab the pointer when clicked on. If both applications specify the timestamp
/// from the event, the second application may wake up faster and successfully grab
/// the pointer before the first application. The first application then will get
/// an indication that the other application grabbed the pointer before its request
/// was processed.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
/// * `pointer_mode` -
/// * `keyboard_mode` -
///
/// # Errors
///
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
///
/// # See
///
/// * `GrabKeyboard`: request
///
/// # Example
///
/// ```text
/// /*
///  * Grabs the pointer actively
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_screen_t *screen, xcb_cursor_t cursor) {
///     xcb_grab_pointer_cookie_t cookie;
///     xcb_grab_pointer_reply_t *reply;
///
///     cookie = xcb_grab_pointer(
///         conn,
///         false,               /* get all pointer events specified by the following mask */
///         screen->root,        /* grab the root window */
///         XCB_NONE,            /* which events to let through */
///         XCB_GRAB_MODE_ASYNC, /* pointer events should continue as normal */
///         XCB_GRAB_MODE_ASYNC, /* keyboard mode */
///         XCB_NONE,            /* confine_to = in which window should the cursor stay */
///         cursor,              /* we change the cursor to whatever the user wanted */
///         XCB_CURRENT_TIME
///     );
///
///     if ((reply = xcb_grab_pointer_reply(conn, cookie, NULL))) {
///         if (reply->status == XCB_GRAB_STATUS_SUCCESS)
///             printf("successfully grabbed the pointer\\n");
///         free(reply);
///     }
/// }
/// ```
pub fn grab_pointer<Conn, A, B, C>(conn: &Conn, owner_events: bool, grab_window: Window, event_mask: EventMask, pointer_mode: GrabMode, keyboard_mode: GrabMode, confine_to: A, cursor: B, time: C) -> Result<Cookie<'_, Conn, GrabPointerReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Window>,
    B: Into<Cursor>,
    C: Into<Timestamp>,
{
    let confine_to: Window = confine_to.into();
    let cursor: Cursor = cursor.into();
    let time: Timestamp = time.into();
    let request0 = GrabPointerRequest {
        owner_events,
        grab_window,
        event_mask,
        pointer_mode,
        keyboard_mode,
        confine_to,
        cursor,
        time,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// release the pointer.
///
/// Releases the pointer and any queued events if you actively grabbed the pointer
/// before using `xcb_grab_pointer`, `xcb_grab_button` or within a normal button
/// press.
///
/// EnterNotify and LeaveNotify events are generated.
///
/// # Fields
///
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The pointer will not be released if `time` is earlier than the
/// last-pointer-grab time or later than the current X server time.
/// * `name_len` - Length (in bytes) of `name`.
/// * `name` - A pattern describing an X core font.
///
/// # See
///
/// * `GrabPointer`: request
/// * `GrabButton`: request
/// * `EnterNotify`: event
/// * `LeaveNotify`: event
pub fn ungrab_pointer<Conn, A>(conn: &Conn, time: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Timestamp>,
{
    let time: Timestamp = time.into();
    let request0 = UngrabPointerRequest {
        time,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Grab pointer button(s).
///
/// This request establishes a passive grab. The pointer is actively grabbed as
/// described in GrabPointer, the last-pointer-grab time is set to the time at
/// which the button was pressed (as transmitted in the ButtonPress event), and the
/// ButtonPress event is reported if all of the following conditions are true:
///
/// The pointer is not grabbed and the specified button is logically pressed when
/// the specified modifier keys are logically down, and no other buttons or
/// modifier keys are logically down.
///
/// The grab-window contains the pointer.
///
/// The confine-to window (if any) is viewable.
///
/// A passive grab on the same button/key combination does not exist on any
/// ancestor of grab-window.
///
/// The interpretation of the remaining arguments is the same as for GrabPointer.
/// The active grab is terminated automatically when the logical state of the
/// pointer has all buttons released, independent of the logical state of modifier
/// keys. Note that the logical state of a device (as seen by means of the
/// protocol) may lag the physical state if device event processing is frozen. This
/// request overrides all previous passive grabs by the same client on the same
/// button/key combinations on the same window. A modifier of AnyModifier is
/// equivalent to issuing the request for all possible modifier combinations
/// (including the combination of no modifiers). It is not required that all
/// specified modifiers have currently assigned keycodes. A button of AnyButton is
/// equivalent to issuing the request for all possible buttons. Otherwise, it is
/// not required that the button specified currently be assigned to a physical
/// button.
///
/// An Access error is generated if some other client has already issued a
/// GrabButton request with the same button/key combination on the same window.
/// When using AnyModifier or AnyButton, the request fails completely (no grabs are
/// established), and an Access error is generated if there is a conflicting grab
/// for any combination. The request has no effect on an active grab.
///
/// # Fields
///
/// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the pointer should be grabbed.
/// * `event_mask` - Specifies which pointer events are reported to the client.
///
/// TODO: which values?
/// * `confine_to` - Specifies the window to confine the pointer in (the user will not be able to
/// move the pointer out of that window).
///
/// The special value `XCB_NONE` means don't confine the pointer.
/// * `cursor` - Specifies the cursor that should be displayed or `XCB_NONE` to not change the
/// cursor.
/// * `modifiers` - The modifiers to grab.
///
/// Using the special value `XCB_MOD_MASK_ANY` means grab the pointer with all
/// possible modifier combinations.
/// * `pointer_mode` -
/// * `keyboard_mode` -
/// * `button` -
///
/// # Errors
///
/// * `Access` - Another client has already issued a GrabButton with the same button/key
/// combination on the same window.
/// * `Value` - TODO: reasons?
/// * `Cursor` - The specified `cursor` does not exist.
/// * `Window` - The specified `window` does not exist.
pub fn grab_button<Conn, A, B>(conn: &Conn, owner_events: bool, grab_window: Window, event_mask: EventMask, pointer_mode: GrabMode, keyboard_mode: GrabMode, confine_to: A, cursor: B, button: ButtonIndex, modifiers: ModMask) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Window>,
    B: Into<Cursor>,
{
    let confine_to: Window = confine_to.into();
    let cursor: Cursor = cursor.into();
    let request0 = GrabButtonRequest {
        owner_events,
        grab_window,
        event_mask,
        pointer_mode,
        keyboard_mode,
        confine_to,
        cursor,
        button,
        modifiers,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn ungrab_button<Conn>(conn: &Conn, button: ButtonIndex, grab_window: Window, modifiers: ModMask) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UngrabButtonRequest {
        button,
        grab_window,
        modifiers,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn change_active_pointer_grab<Conn, A, B>(conn: &Conn, cursor: A, time: B, event_mask: EventMask) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Cursor>,
    B: Into<Timestamp>,
{
    let cursor: Cursor = cursor.into();
    let time: Timestamp = time.into();
    let request0 = ChangeActivePointerGrabRequest {
        cursor,
        time,
        event_mask,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Grab the keyboard.
///
/// Actively grabs control of the keyboard and generates FocusIn and FocusOut
/// events. Further key events are reported only to the grabbing client.
///
/// Any active keyboard grab by this client is overridden. If the keyboard is
/// actively grabbed by some other client, `AlreadyGrabbed` is returned. If
/// `grab_window` is not viewable, `GrabNotViewable` is returned. If the keyboard
/// is frozen by an active grab of another client, `GrabFrozen` is returned. If the
/// specified `time` is earlier than the last-keyboard-grab time or later than the
/// current X server time, `GrabInvalidTime` is returned. Otherwise, the
/// last-keyboard-grab time is set to the specified time.
///
/// # Fields
///
/// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the pointer should be grabbed.
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
/// * `pointer_mode` -
/// * `keyboard_mode` -
///
/// # Errors
///
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
///
/// # See
///
/// * `GrabPointer`: request
///
/// # Example
///
/// ```text
/// /*
///  * Grabs the keyboard actively
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_screen_t *screen) {
///     xcb_grab_keyboard_cookie_t cookie;
///     xcb_grab_keyboard_reply_t *reply;
///
///     cookie = xcb_grab_keyboard(
///         conn,
///         true,                /* report events */
///         screen->root,        /* grab the root window */
///         XCB_CURRENT_TIME,
///         XCB_GRAB_MODE_ASYNC, /* process events as normal, do not require sync */
///         XCB_GRAB_MODE_ASYNC
///     );
///
///     if ((reply = xcb_grab_keyboard_reply(conn, cookie, NULL))) {
///         if (reply->status == XCB_GRAB_STATUS_SUCCESS)
///             printf("successfully grabbed the keyboard\\n");
///
///         free(reply);
///     }
/// }
/// ```
pub fn grab_keyboard<Conn, A>(conn: &Conn, owner_events: bool, grab_window: Window, time: A, pointer_mode: GrabMode, keyboard_mode: GrabMode) -> Result<Cookie<'_, Conn, GrabKeyboardReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Timestamp>,
{
    let time: Timestamp = time.into();
    let request0 = GrabKeyboardRequest {
        owner_events,
        grab_window,
        time,
        pointer_mode,
        keyboard_mode,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn ungrab_keyboard<Conn, A>(conn: &Conn, time: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Timestamp>,
{
    let time: Timestamp = time.into();
    let request0 = UngrabKeyboardRequest {
        time,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Grab keyboard key(s).
///
/// Establishes a passive grab on the keyboard. In the future, the keyboard is
/// actively grabbed (as for `GrabKeyboard`), the last-keyboard-grab time is set to
/// the time at which the key was pressed (as transmitted in the KeyPress event),
/// and the KeyPress event is reported if all of the following conditions are true:
///
/// The keyboard is not grabbed and the specified key (which can itself be a
/// modifier key) is logically pressed when the specified modifier keys are
/// logically down, and no other modifier keys are logically down.
///
/// Either the grab_window is an ancestor of (or is) the focus window, or the
/// grab_window is a descendant of the focus window and contains the pointer.
///
/// A passive grab on the same key combination does not exist on any ancestor of
/// grab_window.
///
/// The interpretation of the remaining arguments is as for XGrabKeyboard.  The active grab is terminated
/// automatically when the logical state of the keyboard has the specified key released (independent of the
/// logical state of the modifier keys), at which point a KeyRelease event is reported to the grabbing window.
///
/// Note that the logical state of a device (as seen by client applications) may lag the physical state if
/// device event processing is frozen.
///
/// A modifiers argument of AnyModifier is equivalent to issuing the request for all possible modifier combinations (including the combination of no modifiers).  It is not required that all modifiers specified
/// have currently assigned KeyCodes.  A keycode argument of AnyKey is equivalent to issuing the request for
/// all possible KeyCodes.  Otherwise, the specified keycode must be in the range specified by min_keycode
/// and max_keycode in the connection setup, or a BadValue error results.
///
/// If some other client has issued a XGrabKey with the same key combination on the same window, a BadAccess
/// error results.  When using AnyModifier or AnyKey, the request fails completely, and a BadAccess error
/// results (no grabs are established) if there is a conflicting grab for any combination.
///
/// # Fields
///
/// * `owner_events` - If 1, the `grab_window` will still get the key events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the key should be grabbed.
/// * `key` - The keycode of the key to grab.
///
/// The special value `XCB_GRAB_ANY` means grab any key.
/// * `modifiers` - The modifiers to grab.
///
/// Using the special value `XCB_MOD_MASK_ANY` means grab the key with all
/// possible modifier combinations.
/// * `pointer_mode` -
/// * `keyboard_mode` -
///
/// # Errors
///
/// * `Access` - Another client has already issued a GrabKey with the same button/key
/// combination on the same window.
/// * `Value` - The key is not `XCB_GRAB_ANY` and not in the range specified by `min_keycode`
/// and `max_keycode` in the connection setup.
/// * `Window` - The specified `window` does not exist.
///
/// # See
///
/// * `GrabKeyboard`: request
pub fn grab_key<Conn, A>(conn: &Conn, owner_events: bool, grab_window: Window, modifiers: ModMask, key: A, pointer_mode: GrabMode, keyboard_mode: GrabMode) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Keycode>,
{
    let key: Keycode = key.into();
    let request0 = GrabKeyRequest {
        owner_events,
        grab_window,
        modifiers,
        key,
        pointer_mode,
        keyboard_mode,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// release a key combination.
///
/// Releases the key combination on `grab_window` if you grabbed it using
/// `xcb_grab_key` before.
///
/// # Fields
///
/// * `key` - The keycode of the specified key combination.
///
/// Using the special value `XCB_GRAB_ANY` means releasing all possible key codes.
/// * `grab_window` - The window on which the grabbed key combination will be released.
/// * `modifiers` - The modifiers of the specified key combination.
///
/// Using the special value `XCB_MOD_MASK_ANY` means releasing the key combination
/// with every possible modifier combination.
///
/// # Errors
///
/// * `Window` - The specified `grab_window` does not exist.
/// * `Value` - TODO: reasons?
///
/// # See
///
/// * `GrabKey`: request
/// * `xev`: program
pub fn ungrab_key<Conn, A>(conn: &Conn, key: A, grab_window: Window, modifiers: ModMask) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Keycode>,
{
    let key: Keycode = key.into();
    let request0 = UngrabKeyRequest {
        key,
        grab_window,
        modifiers,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// release queued events.
///
/// Releases queued events if the client has caused a device (pointer/keyboard) to
/// freeze due to grabbing it actively. This request has no effect if `time` is
/// earlier than the last-grab time of the most recent active grab for this client
/// or if `time` is later than the current X server time.
///
/// # Fields
///
/// * `mode` -
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
///
/// # Errors
///
/// * `Value` - You specified an invalid `mode`.
pub fn allow_events<Conn, A>(conn: &Conn, mode: Allow, time: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Timestamp>,
{
    let time: Timestamp = time.into();
    let request0 = AllowEventsRequest {
        mode,
        time,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn grab_server<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GrabServerRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn ungrab_server<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UngrabServerRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// get pointer coordinates.
///
/// Gets the root window the pointer is logically on and the pointer coordinates
/// relative to the root window's origin.
///
/// # Fields
///
/// * `window` - A window to check if the pointer is on the same screen as `window` (see the
/// `same_screen` field in the reply).
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
pub fn query_pointer<Conn>(conn: &Conn, window: Window) -> Result<Cookie<'_, Conn, QueryPointerReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryPointerRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_motion_events<Conn, A, B>(conn: &Conn, window: Window, start: A, stop: B) -> Result<Cookie<'_, Conn, GetMotionEventsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Timestamp>,
    B: Into<Timestamp>,
{
    let start: Timestamp = start.into();
    let stop: Timestamp = stop.into();
    let request0 = GetMotionEventsRequest {
        window,
        start,
        stop,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn translate_coordinates<Conn>(conn: &Conn, src_window: Window, dst_window: Window, src_x: i16, src_y: i16) -> Result<Cookie<'_, Conn, TranslateCoordinatesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = TranslateCoordinatesRequest {
        src_window,
        dst_window,
        src_x,
        src_y,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// move mouse pointer.
///
/// Moves the mouse pointer to the specified position.
///
/// If `src_window` is not `XCB_NONE` (TODO), the move will only take place if the
/// pointer is inside `src_window` and within the rectangle specified by (`src_x`,
/// `src_y`, `src_width`, `src_height`). The rectangle coordinates are relative to
/// `src_window`.
///
/// If `dst_window` is not `XCB_NONE` (TODO), the pointer will be moved to the
/// offsets (`dst_x`, `dst_y`) relative to `dst_window`. If `dst_window` is
/// `XCB_NONE` (TODO), the pointer will be moved by the offsets (`dst_x`, `dst_y`)
/// relative to the current position of the pointer.
///
/// # Fields
///
/// * `src_window` - If `src_window` is not `XCB_NONE` (TODO), the move will only take place if the
/// pointer is inside `src_window` and within the rectangle specified by (`src_x`,
/// `src_y`, `src_width`, `src_height`). The rectangle coordinates are relative to
/// `src_window`.
/// * `dst_window` - If `dst_window` is not `XCB_NONE` (TODO), the pointer will be moved to the
/// offsets (`dst_x`, `dst_y`) relative to `dst_window`. If `dst_window` is
/// `XCB_NONE` (TODO), the pointer will be moved by the offsets (`dst_x`, `dst_y`)
/// relative to the current position of the pointer.
///
/// # Errors
///
/// * `Window` - TODO: reasons?
///
/// # See
///
/// * `SetInputFocus`: request
pub fn warp_pointer<Conn, A, B>(conn: &Conn, src_window: A, dst_window: B, src_x: i16, src_y: i16, src_width: u16, src_height: u16, dst_x: i16, dst_y: i16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Window>,
    B: Into<Window>,
{
    let src_window: Window = src_window.into();
    let dst_window: Window = dst_window.into();
    let request0 = WarpPointerRequest {
        src_window,
        dst_window,
        src_x,
        src_y,
        src_width,
        src_height,
        dst_x,
        dst_y,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Sets input focus.
///
/// Changes the input focus and the last-focus-change time. If the specified `time`
/// is earlier than the current last-focus-change time, the request is ignored (to
/// avoid race conditions when running X over the network).
///
/// A FocusIn and FocusOut event is generated when focus is changed.
///
/// # Fields
///
/// * `focus` - The window to focus. All keyboard events will be reported to this window. The
/// window must be viewable (TODO), or a `xcb_match_error_t` occurs (TODO).
///
/// If `focus` is `XCB_NONE` (TODO), all keyboard events are
/// discarded until a new focus window is set.
///
/// If `focus` is `XCB_POINTER_ROOT` (TODO), focus is on the root window of the
/// screen on which the pointer is on currently.
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
/// * `revert_to` - Specifies what happens when the `focus` window becomes unviewable (if `focus`
/// is neither `XCB_NONE` nor `XCB_POINTER_ROOT`).
///
/// # Errors
///
/// * `Window` - The specified `focus` window does not exist.
/// * `Match` - The specified `focus` window is not viewable.
/// * `Value` - TODO: Reasons?
///
/// # See
///
/// * `FocusIn`: event
/// * `FocusOut`: event
pub fn set_input_focus<Conn, A, B>(conn: &Conn, revert_to: InputFocus, focus: A, time: B) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Window>,
    B: Into<Timestamp>,
{
    let focus: Window = focus.into();
    let time: Timestamp = time.into();
    let request0 = SetInputFocusRequest {
        revert_to,
        focus,
        time,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_input_focus<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetInputFocusReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetInputFocusRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_keymap<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, QueryKeymapReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryKeymapRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// opens a font.
///
/// Opens any X core font matching the given `name` (for example "-misc-fixed-*").
///
/// Note that X core fonts are deprecated (but still supported) in favor of
/// client-side rendering using Xft.
///
/// # Fields
///
/// * `fid` - The ID with which you will refer to the font, created by `xcb_generate_id`.
/// * `name_len` - Length (in bytes) of `name`.
/// * `name` - A pattern describing an X core font.
///
/// # Errors
///
/// * `Name` - No font matches the given `name`.
///
/// # See
///
/// * `xcb_generate_id`: function
pub fn open_font<'c, 'input, Conn>(conn: &'c Conn, fid: Font, name: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = OpenFontRequest {
        fid,
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn close_font<Conn>(conn: &Conn, font: Font) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CloseFontRequest {
        font,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// query font metrics.
///
/// Queries information associated with the font.
///
/// # Fields
///
/// * `font` - The fontable (Font or Graphics Context) to query.
pub fn query_font<Conn>(conn: &Conn, font: Fontable) -> Result<Cookie<'_, Conn, QueryFontReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryFontRequest {
        font,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// get text extents.
///
/// Query text extents from the X11 server. This request returns the bounding box
/// of the specified 16-bit character string in the specified `font` or the font
/// contained in the specified graphics context.
///
/// `font_ascent` is set to the maximum of the ascent metrics of all characters in
/// the string. `font_descent` is set to the maximum of the descent metrics.
/// `overall_width` is set to the sum of the character-width metrics of all
/// characters in the string. For each character in the string, let W be the sum of
/// the character-width metrics of all characters preceding it in the string. Let L
/// be the left-side-bearing metric of the character plus W. Let R be the
/// right-side-bearing metric of the character plus W. The lbearing member is set
/// to the minimum L of all characters in the string. The rbearing member is set to
/// the maximum R.
///
/// For fonts defined with linear indexing rather than 2-byte matrix indexing, each
/// `xcb_char2b_t` structure is interpreted as a 16-bit number with byte1 as the
/// most significant byte. If the font has no defined default character, undefined
/// characters in the string are taken to have all zero metrics.
///
/// Characters with all zero metrics are ignored. If the font has no defined
/// default_char, the undefined characters in the string are also ignored.
///
/// # Fields
///
/// * `font` - The `font` to calculate text extents in. You can also pass a graphics context.
/// * `string_len` - The number of characters in `string`.
/// * `string` - The text to get text extents for.
///
/// # Errors
///
/// * `GContext` - The specified graphics context does not exist.
/// * `Font` - The specified `font` does not exist.
pub fn query_text_extents<'c, 'input, Conn>(conn: &'c Conn, font: Fontable, string: &'input [Char2b]) -> Result<Cookie<'c, Conn, QueryTextExtentsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryTextExtentsRequest {
        font,
        string: Cow::Borrowed(string),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// get matching font names.
///
/// Gets a list of available font names which match the given `pattern`.
///
/// # Fields
///
/// * `pattern_len` - The length (in bytes) of `pattern`.
/// * `pattern` - A font pattern, for example "-misc-fixed-*".
///
/// The asterisk (*) is a wildcard for any number of characters. The question mark
/// (?) is a wildcard for a single character. Use of uppercase or lowercase does
/// not matter.
/// * `max_names` - The maximum number of fonts to be returned.
pub fn list_fonts<'c, 'input, Conn>(conn: &'c Conn, max_names: u16, pattern: &'input [u8]) -> Result<Cookie<'c, Conn, ListFontsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListFontsRequest {
        max_names,
        pattern: Cow::Borrowed(pattern),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// get matching font names and information.
///
/// Gets a list of available font names which match the given `pattern`.
///
/// # Fields
///
/// * `pattern_len` - The length (in bytes) of `pattern`.
/// * `pattern` - A font pattern, for example "-misc-fixed-*".
///
/// The asterisk (*) is a wildcard for any number of characters. The question mark
/// (?) is a wildcard for a single character. Use of uppercase or lowercase does
/// not matter.
/// * `max_names` - The maximum number of fonts to be returned.
pub fn list_fonts_with_info<'c, 'input, Conn>(conn: &'c Conn, max_names: u16, pattern: &'input [u8]) -> Result<ListFontsWithInfoCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListFontsWithInfoRequest {
        max_names,
        pattern: Cow::Borrowed(pattern),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    Ok(ListFontsWithInfoCookie::new(conn.send_request_with_reply(&slices, fds)?))
}

pub fn set_font_path<'c, 'input, Conn>(conn: &'c Conn, font: &'input [Str]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetFontPathRequest {
        font: Cow::Borrowed(font),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_font_path<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetFontPathReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetFontPathRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Creates a pixmap.
///
/// Creates a pixmap. The pixmap can only be used on the same screen as `drawable`
/// is on and only with drawables of the same `depth`.
///
/// # Fields
///
/// * `depth` - TODO
/// * `pid` - The ID with which you will refer to the new pixmap, created by
/// `xcb_generate_id`.
/// * `drawable` - Drawable to get the screen from.
/// * `width` - The width of the new pixmap.
/// * `height` - The height of the new pixmap.
///
/// # Errors
///
/// * `Value` - TODO: reasons?
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # See
///
/// * `xcb_generate_id`: function
pub fn create_pixmap<Conn>(conn: &Conn, depth: u8, pid: Pixmap, drawable: Drawable, width: u16, height: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreatePixmapRequest {
        depth,
        pid,
        drawable,
        width,
        height,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Destroys a pixmap.
///
/// Deletes the association between the pixmap ID and the pixmap. The pixmap
/// storage will be freed when there are no more references to it.
///
/// # Fields
///
/// * `pixmap` - The pixmap to destroy.
///
/// # Errors
///
/// * `Pixmap` - The specified pixmap does not exist.
pub fn free_pixmap<Conn>(conn: &Conn, pixmap: Pixmap) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreePixmapRequest {
        pixmap,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Creates a graphics context.
///
/// Creates a graphics context. The graphics context can be used with any drawable
/// that has the same root and depth as the specified drawable.
///
/// # Fields
///
/// * `cid` - The ID with which you will refer to the graphics context, created by
/// `xcb_generate_id`.
/// * `drawable` - Drawable to get the root/depth from.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `Match` - TODO: reasons?
/// * `Font` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # See
///
/// * `xcb_generate_id`: function
pub fn create_gc<'c, 'input, Conn>(conn: &'c Conn, cid: Gcontext, drawable: Drawable, value_list: &'input CreateGCAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateGCRequest {
        cid,
        drawable,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// change graphics context components.
///
/// Changes the components specified by `value_mask` for the specified graphics context.
///
/// # Fields
///
/// * `gc` - The graphics context to change.
/// * `value_mask` -
/// * `value_list` - Values for each of the components specified in the bitmask `value_mask`. The
/// order has to correspond to the order of possible `value_mask` bits. See the
/// example.
///
/// # Errors
///
/// * `Font` - TODO: reasons?
/// * `GContext` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # Example
///
/// ```text
/// /*
///  * Changes the foreground color component of the specified graphics context.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_gcontext_t gc, uint32_t fg, uint32_t bg) {
///     /* C99 allows us to use a compact way of changing a single component: */
///     xcb_change_gc(conn, gc, XCB_GC_FOREGROUND, (uint32_t[]){ fg });
///
///     /* The more explicit way. Beware that the order of values is important! */
///     uint32_t mask = 0;
///     mask |= XCB_GC_FOREGROUND;
///     mask |= XCB_GC_BACKGROUND;
///
///     uint32_t values[] = {
///         fg,
///         bg
///     };
///     xcb_change_gc(conn, gc, mask, values);
///     xcb_flush(conn);
/// }
/// ```
pub fn change_gc<'c, 'input, Conn>(conn: &'c Conn, gc: Gcontext, value_list: &'input ChangeGCAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeGCRequest {
        gc,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn copy_gc<Conn>(conn: &Conn, src_gc: Gcontext, dst_gc: Gcontext, value_mask: GC) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CopyGCRequest {
        src_gc,
        dst_gc,
        value_mask,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_dashes<'c, 'input, Conn>(conn: &'c Conn, gc: Gcontext, dash_offset: u16, dashes: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDashesRequest {
        gc,
        dash_offset,
        dashes: Cow::Borrowed(dashes),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_clip_rectangles<'c, 'input, Conn>(conn: &'c Conn, ordering: ClipOrdering, gc: Gcontext, clip_x_origin: i16, clip_y_origin: i16, rectangles: &'input [Rectangle]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetClipRectanglesRequest {
        ordering,
        gc,
        clip_x_origin,
        clip_y_origin,
        rectangles: Cow::Borrowed(rectangles),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Destroys a graphics context.
///
/// Destroys the specified `gc` and all associated storage.
///
/// # Fields
///
/// * `gc` - The graphics context to destroy.
///
/// # Errors
///
/// * `GContext` - The specified graphics context does not exist.
pub fn free_gc<Conn>(conn: &Conn, gc: Gcontext) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeGCRequest {
        gc,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn clear_area<Conn>(conn: &Conn, exposures: bool, window: Window, x: i16, y: i16, width: u16, height: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ClearAreaRequest {
        exposures,
        window,
        x,
        y,
        width,
        height,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// copy areas.
///
/// Copies the specified rectangle from `src_drawable` to `dst_drawable`.
///
/// # Fields
///
/// * `dst_drawable` - The destination drawable (Window or Pixmap).
/// * `src_drawable` - The source drawable (Window or Pixmap).
/// * `gc` - The graphics context to use.
/// * `src_x` - The source X coordinate.
/// * `src_y` - The source Y coordinate.
/// * `dst_x` - The destination X coordinate.
/// * `dst_y` - The destination Y coordinate.
/// * `width` - The width of the area to copy (in pixels).
/// * `height` - The height of the area to copy (in pixels).
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - `src_drawable` has a different root or depth than `dst_drawable`.
pub fn copy_area<Conn>(conn: &Conn, src_drawable: Drawable, dst_drawable: Drawable, gc: Gcontext, src_x: i16, src_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CopyAreaRequest {
        src_drawable,
        dst_drawable,
        gc,
        src_x,
        src_y,
        dst_x,
        dst_y,
        width,
        height,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn copy_plane<Conn>(conn: &Conn, src_drawable: Drawable, dst_drawable: Drawable, gc: Gcontext, src_x: i16, src_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16, bit_plane: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CopyPlaneRequest {
        src_drawable,
        dst_drawable,
        gc,
        src_x,
        src_y,
        dst_x,
        dst_y,
        width,
        height,
        bit_plane,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn poly_point<'c, 'input, Conn>(conn: &'c Conn, coordinate_mode: CoordMode, drawable: Drawable, gc: Gcontext, points: &'input [Point]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyPointRequest {
        coordinate_mode,
        drawable,
        gc,
        points: Cow::Borrowed(points),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// draw lines.
///
/// Draws `points_len`-1 lines between each pair of points (point[i], point[i+1])
/// in the `points` array. The lines are drawn in the order listed in the array.
/// They join correctly at all intermediate points, and if the first and last
/// points coincide, the first and last lines also join correctly. For any given
/// line, a pixel is not drawn more than once. If thin (zero line-width) lines
/// intersect, the intersecting pixels are drawn multiple times. If wide lines
/// intersect, the intersecting pixels are drawn only once, as though the entire
/// request were a single, filled shape.
///
/// # Fields
///
/// * `drawable` - The drawable to draw the line(s) on.
/// * `gc` - The graphics context to use.
/// * `points_len` - The number of `xcb_point_t` structures in `points`.
/// * `points` - An array of points.
/// * `coordinate_mode` -
///
/// # Errors
///
/// * `Drawable` - TODO: reasons?
/// * `GContext` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Value` - TODO: reasons?
///
/// # Example
///
/// ```text
/// /*
///  * Draw a straight line.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_drawable_t drawable, xcb_gcontext_t gc) {
///     xcb_poly_line(conn, XCB_COORD_MODE_ORIGIN, drawable, gc, 2,
///                   (xcb_point_t[]) { {10, 10}, {100, 10} });
///     xcb_flush(conn);
/// }
/// ```
pub fn poly_line<'c, 'input, Conn>(conn: &'c Conn, coordinate_mode: CoordMode, drawable: Drawable, gc: Gcontext, points: &'input [Point]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyLineRequest {
        coordinate_mode,
        drawable,
        gc,
        points: Cow::Borrowed(points),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// draw lines.
///
/// Draws multiple, unconnected lines. For each segment, a line is drawn between
/// (x1, y1) and (x2, y2). The lines are drawn in the order listed in the array of
/// `xcb_segment_t` structures and does not perform joining at coincident
/// endpoints. For any given line, a pixel is not drawn more than once. If lines
/// intersect, the intersecting pixels are drawn multiple times.
///
/// TODO: include the xcb_segment_t data structure
///
/// TODO: an example
///
/// # Fields
///
/// * `drawable` - A drawable (Window or Pixmap) to draw on.
/// * `gc` - The graphics context to use.
///
/// TODO: document which attributes of a gc are used
/// * `segments_len` - The number of `xcb_segment_t` structures in `segments`.
/// * `segments` - An array of `xcb_segment_t` structures.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` does not exist.
/// * `GContext` - The specified `gc` does not exist.
/// * `Match` - TODO: reasons?
pub fn poly_segment<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, segments: &'input [Segment]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolySegmentRequest {
        drawable,
        gc,
        segments: Cow::Borrowed(segments),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn poly_rectangle<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, rectangles: &'input [Rectangle]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyRectangleRequest {
        drawable,
        gc,
        rectangles: Cow::Borrowed(rectangles),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn poly_arc<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, arcs: &'input [Arc]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyArcRequest {
        drawable,
        gc,
        arcs: Cow::Borrowed(arcs),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn fill_poly<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, shape: PolyShape, coordinate_mode: CoordMode, points: &'input [Point]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FillPolyRequest {
        drawable,
        gc,
        shape,
        coordinate_mode,
        points: Cow::Borrowed(points),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Fills rectangles.
///
/// Fills the specified rectangle(s) in the order listed in the array. For any
/// given rectangle, each pixel is not drawn more than once. If rectangles
/// intersect, the intersecting pixels are drawn multiple times.
///
/// # Fields
///
/// * `drawable` - The drawable (Window or Pixmap) to draw on.
/// * `gc` - The graphics context to use.
///
/// The following graphics context components are used: function, plane-mask,
/// fill-style, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
///
/// The following graphics context mode-dependent components are used:
/// foreground, background, tile, stipple, tile-stipple-x-origin, and
/// tile-stipple-y-origin.
/// * `rectangles_len` - The number of `xcb_rectangle_t` structures in `rectangles`.
/// * `rectangles` - The rectangles to fill.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - TODO: reasons?
pub fn poly_fill_rectangle<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, rectangles: &'input [Rectangle]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyFillRectangleRequest {
        drawable,
        gc,
        rectangles: Cow::Borrowed(rectangles),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn poly_fill_arc<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, arcs: &'input [Arc]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyFillArcRequest {
        drawable,
        gc,
        arcs: Cow::Borrowed(arcs),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn put_image<'c, 'input, Conn>(conn: &'c Conn, format: ImageFormat, drawable: Drawable, gc: Gcontext, width: u16, height: u16, dst_x: i16, dst_y: i16, left_pad: u8, depth: u8, data: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PutImageRequest {
        format,
        drawable,
        gc,
        width,
        height,
        dst_x,
        dst_y,
        left_pad,
        depth,
        data: Cow::Borrowed(data),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_image<Conn>(conn: &Conn, format: ImageFormat, drawable: Drawable, x: i16, y: i16, width: u16, height: u16, plane_mask: u32) -> Result<Cookie<'_, Conn, GetImageReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetImageRequest {
        format,
        drawable,
        x,
        y,
        width,
        height,
        plane_mask,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn poly_text8<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, x: i16, y: i16, items: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyText8Request {
        drawable,
        gc,
        x,
        y,
        items: Cow::Borrowed(items),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn poly_text16<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, x: i16, y: i16, items: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PolyText16Request {
        drawable,
        gc,
        x,
        y,
        items: Cow::Borrowed(items),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Draws text.
///
/// Fills the destination rectangle with the background pixel from `gc`, then
/// paints the text with the foreground pixel from `gc`. The upper-left corner of
/// the filled rectangle is at [x, y - font-ascent]. The width is overall-width,
/// the height is font-ascent + font-descent. The overall-width, font-ascent and
/// font-descent are as returned by `xcb_query_text_extents` (TODO).
///
/// Note that using X core fonts is deprecated (but still supported) in favor of
/// client-side rendering using Xft.
///
/// # Fields
///
/// * `drawable` - The drawable (Window or Pixmap) to draw text on.
/// * `string_len` - The length of the `string`. Note that this parameter limited by 255 due to
/// using 8 bits!
/// * `string` - The string to draw. Only the first 255 characters are relevant due to the data
/// type of `string_len`.
/// * `x` - The x coordinate of the first character, relative to the origin of `drawable`.
/// * `y` - The y coordinate of the first character, relative to the origin of `drawable`.
/// * `gc` - The graphics context to use.
///
/// The following graphics context components are used: plane-mask, foreground,
/// background, font, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - TODO: reasons?
///
/// # See
///
/// * `ImageText16`: request
pub fn image_text8<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, x: i16, y: i16, string: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ImageText8Request {
        drawable,
        gc,
        x,
        y,
        string: Cow::Borrowed(string),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Draws text.
///
/// Fills the destination rectangle with the background pixel from `gc`, then
/// paints the text with the foreground pixel from `gc`. The upper-left corner of
/// the filled rectangle is at [x, y - font-ascent]. The width is overall-width,
/// the height is font-ascent + font-descent. The overall-width, font-ascent and
/// font-descent are as returned by `xcb_query_text_extents` (TODO).
///
/// Note that using X core fonts is deprecated (but still supported) in favor of
/// client-side rendering using Xft.
///
/// # Fields
///
/// * `drawable` - The drawable (Window or Pixmap) to draw text on.
/// * `string_len` - The length of the `string` in characters. Note that this parameter limited by
/// 255 due to using 8 bits!
/// * `string` - The string to draw. Only the first 255 characters are relevant due to the data
/// type of `string_len`. Every character uses 2 bytes (hence the 16 in this
/// request's name).
/// * `x` - The x coordinate of the first character, relative to the origin of `drawable`.
/// * `y` - The y coordinate of the first character, relative to the origin of `drawable`.
/// * `gc` - The graphics context to use.
///
/// The following graphics context components are used: plane-mask, foreground,
/// background, font, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - TODO: reasons?
///
/// # See
///
/// * `ImageText8`: request
pub fn image_text16<'c, 'input, Conn>(conn: &'c Conn, drawable: Drawable, gc: Gcontext, x: i16, y: i16, string: &'input [Char2b]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ImageText16Request {
        drawable,
        gc,
        x,
        y,
        string: Cow::Borrowed(string),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_colormap<Conn>(conn: &Conn, alloc: ColormapAlloc, mid: Colormap, window: Window, visual: Visualid) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateColormapRequest {
        alloc,
        mid,
        window,
        visual,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn free_colormap<Conn>(conn: &Conn, cmap: Colormap) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeColormapRequest {
        cmap,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn copy_colormap_and_free<Conn>(conn: &Conn, mid: Colormap, src_cmap: Colormap) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CopyColormapAndFreeRequest {
        mid,
        src_cmap,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn install_colormap<Conn>(conn: &Conn, cmap: Colormap) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = InstallColormapRequest {
        cmap,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn uninstall_colormap<Conn>(conn: &Conn, cmap: Colormap) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UninstallColormapRequest {
        cmap,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn list_installed_colormaps<Conn>(conn: &Conn, window: Window) -> Result<Cookie<'_, Conn, ListInstalledColormapsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListInstalledColormapsRequest {
        window,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Allocate a color.
///
/// Allocates a read-only colormap entry corresponding to the closest RGB value
/// supported by the hardware. If you are using TrueColor, you can take a shortcut
/// and directly calculate the color pixel value to avoid the round trip. But, for
/// example, on 16-bit color setups (VNC), you can easily get the closest supported
/// RGB value to the RGB value you are specifying.
///
/// # Fields
///
/// * `cmap` - TODO
/// * `red` - The red value of your color.
/// * `green` - The green value of your color.
/// * `blue` - The blue value of your color.
///
/// # Errors
///
/// * `Colormap` - The specified colormap `cmap` does not exist.
pub fn alloc_color<Conn>(conn: &Conn, cmap: Colormap, red: u16, green: u16, blue: u16) -> Result<Cookie<'_, Conn, AllocColorReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AllocColorRequest {
        cmap,
        red,
        green,
        blue,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn alloc_named_color<'c, 'input, Conn>(conn: &'c Conn, cmap: Colormap, name: &'input [u8]) -> Result<Cookie<'c, Conn, AllocNamedColorReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AllocNamedColorRequest {
        cmap,
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn alloc_color_cells<Conn>(conn: &Conn, contiguous: bool, cmap: Colormap, colors: u16, planes: u16) -> Result<Cookie<'_, Conn, AllocColorCellsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AllocColorCellsRequest {
        contiguous,
        cmap,
        colors,
        planes,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn alloc_color_planes<Conn>(conn: &Conn, contiguous: bool, cmap: Colormap, colors: u16, reds: u16, greens: u16, blues: u16) -> Result<Cookie<'_, Conn, AllocColorPlanesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AllocColorPlanesRequest {
        contiguous,
        cmap,
        colors,
        reds,
        greens,
        blues,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn free_colors<'c, 'input, Conn>(conn: &'c Conn, cmap: Colormap, plane_mask: u32, pixels: &'input [u32]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeColorsRequest {
        cmap,
        plane_mask,
        pixels: Cow::Borrowed(pixels),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn store_colors<'c, 'input, Conn>(conn: &'c Conn, cmap: Colormap, items: &'input [Coloritem]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = StoreColorsRequest {
        cmap,
        items: Cow::Borrowed(items),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn store_named_color<'c, 'input, Conn>(conn: &'c Conn, flags: ColorFlag, cmap: Colormap, pixel: u32, name: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = StoreNamedColorRequest {
        flags,
        cmap,
        pixel,
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_colors<'c, 'input, Conn>(conn: &'c Conn, cmap: Colormap, pixels: &'input [u32]) -> Result<Cookie<'c, Conn, QueryColorsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryColorsRequest {
        cmap,
        pixels: Cow::Borrowed(pixels),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn lookup_color<'c, 'input, Conn>(conn: &'c Conn, cmap: Colormap, name: &'input [u8]) -> Result<Cookie<'c, Conn, LookupColorReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = LookupColorRequest {
        cmap,
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn create_cursor<Conn, A>(conn: &Conn, cid: Cursor, source: Pixmap, mask: A, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16, x: u16, y: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Pixmap>,
{
    let mask: Pixmap = mask.into();
    let request0 = CreateCursorRequest {
        cid,
        source,
        mask,
        fore_red,
        fore_green,
        fore_blue,
        back_red,
        back_green,
        back_blue,
        x,
        y,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// create cursor.
///
/// Creates a cursor from a font glyph. X provides a set of standard cursor shapes
/// in a special font named cursor. Applications are encouraged to use this
/// interface for their cursors because the font can be customized for the
/// individual display type.
///
/// All pixels which are set to 1 in the source will use the foreground color (as
/// specified by `fore_red`, `fore_green` and `fore_blue`). All pixels set to 0
/// will use the background color (as specified by `back_red`, `back_green` and
/// `back_blue`).
///
/// # Fields
///
/// * `cid` - The ID with which you will refer to the cursor, created by `xcb_generate_id`.
/// * `source_font` - In which font to look for the cursor glyph.
/// * `mask_font` - In which font to look for the mask glyph.
/// * `source_char` - The glyph of `source_font` to use.
/// * `mask_char` - The glyph of `mask_font` to use as a mask: Pixels which are set to 1 define
/// which source pixels are displayed. All pixels which are set to 0 are not
/// displayed.
/// * `fore_red` - The red value of the foreground color.
/// * `fore_green` - The green value of the foreground color.
/// * `fore_blue` - The blue value of the foreground color.
/// * `back_red` - The red value of the background color.
/// * `back_green` - The green value of the background color.
/// * `back_blue` - The blue value of the background color.
///
/// # Errors
///
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
/// * `Font` - The specified `source_font` or `mask_font` does not exist.
/// * `Value` - Either `source_char` or `mask_char` are not defined in `source_font` or `mask_font`, respectively.
pub fn create_glyph_cursor<Conn, A>(conn: &Conn, cid: Cursor, source_font: Font, mask_font: A, source_char: u16, mask_char: u16, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Font>,
{
    let mask_font: Font = mask_font.into();
    let request0 = CreateGlyphCursorRequest {
        cid,
        source_font,
        mask_font,
        source_char,
        mask_char,
        fore_red,
        fore_green,
        fore_blue,
        back_red,
        back_green,
        back_blue,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Deletes a cursor.
///
/// Deletes the association between the cursor resource ID and the specified
/// cursor. The cursor is freed when no other resource references it.
///
/// # Fields
///
/// * `cursor` - The cursor to destroy.
///
/// # Errors
///
/// * `Cursor` - The specified cursor does not exist.
pub fn free_cursor<Conn>(conn: &Conn, cursor: Cursor) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeCursorRequest {
        cursor,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn recolor_cursor<Conn>(conn: &Conn, cursor: Cursor, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = RecolorCursorRequest {
        cursor,
        fore_red,
        fore_green,
        fore_blue,
        back_red,
        back_green,
        back_blue,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_best_size<Conn>(conn: &Conn, class: QueryShapeOf, drawable: Drawable, width: u16, height: u16) -> Result<Cookie<'_, Conn, QueryBestSizeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryBestSizeRequest {
        class,
        drawable,
        width,
        height,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// check if extension is present.
///
/// Determines if the specified extension is present on this X11 server.
///
/// Every extension has a unique `major_opcode` to identify requests, the minor
/// opcodes and request formats are extension-specific. If the extension provides
/// events and errors, the `first_event` and `first_error` fields in the reply are
/// set accordingly.
///
/// There should rarely be a need to use this request directly, XCB provides the
/// `xcb_get_extension_data` function instead.
///
/// # Fields
///
/// * `name_len` - The length of `name` in bytes.
/// * `name` - The name of the extension to query, for example "RANDR". This is case
/// sensitive!
///
/// # See
///
/// * `xdpyinfo`: program
/// * `xcb_get_extension_data`: function
pub fn query_extension<'c, 'input, Conn>(conn: &'c Conn, name: &'input [u8]) -> Result<Cookie<'c, Conn, QueryExtensionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryExtensionRequest {
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_extensions<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, ListExtensionsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListExtensionsRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_keyboard_mapping<'c, 'input, Conn>(conn: &'c Conn, keycode_count: u8, first_keycode: Keycode, keysyms_per_keycode: u8, keysyms: &'input [Keysym]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeKeyboardMappingRequest {
        keycode_count,
        first_keycode,
        keysyms_per_keycode,
        keysyms: Cow::Borrowed(keysyms),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_keyboard_mapping<Conn>(conn: &Conn, first_keycode: Keycode, count: u8) -> Result<Cookie<'_, Conn, GetKeyboardMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetKeyboardMappingRequest {
        first_keycode,
        count,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_keyboard_control<'c, 'input, Conn>(conn: &'c Conn, value_list: &'input ChangeKeyboardControlAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeKeyboardControlRequest {
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_keyboard_control<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetKeyboardControlReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetKeyboardControlRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn bell<Conn>(conn: &Conn, percent: i8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = BellRequest {
        percent,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn change_pointer_control<Conn>(conn: &Conn, acceleration_numerator: i16, acceleration_denominator: i16, threshold: i16, do_acceleration: bool, do_threshold: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangePointerControlRequest {
        acceleration_numerator,
        acceleration_denominator,
        threshold,
        do_acceleration,
        do_threshold,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_pointer_control<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetPointerControlReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetPointerControlRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_screen_saver<Conn>(conn: &Conn, timeout: i16, interval: i16, prefer_blanking: Blanking, allow_exposures: Exposures) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetScreenSaverRequest {
        timeout,
        interval,
        prefer_blanking,
        allow_exposures,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_screen_saver<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetScreenSaverReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetScreenSaverRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn change_hosts<'c, 'input, Conn>(conn: &'c Conn, mode: HostMode, family: Family, address: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangeHostsRequest {
        mode,
        family,
        address: Cow::Borrowed(address),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn list_hosts<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, ListHostsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListHostsRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_access_control<Conn>(conn: &Conn, mode: AccessControl) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetAccessControlRequest {
        mode,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_close_down_mode<Conn>(conn: &Conn, mode: CloseDown) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetCloseDownModeRequest {
        mode,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// kills a client.
///
/// Forces a close down of the client that created the specified `resource`.
///
/// # Fields
///
/// * `resource` - Any resource belonging to the client (for example a Window), used to identify
/// the client connection.
///
/// The special value of `XCB_KILL_ALL_TEMPORARY`, the resources of all clients
/// that have terminated in `RetainTemporary` (TODO) are destroyed.
///
/// # Errors
///
/// * `Value` - The specified `resource` does not exist.
///
/// # See
///
/// * `xkill`: program
pub fn kill_client<Conn, A>(conn: &Conn, resource: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<u32>,
{
    let resource: u32 = resource.into();
    let request0 = KillClientRequest {
        resource,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn rotate_properties<'c, 'input, Conn>(conn: &'c Conn, window: Window, delta: i16, atoms: &'input [Atom]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = RotatePropertiesRequest {
        window,
        delta,
        atoms: Cow::Borrowed(atoms),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn force_screen_saver<Conn>(conn: &Conn, mode: ScreenSaver) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ForceScreenSaverRequest {
        mode,
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_pointer_mapping<'c, 'input, Conn>(conn: &'c Conn, map: &'input [u8]) -> Result<Cookie<'c, Conn, SetPointerMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetPointerMappingRequest {
        map: Cow::Borrowed(map),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_pointer_mapping<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetPointerMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetPointerMappingRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_modifier_mapping<'c, 'input, Conn>(conn: &'c Conn, keycodes: &'input [Keycode]) -> Result<Cookie<'c, Conn, SetModifierMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetModifierMappingRequest {
        keycodes: Cow::Borrowed(keycodes),
    };
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_modifier_mapping<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetModifierMappingReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetModifierMappingRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn no_operation<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = NoOperationRequest;
    let (bytes, fds) = request0.serialize();
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    /// Creates a window.
    ///
    /// Creates an unmapped window as child of the specified `parent` window. A
    /// CreateNotify event will be generated. The new window is placed on top in the
    /// stacking order with respect to siblings.
    ///
    /// The coordinate system has the X axis horizontal and the Y axis vertical with
    /// the origin [0, 0] at the upper-left corner. Coordinates are integral, in terms
    /// of pixels, and coincide with pixel centers. Each window and pixmap has its own
    /// coordinate system. For a window, the origin is inside the border at the inside,
    /// upper-left corner.
    ///
    /// The created window is not yet displayed (mapped), call `xcb_map_window` to
    /// display it.
    ///
    /// The created window will initially use the same cursor as its parent.
    ///
    /// # Fields
    ///
    /// * `wid` - The ID with which you will refer to the new window, created by
    /// `xcb_generate_id`.
    /// * `depth` - Specifies the new window's depth (TODO: what unit?).
    ///
    /// The special value `XCB_COPY_FROM_PARENT` means the depth is taken from the
    /// `parent` window.
    /// * `visual` - Specifies the id for the new window's visual.
    ///
    /// The special value `XCB_COPY_FROM_PARENT` means the visual is taken from the
    /// `parent` window.
    /// * `class` -
    /// * `parent` - The parent window of the new window.
    /// * `border_width` - TODO:
    ///
    /// Must be zero if the `class` is `InputOnly` or a `xcb_match_error_t` occurs.
    /// * `x` - The X coordinate of the new window.
    /// * `y` - The Y coordinate of the new window.
    /// * `width` - The width of the new window.
    /// * `height` - The height of the new window.
    ///
    /// # Errors
    ///
    /// * `Colormap` - TODO: reasons?
    /// * `Match` - TODO: reasons?
    /// * `Cursor` - TODO: reasons?
    /// * `Pixmap` - TODO: reasons?
    /// * `Value` - TODO: reasons?
    /// * `Window` - TODO: reasons?
    /// * `Alloc` - The X server could not allocate the requested resources (no memory?).
    ///
    /// # See
    ///
    /// * `xcb_generate_id`: function
    /// * `MapWindow`: request
    /// * `CreateNotify`: event
    fn create_window<'c, 'input>(&'c self, depth: u8, wid: Window, parent: Window, x: i16, y: i16, width: u16, height: u16, border_width: u16, class: WindowClass, visual: Visualid, value_list: &'input CreateWindowAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_window(self, depth, wid, parent, x, y, width, height, border_width, class, visual, value_list)
    }
    /// change window attributes.
    ///
    /// Changes the attributes specified by `value_mask` for the specified `window`.
    ///
    /// # Fields
    ///
    /// * `window` - The window to change.
    /// * `value_mask` -
    /// * `value_list` - Values for each of the attributes specified in the bitmask `value_mask`. The
    /// order has to correspond to the order of possible `value_mask` bits. See the
    /// example.
    ///
    /// # Errors
    ///
    /// * `Access` - TODO: reasons?
    /// * `Colormap` - TODO: reasons?
    /// * `Cursor` - TODO: reasons?
    /// * `Match` - TODO: reasons?
    /// * `Pixmap` - TODO: reasons?
    /// * `Value` - TODO: reasons?
    /// * `Window` - The specified `window` does not exist.
    fn change_window_attributes<'c, 'input>(&'c self, window: Window, value_list: &'input ChangeWindowAttributesAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_window_attributes(self, window, value_list)
    }
    /// Gets window attributes.
    ///
    /// Gets the current attributes for the specified `window`.
    ///
    /// # Fields
    ///
    /// * `window` - The window to get the attributes from.
    ///
    /// # Errors
    ///
    /// * `Window` - The specified `window` does not exist.
    /// * `Drawable` - TODO: reasons?
    fn get_window_attributes(&self, window: Window) -> Result<Cookie<'_, Self, GetWindowAttributesReply>, ConnectionError>
    {
        get_window_attributes(self, window)
    }
    /// Destroys a window.
    ///
    /// Destroys the specified window and all of its subwindows. A DestroyNotify event
    /// is generated for each destroyed window (a DestroyNotify event is first generated
    /// for any given window's inferiors). If the window was mapped, it will be
    /// automatically unmapped before destroying.
    ///
    /// Calling DestroyWindow on the root window will do nothing.
    ///
    /// # Fields
    ///
    /// * `window` - The window to destroy.
    ///
    /// # Errors
    ///
    /// * `Window` - The specified window does not exist.
    ///
    /// # See
    ///
    /// * `DestroyNotify`: event
    /// * `MapWindow`: request
    /// * `UnmapWindow`: request
    fn destroy_window(&self, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        destroy_window(self, window)
    }
    fn destroy_subwindows(&self, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        destroy_subwindows(self, window)
    }
    /// Changes a client's save set.
    ///
    /// TODO: explain what the save set is for.
    ///
    /// This function either adds or removes the specified window to the client's (your
    /// application's) save set.
    ///
    /// # Fields
    ///
    /// * `mode` - Insert to add the specified window to the save set or Delete to delete it from the save set.
    /// * `window` - The window to add or delete to/from your save set.
    ///
    /// # Errors
    ///
    /// * `Match` - You created the specified window. This does not make sense, you can only add
    /// windows created by other clients to your save set.
    /// * `Value` - You specified an invalid mode.
    /// * `Window` - The specified window does not exist.
    ///
    /// # See
    ///
    /// * `ReparentWindow`: request
    fn change_save_set(&self, mode: SetMode, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        change_save_set(self, mode, window)
    }
    /// Reparents a window.
    ///
    /// Makes the specified window a child of the specified parent window. If the
    /// window is mapped, it will automatically be unmapped before reparenting and
    /// re-mapped after reparenting. The window is placed in the stacking order on top
    /// with respect to sibling windows.
    ///
    /// After reparenting, a ReparentNotify event is generated.
    ///
    /// # Fields
    ///
    /// * `window` - The window to reparent.
    /// * `parent` - The new parent of the window.
    /// * `x` - The X position of the window within its new parent.
    /// * `y` - The Y position of the window within its new parent.
    ///
    /// # Errors
    ///
    /// * `Match` - The new parent window is not on the same screen as the old parent window.
    /// 
    /// The new parent window is the specified window or an inferior of the specified window.
    /// 
    /// The new parent is InputOnly and the window is not.
    /// 
    /// The specified window has a ParentRelative background and the new parent window is not the same depth as the specified window.
    /// * `Window` - The specified window does not exist.
    ///
    /// # See
    ///
    /// * `ReparentNotify`: event
    /// * `MapWindow`: request
    /// * `UnmapWindow`: request
    fn reparent_window(&self, window: Window, parent: Window, x: i16, y: i16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        reparent_window(self, window, parent, x, y)
    }
    /// Makes a window visible.
    ///
    /// Maps the specified window. This means making the window visible (as long as its
    /// parent is visible).
    ///
    /// This MapWindow request will be translated to a MapRequest request if a window
    /// manager is running. The window manager then decides to either map the window or
    /// not. Set the override-redirect window attribute to true if you want to bypass
    /// this mechanism.
    ///
    /// If the window manager decides to map the window (or if no window manager is
    /// running), a MapNotify event is generated.
    ///
    /// If the window becomes viewable and no earlier contents for it are remembered,
    /// the X server tiles the window with its background. If the window's background
    /// is undefined, the existing screen contents are not altered, and the X server
    /// generates zero or more Expose events.
    ///
    /// If the window type is InputOutput, an Expose event will be generated when the
    /// window becomes visible. The normal response to an Expose event should be to
    /// repaint the window.
    ///
    /// # Fields
    ///
    /// * `window` - The window to make visible.
    ///
    /// # Errors
    ///
    /// * `Match` - The specified window does not exist.
    ///
    /// # See
    ///
    /// * `MapNotify`: event
    /// * `Expose`: event
    /// * `UnmapWindow`: request
    fn map_window(&self, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        map_window(self, window)
    }
    fn map_subwindows(&self, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        map_subwindows(self, window)
    }
    /// Makes a window invisible.
    ///
    /// Unmaps the specified window. This means making the window invisible (and all
    /// its child windows).
    ///
    /// Unmapping a window leads to the `UnmapNotify` event being generated. Also,
    /// `Expose` events are generated for formerly obscured windows.
    ///
    /// # Fields
    ///
    /// * `window` - The window to make invisible.
    ///
    /// # Errors
    ///
    /// * `Window` - The specified window does not exist.
    ///
    /// # See
    ///
    /// * `UnmapNotify`: event
    /// * `Expose`: event
    /// * `MapWindow`: request
    fn unmap_window(&self, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        unmap_window(self, window)
    }
    fn unmap_subwindows(&self, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        unmap_subwindows(self, window)
    }
    /// Configures window attributes.
    ///
    /// Configures a window's size, position, border width and stacking order.
    ///
    /// # Fields
    ///
    /// * `window` - The window to configure.
    /// * `value_mask` - Bitmask of attributes to change.
    /// * `value_list` - New values, corresponding to the attributes in value_mask. The order has to
    /// correspond to the order of possible `value_mask` bits. See the example.
    ///
    /// # Errors
    ///
    /// * `Match` - You specified a Sibling without also specifying StackMode or the window is not
    /// actually a Sibling.
    /// * `Window` - The specified window does not exist. TODO: any other reason?
    /// * `Value` - TODO: reasons?
    ///
    /// # See
    ///
    /// * `MapNotify`: event
    /// * `Expose`: event
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Configures the given window to the left upper corner
    ///  * with a size of 1024x768 pixels.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *c, xcb_window_t window) {
    ///     uint16_t mask = 0;
    ///
    ///     mask |= XCB_CONFIG_WINDOW_X;
    ///     mask |= XCB_CONFIG_WINDOW_Y;
    ///     mask |= XCB_CONFIG_WINDOW_WIDTH;
    ///     mask |= XCB_CONFIG_WINDOW_HEIGHT;
    ///
    ///     const uint32_t values[] = {
    ///         0,    /* x */
    ///         0,    /* y */
    ///         1024, /* width */
    ///         768   /* height */
    ///     };
    ///
    ///     xcb_configure_window(c, window, mask, values);
    ///     xcb_flush(c);
    /// }
    /// ```
    fn configure_window<'c, 'input>(&'c self, window: Window, value_list: &'input ConfigureWindowAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        configure_window(self, window, value_list)
    }
    /// Change window stacking order.
    ///
    /// If `direction` is `XCB_CIRCULATE_RAISE_LOWEST`, the lowest mapped child (if
    /// any) will be raised to the top of the stack.
    ///
    /// If `direction` is `XCB_CIRCULATE_LOWER_HIGHEST`, the highest mapped child will
    /// be lowered to the bottom of the stack.
    ///
    /// # Fields
    ///
    /// * `direction` -
    /// * `window` - The window to raise/lower (depending on `direction`).
    ///
    /// # Errors
    ///
    /// * `Window` - The specified `window` does not exist.
    /// * `Value` - The specified `direction` is invalid.
    fn circulate_window(&self, direction: Circulate, window: Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        circulate_window(self, direction, window)
    }
    /// Get current window geometry.
    ///
    /// Gets the current geometry of the specified drawable (either `Window` or `Pixmap`).
    ///
    /// # Fields
    ///
    /// * `drawable` - The drawable (`Window` or `Pixmap`) of which the geometry will be received.
    ///
    /// # Errors
    ///
    /// * `Drawable` - TODO: reasons?
    /// * `Window` - TODO: reasons?
    ///
    /// # See
    ///
    /// * `xwininfo`: program
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Displays the x and y position of the given window.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *c, xcb_window_t window) {
    ///     xcb_get_geometry_cookie_t cookie;
    ///     xcb_get_geometry_reply_t *reply;
    ///
    ///     cookie = xcb_get_geometry(c, window);
    ///     /* ... do other work here if possible ... */
    ///     if ((reply = xcb_get_geometry_reply(c, cookie, NULL))) {
    ///         printf("This window is at %d, %d\\n", reply->x, reply->y);
    ///     }
    ///     free(reply);
    /// }
    /// ```
    fn get_geometry(&self, drawable: Drawable) -> Result<Cookie<'_, Self, GetGeometryReply>, ConnectionError>
    {
        get_geometry(self, drawable)
    }
    /// query the window tree.
    ///
    /// Gets the root window ID, parent window ID and list of children windows for the
    /// specified `window`. The children are listed in bottom-to-top stacking order.
    ///
    /// # Fields
    ///
    /// * `window` - The `window` to query.
    ///
    /// # See
    ///
    /// * `xwininfo`: program
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Displays the root, parent and children of the specified window.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *conn, xcb_window_t window) {
    ///     xcb_query_tree_cookie_t cookie;
    ///     xcb_query_tree_reply_t *reply;
    ///
    ///     cookie = xcb_query_tree(conn, window);
    ///     if ((reply = xcb_query_tree_reply(conn, cookie, NULL))) {
    ///         printf("root = 0x%08x\\n", reply->root);
    ///         printf("parent = 0x%08x\\n", reply->parent);
    ///
    ///         xcb_window_t *children = xcb_query_tree_children(reply);
    ///         for (int i = 0; i < xcb_query_tree_children_length(reply); i++)
    ///             printf("child window = 0x%08x\\n", children[i]);
    ///
    ///         free(reply);
    ///     }
    /// }
    /// ```
    fn query_tree(&self, window: Window) -> Result<Cookie<'_, Self, QueryTreeReply>, ConnectionError>
    {
        query_tree(self, window)
    }
    /// Get atom identifier by name.
    ///
    /// Retrieves the identifier (xcb_atom_t TODO) for the atom with the specified
    /// name. Atoms are used in protocols like EWMH, for example to store window titles
    /// (`_NET_WM_NAME` atom) as property of a window.
    ///
    /// If `only_if_exists` is 0, the atom will be created if it does not already exist.
    /// If `only_if_exists` is 1, `XCB_ATOM_NONE` will be returned if the atom does
    /// not yet exist.
    ///
    /// # Fields
    ///
    /// * `name_len` - The length of the following `name`.
    /// * `name` - The name of the atom.
    /// * `only_if_exists` - Return a valid atom id only if the atom already exists.
    ///
    /// # Errors
    ///
    /// * `Alloc` - TODO: reasons?
    /// * `Value` - A value other than 0 or 1 was specified for `only_if_exists`.
    ///
    /// # See
    ///
    /// * `xlsatoms`: program
    /// * `GetAtomName`: request
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Resolves the _NET_WM_NAME atom.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *c) {
    ///     xcb_intern_atom_cookie_t cookie;
    ///     xcb_intern_atom_reply_t *reply;
    ///
    ///     cookie = xcb_intern_atom(c, 0, strlen("_NET_WM_NAME"), "_NET_WM_NAME");
    ///     /* ... do other work here if possible ... */
    ///     if ((reply = xcb_intern_atom_reply(c, cookie, NULL))) {
    ///         printf("The _NET_WM_NAME atom has ID %u\n", reply->atom);
    ///         free(reply);
    ///     }
    /// }
    /// ```
    fn intern_atom<'c, 'input>(&'c self, only_if_exists: bool, name: &'input [u8]) -> Result<Cookie<'c, Self, InternAtomReply>, ConnectionError>
    {
        intern_atom(self, only_if_exists, name)
    }
    fn get_atom_name(&self, atom: Atom) -> Result<Cookie<'_, Self, GetAtomNameReply>, ConnectionError>
    {
        get_atom_name(self, atom)
    }
    /// Changes a window property.
    ///
    /// Sets or updates a property on the specified `window`. Properties are for
    /// example the window title (`WM_NAME`) or its minimum size (`WM_NORMAL_HINTS`).
    /// Protocols such as EWMH also use properties - for example EWMH defines the
    /// window title, encoded as UTF-8 string, in the `_NET_WM_NAME` property.
    ///
    /// # Fields
    ///
    /// * `window` - The window whose property you want to change.
    /// * `mode` -
    /// * `property` - The property you want to change (an atom).
    /// * `type` - The type of the property you want to change (an atom).
    /// * `format` - Specifies whether the data should be viewed as a list of 8-bit, 16-bit or
    /// 32-bit quantities. Possible values are 8, 16 and 32. This information allows
    /// the X server to correctly perform byte-swap operations as necessary.
    /// * `data_len` - Specifies the number of elements (see `format`).
    /// * `data` - The property data.
    ///
    /// # Errors
    ///
    /// * `Match` - TODO: reasons?
    /// * `Value` - TODO: reasons?
    /// * `Window` - The specified `window` does not exist.
    /// * `Atom` - `property` or `type` do not refer to a valid atom.
    /// * `Alloc` - The X server could not store the property (no memory?).
    ///
    /// # See
    ///
    /// * `InternAtom`: request
    /// * `xprop`: program
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Sets the WM_NAME property of the window to "XCB Example".
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *conn, xcb_window_t window) {
    ///     xcb_change_property(conn,
    ///         XCB_PROP_MODE_REPLACE,
    ///         window,
    ///         XCB_ATOM_WM_NAME,
    ///         XCB_ATOM_STRING,
    ///         8,
    ///         strlen("XCB Example"),
    ///         "XCB Example");
    ///     xcb_flush(conn);
    /// }
    /// ```
    fn change_property<'c, 'input, A, B>(&'c self, mode: PropMode, window: Window, property: A, type_: B, format: u8, data_len: u32, data: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    where
        A: Into<Atom>,
        B: Into<Atom>,
    {
        change_property(self, mode, window, property, type_, format, data_len, data)
    }
    fn delete_property(&self, window: Window, property: Atom) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        delete_property(self, window, property)
    }
    /// Gets a window property.
    ///
    /// Gets the specified `property` from the specified `window`. Properties are for
    /// example the window title (`WM_NAME`) or its minimum size (`WM_NORMAL_HINTS`).
    /// Protocols such as EWMH also use properties - for example EWMH defines the
    /// window title, encoded as UTF-8 string, in the `_NET_WM_NAME` property.
    ///
    /// TODO: talk about `type`
    ///
    /// TODO: talk about `delete`
    ///
    /// TODO: talk about the offset/length thing. what's a valid use case?
    ///
    /// # Fields
    ///
    /// * `window` - The window whose property you want to get.
    /// * `delete` - Whether the property should actually be deleted. For deleting a property, the
    /// specified `type` has to match the actual property type.
    /// * `property` - The property you want to get (an atom).
    /// * `type` - The type of the property you want to get (an atom).
    /// * `long_offset` - Specifies the offset (in 32-bit multiples) in the specified property where the
    /// data is to be retrieved.
    /// * `long_length` - Specifies how many 32-bit multiples of data should be retrieved (e.g. if you
    /// set `long_length` to 4, you will receive 16 bytes of data).
    ///
    /// # Errors
    ///
    /// * `Window` - The specified `window` does not exist.
    /// * `Atom` - `property` or `type` do not refer to a valid atom.
    /// * `Value` - The specified `long_offset` is beyond the actual property length (e.g. the
    /// property has a length of 3 bytes and you are setting `long_offset` to 1,
    /// resulting in a byte offset of 4).
    ///
    /// # See
    ///
    /// * `InternAtom`: request
    /// * `xprop`: program
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Prints the WM_NAME property of the window.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *c, xcb_window_t window) {
    ///     xcb_get_property_cookie_t cookie;
    ///     xcb_get_property_reply_t *reply;
    ///
    ///     /* These atoms are predefined in the X11 protocol. */
    ///     xcb_atom_t property = XCB_ATOM_WM_NAME;
    ///     xcb_atom_t type = XCB_ATOM_STRING;
    ///
    ///     // TODO: a reasonable long_length for WM_NAME?
    ///     cookie = xcb_get_property(c, 0, window, property, type, 0, 0);
    ///     if ((reply = xcb_get_property_reply(c, cookie, NULL))) {
    ///         int len = xcb_get_property_value_length(reply);
    ///         if (len == 0) {
    ///             printf("TODO\\n");
    ///             free(reply);
    ///             return;
    ///         }
    ///         printf("WM_NAME is %.*s\\n", len,
    ///                (char*)xcb_get_property_value(reply));
    ///     }
    ///     free(reply);
    /// }
    /// ```
    fn get_property<A, B>(&self, delete: bool, window: Window, property: A, type_: B, long_offset: u32, long_length: u32) -> Result<Cookie<'_, Self, GetPropertyReply>, ConnectionError>
    where
        A: Into<Atom>,
        B: Into<Atom>,
    {
        get_property(self, delete, window, property, type_, long_offset, long_length)
    }
    fn list_properties(&self, window: Window) -> Result<Cookie<'_, Self, ListPropertiesReply>, ConnectionError>
    {
        list_properties(self, window)
    }
    /// Sets the owner of a selection.
    ///
    /// Makes `window` the owner of the selection `selection` and updates the
    /// last-change time of the specified selection.
    ///
    /// TODO: briefly explain what a selection is.
    ///
    /// # Fields
    ///
    /// * `selection` - The selection.
    /// * `owner` - The new owner of the selection.
    ///
    /// The special value `XCB_NONE` means that the selection will have no owner.
    /// * `time` - Timestamp to avoid race conditions when running X over the network.
    ///
    /// The selection will not be changed if `time` is earlier than the current
    /// last-change time of the `selection` or is later than the current X server time.
    /// Otherwise, the last-change time is set to the specified time.
    ///
    /// The special value `XCB_CURRENT_TIME` will be replaced with the current server
    /// time.
    ///
    /// # Errors
    ///
    /// * `Atom` - `selection` does not refer to a valid atom.
    ///
    /// # See
    ///
    /// * `SetSelectionOwner`: request
    fn set_selection_owner<A, B>(&self, owner: A, selection: Atom, time: B) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Window>,
        B: Into<Timestamp>,
    {
        set_selection_owner(self, owner, selection, time)
    }
    /// Gets the owner of a selection.
    ///
    /// Gets the owner of the specified selection.
    ///
    /// TODO: briefly explain what a selection is.
    ///
    /// # Fields
    ///
    /// * `selection` - The selection.
    ///
    /// # Errors
    ///
    /// * `Atom` - `selection` does not refer to a valid atom.
    ///
    /// # See
    ///
    /// * `SetSelectionOwner`: request
    fn get_selection_owner(&self, selection: Atom) -> Result<Cookie<'_, Self, GetSelectionOwnerReply>, ConnectionError>
    {
        get_selection_owner(self, selection)
    }
    fn convert_selection<A, B>(&self, requestor: Window, selection: Atom, target: Atom, property: A, time: B) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Atom>,
        B: Into<Timestamp>,
    {
        convert_selection(self, requestor, selection, target, property, time)
    }
    /// send an event.
    ///
    /// Identifies the `destination` window, determines which clients should receive
    /// the specified event and ignores any active grabs.
    ///
    /// The `event` must be one of the core events or an event defined by an extension,
    /// so that the X server can correctly byte-swap the contents as necessary. The
    /// contents of `event` are otherwise unaltered and unchecked except for the
    /// `send_event` field which is forced to 'true'.
    ///
    /// # Fields
    ///
    /// * `destination` - The window to send this event to. Every client which selects any event within
    /// `event_mask` on `destination` will get the event.
    ///
    /// The special value `XCB_SEND_EVENT_DEST_POINTER_WINDOW` refers to the window
    /// that contains the mouse pointer.
    ///
    /// The special value `XCB_SEND_EVENT_DEST_ITEM_FOCUS` refers to the window which
    /// has the keyboard focus.
    /// * `event_mask` - Event_mask for determining which clients should receive the specified event.
    /// See `destination` and `propagate`.
    /// * `propagate` - If `propagate` is true and no clients have selected any event on `destination`,
    /// the destination is replaced with the closest ancestor of `destination` for
    /// which some client has selected a type in `event_mask` and for which no
    /// intervening window has that type in its do-not-propagate-mask. If no such
    /// window exists or if the window is an ancestor of the focus window and
    /// `InputFocus` was originally specified as the destination, the event is not sent
    /// to any clients. Otherwise, the event is reported to every client selecting on
    /// the final destination any of the types specified in `event_mask`.
    /// * `event` - The event to send to the specified `destination`.
    ///
    /// # Errors
    ///
    /// * `Window` - The specified `destination` window does not exist.
    /// * `Value` - The given `event` is neither a core event nor an event defined by an extension.
    ///
    /// # See
    ///
    /// * `ConfigureNotify`: event
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Tell the given window that it was configured to a size of 800x600 pixels.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *conn, xcb_window_t window) {
    ///     /* Every X11 event is 32 bytes long. Therefore, XCB will copy 32 bytes.
    ///      * In order to properly initialize these bytes, we allocate 32 bytes even
    ///      * though we only need less for an xcb_configure_notify_event_t */
    ///     xcb_configure_notify_event_t *event = calloc(32, 1);
    ///
    ///     event->event = window;
    ///     event->window = window;
    ///     event->response_type = XCB_CONFIGURE_NOTIFY;
    ///
    ///     event->x = 0;
    ///     event->y = 0;
    ///     event->width = 800;
    ///     event->height = 600;
    ///
    ///     event->border_width = 0;
    ///     event->above_sibling = XCB_NONE;
    ///     event->override_redirect = false;
    ///
    ///     xcb_send_event(conn, false, window, XCB_EVENT_MASK_STRUCTURE_NOTIFY,
    ///                    (char*)event);
    ///     xcb_flush(conn);
    ///     free(event);
    /// }
    /// ```
    fn send_event<A, B>(&self, propagate: bool, destination: A, event_mask: EventMask, event: B) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Window>,
        B: Into<[u8; 32]>,
    {
        send_event(self, propagate, destination, event_mask, event)
    }
    /// Grab the pointer.
    ///
    /// Actively grabs control of the pointer. Further pointer events are reported only to the grabbing client. Overrides any active pointer grab by this client.
    ///
    /// # Fields
    ///
    /// * `event_mask` - Specifies which pointer events are reported to the client.
    ///
    /// TODO: which values?
    /// * `confine_to` - Specifies the window to confine the pointer in (the user will not be able to
    /// move the pointer out of that window).
    ///
    /// The special value `XCB_NONE` means don't confine the pointer.
    /// * `cursor` - Specifies the cursor that should be displayed or `XCB_NONE` to not change the
    /// cursor.
    /// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
    /// reported to the `grab_window`.
    /// * `grab_window` - Specifies the window on which the pointer should be grabbed.
    /// * `time` - The time argument allows you to avoid certain circumstances that come up if
    /// applications take a long time to respond or if there are long network delays.
    /// Consider a situation where you have two applications, both of which normally
    /// grab the pointer when clicked on. If both applications specify the timestamp
    /// from the event, the second application may wake up faster and successfully grab
    /// the pointer before the first application. The first application then will get
    /// an indication that the other application grabbed the pointer before its request
    /// was processed.
    ///
    /// The special value `XCB_CURRENT_TIME` will be replaced with the current server
    /// time.
    /// * `pointer_mode` -
    /// * `keyboard_mode` -
    ///
    /// # Errors
    ///
    /// * `Value` - TODO: reasons?
    /// * `Window` - The specified `window` does not exist.
    ///
    /// # See
    ///
    /// * `GrabKeyboard`: request
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Grabs the pointer actively
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *conn, xcb_screen_t *screen, xcb_cursor_t cursor) {
    ///     xcb_grab_pointer_cookie_t cookie;
    ///     xcb_grab_pointer_reply_t *reply;
    ///
    ///     cookie = xcb_grab_pointer(
    ///         conn,
    ///         false,               /* get all pointer events specified by the following mask */
    ///         screen->root,        /* grab the root window */
    ///         XCB_NONE,            /* which events to let through */
    ///         XCB_GRAB_MODE_ASYNC, /* pointer events should continue as normal */
    ///         XCB_GRAB_MODE_ASYNC, /* keyboard mode */
    ///         XCB_NONE,            /* confine_to = in which window should the cursor stay */
    ///         cursor,              /* we change the cursor to whatever the user wanted */
    ///         XCB_CURRENT_TIME
    ///     );
    ///
    ///     if ((reply = xcb_grab_pointer_reply(conn, cookie, NULL))) {
    ///         if (reply->status == XCB_GRAB_STATUS_SUCCESS)
    ///             printf("successfully grabbed the pointer\\n");
    ///         free(reply);
    ///     }
    /// }
    /// ```
    fn grab_pointer<A, B, C>(&self, owner_events: bool, grab_window: Window, event_mask: EventMask, pointer_mode: GrabMode, keyboard_mode: GrabMode, confine_to: A, cursor: B, time: C) -> Result<Cookie<'_, Self, GrabPointerReply>, ConnectionError>
    where
        A: Into<Window>,
        B: Into<Cursor>,
        C: Into<Timestamp>,
    {
        grab_pointer(self, owner_events, grab_window, event_mask, pointer_mode, keyboard_mode, confine_to, cursor, time)
    }
    /// release the pointer.
    ///
    /// Releases the pointer and any queued events if you actively grabbed the pointer
    /// before using `xcb_grab_pointer`, `xcb_grab_button` or within a normal button
    /// press.
    ///
    /// EnterNotify and LeaveNotify events are generated.
    ///
    /// # Fields
    ///
    /// * `time` - Timestamp to avoid race conditions when running X over the network.
    ///
    /// The pointer will not be released if `time` is earlier than the
    /// last-pointer-grab time or later than the current X server time.
    /// * `name_len` - Length (in bytes) of `name`.
    /// * `name` - A pattern describing an X core font.
    ///
    /// # See
    ///
    /// * `GrabPointer`: request
    /// * `GrabButton`: request
    /// * `EnterNotify`: event
    /// * `LeaveNotify`: event
    fn ungrab_pointer<A>(&self, time: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Timestamp>,
    {
        ungrab_pointer(self, time)
    }
    /// Grab pointer button(s).
    ///
    /// This request establishes a passive grab. The pointer is actively grabbed as
    /// described in GrabPointer, the last-pointer-grab time is set to the time at
    /// which the button was pressed (as transmitted in the ButtonPress event), and the
    /// ButtonPress event is reported if all of the following conditions are true:
    ///
    /// The pointer is not grabbed and the specified button is logically pressed when
    /// the specified modifier keys are logically down, and no other buttons or
    /// modifier keys are logically down.
    ///
    /// The grab-window contains the pointer.
    ///
    /// The confine-to window (if any) is viewable.
    ///
    /// A passive grab on the same button/key combination does not exist on any
    /// ancestor of grab-window.
    ///
    /// The interpretation of the remaining arguments is the same as for GrabPointer.
    /// The active grab is terminated automatically when the logical state of the
    /// pointer has all buttons released, independent of the logical state of modifier
    /// keys. Note that the logical state of a device (as seen by means of the
    /// protocol) may lag the physical state if device event processing is frozen. This
    /// request overrides all previous passive grabs by the same client on the same
    /// button/key combinations on the same window. A modifier of AnyModifier is
    /// equivalent to issuing the request for all possible modifier combinations
    /// (including the combination of no modifiers). It is not required that all
    /// specified modifiers have currently assigned keycodes. A button of AnyButton is
    /// equivalent to issuing the request for all possible buttons. Otherwise, it is
    /// not required that the button specified currently be assigned to a physical
    /// button.
    ///
    /// An Access error is generated if some other client has already issued a
    /// GrabButton request with the same button/key combination on the same window.
    /// When using AnyModifier or AnyButton, the request fails completely (no grabs are
    /// established), and an Access error is generated if there is a conflicting grab
    /// for any combination. The request has no effect on an active grab.
    ///
    /// # Fields
    ///
    /// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
    /// reported to the `grab_window`.
    /// * `grab_window` - Specifies the window on which the pointer should be grabbed.
    /// * `event_mask` - Specifies which pointer events are reported to the client.
    ///
    /// TODO: which values?
    /// * `confine_to` - Specifies the window to confine the pointer in (the user will not be able to
    /// move the pointer out of that window).
    ///
    /// The special value `XCB_NONE` means don't confine the pointer.
    /// * `cursor` - Specifies the cursor that should be displayed or `XCB_NONE` to not change the
    /// cursor.
    /// * `modifiers` - The modifiers to grab.
    ///
    /// Using the special value `XCB_MOD_MASK_ANY` means grab the pointer with all
    /// possible modifier combinations.
    /// * `pointer_mode` -
    /// * `keyboard_mode` -
    /// * `button` -
    ///
    /// # Errors
    ///
    /// * `Access` - Another client has already issued a GrabButton with the same button/key
    /// combination on the same window.
    /// * `Value` - TODO: reasons?
    /// * `Cursor` - The specified `cursor` does not exist.
    /// * `Window` - The specified `window` does not exist.
    fn grab_button<A, B>(&self, owner_events: bool, grab_window: Window, event_mask: EventMask, pointer_mode: GrabMode, keyboard_mode: GrabMode, confine_to: A, cursor: B, button: ButtonIndex, modifiers: ModMask) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Window>,
        B: Into<Cursor>,
    {
        grab_button(self, owner_events, grab_window, event_mask, pointer_mode, keyboard_mode, confine_to, cursor, button, modifiers)
    }
    fn ungrab_button(&self, button: ButtonIndex, grab_window: Window, modifiers: ModMask) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        ungrab_button(self, button, grab_window, modifiers)
    }
    fn change_active_pointer_grab<A, B>(&self, cursor: A, time: B, event_mask: EventMask) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Cursor>,
        B: Into<Timestamp>,
    {
        change_active_pointer_grab(self, cursor, time, event_mask)
    }
    /// Grab the keyboard.
    ///
    /// Actively grabs control of the keyboard and generates FocusIn and FocusOut
    /// events. Further key events are reported only to the grabbing client.
    ///
    /// Any active keyboard grab by this client is overridden. If the keyboard is
    /// actively grabbed by some other client, `AlreadyGrabbed` is returned. If
    /// `grab_window` is not viewable, `GrabNotViewable` is returned. If the keyboard
    /// is frozen by an active grab of another client, `GrabFrozen` is returned. If the
    /// specified `time` is earlier than the last-keyboard-grab time or later than the
    /// current X server time, `GrabInvalidTime` is returned. Otherwise, the
    /// last-keyboard-grab time is set to the specified time.
    ///
    /// # Fields
    ///
    /// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
    /// reported to the `grab_window`.
    /// * `grab_window` - Specifies the window on which the pointer should be grabbed.
    /// * `time` - Timestamp to avoid race conditions when running X over the network.
    ///
    /// The special value `XCB_CURRENT_TIME` will be replaced with the current server
    /// time.
    /// * `pointer_mode` -
    /// * `keyboard_mode` -
    ///
    /// # Errors
    ///
    /// * `Value` - TODO: reasons?
    /// * `Window` - The specified `window` does not exist.
    ///
    /// # See
    ///
    /// * `GrabPointer`: request
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Grabs the keyboard actively
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *conn, xcb_screen_t *screen) {
    ///     xcb_grab_keyboard_cookie_t cookie;
    ///     xcb_grab_keyboard_reply_t *reply;
    ///
    ///     cookie = xcb_grab_keyboard(
    ///         conn,
    ///         true,                /* report events */
    ///         screen->root,        /* grab the root window */
    ///         XCB_CURRENT_TIME,
    ///         XCB_GRAB_MODE_ASYNC, /* process events as normal, do not require sync */
    ///         XCB_GRAB_MODE_ASYNC
    ///     );
    ///
    ///     if ((reply = xcb_grab_keyboard_reply(conn, cookie, NULL))) {
    ///         if (reply->status == XCB_GRAB_STATUS_SUCCESS)
    ///             printf("successfully grabbed the keyboard\\n");
    ///
    ///         free(reply);
    ///     }
    /// }
    /// ```
    fn grab_keyboard<A>(&self, owner_events: bool, grab_window: Window, time: A, pointer_mode: GrabMode, keyboard_mode: GrabMode) -> Result<Cookie<'_, Self, GrabKeyboardReply>, ConnectionError>
    where
        A: Into<Timestamp>,
    {
        grab_keyboard(self, owner_events, grab_window, time, pointer_mode, keyboard_mode)
    }
    fn ungrab_keyboard<A>(&self, time: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Timestamp>,
    {
        ungrab_keyboard(self, time)
    }
    /// Grab keyboard key(s).
    ///
    /// Establishes a passive grab on the keyboard. In the future, the keyboard is
    /// actively grabbed (as for `GrabKeyboard`), the last-keyboard-grab time is set to
    /// the time at which the key was pressed (as transmitted in the KeyPress event),
    /// and the KeyPress event is reported if all of the following conditions are true:
    ///
    /// The keyboard is not grabbed and the specified key (which can itself be a
    /// modifier key) is logically pressed when the specified modifier keys are
    /// logically down, and no other modifier keys are logically down.
    ///
    /// Either the grab_window is an ancestor of (or is) the focus window, or the
    /// grab_window is a descendant of the focus window and contains the pointer.
    ///
    /// A passive grab on the same key combination does not exist on any ancestor of
    /// grab_window.
    ///
    /// The interpretation of the remaining arguments is as for XGrabKeyboard.  The active grab is terminated
    /// automatically when the logical state of the keyboard has the specified key released (independent of the
    /// logical state of the modifier keys), at which point a KeyRelease event is reported to the grabbing window.
    ///
    /// Note that the logical state of a device (as seen by client applications) may lag the physical state if
    /// device event processing is frozen.
    ///
    /// A modifiers argument of AnyModifier is equivalent to issuing the request for all possible modifier combinations (including the combination of no modifiers).  It is not required that all modifiers specified
    /// have currently assigned KeyCodes.  A keycode argument of AnyKey is equivalent to issuing the request for
    /// all possible KeyCodes.  Otherwise, the specified keycode must be in the range specified by min_keycode
    /// and max_keycode in the connection setup, or a BadValue error results.
    ///
    /// If some other client has issued a XGrabKey with the same key combination on the same window, a BadAccess
    /// error results.  When using AnyModifier or AnyKey, the request fails completely, and a BadAccess error
    /// results (no grabs are established) if there is a conflicting grab for any combination.
    ///
    /// # Fields
    ///
    /// * `owner_events` - If 1, the `grab_window` will still get the key events. If 0, events are not
    /// reported to the `grab_window`.
    /// * `grab_window` - Specifies the window on which the key should be grabbed.
    /// * `key` - The keycode of the key to grab.
    ///
    /// The special value `XCB_GRAB_ANY` means grab any key.
    /// * `modifiers` - The modifiers to grab.
    ///
    /// Using the special value `XCB_MOD_MASK_ANY` means grab the key with all
    /// possible modifier combinations.
    /// * `pointer_mode` -
    /// * `keyboard_mode` -
    ///
    /// # Errors
    ///
    /// * `Access` - Another client has already issued a GrabKey with the same button/key
    /// combination on the same window.
    /// * `Value` - The key is not `XCB_GRAB_ANY` and not in the range specified by `min_keycode`
    /// and `max_keycode` in the connection setup.
    /// * `Window` - The specified `window` does not exist.
    ///
    /// # See
    ///
    /// * `GrabKeyboard`: request
    fn grab_key<A>(&self, owner_events: bool, grab_window: Window, modifiers: ModMask, key: A, pointer_mode: GrabMode, keyboard_mode: GrabMode) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Keycode>,
    {
        grab_key(self, owner_events, grab_window, modifiers, key, pointer_mode, keyboard_mode)
    }
    /// release a key combination.
    ///
    /// Releases the key combination on `grab_window` if you grabbed it using
    /// `xcb_grab_key` before.
    ///
    /// # Fields
    ///
    /// * `key` - The keycode of the specified key combination.
    ///
    /// Using the special value `XCB_GRAB_ANY` means releasing all possible key codes.
    /// * `grab_window` - The window on which the grabbed key combination will be released.
    /// * `modifiers` - The modifiers of the specified key combination.
    ///
    /// Using the special value `XCB_MOD_MASK_ANY` means releasing the key combination
    /// with every possible modifier combination.
    ///
    /// # Errors
    ///
    /// * `Window` - The specified `grab_window` does not exist.
    /// * `Value` - TODO: reasons?
    ///
    /// # See
    ///
    /// * `GrabKey`: request
    /// * `xev`: program
    fn ungrab_key<A>(&self, key: A, grab_window: Window, modifiers: ModMask) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Keycode>,
    {
        ungrab_key(self, key, grab_window, modifiers)
    }
    /// release queued events.
    ///
    /// Releases queued events if the client has caused a device (pointer/keyboard) to
    /// freeze due to grabbing it actively. This request has no effect if `time` is
    /// earlier than the last-grab time of the most recent active grab for this client
    /// or if `time` is later than the current X server time.
    ///
    /// # Fields
    ///
    /// * `mode` -
    /// * `time` - Timestamp to avoid race conditions when running X over the network.
    ///
    /// The special value `XCB_CURRENT_TIME` will be replaced with the current server
    /// time.
    ///
    /// # Errors
    ///
    /// * `Value` - You specified an invalid `mode`.
    fn allow_events<A>(&self, mode: Allow, time: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Timestamp>,
    {
        allow_events(self, mode, time)
    }
    fn grab_server(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        grab_server(self)
    }
    fn ungrab_server(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        ungrab_server(self)
    }
    /// get pointer coordinates.
    ///
    /// Gets the root window the pointer is logically on and the pointer coordinates
    /// relative to the root window's origin.
    ///
    /// # Fields
    ///
    /// * `window` - A window to check if the pointer is on the same screen as `window` (see the
    /// `same_screen` field in the reply).
    ///
    /// # Errors
    ///
    /// * `Window` - The specified `window` does not exist.
    fn query_pointer(&self, window: Window) -> Result<Cookie<'_, Self, QueryPointerReply>, ConnectionError>
    {
        query_pointer(self, window)
    }
    fn get_motion_events<A, B>(&self, window: Window, start: A, stop: B) -> Result<Cookie<'_, Self, GetMotionEventsReply>, ConnectionError>
    where
        A: Into<Timestamp>,
        B: Into<Timestamp>,
    {
        get_motion_events(self, window, start, stop)
    }
    fn translate_coordinates(&self, src_window: Window, dst_window: Window, src_x: i16, src_y: i16) -> Result<Cookie<'_, Self, TranslateCoordinatesReply>, ConnectionError>
    {
        translate_coordinates(self, src_window, dst_window, src_x, src_y)
    }
    /// move mouse pointer.
    ///
    /// Moves the mouse pointer to the specified position.
    ///
    /// If `src_window` is not `XCB_NONE` (TODO), the move will only take place if the
    /// pointer is inside `src_window` and within the rectangle specified by (`src_x`,
    /// `src_y`, `src_width`, `src_height`). The rectangle coordinates are relative to
    /// `src_window`.
    ///
    /// If `dst_window` is not `XCB_NONE` (TODO), the pointer will be moved to the
    /// offsets (`dst_x`, `dst_y`) relative to `dst_window`. If `dst_window` is
    /// `XCB_NONE` (TODO), the pointer will be moved by the offsets (`dst_x`, `dst_y`)
    /// relative to the current position of the pointer.
    ///
    /// # Fields
    ///
    /// * `src_window` - If `src_window` is not `XCB_NONE` (TODO), the move will only take place if the
    /// pointer is inside `src_window` and within the rectangle specified by (`src_x`,
    /// `src_y`, `src_width`, `src_height`). The rectangle coordinates are relative to
    /// `src_window`.
    /// * `dst_window` - If `dst_window` is not `XCB_NONE` (TODO), the pointer will be moved to the
    /// offsets (`dst_x`, `dst_y`) relative to `dst_window`. If `dst_window` is
    /// `XCB_NONE` (TODO), the pointer will be moved by the offsets (`dst_x`, `dst_y`)
    /// relative to the current position of the pointer.
    ///
    /// # Errors
    ///
    /// * `Window` - TODO: reasons?
    ///
    /// # See
    ///
    /// * `SetInputFocus`: request
    fn warp_pointer<A, B>(&self, src_window: A, dst_window: B, src_x: i16, src_y: i16, src_width: u16, src_height: u16, dst_x: i16, dst_y: i16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Window>,
        B: Into<Window>,
    {
        warp_pointer(self, src_window, dst_window, src_x, src_y, src_width, src_height, dst_x, dst_y)
    }
    /// Sets input focus.
    ///
    /// Changes the input focus and the last-focus-change time. If the specified `time`
    /// is earlier than the current last-focus-change time, the request is ignored (to
    /// avoid race conditions when running X over the network).
    ///
    /// A FocusIn and FocusOut event is generated when focus is changed.
    ///
    /// # Fields
    ///
    /// * `focus` - The window to focus. All keyboard events will be reported to this window. The
    /// window must be viewable (TODO), or a `xcb_match_error_t` occurs (TODO).
    ///
    /// If `focus` is `XCB_NONE` (TODO), all keyboard events are
    /// discarded until a new focus window is set.
    ///
    /// If `focus` is `XCB_POINTER_ROOT` (TODO), focus is on the root window of the
    /// screen on which the pointer is on currently.
    /// * `time` - Timestamp to avoid race conditions when running X over the network.
    ///
    /// The special value `XCB_CURRENT_TIME` will be replaced with the current server
    /// time.
    /// * `revert_to` - Specifies what happens when the `focus` window becomes unviewable (if `focus`
    /// is neither `XCB_NONE` nor `XCB_POINTER_ROOT`).
    ///
    /// # Errors
    ///
    /// * `Window` - The specified `focus` window does not exist.
    /// * `Match` - The specified `focus` window is not viewable.
    /// * `Value` - TODO: Reasons?
    ///
    /// # See
    ///
    /// * `FocusIn`: event
    /// * `FocusOut`: event
    fn set_input_focus<A, B>(&self, revert_to: InputFocus, focus: A, time: B) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Window>,
        B: Into<Timestamp>,
    {
        set_input_focus(self, revert_to, focus, time)
    }
    fn get_input_focus(&self) -> Result<Cookie<'_, Self, GetInputFocusReply>, ConnectionError>
    {
        get_input_focus(self)
    }
    fn query_keymap(&self) -> Result<Cookie<'_, Self, QueryKeymapReply>, ConnectionError>
    {
        query_keymap(self)
    }
    /// opens a font.
    ///
    /// Opens any X core font matching the given `name` (for example "-misc-fixed-*").
    ///
    /// Note that X core fonts are deprecated (but still supported) in favor of
    /// client-side rendering using Xft.
    ///
    /// # Fields
    ///
    /// * `fid` - The ID with which you will refer to the font, created by `xcb_generate_id`.
    /// * `name_len` - Length (in bytes) of `name`.
    /// * `name` - A pattern describing an X core font.
    ///
    /// # Errors
    ///
    /// * `Name` - No font matches the given `name`.
    ///
    /// # See
    ///
    /// * `xcb_generate_id`: function
    fn open_font<'c, 'input>(&'c self, fid: Font, name: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        open_font(self, fid, name)
    }
    fn close_font(&self, font: Font) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        close_font(self, font)
    }
    /// query font metrics.
    ///
    /// Queries information associated with the font.
    ///
    /// # Fields
    ///
    /// * `font` - The fontable (Font or Graphics Context) to query.
    fn query_font(&self, font: Fontable) -> Result<Cookie<'_, Self, QueryFontReply>, ConnectionError>
    {
        query_font(self, font)
    }
    /// get text extents.
    ///
    /// Query text extents from the X11 server. This request returns the bounding box
    /// of the specified 16-bit character string in the specified `font` or the font
    /// contained in the specified graphics context.
    ///
    /// `font_ascent` is set to the maximum of the ascent metrics of all characters in
    /// the string. `font_descent` is set to the maximum of the descent metrics.
    /// `overall_width` is set to the sum of the character-width metrics of all
    /// characters in the string. For each character in the string, let W be the sum of
    /// the character-width metrics of all characters preceding it in the string. Let L
    /// be the left-side-bearing metric of the character plus W. Let R be the
    /// right-side-bearing metric of the character plus W. The lbearing member is set
    /// to the minimum L of all characters in the string. The rbearing member is set to
    /// the maximum R.
    ///
    /// For fonts defined with linear indexing rather than 2-byte matrix indexing, each
    /// `xcb_char2b_t` structure is interpreted as a 16-bit number with byte1 as the
    /// most significant byte. If the font has no defined default character, undefined
    /// characters in the string are taken to have all zero metrics.
    ///
    /// Characters with all zero metrics are ignored. If the font has no defined
    /// default_char, the undefined characters in the string are also ignored.
    ///
    /// # Fields
    ///
    /// * `font` - The `font` to calculate text extents in. You can also pass a graphics context.
    /// * `string_len` - The number of characters in `string`.
    /// * `string` - The text to get text extents for.
    ///
    /// # Errors
    ///
    /// * `GContext` - The specified graphics context does not exist.
    /// * `Font` - The specified `font` does not exist.
    fn query_text_extents<'c, 'input>(&'c self, font: Fontable, string: &'input [Char2b]) -> Result<Cookie<'c, Self, QueryTextExtentsReply>, ConnectionError>
    {
        query_text_extents(self, font, string)
    }
    /// get matching font names.
    ///
    /// Gets a list of available font names which match the given `pattern`.
    ///
    /// # Fields
    ///
    /// * `pattern_len` - The length (in bytes) of `pattern`.
    /// * `pattern` - A font pattern, for example "-misc-fixed-*".
    ///
    /// The asterisk (*) is a wildcard for any number of characters. The question mark
    /// (?) is a wildcard for a single character. Use of uppercase or lowercase does
    /// not matter.
    /// * `max_names` - The maximum number of fonts to be returned.
    fn list_fonts<'c, 'input>(&'c self, max_names: u16, pattern: &'input [u8]) -> Result<Cookie<'c, Self, ListFontsReply>, ConnectionError>
    {
        list_fonts(self, max_names, pattern)
    }
    /// get matching font names and information.
    ///
    /// Gets a list of available font names which match the given `pattern`.
    ///
    /// # Fields
    ///
    /// * `pattern_len` - The length (in bytes) of `pattern`.
    /// * `pattern` - A font pattern, for example "-misc-fixed-*".
    ///
    /// The asterisk (*) is a wildcard for any number of characters. The question mark
    /// (?) is a wildcard for a single character. Use of uppercase or lowercase does
    /// not matter.
    /// * `max_names` - The maximum number of fonts to be returned.
    fn list_fonts_with_info<'c, 'input>(&'c self, max_names: u16, pattern: &'input [u8]) -> Result<ListFontsWithInfoCookie<'c, Self>, ConnectionError>
    {
        list_fonts_with_info(self, max_names, pattern)
    }
    fn set_font_path<'c, 'input>(&'c self, font: &'input [Str]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_font_path(self, font)
    }
    fn get_font_path(&self) -> Result<Cookie<'_, Self, GetFontPathReply>, ConnectionError>
    {
        get_font_path(self)
    }
    /// Creates a pixmap.
    ///
    /// Creates a pixmap. The pixmap can only be used on the same screen as `drawable`
    /// is on and only with drawables of the same `depth`.
    ///
    /// # Fields
    ///
    /// * `depth` - TODO
    /// * `pid` - The ID with which you will refer to the new pixmap, created by
    /// `xcb_generate_id`.
    /// * `drawable` - Drawable to get the screen from.
    /// * `width` - The width of the new pixmap.
    /// * `height` - The height of the new pixmap.
    ///
    /// # Errors
    ///
    /// * `Value` - TODO: reasons?
    /// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
    /// * `Alloc` - The X server could not allocate the requested resources (no memory?).
    ///
    /// # See
    ///
    /// * `xcb_generate_id`: function
    fn create_pixmap(&self, depth: u8, pid: Pixmap, drawable: Drawable, width: u16, height: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_pixmap(self, depth, pid, drawable, width, height)
    }
    /// Destroys a pixmap.
    ///
    /// Deletes the association between the pixmap ID and the pixmap. The pixmap
    /// storage will be freed when there are no more references to it.
    ///
    /// # Fields
    ///
    /// * `pixmap` - The pixmap to destroy.
    ///
    /// # Errors
    ///
    /// * `Pixmap` - The specified pixmap does not exist.
    fn free_pixmap(&self, pixmap: Pixmap) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_pixmap(self, pixmap)
    }
    /// Creates a graphics context.
    ///
    /// Creates a graphics context. The graphics context can be used with any drawable
    /// that has the same root and depth as the specified drawable.
    ///
    /// # Fields
    ///
    /// * `cid` - The ID with which you will refer to the graphics context, created by
    /// `xcb_generate_id`.
    /// * `drawable` - Drawable to get the root/depth from.
    ///
    /// # Errors
    ///
    /// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
    /// * `Match` - TODO: reasons?
    /// * `Font` - TODO: reasons?
    /// * `Pixmap` - TODO: reasons?
    /// * `Value` - TODO: reasons?
    /// * `Alloc` - The X server could not allocate the requested resources (no memory?).
    ///
    /// # See
    ///
    /// * `xcb_generate_id`: function
    fn create_gc<'c, 'input>(&'c self, cid: Gcontext, drawable: Drawable, value_list: &'input CreateGCAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_gc(self, cid, drawable, value_list)
    }
    /// change graphics context components.
    ///
    /// Changes the components specified by `value_mask` for the specified graphics context.
    ///
    /// # Fields
    ///
    /// * `gc` - The graphics context to change.
    /// * `value_mask` -
    /// * `value_list` - Values for each of the components specified in the bitmask `value_mask`. The
    /// order has to correspond to the order of possible `value_mask` bits. See the
    /// example.
    ///
    /// # Errors
    ///
    /// * `Font` - TODO: reasons?
    /// * `GContext` - TODO: reasons?
    /// * `Match` - TODO: reasons?
    /// * `Pixmap` - TODO: reasons?
    /// * `Value` - TODO: reasons?
    /// * `Alloc` - The X server could not allocate the requested resources (no memory?).
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Changes the foreground color component of the specified graphics context.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *conn, xcb_gcontext_t gc, uint32_t fg, uint32_t bg) {
    ///     /* C99 allows us to use a compact way of changing a single component: */
    ///     xcb_change_gc(conn, gc, XCB_GC_FOREGROUND, (uint32_t[]){ fg });
    ///
    ///     /* The more explicit way. Beware that the order of values is important! */
    ///     uint32_t mask = 0;
    ///     mask |= XCB_GC_FOREGROUND;
    ///     mask |= XCB_GC_BACKGROUND;
    ///
    ///     uint32_t values[] = {
    ///         fg,
    ///         bg
    ///     };
    ///     xcb_change_gc(conn, gc, mask, values);
    ///     xcb_flush(conn);
    /// }
    /// ```
    fn change_gc<'c, 'input>(&'c self, gc: Gcontext, value_list: &'input ChangeGCAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_gc(self, gc, value_list)
    }
    fn copy_gc(&self, src_gc: Gcontext, dst_gc: Gcontext, value_mask: GC) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        copy_gc(self, src_gc, dst_gc, value_mask)
    }
    fn set_dashes<'c, 'input>(&'c self, gc: Gcontext, dash_offset: u16, dashes: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_dashes(self, gc, dash_offset, dashes)
    }
    fn set_clip_rectangles<'c, 'input>(&'c self, ordering: ClipOrdering, gc: Gcontext, clip_x_origin: i16, clip_y_origin: i16, rectangles: &'input [Rectangle]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_clip_rectangles(self, ordering, gc, clip_x_origin, clip_y_origin, rectangles)
    }
    /// Destroys a graphics context.
    ///
    /// Destroys the specified `gc` and all associated storage.
    ///
    /// # Fields
    ///
    /// * `gc` - The graphics context to destroy.
    ///
    /// # Errors
    ///
    /// * `GContext` - The specified graphics context does not exist.
    fn free_gc(&self, gc: Gcontext) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_gc(self, gc)
    }
    fn clear_area(&self, exposures: bool, window: Window, x: i16, y: i16, width: u16, height: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        clear_area(self, exposures, window, x, y, width, height)
    }
    /// copy areas.
    ///
    /// Copies the specified rectangle from `src_drawable` to `dst_drawable`.
    ///
    /// # Fields
    ///
    /// * `dst_drawable` - The destination drawable (Window or Pixmap).
    /// * `src_drawable` - The source drawable (Window or Pixmap).
    /// * `gc` - The graphics context to use.
    /// * `src_x` - The source X coordinate.
    /// * `src_y` - The source Y coordinate.
    /// * `dst_x` - The destination X coordinate.
    /// * `dst_y` - The destination Y coordinate.
    /// * `width` - The width of the area to copy (in pixels).
    /// * `height` - The height of the area to copy (in pixels).
    ///
    /// # Errors
    ///
    /// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
    /// * `GContext` - The specified graphics context does not exist.
    /// * `Match` - `src_drawable` has a different root or depth than `dst_drawable`.
    fn copy_area(&self, src_drawable: Drawable, dst_drawable: Drawable, gc: Gcontext, src_x: i16, src_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        copy_area(self, src_drawable, dst_drawable, gc, src_x, src_y, dst_x, dst_y, width, height)
    }
    fn copy_plane(&self, src_drawable: Drawable, dst_drawable: Drawable, gc: Gcontext, src_x: i16, src_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16, bit_plane: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        copy_plane(self, src_drawable, dst_drawable, gc, src_x, src_y, dst_x, dst_y, width, height, bit_plane)
    }
    fn poly_point<'c, 'input>(&'c self, coordinate_mode: CoordMode, drawable: Drawable, gc: Gcontext, points: &'input [Point]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_point(self, coordinate_mode, drawable, gc, points)
    }
    /// draw lines.
    ///
    /// Draws `points_len`-1 lines between each pair of points (point[i], point[i+1])
    /// in the `points` array. The lines are drawn in the order listed in the array.
    /// They join correctly at all intermediate points, and if the first and last
    /// points coincide, the first and last lines also join correctly. For any given
    /// line, a pixel is not drawn more than once. If thin (zero line-width) lines
    /// intersect, the intersecting pixels are drawn multiple times. If wide lines
    /// intersect, the intersecting pixels are drawn only once, as though the entire
    /// request were a single, filled shape.
    ///
    /// # Fields
    ///
    /// * `drawable` - The drawable to draw the line(s) on.
    /// * `gc` - The graphics context to use.
    /// * `points_len` - The number of `xcb_point_t` structures in `points`.
    /// * `points` - An array of points.
    /// * `coordinate_mode` -
    ///
    /// # Errors
    ///
    /// * `Drawable` - TODO: reasons?
    /// * `GContext` - TODO: reasons?
    /// * `Match` - TODO: reasons?
    /// * `Value` - TODO: reasons?
    ///
    /// # Example
    ///
    /// ```text
    /// /*
    ///  * Draw a straight line.
    ///  *
    ///  */
    /// void my_example(xcb_connection_t *conn, xcb_drawable_t drawable, xcb_gcontext_t gc) {
    ///     xcb_poly_line(conn, XCB_COORD_MODE_ORIGIN, drawable, gc, 2,
    ///                   (xcb_point_t[]) { {10, 10}, {100, 10} });
    ///     xcb_flush(conn);
    /// }
    /// ```
    fn poly_line<'c, 'input>(&'c self, coordinate_mode: CoordMode, drawable: Drawable, gc: Gcontext, points: &'input [Point]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_line(self, coordinate_mode, drawable, gc, points)
    }
    /// draw lines.
    ///
    /// Draws multiple, unconnected lines. For each segment, a line is drawn between
    /// (x1, y1) and (x2, y2). The lines are drawn in the order listed in the array of
    /// `xcb_segment_t` structures and does not perform joining at coincident
    /// endpoints. For any given line, a pixel is not drawn more than once. If lines
    /// intersect, the intersecting pixels are drawn multiple times.
    ///
    /// TODO: include the xcb_segment_t data structure
    ///
    /// TODO: an example
    ///
    /// # Fields
    ///
    /// * `drawable` - A drawable (Window or Pixmap) to draw on.
    /// * `gc` - The graphics context to use.
    ///
    /// TODO: document which attributes of a gc are used
    /// * `segments_len` - The number of `xcb_segment_t` structures in `segments`.
    /// * `segments` - An array of `xcb_segment_t` structures.
    ///
    /// # Errors
    ///
    /// * `Drawable` - The specified `drawable` does not exist.
    /// * `GContext` - The specified `gc` does not exist.
    /// * `Match` - TODO: reasons?
    fn poly_segment<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, segments: &'input [Segment]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_segment(self, drawable, gc, segments)
    }
    fn poly_rectangle<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, rectangles: &'input [Rectangle]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_rectangle(self, drawable, gc, rectangles)
    }
    fn poly_arc<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, arcs: &'input [Arc]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_arc(self, drawable, gc, arcs)
    }
    fn fill_poly<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, shape: PolyShape, coordinate_mode: CoordMode, points: &'input [Point]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        fill_poly(self, drawable, gc, shape, coordinate_mode, points)
    }
    /// Fills rectangles.
    ///
    /// Fills the specified rectangle(s) in the order listed in the array. For any
    /// given rectangle, each pixel is not drawn more than once. If rectangles
    /// intersect, the intersecting pixels are drawn multiple times.
    ///
    /// # Fields
    ///
    /// * `drawable` - The drawable (Window or Pixmap) to draw on.
    /// * `gc` - The graphics context to use.
    ///
    /// The following graphics context components are used: function, plane-mask,
    /// fill-style, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
    ///
    /// The following graphics context mode-dependent components are used:
    /// foreground, background, tile, stipple, tile-stipple-x-origin, and
    /// tile-stipple-y-origin.
    /// * `rectangles_len` - The number of `xcb_rectangle_t` structures in `rectangles`.
    /// * `rectangles` - The rectangles to fill.
    ///
    /// # Errors
    ///
    /// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
    /// * `GContext` - The specified graphics context does not exist.
    /// * `Match` - TODO: reasons?
    fn poly_fill_rectangle<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, rectangles: &'input [Rectangle]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_fill_rectangle(self, drawable, gc, rectangles)
    }
    fn poly_fill_arc<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, arcs: &'input [Arc]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_fill_arc(self, drawable, gc, arcs)
    }
    fn put_image<'c, 'input>(&'c self, format: ImageFormat, drawable: Drawable, gc: Gcontext, width: u16, height: u16, dst_x: i16, dst_y: i16, left_pad: u8, depth: u8, data: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        put_image(self, format, drawable, gc, width, height, dst_x, dst_y, left_pad, depth, data)
    }
    fn get_image(&self, format: ImageFormat, drawable: Drawable, x: i16, y: i16, width: u16, height: u16, plane_mask: u32) -> Result<Cookie<'_, Self, GetImageReply>, ConnectionError>
    {
        get_image(self, format, drawable, x, y, width, height, plane_mask)
    }
    fn poly_text8<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, x: i16, y: i16, items: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_text8(self, drawable, gc, x, y, items)
    }
    fn poly_text16<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, x: i16, y: i16, items: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        poly_text16(self, drawable, gc, x, y, items)
    }
    /// Draws text.
    ///
    /// Fills the destination rectangle with the background pixel from `gc`, then
    /// paints the text with the foreground pixel from `gc`. The upper-left corner of
    /// the filled rectangle is at [x, y - font-ascent]. The width is overall-width,
    /// the height is font-ascent + font-descent. The overall-width, font-ascent and
    /// font-descent are as returned by `xcb_query_text_extents` (TODO).
    ///
    /// Note that using X core fonts is deprecated (but still supported) in favor of
    /// client-side rendering using Xft.
    ///
    /// # Fields
    ///
    /// * `drawable` - The drawable (Window or Pixmap) to draw text on.
    /// * `string_len` - The length of the `string`. Note that this parameter limited by 255 due to
    /// using 8 bits!
    /// * `string` - The string to draw. Only the first 255 characters are relevant due to the data
    /// type of `string_len`.
    /// * `x` - The x coordinate of the first character, relative to the origin of `drawable`.
    /// * `y` - The y coordinate of the first character, relative to the origin of `drawable`.
    /// * `gc` - The graphics context to use.
    ///
    /// The following graphics context components are used: plane-mask, foreground,
    /// background, font, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
    ///
    /// # Errors
    ///
    /// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
    /// * `GContext` - The specified graphics context does not exist.
    /// * `Match` - TODO: reasons?
    ///
    /// # See
    ///
    /// * `ImageText16`: request
    fn image_text8<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, x: i16, y: i16, string: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        image_text8(self, drawable, gc, x, y, string)
    }
    /// Draws text.
    ///
    /// Fills the destination rectangle with the background pixel from `gc`, then
    /// paints the text with the foreground pixel from `gc`. The upper-left corner of
    /// the filled rectangle is at [x, y - font-ascent]. The width is overall-width,
    /// the height is font-ascent + font-descent. The overall-width, font-ascent and
    /// font-descent are as returned by `xcb_query_text_extents` (TODO).
    ///
    /// Note that using X core fonts is deprecated (but still supported) in favor of
    /// client-side rendering using Xft.
    ///
    /// # Fields
    ///
    /// * `drawable` - The drawable (Window or Pixmap) to draw text on.
    /// * `string_len` - The length of the `string` in characters. Note that this parameter limited by
    /// 255 due to using 8 bits!
    /// * `string` - The string to draw. Only the first 255 characters are relevant due to the data
    /// type of `string_len`. Every character uses 2 bytes (hence the 16 in this
    /// request's name).
    /// * `x` - The x coordinate of the first character, relative to the origin of `drawable`.
    /// * `y` - The y coordinate of the first character, relative to the origin of `drawable`.
    /// * `gc` - The graphics context to use.
    ///
    /// The following graphics context components are used: plane-mask, foreground,
    /// background, font, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
    ///
    /// # Errors
    ///
    /// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
    /// * `GContext` - The specified graphics context does not exist.
    /// * `Match` - TODO: reasons?
    ///
    /// # See
    ///
    /// * `ImageText8`: request
    fn image_text16<'c, 'input>(&'c self, drawable: Drawable, gc: Gcontext, x: i16, y: i16, string: &'input [Char2b]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        image_text16(self, drawable, gc, x, y, string)
    }
    fn create_colormap(&self, alloc: ColormapAlloc, mid: Colormap, window: Window, visual: Visualid) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_colormap(self, alloc, mid, window, visual)
    }
    fn free_colormap(&self, cmap: Colormap) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_colormap(self, cmap)
    }
    fn copy_colormap_and_free(&self, mid: Colormap, src_cmap: Colormap) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        copy_colormap_and_free(self, mid, src_cmap)
    }
    fn install_colormap(&self, cmap: Colormap) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        install_colormap(self, cmap)
    }
    fn uninstall_colormap(&self, cmap: Colormap) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        uninstall_colormap(self, cmap)
    }
    fn list_installed_colormaps(&self, window: Window) -> Result<Cookie<'_, Self, ListInstalledColormapsReply>, ConnectionError>
    {
        list_installed_colormaps(self, window)
    }
    /// Allocate a color.
    ///
    /// Allocates a read-only colormap entry corresponding to the closest RGB value
    /// supported by the hardware. If you are using TrueColor, you can take a shortcut
    /// and directly calculate the color pixel value to avoid the round trip. But, for
    /// example, on 16-bit color setups (VNC), you can easily get the closest supported
    /// RGB value to the RGB value you are specifying.
    ///
    /// # Fields
    ///
    /// * `cmap` - TODO
    /// * `red` - The red value of your color.
    /// * `green` - The green value of your color.
    /// * `blue` - The blue value of your color.
    ///
    /// # Errors
    ///
    /// * `Colormap` - The specified colormap `cmap` does not exist.
    fn alloc_color(&self, cmap: Colormap, red: u16, green: u16, blue: u16) -> Result<Cookie<'_, Self, AllocColorReply>, ConnectionError>
    {
        alloc_color(self, cmap, red, green, blue)
    }
    fn alloc_named_color<'c, 'input>(&'c self, cmap: Colormap, name: &'input [u8]) -> Result<Cookie<'c, Self, AllocNamedColorReply>, ConnectionError>
    {
        alloc_named_color(self, cmap, name)
    }
    fn alloc_color_cells(&self, contiguous: bool, cmap: Colormap, colors: u16, planes: u16) -> Result<Cookie<'_, Self, AllocColorCellsReply>, ConnectionError>
    {
        alloc_color_cells(self, contiguous, cmap, colors, planes)
    }
    fn alloc_color_planes(&self, contiguous: bool, cmap: Colormap, colors: u16, reds: u16, greens: u16, blues: u16) -> Result<Cookie<'_, Self, AllocColorPlanesReply>, ConnectionError>
    {
        alloc_color_planes(self, contiguous, cmap, colors, reds, greens, blues)
    }
    fn free_colors<'c, 'input>(&'c self, cmap: Colormap, plane_mask: u32, pixels: &'input [u32]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        free_colors(self, cmap, plane_mask, pixels)
    }
    fn store_colors<'c, 'input>(&'c self, cmap: Colormap, items: &'input [Coloritem]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        store_colors(self, cmap, items)
    }
    fn store_named_color<'c, 'input>(&'c self, flags: ColorFlag, cmap: Colormap, pixel: u32, name: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        store_named_color(self, flags, cmap, pixel, name)
    }
    fn query_colors<'c, 'input>(&'c self, cmap: Colormap, pixels: &'input [u32]) -> Result<Cookie<'c, Self, QueryColorsReply>, ConnectionError>
    {
        query_colors(self, cmap, pixels)
    }
    fn lookup_color<'c, 'input>(&'c self, cmap: Colormap, name: &'input [u8]) -> Result<Cookie<'c, Self, LookupColorReply>, ConnectionError>
    {
        lookup_color(self, cmap, name)
    }
    fn create_cursor<A>(&self, cid: Cursor, source: Pixmap, mask: A, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16, x: u16, y: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Pixmap>,
    {
        create_cursor(self, cid, source, mask, fore_red, fore_green, fore_blue, back_red, back_green, back_blue, x, y)
    }
    /// create cursor.
    ///
    /// Creates a cursor from a font glyph. X provides a set of standard cursor shapes
    /// in a special font named cursor. Applications are encouraged to use this
    /// interface for their cursors because the font can be customized for the
    /// individual display type.
    ///
    /// All pixels which are set to 1 in the source will use the foreground color (as
    /// specified by `fore_red`, `fore_green` and `fore_blue`). All pixels set to 0
    /// will use the background color (as specified by `back_red`, `back_green` and
    /// `back_blue`).
    ///
    /// # Fields
    ///
    /// * `cid` - The ID with which you will refer to the cursor, created by `xcb_generate_id`.
    /// * `source_font` - In which font to look for the cursor glyph.
    /// * `mask_font` - In which font to look for the mask glyph.
    /// * `source_char` - The glyph of `source_font` to use.
    /// * `mask_char` - The glyph of `mask_font` to use as a mask: Pixels which are set to 1 define
    /// which source pixels are displayed. All pixels which are set to 0 are not
    /// displayed.
    /// * `fore_red` - The red value of the foreground color.
    /// * `fore_green` - The green value of the foreground color.
    /// * `fore_blue` - The blue value of the foreground color.
    /// * `back_red` - The red value of the background color.
    /// * `back_green` - The green value of the background color.
    /// * `back_blue` - The blue value of the background color.
    ///
    /// # Errors
    ///
    /// * `Alloc` - The X server could not allocate the requested resources (no memory?).
    /// * `Font` - The specified `source_font` or `mask_font` does not exist.
    /// * `Value` - Either `source_char` or `mask_char` are not defined in `source_font` or `mask_font`, respectively.
    fn create_glyph_cursor<A>(&self, cid: Cursor, source_font: Font, mask_font: A, source_char: u16, mask_char: u16, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Font>,
    {
        create_glyph_cursor(self, cid, source_font, mask_font, source_char, mask_char, fore_red, fore_green, fore_blue, back_red, back_green, back_blue)
    }
    /// Deletes a cursor.
    ///
    /// Deletes the association between the cursor resource ID and the specified
    /// cursor. The cursor is freed when no other resource references it.
    ///
    /// # Fields
    ///
    /// * `cursor` - The cursor to destroy.
    ///
    /// # Errors
    ///
    /// * `Cursor` - The specified cursor does not exist.
    fn free_cursor(&self, cursor: Cursor) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_cursor(self, cursor)
    }
    fn recolor_cursor(&self, cursor: Cursor, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        recolor_cursor(self, cursor, fore_red, fore_green, fore_blue, back_red, back_green, back_blue)
    }
    fn query_best_size(&self, class: QueryShapeOf, drawable: Drawable, width: u16, height: u16) -> Result<Cookie<'_, Self, QueryBestSizeReply>, ConnectionError>
    {
        query_best_size(self, class, drawable, width, height)
    }
    /// check if extension is present.
    ///
    /// Determines if the specified extension is present on this X11 server.
    ///
    /// Every extension has a unique `major_opcode` to identify requests, the minor
    /// opcodes and request formats are extension-specific. If the extension provides
    /// events and errors, the `first_event` and `first_error` fields in the reply are
    /// set accordingly.
    ///
    /// There should rarely be a need to use this request directly, XCB provides the
    /// `xcb_get_extension_data` function instead.
    ///
    /// # Fields
    ///
    /// * `name_len` - The length of `name` in bytes.
    /// * `name` - The name of the extension to query, for example "RANDR". This is case
    /// sensitive!
    ///
    /// # See
    ///
    /// * `xdpyinfo`: program
    /// * `xcb_get_extension_data`: function
    fn query_extension<'c, 'input>(&'c self, name: &'input [u8]) -> Result<Cookie<'c, Self, QueryExtensionReply>, ConnectionError>
    {
        query_extension(self, name)
    }
    fn list_extensions(&self) -> Result<Cookie<'_, Self, ListExtensionsReply>, ConnectionError>
    {
        list_extensions(self)
    }
    fn change_keyboard_mapping<'c, 'input>(&'c self, keycode_count: u8, first_keycode: Keycode, keysyms_per_keycode: u8, keysyms: &'input [Keysym]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_keyboard_mapping(self, keycode_count, first_keycode, keysyms_per_keycode, keysyms)
    }
    fn get_keyboard_mapping(&self, first_keycode: Keycode, count: u8) -> Result<Cookie<'_, Self, GetKeyboardMappingReply>, ConnectionError>
    {
        get_keyboard_mapping(self, first_keycode, count)
    }
    fn change_keyboard_control<'c, 'input>(&'c self, value_list: &'input ChangeKeyboardControlAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_keyboard_control(self, value_list)
    }
    fn get_keyboard_control(&self) -> Result<Cookie<'_, Self, GetKeyboardControlReply>, ConnectionError>
    {
        get_keyboard_control(self)
    }
    fn bell(&self, percent: i8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        bell(self, percent)
    }
    fn change_pointer_control(&self, acceleration_numerator: i16, acceleration_denominator: i16, threshold: i16, do_acceleration: bool, do_threshold: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        change_pointer_control(self, acceleration_numerator, acceleration_denominator, threshold, do_acceleration, do_threshold)
    }
    fn get_pointer_control(&self) -> Result<Cookie<'_, Self, GetPointerControlReply>, ConnectionError>
    {
        get_pointer_control(self)
    }
    fn set_screen_saver(&self, timeout: i16, interval: i16, prefer_blanking: Blanking, allow_exposures: Exposures) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_screen_saver(self, timeout, interval, prefer_blanking, allow_exposures)
    }
    fn get_screen_saver(&self) -> Result<Cookie<'_, Self, GetScreenSaverReply>, ConnectionError>
    {
        get_screen_saver(self)
    }
    fn change_hosts<'c, 'input>(&'c self, mode: HostMode, family: Family, address: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_hosts(self, mode, family, address)
    }
    fn list_hosts(&self) -> Result<Cookie<'_, Self, ListHostsReply>, ConnectionError>
    {
        list_hosts(self)
    }
    fn set_access_control(&self, mode: AccessControl) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_access_control(self, mode)
    }
    fn set_close_down_mode(&self, mode: CloseDown) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_close_down_mode(self, mode)
    }
    /// kills a client.
    ///
    /// Forces a close down of the client that created the specified `resource`.
    ///
    /// # Fields
    ///
    /// * `resource` - Any resource belonging to the client (for example a Window), used to identify
    /// the client connection.
    ///
    /// The special value of `XCB_KILL_ALL_TEMPORARY`, the resources of all clients
    /// that have terminated in `RetainTemporary` (TODO) are destroyed.
    ///
    /// # Errors
    ///
    /// * `Value` - The specified `resource` does not exist.
    ///
    /// # See
    ///
    /// * `xkill`: program
    fn kill_client<A>(&self, resource: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<u32>,
    {
        kill_client(self, resource)
    }
    fn rotate_properties<'c, 'input>(&'c self, window: Window, delta: i16, atoms: &'input [Atom]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        rotate_properties(self, window, delta, atoms)
    }
    fn force_screen_saver(&self, mode: ScreenSaver) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        force_screen_saver(self, mode)
    }
    fn set_pointer_mapping<'c, 'input>(&'c self, map: &'input [u8]) -> Result<Cookie<'c, Self, SetPointerMappingReply>, ConnectionError>
    {
        set_pointer_mapping(self, map)
    }
    fn get_pointer_mapping(&self) -> Result<Cookie<'_, Self, GetPointerMappingReply>, ConnectionError>
    {
        get_pointer_mapping(self)
    }
    fn set_modifier_mapping<'c, 'input>(&'c self, keycodes: &'input [Keycode]) -> Result<Cookie<'c, Self, SetModifierMappingReply>, ConnectionError>
    {
        set_modifier_mapping(self, keycodes)
    }
    fn get_modifier_mapping(&self) -> Result<Cookie<'_, Self, GetModifierMappingReply>, ConnectionError>
    {
        get_modifier_mapping(self)
    }
    fn no_operation(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        no_operation(self)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}

/// A RAII-like wrapper around a [Pixmap].
///
/// Instances of this struct represent a Pixmap that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct PixmapWrapper<C: RequestConnection>(C, Pixmap);

impl<C: RequestConnection> PixmapWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_pixmap(conn: C, id: Pixmap) -> Self {
        PixmapWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn pixmap(&self) -> Pixmap {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_pixmap(self) -> Pixmap {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> PixmapWrapper<&'c C>
{
    /// Create a new Pixmap and return a Pixmap wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_pixmap] that allocates an id for the Pixmap.
    /// This function returns the resulting `PixmapWrapper` that owns the created Pixmap and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_pixmap].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_pixmap].
    pub fn create_pixmap_and_get_cookie(conn: &'c C, depth: u8, drawable: Drawable, width: u16, height: u16) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let pid = conn.generate_id()?;
        let cookie = create_pixmap(conn, depth, pid, drawable, width, height)?;
        Ok((Self::for_pixmap(conn, pid), cookie))
    }
}
impl<C: X11Connection> PixmapWrapper<C>
{
    /// Create a new Pixmap and return a Pixmap wrapper
    ///
    /// This is a thin wrapper around [create_pixmap] that allocates an id for the Pixmap.
    /// This function returns the resulting `PixmapWrapper` that owns the created Pixmap and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_pixmap].
    pub fn create_pixmap(conn: C, depth: u8, drawable: Drawable, width: u16, height: u16) -> Result<Self, ReplyOrIdError>
    {
        let pid = conn.generate_id()?;
        let _ = create_pixmap(&conn, depth, pid, drawable, width, height)?;
        Ok(Self::for_pixmap(conn, pid))
    }
}

impl<C: RequestConnection> From<&PixmapWrapper<C>> for Pixmap {
    fn from(from: &PixmapWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for PixmapWrapper<C> {
    fn drop(&mut self) {
        let _ = free_pixmap(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Window].
///
/// Instances of this struct represent a Window that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct WindowWrapper<C: RequestConnection>(C, Window);

impl<C: RequestConnection> WindowWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_window(conn: C, id: Window) -> Self {
        WindowWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn window(&self) -> Window {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_window(self) -> Window {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> WindowWrapper<&'c C>
{
    /// Create a new Window and return a Window wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_window] that allocates an id for the Window.
    /// This function returns the resulting `WindowWrapper` that owns the created Window and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_window].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_window].
    pub fn create_window_and_get_cookie(conn: &'c C, depth: u8, parent: Window, x: i16, y: i16, width: u16, height: u16, border_width: u16, class: WindowClass, visual: Visualid, value_list: &CreateWindowAux) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let wid = conn.generate_id()?;
        let cookie = create_window(conn, depth, wid, parent, x, y, width, height, border_width, class, visual, value_list)?;
        Ok((Self::for_window(conn, wid), cookie))
    }
}
impl<C: X11Connection> WindowWrapper<C>
{
    /// Create a new Window and return a Window wrapper
    ///
    /// This is a thin wrapper around [create_window] that allocates an id for the Window.
    /// This function returns the resulting `WindowWrapper` that owns the created Window and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_window].
    pub fn create_window(conn: C, depth: u8, parent: Window, x: i16, y: i16, width: u16, height: u16, border_width: u16, class: WindowClass, visual: Visualid, value_list: &CreateWindowAux) -> Result<Self, ReplyOrIdError>
    {
        let wid = conn.generate_id()?;
        let _ = create_window(&conn, depth, wid, parent, x, y, width, height, border_width, class, visual, value_list)?;
        Ok(Self::for_window(conn, wid))
    }
}

impl<C: RequestConnection> From<&WindowWrapper<C>> for Window {
    fn from(from: &WindowWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for WindowWrapper<C> {
    fn drop(&mut self) {
        let _ = destroy_window(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Font].
///
/// Instances of this struct represent a Font that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct FontWrapper<C: RequestConnection>(C, Font);

impl<C: RequestConnection> FontWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_font(conn: C, id: Font) -> Self {
        FontWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn font(&self) -> Font {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_font(self) -> Font {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> FontWrapper<&'c C>
{
    /// Create a new Font and return a Font wrapper and a cookie.
    ///
    /// This is a thin wrapper around [open_font] that allocates an id for the Font.
    /// This function returns the resulting `FontWrapper` that owns the created Font and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [open_font].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [open_font].
    pub fn open_font_and_get_cookie(conn: &'c C, name: &[u8]) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let fid = conn.generate_id()?;
        let cookie = open_font(conn, fid, name)?;
        Ok((Self::for_font(conn, fid), cookie))
    }
}
impl<C: X11Connection> FontWrapper<C>
{
    /// Create a new Font and return a Font wrapper
    ///
    /// This is a thin wrapper around [open_font] that allocates an id for the Font.
    /// This function returns the resulting `FontWrapper` that owns the created Font and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [open_font].
    pub fn open_font(conn: C, name: &[u8]) -> Result<Self, ReplyOrIdError>
    {
        let fid = conn.generate_id()?;
        let _ = open_font(&conn, fid, name)?;
        Ok(Self::for_font(conn, fid))
    }
}

impl<C: RequestConnection> From<&FontWrapper<C>> for Font {
    fn from(from: &FontWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for FontWrapper<C> {
    fn drop(&mut self) {
        let _ = close_font(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Gcontext].
///
/// Instances of this struct represent a Gcontext that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct GcontextWrapper<C: RequestConnection>(C, Gcontext);

impl<C: RequestConnection> GcontextWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_gcontext(conn: C, id: Gcontext) -> Self {
        GcontextWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn gcontext(&self) -> Gcontext {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_gcontext(self) -> Gcontext {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> GcontextWrapper<&'c C>
{
    /// Create a new Gcontext and return a Gcontext wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_gc] that allocates an id for the Gcontext.
    /// This function returns the resulting `GcontextWrapper` that owns the created Gcontext and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_gc].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_gc].
    pub fn create_gc_and_get_cookie(conn: &'c C, drawable: Drawable, value_list: &CreateGCAux) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let cid = conn.generate_id()?;
        let cookie = create_gc(conn, cid, drawable, value_list)?;
        Ok((Self::for_gcontext(conn, cid), cookie))
    }
}
impl<C: X11Connection> GcontextWrapper<C>
{
    /// Create a new Gcontext and return a Gcontext wrapper
    ///
    /// This is a thin wrapper around [create_gc] that allocates an id for the Gcontext.
    /// This function returns the resulting `GcontextWrapper` that owns the created Gcontext and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_gc].
    pub fn create_gc(conn: C, drawable: Drawable, value_list: &CreateGCAux) -> Result<Self, ReplyOrIdError>
    {
        let cid = conn.generate_id()?;
        let _ = create_gc(&conn, cid, drawable, value_list)?;
        Ok(Self::for_gcontext(conn, cid))
    }
}

impl<C: RequestConnection> From<&GcontextWrapper<C>> for Gcontext {
    fn from(from: &GcontextWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for GcontextWrapper<C> {
    fn drop(&mut self) {
        let _ = free_gc(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Colormap].
///
/// Instances of this struct represent a Colormap that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct ColormapWrapper<C: RequestConnection>(C, Colormap);

impl<C: RequestConnection> ColormapWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_colormap(conn: C, id: Colormap) -> Self {
        ColormapWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn colormap(&self) -> Colormap {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_colormap(self) -> Colormap {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> ColormapWrapper<&'c C>
{
    /// Create a new Colormap and return a Colormap wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_colormap] that allocates an id for the Colormap.
    /// This function returns the resulting `ColormapWrapper` that owns the created Colormap and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_colormap].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_colormap].
    pub fn create_colormap_and_get_cookie(conn: &'c C, alloc: ColormapAlloc, window: Window, visual: Visualid) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let mid = conn.generate_id()?;
        let cookie = create_colormap(conn, alloc, mid, window, visual)?;
        Ok((Self::for_colormap(conn, mid), cookie))
    }
}
impl<C: X11Connection> ColormapWrapper<C>
{
    /// Create a new Colormap and return a Colormap wrapper
    ///
    /// This is a thin wrapper around [create_colormap] that allocates an id for the Colormap.
    /// This function returns the resulting `ColormapWrapper` that owns the created Colormap and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_colormap].
    pub fn create_colormap(conn: C, alloc: ColormapAlloc, window: Window, visual: Visualid) -> Result<Self, ReplyOrIdError>
    {
        let mid = conn.generate_id()?;
        let _ = create_colormap(&conn, alloc, mid, window, visual)?;
        Ok(Self::for_colormap(conn, mid))
    }
}

impl<C: RequestConnection> From<&ColormapWrapper<C>> for Colormap {
    fn from(from: &ColormapWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for ColormapWrapper<C> {
    fn drop(&mut self) {
        let _ = free_colormap(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Cursor].
///
/// Instances of this struct represent a Cursor that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct CursorWrapper<C: RequestConnection>(C, Cursor);

impl<C: RequestConnection> CursorWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_cursor(conn: C, id: Cursor) -> Self {
        CursorWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn cursor(&self) -> Cursor {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_cursor(self) -> Cursor {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> CursorWrapper<&'c C>
{
    /// Create a new Cursor and return a Cursor wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_cursor] that allocates an id for the Cursor.
    /// This function returns the resulting `CursorWrapper` that owns the created Cursor and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_cursor].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_cursor].
    pub fn create_cursor_and_get_cookie<A>(conn: &'c C, source: Pixmap, mask: A, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16, x: u16, y: u16) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    where
        A: Into<Pixmap>,
    {
        let cid = conn.generate_id()?;
        let cookie = create_cursor(conn, cid, source, mask, fore_red, fore_green, fore_blue, back_red, back_green, back_blue, x, y)?;
        Ok((Self::for_cursor(conn, cid), cookie))
    }
}
impl<C: X11Connection> CursorWrapper<C>
{
    /// Create a new Cursor and return a Cursor wrapper
    ///
    /// This is a thin wrapper around [create_cursor] that allocates an id for the Cursor.
    /// This function returns the resulting `CursorWrapper` that owns the created Cursor and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_cursor].
    pub fn create_cursor<A>(conn: C, source: Pixmap, mask: A, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16, x: u16, y: u16) -> Result<Self, ReplyOrIdError>
    where
        A: Into<Pixmap>,
    {
        let cid = conn.generate_id()?;
        let _ = create_cursor(&conn, cid, source, mask, fore_red, fore_green, fore_blue, back_red, back_green, back_blue, x, y)?;
        Ok(Self::for_cursor(conn, cid))
    }
}

impl<'c, C: X11Connection> CursorWrapper<&'c C>
{
    /// Create a new Cursor and return a Cursor wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_glyph_cursor] that allocates an id for the Cursor.
    /// This function returns the resulting `CursorWrapper` that owns the created Cursor and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_glyph_cursor].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_glyph_cursor].
    pub fn create_glyph_cursor_and_get_cookie<A>(conn: &'c C, source_font: Font, mask_font: A, source_char: u16, mask_char: u16, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    where
        A: Into<Font>,
    {
        let cid = conn.generate_id()?;
        let cookie = create_glyph_cursor(conn, cid, source_font, mask_font, source_char, mask_char, fore_red, fore_green, fore_blue, back_red, back_green, back_blue)?;
        Ok((Self::for_cursor(conn, cid), cookie))
    }
}
impl<C: X11Connection> CursorWrapper<C>
{
    /// Create a new Cursor and return a Cursor wrapper
    ///
    /// This is a thin wrapper around [create_glyph_cursor] that allocates an id for the Cursor.
    /// This function returns the resulting `CursorWrapper` that owns the created Cursor and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_glyph_cursor].
    pub fn create_glyph_cursor<A>(conn: C, source_font: Font, mask_font: A, source_char: u16, mask_char: u16, fore_red: u16, fore_green: u16, fore_blue: u16, back_red: u16, back_green: u16, back_blue: u16) -> Result<Self, ReplyOrIdError>
    where
        A: Into<Font>,
    {
        let cid = conn.generate_id()?;
        let _ = create_glyph_cursor(&conn, cid, source_font, mask_font, source_char, mask_char, fore_red, fore_green, fore_blue, back_red, back_green, back_blue)?;
        Ok(Self::for_cursor(conn, cid))
    }
}

impl<C: RequestConnection> From<&CursorWrapper<C>> for Cursor {
    fn from(from: &CursorWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for CursorWrapper<C> {
    fn drop(&mut self) {
        let _ = free_cursor(&self.0, self.1);
    }
}
