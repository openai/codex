use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
struct SyncData(AtomicBool);

impl client_rs::ObjectData for SyncData {
    fn event(
        self: Arc<Self>,
        _: &client_rs::Backend,
        msg: Message<client_rs::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn client_rs::ObjectData>> {
        assert_eq!(msg.opcode, 0);
        assert!(matches!(&msg.args[..], [Argument::Uint(_)]));
        self.0.store(true, Ordering::SeqCst);
        None
    }

    fn destroyed(&self, _: client_rs::ObjectId) {}
}

impl client_sys::ObjectData for SyncData {
    fn event(
        self: Arc<Self>,
        _: &client_sys::Backend,
        msg: Message<client_sys::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn client_sys::ObjectData>> {
        assert_eq!(msg.opcode, 0);
        assert!(matches!(&msg.args[..], [Argument::Uint(_)]));
        self.0.store(true, Ordering::SeqCst);
        None
    }

    fn destroyed(&self, _: client_sys::ObjectId) {}
}

// send a wl_display.sync request and receive the response
expand_test!(sync, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    // send the request
    let client_display = client.display_id();
    let sync_data = Arc::new(SyncData(AtomicBool::new(false)));
    let sync_id = client
        .send_request(
            message!(client_display, 0, [Argument::NewId(client_backend::ObjectId::null())]),
            Some(sync_data.clone()),
            Some((&interfaces::WL_CALLBACK_INTERFACE, 1)),
        )
        .unwrap();
    client.flush().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    // process it server-side
    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    // ensure the answer is received client-side
    client.prepare_read().unwrap().read().unwrap();
    assert!(sync_data.0.load(Ordering::SeqCst));
    // and the sync object should be dead
    assert!(client.get_data(sync_id).is_err());
});

expand_test!(panic test_bad_placeholder, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    // send the request
    let client_display = client.display_id();
    let sync_data = Arc::new(SyncData(AtomicBool::new(false)));
    let sync_id = client
        .send_request(
            message!(client_display, 0, [Argument::NewId(client_backend::ObjectId::null())]),
            Some(sync_data.clone()),
            Some((&interfaces::WL_REGISTRY_INTERFACE, 1))
        )
        .unwrap();
    client.flush().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    // process it server-side
    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    // ensure the answer is received client-side
    client.prepare_read().unwrap().read().unwrap();
    assert!(sync_data.0.load(Ordering::SeqCst));
    // and the sync object should be dead
    assert!(client.get_data(sync_id).is_err());
});

expand_test!(panic test_bad_signature, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    // send the request
    let client_display = client.display_id();
    let sync_data = Arc::new(SyncData(AtomicBool::new(false)));
    let sync_id = client
        .send_request(message!(client_display, 0, [Argument::Uint(1)]), Some(sync_data.clone()), None)
        .unwrap();
    client.flush().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    // process it server-side
    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    // ensure the answer is received client-side
    client.prepare_read().unwrap().read().unwrap();
    assert!(sync_data.0.load(Ordering::SeqCst));
    // and the sync object should be dead
    assert!(client.get_data(sync_id).is_err());
});
