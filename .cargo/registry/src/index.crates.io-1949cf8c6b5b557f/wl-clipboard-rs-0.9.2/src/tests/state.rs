//! Test compositor implementation.
//!
//! This module contains the test compositor ([`State`]), which boils down to a minimal wlr-data-control protocol
//! implementation. The compositor can be initialized with an arbitrary set of seats, each offering arbitrary clipboard
//! contents in their regular and primary selections. Then the compositor handles all wlr-data-control interactions, such
//! as copying and pasting.

use std::collections::HashMap;
use std::io::Write;
use std::os::fd::AsFd;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::mpsc::Sender;

use os_pipe::PipeWriter;
use proptest::prelude::*;
use proptest_derive::Arbitrary;
use rustix::fs::{fcntl_setfl, OFlags};
use wayland_protocols_wlr::data_control::v1::server::zwlr_data_control_device_v1::{
    self, ZwlrDataControlDeviceV1,
};
use wayland_protocols_wlr::data_control::v1::server::zwlr_data_control_manager_v1::{
    self, ZwlrDataControlManagerV1,
};
use wayland_protocols_wlr::data_control::v1::server::zwlr_data_control_offer_v1::{
    self, ZwlrDataControlOfferV1,
};
use wayland_protocols_wlr::data_control::v1::server::zwlr_data_control_source_v1::{
    self, ZwlrDataControlSourceV1,
};
use wayland_server::protocol::wl_seat::WlSeat;
use wayland_server::{Dispatch, GlobalDispatch, Resource};

use super::TestServer;
use crate::server_ignore_global_impl;

#[derive(Debug, Clone, Arbitrary)]
pub enum OfferInfo {
    Buffered {
        #[proptest(
            strategy = "prop::collection::hash_map(any::<String>(), prop::collection::vec(any::<u8>(), 0..5), 0..5)"
        )]
        data: HashMap<String, Vec<u8>>,
    },
    #[proptest(skip)]
    Runtime { source: ZwlrDataControlSourceV1 },
}

impl Default for OfferInfo {
    fn default() -> Self {
        Self::Buffered {
            data: HashMap::new(),
        }
    }
}

impl OfferInfo {
    fn mime_types(&self, state: &State) -> Vec<String> {
        match self {
            OfferInfo::Buffered { data } => data.keys().cloned().collect(),
            OfferInfo::Runtime { source } => state.sources[source].clone(),
        }
    }

    pub fn data(&self) -> &HashMap<String, Vec<u8>> {
        match self {
            OfferInfo::Buffered { data } => data,
            OfferInfo::Runtime { .. } => panic!(),
        }
    }
}

#[derive(Debug, Clone, Default, Arbitrary)]
pub struct SeatInfo {
    pub offer: Option<OfferInfo>,
    pub primary_offer: Option<OfferInfo>,
}

#[derive(Debug, Clone, Default, Arbitrary)]
pub struct State {
    #[proptest(strategy = "prop::collection::hash_map(any::<String>(), any::<SeatInfo>(), 0..5)")]
    pub seats: HashMap<String, SeatInfo>,
    #[proptest(value = "HashMap::new()")]
    pub sources: HashMap<ZwlrDataControlSourceV1, Vec<String>>,
    #[proptest(value = "None")]
    pub selection_updated_sender: Option<Sender<Option<Vec<String>>>>,
    pub set_nonblock_on_write_fd: bool,
}

server_ignore_global_impl!(State => [ZwlrDataControlManagerV1]);

impl State {
    pub fn create_seats(&self, server: &TestServer<Self>) {
        for name in self.seats.keys() {
            server
                .display
                .handle()
                .create_global::<Self, WlSeat, _>(6, name.clone());
        }
    }
}

impl GlobalDispatch<WlSeat, String> for State {
    fn bind(
        _state: &mut Self,
        _handle: &wayland_server::DisplayHandle,
        _client: &wayland_server::Client,
        resource: wayland_server::New<WlSeat>,
        name: &String,
        data_init: &mut wayland_server::DataInit<'_, Self>,
    ) {
        let seat = data_init.init(resource, name.clone());
        seat.name((*name).to_owned());
    }
}

