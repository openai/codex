#![allow(dead_code, non_snake_case)]

use std::os::fd::OwnedFd;
use std::sync::Arc;

use crate::protocol::{Argument, Message};

use crate::rs::{client as client_rs, server as server_rs};
use crate::sys::{client as client_sys, server as server_sys};

macro_rules! expand_test {
    ($test_name:ident, $test_body:tt) => {
        expand_test!(__list_expand, __no_panic, $test_name, $test_body);
    };
    (panic $test_name:ident, $test_body:tt) => {
        expand_test!(__list_expand, __panic, $test_name, $test_body);
    };
    (__list_expand, $panic:tt, $test_name:ident, $test_body:tt) => {
        expand_test!(__expand, $panic, $test_name, client_rs, server_rs, $test_body);
        expand_test!(__expand, $panic, $test_name, client_sys, server_rs, $test_body);
        expand_test!(__expand, $panic, $test_name, client_rs, server_sys, $test_body);
        expand_test!(__expand, $panic, $test_name, client_sys, server_sys, $test_body);
    };
    (__expand, __panic, $test_name: ident, $client_backend: ty, $server_backend: ident, $test_body: tt) => {
        concat_idents::concat_idents!(fn_name = $test_name, __, $client_backend, __, $server_backend {
            #[test]
            #[should_panic]
            #[allow(unused_imports)]
            fn fn_name() {
                let _ = env_logger::builder().is_test(true).try_init();
                use $client_backend as client_backend;
                use $server_backend as server_backend;
                $test_body
            }
        });
    };
    (__expand, __no_panic, $test_name: ident, $client_backend: ty, $server_backend: ident, $test_body: tt) => {
        concat_idents::concat_idents!(fn_name = $test_name, __, $client_backend, __, $server_backend {
            #[test]
            #[allow(unused_imports)]
            fn fn_name() {
                let _ = env_logger::builder().is_test(true).try_init();
                use $client_backend as client_backend;
                use $server_backend as server_backend;
                $test_body
            }
        });
    };
}

mod interfaces {
    use crate as wayland_backend;
    wayland_scanner::generate_interfaces!(
        "../wayland-scanner/tests/scanner_assets/test-protocol.xml"
    );
}

mod destructors;
mod many_args;
mod object_args;
mod protocol_error;
mod server_created_objects;
mod sync;

/*
 * Assertion of Send/Sync for all relevant objects
 */

fn ensure_both<I: Send + Sync>() {}

#[allow(dead_code)]
fn send_sync_client_rs() {
    ensure_both::<client_rs::Backend>();
    ensure_both::<client_rs::ObjectId>();
}

#[allow(dead_code)]
fn send_sync_client_sys() {
    ensure_both::<client_sys::Backend>();
    ensure_both::<client_sys::ObjectId>();
}

#[allow(dead_code)]
fn send_sync_server_rs() {
    ensure_both::<server_rs::Backend<()>>();
    ensure_both::<server_rs::ObjectId>();
    ensure_both::<server_rs::GlobalId>();
    ensure_both::<server_rs::ClientId>();
}

#[allow(dead_code)]
fn send_sync_server_sys() {
    ensure_both::<server_sys::Backend<()>>();
    ensure_both::<server_sys::ObjectId>();
    ensure_both::<server_sys::GlobalId>();
    ensure_both::<server_sys::ClientId>();
}

/*
 * A "do nothing" data as a helper
 */
struct DoNothingData;

// Server Global Handler

impl<D> server_rs::GlobalHandler<D> for DoNothingData {
    fn bind(
        self: Arc<Self>,
        _: &server_rs::Handle,
        _: &mut D,
        _: server_rs::ClientId,
        _: server_rs::GlobalId,
        _: server_rs::ObjectId,
    ) -> Arc<dyn server_rs::ObjectData<D>> {
        self
    }
}

impl<D> server_sys::GlobalHandler<D> for DoNothingData {
    fn bind(
        self: Arc<Self>,
        _: &server_sys::Handle,
        _: &mut D,
        _: server_sys::ClientId,
        _: server_sys::GlobalId,
        _: server_sys::ObjectId,
    ) -> Arc<dyn server_sys::ObjectData<D>> {
        self
    }
}

// Server Object Data

impl<D> server_rs::ObjectData<D> for DoNothingData {
    fn request(
        self: Arc<Self>,
        _: &server_rs::Handle,
        _: &mut D,
        _: server_rs::ClientId,
        _: Message<server_rs::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn server_rs::ObjectData<D>>> {
        None
    }

    fn destroyed(
        self: Arc<Self>,
        _handle: &server_rs::Handle,
        _: &mut D,
        _: server_rs::ClientId,
        _: server_rs::ObjectId,
    ) {
    }
}

impl<D> server_sys::ObjectData<D> for DoNothingData {
    fn request(
        self: Arc<Self>,
        _: &server_sys::Handle,
        _: &mut D,
        _: server_sys::ClientId,
        _: Message<server_sys::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn server_sys::ObjectData<D>>> {
        None
    }

    fn destroyed(
        self: Arc<Self>,
        _handle: &server_sys::Handle,
        _: &mut D,
        _: server_sys::ClientId,
        _: server_sys::ObjectId,
    ) {
    }
}

// Client Object Data

impl client_rs::ObjectData for DoNothingData {
    fn event(
        self: Arc<Self>,
        _: &client_rs::Backend,
        _: Message<client_rs::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn client_rs::ObjectData>> {
        None
    }

    fn destroyed(&self, _: client_rs::ObjectId) {}
}

impl client_sys::ObjectData for DoNothingData {
    fn event(
        self: Arc<Self>,
        _: &client_sys::Backend,
        _: Message<client_sys::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn client_sys::ObjectData>> {
        None
    }

    fn destroyed(&self, _: client_sys::ObjectId) {}
}
