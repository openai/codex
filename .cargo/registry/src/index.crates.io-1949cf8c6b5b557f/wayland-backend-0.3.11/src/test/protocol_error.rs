use std::{
    ffi::CString,
    sync::{Arc, Mutex},
};

use crate::rs::socket::{BufferedSocket, Socket};

use super::*;

struct ServerData<Id>(Arc<Mutex<Option<Id>>>);

impl server_rs::GlobalHandler<()> for ServerData<server_rs::ObjectId> {
    fn bind(
        self: Arc<Self>,
        _: &server_rs::Handle,
        _: &mut (),
        _: server_rs::ClientId,
        _: server_rs::GlobalId,
        object_id: server_rs::ObjectId,
    ) -> Arc<dyn server_rs::ObjectData<()>> {
        *(self.0.lock().unwrap()) = Some(object_id);
        Arc::new(DoNothingData)
    }
}

impl server_sys::GlobalHandler<()> for ServerData<server_sys::ObjectId> {
    fn bind(
        self: Arc<Self>,
        _: &server_sys::Handle,
        _: &mut (),
        _: server_sys::ClientId,
        _: server_sys::GlobalId,
        object_id: server_sys::ObjectId,
    ) -> Arc<dyn server_sys::ObjectData<()>> {
        *(self.0.lock().unwrap()) = Some(object_id);
        Arc::new(DoNothingData)
    }
}

expand_test!(protocol_error, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let object_id = Arc::new(Mutex::new(None));

    // Prepare a global
    server.handle().create_global(
        &interfaces::TEST_GLOBAL_INTERFACE,
        3,
        Arc::new(ServerData(object_id.clone())),
    );

    // get the registry client-side
    let client_display = client.display_id();
    let registry_id = client
        .send_request(
            message!(client_display, 1, [Argument::NewId(client_backend::ObjectId::null())],),
            Some(Arc::new(DoNothingData)),
            Some((&interfaces::WL_REGISTRY_INTERFACE, 1)),
        )
        .unwrap();
    // create the test global
    client
        .send_request(
            message!(
                registry_id,
                0,
                [
                    Argument::Uint(1),
                    Argument::Str(Some(Box::new(
                        CString::new(interfaces::TEST_GLOBAL_INTERFACE.name.as_bytes()).unwrap(),
                    ))),
                    Argument::Uint(3),
                    Argument::NewId(client_backend::ObjectId::null()),
                ],
            ),
            Some(Arc::new(DoNothingData)),
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 3)),
        )
        .unwrap();

    client.flush().unwrap();
    server.dispatch_all_clients(&mut ()).unwrap();

    // get the object_id for the global
    let oid = object_id.lock().unwrap().clone().unwrap();

    // post the error
    server.handle().post_error(oid, 42, CString::new("I don't like you.".as_bytes()).unwrap());

    server.flush(None).unwrap();
    let ret = client.prepare_read().unwrap().read();

    match ret {
        Err(client_backend::WaylandError::Protocol(err)) => {
            assert_eq!(err.code, 42);
            assert_eq!(err.object_id, 3);
            assert_eq!(err.object_interface, "test_global");
            if std::any::TypeId::of::<client_backend::Backend>()
                == std::any::TypeId::of::<client_rs::Backend>()
            {
                // only the RS client backed can retrieve the error message
                assert_eq!(err.message, "I don't like you.");
            }
        }
        _ => panic!("Bad ret: {ret:?}"),
    }
});

expand_test!(client_wrong_id, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::<()>::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();

    let mut socket = BufferedSocket::new(Socket::from(tx));

    socket
        .write_message(&Message {
            sender_id: 1, // wl_display
            opcode: 1,    // wl_registry
            args: smallvec::smallvec![
                Argument::NewId(3), // should be 2
            ],
        })
        .unwrap();
    socket.flush().unwrap();

    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();

    // server should have killed us due to the error, but it might send us that error first
    let ret = socket.fill_incoming_buffers().and_then(|_| socket.fill_incoming_buffers());
    assert!(ret.is_err());
});

