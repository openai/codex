//! Getting the offered MIME types and the clipboard contents.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::io;
use std::os::fd::AsFd;

use os_pipe::{pipe, PipeReader};
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{
    delegate_dispatch, event_created_child, ConnectError, Dispatch, DispatchError, EventQueue,
};

use crate::common::{self, initialize};
use crate::data_control::{self, impl_dispatch_device, impl_dispatch_manager, impl_dispatch_offer};
use crate::utils::is_text;

/// The clipboard to operate on.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum ClipboardType {
    /// The regular clipboard.
    #[default]
    Regular,
    /// The "primary" clipboard.
    ///
    /// Working with the "primary" clipboard requires the compositor to support ext-data-control,
    /// or wlr-data-control version 2 or above.
    Primary,
}

/// MIME types that can be requested from the clipboard.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, PartialOrd, Ord)]
pub enum MimeType<'a> {
    /// Request any available MIME type.
    ///
    /// If multiple MIME types are offered, the requested MIME type is unspecified and depends on
    /// the order they are received from the Wayland compositor. However, plain text formats are
    /// prioritized, so if a plain text format is available among others then it will be requested.
    Any,
    /// Request a plain text MIME type.
    ///
    /// This will request one of the multiple common plain text MIME types. It will prioritize MIME
    /// types known to return UTF-8 text.
    Text,
    /// Request the given MIME type, and if it's not available fall back to `MimeType::Text`.
    ///
    /// Example use-case: pasting `text/html` should try `text/html` first, but if it's not
    /// available, any other plain text format will do fine too.
    TextWithPriority(&'a str),
    /// Request a specific MIME type.
    Specific(&'a str),
}

/// Seat to operate on.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, PartialOrd, Ord, Default)]
pub enum Seat<'a> {
    /// Operate on one of the existing seats depending on the order returned by the compositor.
    ///
    /// This is perfectly fine when only a single seat is present, so for most configurations.
    #[default]
    Unspecified,
    /// Operate on a seat with the given name.
    Specific(&'a str),
}

struct State {
    common: common::State,
    // The value is the set of MIME types in the offer.
    // TODO: We never remove offers from here, even if we don't use them or after destroying them.
    offers: HashMap<data_control::Offer, HashSet<String>>,
    got_primary_selection: bool,
}

delegate_dispatch!(State: [WlSeat: ()] => common::State);

impl AsMut<common::State> for State {
    fn as_mut(&mut self) -> &mut common::State {
        &mut self.common
    }
}

/// Errors that can occur for pasting and listing MIME types.
///
/// You may want to ignore some of these errors (rather than show an error message), like
/// `NoSeats`, `ClipboardEmpty` or `NoMimeType` as they are essentially equivalent to an empty
/// clipboard.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("There are no seats")]
    NoSeats,

    #[error("The clipboard of the requested seat is empty")]
    ClipboardEmpty,

    #[error("No suitable type of content copied")]
    NoMimeType,

    #[error("Couldn't open the provided Wayland socket")]
    SocketOpenError(#[source] io::Error),

    #[error("Couldn't connect to the Wayland compositor")]
    WaylandConnection(#[source] ConnectError),

    #[error("Wayland compositor communication error")]
    WaylandCommunication(#[source] DispatchError),

    #[error(
        "A required Wayland protocol ({} version {}) is not supported by the compositor",
        name,
        version
    )]
    MissingProtocol { name: &'static str, version: u32 },

    #[error("The compositor does not support primary selection")]
    PrimarySelectionUnsupported,

    #[error("The requested seat was not found")]
    SeatNotFound,

    #[error("Couldn't create a pipe for content transfer")]
    PipeCreation(#[source] io::Error),
}

impl From<common::Error> for Error {
    fn from(x: common::Error) -> Self {
        use common::Error::*;

        match x {
            SocketOpenError(err) => Error::SocketOpenError(err),
            WaylandConnection(err) => Error::WaylandConnection(err),
            WaylandCommunication(err) => Error::WaylandCommunication(err.into()),
            MissingProtocol { name, version } => Error::MissingProtocol { name, version },
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl_dispatch_manager!(State);

impl_dispatch_device!(State, WlSeat, |state: &mut Self, event, seat| {
    match event {
        Event::DataOffer { id } => {
            let offer = data_control::Offer::from(id);
            state.offers.insert(offer, HashSet::new());
        }
        Event::Selection { id } => {
            let offer = id.map(data_control::Offer::from);
            let seat = state.common.seats.get_mut(seat).unwrap();
            seat.set_offer(offer);
        }
        Event::Finished => {
            // Destroy the device stored in the seat as it's no longer valid.
            let seat = state.common.seats.get_mut(seat).unwrap();
            seat.set_device(None);
        }
        Event::PrimarySelection { id } => {
            let offer = id.map(data_control::Offer::from);
            state.got_primary_selection = true;
            let seat = state.common.seats.get_mut(seat).unwrap();
            seat.set_primary_offer(offer);
        }
        _ => (),
    }
});

impl_dispatch_offer!(State, |state: &mut Self,
                             offer: data_control::Offer,
                             event| {
    if let Event::Offer { mime_type } = event {
        state.offers.get_mut(&offer).unwrap().insert(mime_type);
    }
});

fn get_offer(
    primary: bool,
    seat: Seat<'_>,
    socket_name: Option<OsString>,
) -> Result<(EventQueue<State>, State, data_control::Offer), Error> {
    let (mut queue, mut common) = initialize(primary, socket_name)?;

    // Check if there are no seats.
    if common.seats.is_empty() {
        return Err(Error::NoSeats);
    }

    // Go through the seats and get their data devices.
    for (seat, data) in &mut common.seats {
        let device = common
            .clipboard_manager
            .get_data_device(seat, &queue.handle(), seat.clone());
        data.set_device(Some(device));
    }

    let mut state = State {
        common,
        offers: HashMap::new(),
        got_primary_selection: false,
    };

    // Retrieve all seat names and offers.
    queue
        .roundtrip(&mut state)
        .map_err(Error::WaylandCommunication)?;

    // Check if the compositor supports primary selection.
    if primary && !state.got_primary_selection {
        return Err(Error::PrimarySelectionUnsupported);
    }

    // Figure out which offer we're interested in.
    let data = match seat {
        Seat::Unspecified => state.common.seats.values().next(),
        Seat::Specific(name) => state
            .common
            .seats
            .values()
            .find(|data| data.name.as_deref() == Some(name)),
    };

    let Some(data) = data else {
        return Err(Error::SeatNotFound);
    };

    let offer = if primary {
        &data.primary_offer
    } else {
        &data.offer
    };

    // Check if we found anything.
    match offer.clone() {
        Some(offer) => Ok((queue, state, offer)),
        None => Err(Error::ClipboardEmpty),
    }
}

/// Retrieves the offered MIME types.
///
/// If `seat` is `None`, uses an unspecified seat (it depends on the order returned by the
/// compositor). This is perfectly fine when only a single seat is present, so for most
/// configurations.
///
/// # Examples
///
/// ```no_run
/// # extern crate wl_clipboard_rs;
/// # use wl_clipboard_rs::paste::Error;
/// # fn foo() -> Result<(), Error> {
/// use wl_clipboard_rs::{paste::{get_mime_types, ClipboardType, Seat}};
///
/// let mime_types = get_mime_types(ClipboardType::Regular, Seat::Unspecified)?;
/// for mime_type in mime_types {
///     println!("{}", mime_type);
/// }
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn get_mime_types(clipboard: ClipboardType, seat: Seat<'_>) -> Result<HashSet<String>, Error> {
    get_mime_types_internal(clipboard, seat, None)
}

// The internal function accepts the socket name, used for tests.
pub(crate) fn get_mime_types_internal(
    clipboard: ClipboardType,
    seat: Seat<'_>,
    socket_name: Option<OsString>,
) -> Result<HashSet<String>, Error> {
    let primary = clipboard == ClipboardType::Primary;
    let (_, mut state, offer) = get_offer(primary, seat, socket_name)?;
    Ok(state.offers.remove(&offer).unwrap())
}

/// Retrieves the clipboard contents.
///
/// This function returns a tuple of the reading end of a pipe containing the clipboard contents
/// and the actual MIME type of the contents.
///
/// If `seat` is `None`, uses an unspecified seat (it depends on the order returned by the
/// compositor). This is perfectly fine when only a single seat is present, so for most
/// configurations.
///
/// # Examples
///
/// ```no_run
/// # extern crate wl_clipboard_rs;
/// # fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// use std::io::Read;
/// use wl_clipboard_rs::{paste::{get_contents, ClipboardType, Error, MimeType, Seat}};
///
/// let result = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Any);
/// match result {
///     Ok((mut pipe, mime_type)) => {
///         println!("Got data of the {} MIME type", &mime_type);
///
///         let mut contents = vec![];
///         pipe.read_to_end(&mut contents)?;
///         println!("Read {} bytes of data", contents.len());
///     }
///
///     Err(Error::NoSeats) | Err(Error::ClipboardEmpty) | Err(Error::NoMimeType) => {
///         // The clipboard is empty, nothing to worry about.
///     }
///
///     Err(err) => Err(err)?
/// }
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn get_contents(
    clipboard: ClipboardType,
    seat: Seat<'_>,
    mime_type: MimeType<'_>,
) -> Result<(PipeReader, String), Error> {
    get_contents_internal(clipboard, seat, mime_type, None)
}

// The internal function accepts the socket name, used for tests.
pub(crate) fn get_contents_internal(
    clipboard: ClipboardType,
    seat: Seat<'_>,
    mime_type: MimeType<'_>,
    socket_name: Option<OsString>,
) -> Result<(PipeReader, String), Error> {
    let primary = clipboard == ClipboardType::Primary;
    let (mut queue, mut state, offer) = get_offer(primary, seat, socket_name)?;

    let mut mime_types = state.offers.remove(&offer).unwrap();

    // Find the desired MIME type.
    let mime_type = match mime_type {
        MimeType::Any => mime_types
            .take("text/plain;charset=utf-8")
            .or_else(|| mime_types.take("UTF8_STRING"))
            .or_else(|| mime_types.iter().find(|x| is_text(x)).cloned())
            .or_else(|| mime_types.drain().next()),
        MimeType::Text => mime_types
            .take("text/plain;charset=utf-8")
            .or_else(|| mime_types.take("UTF8_STRING"))
            .or_else(|| mime_types.drain().find(|x| is_text(x))),
        MimeType::TextWithPriority(priority) => mime_types
            .take(priority)
            .or_else(|| mime_types.take("text/plain;charset=utf-8"))
            .or_else(|| mime_types.take("UTF8_STRING"))
            .or_else(|| mime_types.drain().find(|x| is_text(x))),
        MimeType::Specific(mime_type) => mime_types.take(mime_type),
    };

    // Check if a suitable MIME type is copied.
    if mime_type.is_none() {
        return Err(Error::NoMimeType);
    }

    let mime_type = mime_type.unwrap();

    // Create a pipe for content transfer.
    let (read, write) = pipe().map_err(Error::PipeCreation)?;

    // Start the transfer.
    offer.receive(mime_type.clone(), write.as_fd());
    drop(write);

    // A flush() is not enough here, it will result in sometimes pasting empty contents. I suspect this is due to a
    // race between the compositor reacting to the receive request, and the compositor reacting to wl-paste
    // disconnecting after queue is dropped. The roundtrip solves that race.
    queue
        .roundtrip(&mut state)
        .map_err(Error::WaylandCommunication)?;

    Ok((read, mime_type))
}