impl Dispatch<WlSeat, String> for State {
    fn request(
        _state: &mut Self,
        _client: &wayland_server::Client,
        _seat: &WlSeat,
        _request: <WlSeat as wayland_server::Resource>::Request,
        _name: &String,
        _dhandle: &wayland_server::DisplayHandle,
        _data_init: &mut wayland_server::DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<ZwlrDataControlManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        client: &wayland_server::Client,
        manager: &ZwlrDataControlManagerV1,
        request: <ZwlrDataControlManagerV1 as wayland_server::Resource>::Request,
        _data: &(),
        dhandle: &wayland_server::DisplayHandle,
        data_init: &mut wayland_server::DataInit<'_, Self>,
    ) {
        match request {
            zwlr_data_control_manager_v1::Request::GetDataDevice { id, seat } => {
                let name: &String = seat.data().unwrap();
                let info = &state.seats[name];

                let data_device = data_init.init(id, (*name).clone());

                let create_offer = |offer_info: &OfferInfo, is_primary: bool| {
                    let offer = client
                        .create_resource::<_, _, Self>(
                            dhandle,
                            manager.version(),
                            (name.clone(), is_primary),
                        )
                        .unwrap();
                    data_device.data_offer(&offer);

                    for mime_type in offer_info.mime_types(state) {
                        offer.offer(mime_type);
                    }

                    offer
                };

                let selection = info
                    .offer
                    .as_ref()
                    .map(|offer_info| create_offer(offer_info, false));
                data_device.selection(selection.as_ref());

                let primary_selection = info
                    .primary_offer
                    .as_ref()
                    .map(|offer_info| create_offer(offer_info, true));
                data_device.primary_selection(primary_selection.as_ref());
            }
            zwlr_data_control_manager_v1::Request::CreateDataSource { id } => {
                let source = data_init.init(id, AtomicU8::new(0));
                state.sources.insert(source, vec![]);
            }
            _ => (),
        }
    }
}

impl Dispatch<ZwlrDataControlDeviceV1, String> for State {
    fn request(
        state: &mut Self,
        _client: &wayland_server::Client,
        _resource: &ZwlrDataControlDeviceV1,
        request: <ZwlrDataControlDeviceV1 as Resource>::Request,
        name: &String,
        _dhandle: &wayland_server::DisplayHandle,
        _data_init: &mut wayland_server::DataInit<'_, Self>,
    ) {
        match request {
            zwlr_data_control_device_v1::Request::SetSelection { source } => {
                let mime_types = source.as_ref().map(|source| state.sources[source].clone());

                let info = state.seats.get_mut(name).unwrap();

                if let Some(source) = &source {
                    source.data::<AtomicU8>().unwrap().fetch_add(1, SeqCst);
                }
                if let Some(OfferInfo::Runtime { source }) = &info.offer {
                    if source.data::<AtomicU8>().unwrap().fetch_sub(1, SeqCst) == 1 {
                        source.cancelled();
                    }
                }
                info.offer = source.map(|source| OfferInfo::Runtime { source });

                if let Some(sender) = &state.selection_updated_sender {
                    let _ = sender.send(mime_types);
                }
            }
            zwlr_data_control_device_v1::Request::SetPrimarySelection { source } => {
                let mime_types = source.as_ref().map(|source| state.sources[source].clone());

                let info = state.seats.get_mut(name).unwrap();

                if let Some(source) = &source {
                    source.data::<AtomicU8>().unwrap().fetch_add(1, SeqCst);
                }
                if let Some(OfferInfo::Runtime { source }) = &info.primary_offer {
                    if source.data::<AtomicU8>().unwrap().fetch_sub(1, SeqCst) == 1 {
                        source.cancelled();
                    }
                }
                info.primary_offer = source.map(|source| OfferInfo::Runtime { source });

                if let Some(sender) = &state.selection_updated_sender {
                    let _ = sender.send(mime_types);
                }
            }
            _ => (),
        }
    }
}

impl Dispatch<ZwlrDataControlOfferV1, (String, bool)> for State {
    fn request(
        state: &mut Self,
        _client: &wayland_server::Client,
        _resource: &ZwlrDataControlOfferV1,
        request: <ZwlrDataControlOfferV1 as Resource>::Request,
        (name, is_primary): &(String, bool),
        _dhandle: &wayland_server::DisplayHandle,
        _data_init: &mut wayland_server::DataInit<'_, Self>,
    ) {
        if let zwlr_data_control_offer_v1::Request::Receive { mime_type, fd } = request {
            let info = &state.seats[name];
            let offer_info = if *is_primary {
                info.primary_offer.as_ref().unwrap()
            } else {
                info.offer.as_ref().unwrap()
            };

            match offer_info {
                OfferInfo::Buffered { data } => {
                    let mut write = PipeWriter::from(fd);
                    let _ = write.write_all(&data[mime_type.as_str()]);
                }
                OfferInfo::Runtime { source } => {
                    if state.set_nonblock_on_write_fd {
                        fcntl_setfl(&fd, OFlags::NONBLOCK).unwrap();
                    }

                    source.send(mime_type, fd.as_fd())
                }
            }
        }
    }
}

impl Dispatch<ZwlrDataControlSourceV1, AtomicU8> for State {
    fn request(
        state: &mut Self,
        _client: &wayland_server::Client,
        source: &ZwlrDataControlSourceV1,
        request: <ZwlrDataControlSourceV1 as Resource>::Request,
        _data: &AtomicU8,
        _dhandle: &wayland_server::DisplayHandle,
        _data_init: &mut wayland_server::DataInit<'_, Self>,
    ) {
        if let zwlr_data_control_source_v1::Request::Offer { mime_type } = request {
            state.sources.get_mut(source).unwrap().push(mime_type);
        }
    }
}