expand_test!(client_wrong_opcode, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::<()>::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();

    let mut socket = BufferedSocket::new(Socket::from(tx));

    socket
        .write_message(&Message {
            sender_id: 1, // wl_display
            opcode: 42,   // inexistant
            args: smallvec::smallvec![],
        })
        .unwrap();
    socket.flush().unwrap();

    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();

    // server should have killed us due to the error, but it might send us that error first
    let ret = socket.fill_incoming_buffers().and_then(|_| socket.fill_incoming_buffers());
    assert!(ret.is_err());
});

expand_test!(client_wrong_sender, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::<()>::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();

    let mut socket = BufferedSocket::new(Socket::from(tx));

    socket
        .write_message(&Message {
            sender_id: 2, // inexistant
            opcode: 0,    //
            args: smallvec::smallvec![],
        })
        .unwrap();
    socket.flush().unwrap();

    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();

    // server should have killed us due to the error, but it might send us that error first
    let ret = socket.fill_incoming_buffers().and_then(|_| socket.fill_incoming_buffers());
    assert!(ret.is_err());
});

struct ProtocolErrorServerData;

impl server_rs::GlobalHandler<()> for ProtocolErrorServerData {
    fn bind(
        self: Arc<Self>,
        _: &server_rs::Handle,
        _: &mut (),
        _: server_rs::ClientId,
        _: server_rs::GlobalId,
        _: server_rs::ObjectId,
    ) -> Arc<dyn server_rs::ObjectData<()>> {
        Arc::new(ProtocolErrorServerData)
    }
}

impl server_sys::GlobalHandler<()> for ProtocolErrorServerData {
    fn bind(
        self: Arc<Self>,
        _: &server_sys::Handle,
        _: &mut (),
        _: server_sys::ClientId,
        _: server_sys::GlobalId,
        _: server_sys::ObjectId,
    ) -> Arc<dyn server_sys::ObjectData<()>> {
        Arc::new(ProtocolErrorServerData)
    }
}

impl<D> server_rs::ObjectData<D> for ProtocolErrorServerData {
    fn request(
        self: Arc<Self>,
        handle: &server_rs::Handle,
        _: &mut D,
        _: server_rs::ClientId,
        msg: Message<server_rs::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn server_rs::ObjectData<D>>> {
        handle.post_error(msg.sender_id, 0, CString::new("I don't like you.".as_bytes()).unwrap());
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

impl<D> server_sys::ObjectData<D> for ProtocolErrorServerData {
    fn request(
        self: Arc<Self>,
        handle: &server_sys::Handle,
        _: &mut D,
        _: server_sys::ClientId,
        msg: Message<server_sys::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn server_sys::ObjectData<D>>> {
        handle.post_error(msg.sender_id, 0, CString::new("I don't like you.".as_bytes()).unwrap());
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

expand_test!(protocol_error_in_request_without_object_init, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    // Prepare a global
    server.handle().create_global(
        &interfaces::TEST_GLOBAL_INTERFACE,
        3,
        // The user code will provide an user data even if it triggers a protocol error
        // (and thus destroys the object)
        Arc::new(ProtocolErrorServerData),
    );

    // get the registry client-side
    let client_display = client.display_id();
    let registry_id = client
        .send_request(
            message!(client_display, 1, [Argument::NewId(client_backend::ObjectId::null())],),
            Some(Arc::new(DoNothingData)),
            Some((&interfaces::WL_REGISTRY_INTERFACE, 1)),
        )
        .unwrap();
    // create the test global
    let test_global_id = client
        .send_request(
            message!(
                registry_id,
                0,
                [
                    Argument::Uint(1),
                    Argument::Str(Some(Box::new(
                        CString::new(interfaces::TEST_GLOBAL_INTERFACE.name.as_bytes()).unwrap(),
                    ))),
                    Argument::Uint(3),
                    Argument::NewId(client_backend::ObjectId::null()),
                ],
            ),
            Some(Arc::new(DoNothingData)),
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 3)),
        )
        .unwrap();

    client.flush().unwrap();
    server.dispatch_all_clients(&mut ()).unwrap();

    // Now, the client sends a request, which will trigger a protocol error
    client
        .send_request(
            message!(test_global_id, 1, [Argument::NewId(client_backend::ObjectId::null())]),
            Some(Arc::new(DoNothingData)),
            None,
        )
        .unwrap();

    client.flush().unwrap();
    server.dispatch_all_clients(&mut ()).unwrap();

    // the server should not panic, and gracefull accept that the user did not provide any object data for
    // the already destroyed object
});
