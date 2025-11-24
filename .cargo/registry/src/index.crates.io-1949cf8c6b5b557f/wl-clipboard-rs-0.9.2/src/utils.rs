//! Helper functions.

use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::{env, io};

use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{
    event_created_child, ConnectError, Connection, Dispatch, DispatchError, Proxy,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1;

use crate::data_control::{
    impl_dispatch_device, impl_dispatch_manager, impl_dispatch_offer, Manager,
};

/// Checks if the given MIME type represents plain text.
///
/// # Examples
///
/// ```
/// use wl_clipboard_rs::utils::is_text;
///
/// assert!(is_text("text/plain"));
/// assert!(!is_text("application/octet-stream"));
/// ```
pub fn is_text(mime_type: &str) -> bool {
    match mime_type {
        "TEXT" | "STRING" | "UTF8_STRING" => true,
        x if x.starts_with("text/") => true,
        _ => false,
    }
}

struct PrimarySelectionState {
    // Any seat that we get from the compositor.
    seat: Option<WlSeat>,
    clipboard_manager: Option<Manager>,
    saw_zwlr_v1: bool,
    got_primary_selection: bool,
}

impl Dispatch<WlRegistry, ()> for PrimarySelectionState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: <WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == WlSeat::interface().name && version >= 2 && state.seat.is_none() {
                let seat = registry.bind(name, 2, qh, ());
                state.seat = Some(seat);
            }

            if state.clipboard_manager.is_none() {
                if interface == ZwlrDataControlManagerV1::interface().name {
                    if version == 1 {
                        state.saw_zwlr_v1 = true;
                    } else {
                        let manager = registry.bind(name, 2, qh, ());
                        state.clipboard_manager = Some(Manager::Zwlr(manager));
                    }
                }

                if interface == ExtDataControlManagerV1::interface().name {
                    let manager = registry.bind(name, 1, qh, ());
                    state.clipboard_manager = Some(Manager::Ext(manager));
                }
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for PrimarySelectionState {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: <WlSeat as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl_dispatch_manager!(PrimarySelectionState);

impl_dispatch_device!(PrimarySelectionState, (), |state: &mut Self, event, _| {
    if let Event::PrimarySelection { id: _ } = event {
        state.got_primary_selection = true;
    }
});

impl_dispatch_offer!(PrimarySelectionState);

/// Errors that can occur when checking whether the primary selection is supported.
#[derive(thiserror::Error, Debug)]
pub enum PrimarySelectionCheckError {
    #[error("There are no seats")]
    NoSeats,

    #[error("Couldn't open the provided Wayland socket")]
    SocketOpenError(#[source] io::Error),

    #[error("Couldn't connect to the Wayland compositor")]
    WaylandConnection(#[source] ConnectError),

    #[error("Wayland compositor communication error")]
    WaylandCommunication(#[source] DispatchError),

    #[error(
        "A required Wayland protocol (ext-data-control, or wlr-data-control version 1) \
         is not supported by the compositor"
    )]
    MissingProtocol,
}

/// Checks if the compositor supports the primary selection.
///
/// # Examples
///
/// ```no_run
/// # extern crate wl_clipboard_rs;
/// # fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// use wl_clipboard_rs::utils::{is_primary_selection_supported, PrimarySelectionCheckError};
///
/// match is_primary_selection_supported() {
///     Ok(supported) => {
///         // We have our definitive result. False means that ext/wlr-data-control is present
///         // and did not signal the primary selection support, or that only wlr-data-control
///         // version 1 is present (which does not support primary selection).
///     },
///     Err(PrimarySelectionCheckError::NoSeats) => {
///         // Impossible to give a definitive result. Primary selection may or may not be
///         // supported.
///
///         // The required protocol (ext-data-control, or wlr-data-control version 2) is there,
///         // but there are no seats. Unfortunately, at least one seat is needed to check for the
///         // primary clipboard support.
///     },
///     Err(PrimarySelectionCheckError::MissingProtocol) => {
///         // The data-control protocol (required for wl-clipboard-rs operation) is not
///         // supported by the compositor.
///     },
///     Err(_) => {
///         // Some communication error occurred.
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn is_primary_selection_supported() -> Result<bool, PrimarySelectionCheckError> {
    is_primary_selection_supported_internal(None)
}

pub(crate) fn is_primary_selection_supported_internal(
    socket_name: Option<OsString>,
) -> Result<bool, PrimarySelectionCheckError> {
    // Connect to the Wayland compositor.
    let conn = match socket_name {
        Some(name) => {
            let mut socket_path = env::var_os("XDG_RUNTIME_DIR")
                .map(Into::<PathBuf>::into)
                .ok_or(ConnectError::NoCompositor)
                .map_err(PrimarySelectionCheckError::WaylandConnection)?;
            if !socket_path.is_absolute() {
                return Err(PrimarySelectionCheckError::WaylandConnection(
                    ConnectError::NoCompositor,
                ));
            }
            socket_path.push(name);

            let stream = UnixStream::connect(socket_path)
                .map_err(PrimarySelectionCheckError::SocketOpenError)?;
            Connection::from_socket(stream)
        }
        None => Connection::connect_to_env(),
    }
    .map_err(PrimarySelectionCheckError::WaylandConnection)?;
    let display = conn.display();

    let mut queue = conn.new_event_queue();
    let qh = queue.handle();

    let mut state = PrimarySelectionState {
        seat: None,
        clipboard_manager: None,
        saw_zwlr_v1: false,
        got_primary_selection: false,
    };

    // Retrieve the global interfaces.
    let _registry = display.get_registry(&qh, ());
    queue
        .roundtrip(&mut state)
        .map_err(PrimarySelectionCheckError::WaylandCommunication)?;

    // If data control is present but is version 1, then return false as version 1 does not support
    // primary clipboard.
    if state.clipboard_manager.is_none() && state.saw_zwlr_v1 {
        return Ok(false);
    }

    // Verify that we got the clipboard manager.
    let Some(ref clipboard_manager) = state.clipboard_manager else {
        return Err(PrimarySelectionCheckError::MissingProtocol);
    };

    // Check if there are no seats.
    let Some(ref seat) = state.seat else {
        return Err(PrimarySelectionCheckError::NoSeats);
    };

    clipboard_manager.get_data_device(seat, &qh, ());

    queue
        .roundtrip(&mut state)
        .map_err(PrimarySelectionCheckError::WaylandCommunication)?;

    Ok(state.got_primary_selection)
}
