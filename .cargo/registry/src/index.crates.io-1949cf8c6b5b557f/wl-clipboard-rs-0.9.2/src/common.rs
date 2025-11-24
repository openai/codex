use std::collections::HashMap;
use std::ffi::OsString;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::{env, io};

use wayland_backend::client::WaylandError;
use wayland_client::globals::{registry_queue_init, GlobalError, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::{ConnectError, Connection, Dispatch, EventQueue, Proxy};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1;

use crate::data_control::Manager;
use crate::seat_data::SeatData;

pub struct State {
    pub seats: HashMap<WlSeat, SeatData>,
    pub clipboard_manager: Manager,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Couldn't open the provided Wayland socket")]
    SocketOpenError(#[source] io::Error),

    #[error("Couldn't connect to the Wayland compositor")]
    WaylandConnection(#[source] ConnectError),

    #[error("Wayland compositor communication error")]
    WaylandCommunication(#[source] WaylandError),

    #[error(
        "A required Wayland protocol ({name} version {version}) is not supported by the compositor"
    )]
    MissingProtocol { name: &'static str, version: u32 },
}

impl<S> Dispatch<WlSeat, (), S> for State
where
    S: Dispatch<WlSeat, ()> + AsMut<State>,
{
    fn event(
        parent: &mut S,
        seat: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<S>,
    ) {
        let state = parent.as_mut();

        if let wl_seat::Event::Name { name } = event {
            state.seats.get_mut(seat).unwrap().set_name(name);
        }
    }
}

pub fn initialize<S>(
    primary: bool,
    socket_name: Option<OsString>,
) -> Result<(EventQueue<S>, State), Error>
where
    S: Dispatch<WlRegistry, GlobalListContents> + 'static,
    S: Dispatch<ZwlrDataControlManagerV1, ()>,
    S: Dispatch<ExtDataControlManagerV1, ()>,
    S: Dispatch<WlSeat, ()>,
    S: AsMut<State>,
{
    // Connect to the Wayland compositor.
    let conn = match socket_name {
        Some(name) => {
            let mut socket_path = env::var_os("XDG_RUNTIME_DIR")
                .map(Into::<PathBuf>::into)
                .ok_or(ConnectError::NoCompositor)
                .map_err(Error::WaylandConnection)?;
            if !socket_path.is_absolute() {
                return Err(Error::WaylandConnection(ConnectError::NoCompositor));
            }
            socket_path.push(name);

            let stream = UnixStream::connect(socket_path).map_err(Error::SocketOpenError)?;
            Connection::from_socket(stream)
        }
        None => Connection::connect_to_env(),
    }
    .map_err(Error::WaylandConnection)?;

    // Retrieve the global interfaces.
    let (globals, queue) =
        registry_queue_init::<S>(&conn).map_err(|err| match err {
                                           GlobalError::Backend(err) => Error::WaylandCommunication(err),
                                           GlobalError::InvalidId(err) => panic!("How's this possible? \
                                                                                  Is there no wl_registry? \
                                                                                  {:?}",
                                                                                 err),
                                       })?;
    let qh = &queue.handle();

    // Verify that we got the clipboard manager.
    let ext_manager = globals.bind(qh, 1..=1, ()).ok().map(Manager::Ext);

    let wlr_v = if primary { 2 } else { 1 };
    let wlr_manager = || globals.bind(qh, wlr_v..=wlr_v, ()).ok().map(Manager::Zwlr);

    let clipboard_manager = match ext_manager.or_else(wlr_manager) {
        Some(manager) => manager,
        None => {
            return Err(Error::MissingProtocol {
                name: "ext-data-control, or wlr-data-control",
                version: wlr_v,
            })
        }
    };

    let registry = globals.registry();
    let seats = globals.contents().with_list(|globals| {
        globals
            .iter()
            .filter(|global| global.interface == WlSeat::interface().name && global.version >= 2)
            .map(|global| {
                let seat = registry.bind(global.name, 2, qh, ());
                (seat, SeatData::default())
            })
            .collect()
    });

    let state = State {
        seats,
        clipboard_manager,
    };

    Ok((queue, state))
}
