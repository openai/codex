use std::{
    ffi::CString,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::protocol::Message;

use super::*;

struct ServerData(AtomicBool);

macro_rules! impl_server_objectdata {
    ($server_backend:tt) => {
        impl $server_backend::ObjectData<()> for ServerData {
            fn request(
                self: Arc<Self>,
                handle: &$server_backend::Handle,
                _: &mut (),
                _: $server_backend::ClientId,
                msg: Message<$server_backend::ObjectId, OwnedFd>,
            ) -> Option<Arc<dyn $server_backend::ObjectData<()>>> {
                if msg.opcode == 1 {
                    assert_eq!(
                        handle.object_info(msg.sender_id.clone()).unwrap().interface.name,
                        "test_global"
                    );
                    if let [Argument::NewId(secondary)] = &msg.args[..] {
                        handle
                            .send_event(message!(msg.sender_id, 1, [Argument::Object(secondary.clone())]))
                            .unwrap();
                        return Some(self);
                    } else {
                        panic!("Bad argument list!");
                    }
                } else if msg.opcode == 2{
                    return Some(self);
                } else if msg.opcode == 3 {
                    assert_eq!(handle.object_info(msg.sender_id).unwrap().interface.name, "test_global");
                    if let [Argument::Object(secondary), Argument::Object(tertiary), Argument::Uint(u)] =
                        &msg.args[..]
                    {
                        assert_eq!(
                            handle.object_info(secondary.clone()).unwrap().interface.name,
                            "secondary"
                        );
                        if *u == 1 {
                            assert!(tertiary.is_null());
                        } else if *u == 2 {
                            assert_eq!(
                                handle.object_info(tertiary.clone()).unwrap().interface.name,
                                "tertiary"
                            );
                            self.0.store(true, Ordering::SeqCst);
                        }
                    } else {
                        panic!("Bad argument list!");
                    }
                } else if msg.opcode == 6 {
                    if let [Argument::NewId(_), Argument::Object(sec), Argument::Object(ter)] = &msg.args[..] {
                        assert!(sec.is_null());
                        assert!(&ter.interface().name == &interfaces::TERTIARY_INTERFACE.name);
                    } else {
                        panic!("Bad argument list!");
                    }
                    self.0.store(true, Ordering::SeqCst);
                    return Some(self)
                }
                None
            }

            fn destroyed(
                self: Arc<Self>,
                _: &$server_backend::Handle,
                _: &mut (),
                _: $server_backend::ClientId,
                _: $server_backend::ObjectId
            ) {
            }
        }

        impl $server_backend::GlobalHandler<()> for ServerData {
            fn bind(
                self: Arc<Self>,
                _: &$server_backend::Handle,
                _: &mut (),
                _: $server_backend::ClientId,
                _: $server_backend::GlobalId,
                _: $server_backend::ObjectId
            ) -> Arc<dyn $server_backend::ObjectData<()>> {
                self
            }
        }
    }
}

impl_server_objectdata!(server_rs);
impl_server_objectdata!(server_sys);

struct ClientData(AtomicBool);

macro_rules! impl_client_objectdata {
    ($client_backend:tt) => {
        impl $client_backend::ObjectData for ClientData {
            fn event(
                self: Arc<Self>,
                handle: &$client_backend::Backend,
                msg: Message<$client_backend::ObjectId, OwnedFd>,
            ) -> Option<Arc<dyn $client_backend::ObjectData>> {
                assert_eq!(msg.opcode, 1);
                if let [Argument::Object(secondary)] = &msg.args[..] {
                    let info = handle.info(secondary.clone()).unwrap();
                    assert_eq!(info.id, 4);
                    assert_eq!(info.interface.name, "secondary");
                } else {
                    panic!("Bad argument list!");
                }
                self.0.store(true, Ordering::SeqCst);
                None
            }
            fn destroyed(&self, _object_id: $client_backend::ObjectId) {}
        }
    };
}

impl_client_objectdata!(client_rs);
impl_client_objectdata!(client_sys);

// create a global and create objects
expand_test!(create_objects, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let server_data = Arc::new(ServerData(AtomicBool::new(false)));
    let client_data = Arc::new(ClientData(AtomicBool::new(false)));

    // Prepare a global
    server.handle().create_global(&interfaces::TEST_GLOBAL_INTERFACE, 3, server_data.clone());

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
            Some(client_data.clone()),
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 3)),
        )
        .unwrap();
    // create the two objects
    let secondary_id = client
        .send_request(
            message!(
                test_global_id.clone(),
                1,
                [Argument::NewId(client_backend::ObjectId::null())]
            ),
            Some(client_data.clone()),
            None,
        )
        .unwrap();
    let tertiary_id = client
        .send_request(
            message!(
                test_global_id.clone(),
                2,
                [Argument::NewId(client_backend::ObjectId::null())]
            ),
            Some(client_data.clone()),
            None,
        )
        .unwrap();
    // link them
    client
        .send_request(
            message!(
                test_global_id.clone(),
                3,
                [
                    Argument::Object(secondary_id.clone()),
                    Argument::Object(client_backend::ObjectId::null()),
                    Argument::Uint(1),
                ],
            ),
            None,
            None,
        )
        .unwrap();
    client
        .send_request(
            message!(
                test_global_id,
                3,
                [Argument::Object(secondary_id), Argument::Object(tertiary_id), Argument::Uint(2)],
            ),
            None,
            None,
        )
        .unwrap();

    client.flush().unwrap();
    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();
    client.prepare_read().unwrap().read().unwrap();

    assert!(server_data.0.load(Ordering::SeqCst));
    assert!(client_data.0.load(Ordering::SeqCst));
});

expand_test!(panic bad_interface, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let server = server_backend::Backend::<()>::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let server_data = Arc::new(ServerData(AtomicBool::new(false)));

    // Prepare a global
    server.handle().create_global(&interfaces::TEST_GLOBAL_INTERFACE, 3, server_data);

    // get the registry client-side
    let client_display = client.display_id();
    let registry_id = client
        .send_request(
            message!(client_display, 1, [Argument::NewId(client_backend::ObjectId::null())],),
            Some(Arc::new(DoNothingData)),
            Some((&interfaces::WL_REGISTRY_INTERFACE, 1))
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
            None,
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 3))
        )
        .unwrap();
    // create the two objects
    let secondary_id = client
        .send_request(message!(test_global_id.clone(), 1, [Argument::NewId(client_backend::ObjectId::null())]), Some(Arc::new(DoNothingData)), None)
        .unwrap();
    let tertiary_id = client
        .send_request(message!(test_global_id.clone(), 2, [Argument::NewId(client_backend::ObjectId::null())]), Some(Arc::new(DoNothingData)), None)
        .unwrap();
    // link them, argument order is wrong, should panic
    client
        .send_request(
            message!(
                test_global_id,
                3,
                [Argument::Object(tertiary_id), Argument::Object(secondary_id), Argument::Uint(42)],
            ),
            None,
            None,
        )
        .unwrap();
});

expand_test!(panic double_null, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let server = server_backend::Backend::<()>::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let server_data = Arc::new(ServerData(AtomicBool::new(false)));

    // Prepare a global
    server.handle().create_global(&interfaces::TEST_GLOBAL_INTERFACE, 3, server_data);

    // get the registry client-side
    let client_display = client.display_id();
    let registry_id = client
        .send_request(
            message!(client_display, 1, [Argument::NewId(client_backend::ObjectId::null())],),
            Some(Arc::new(DoNothingData)),
            Some((&interfaces::WL_REGISTRY_INTERFACE, 1))
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
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 3))
        )
        .unwrap();
    // link two objects, first object cannot be null, should panic
    client
        .send_request(
            message!(
                test_global_id,
                3,
                [
                    Argument::Object(client_backend::ObjectId::null()),
                    Argument::Object(client_backend::ObjectId::null()),
                    Argument::Uint(42)
                ],
            ),
            None,
            None,
        )
        .unwrap();
});

expand_test!(null_obj_followed_by_interface, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let server = server_backend::Backend::<()>::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let server_data = Arc::new(ServerData(AtomicBool::new(false)));

    // Prepare a global
    server.handle().create_global(&interfaces::TEST_GLOBAL_INTERFACE, 3, server_data);

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
    // create the an object
    let tertiary_id = client
        .send_request(
            message!(
                test_global_id.clone(),
                2,
                [Argument::NewId(client_backend::ObjectId::null())]
            ),
            Some(Arc::new(DoNothingData)),
            None,
        )
        .unwrap();

    // link it, first is null but the second is not, this should work fine
    client
        .send_request(
            message!(
                test_global_id,
                5,
                [
                    Argument::Object(client_backend::ObjectId::null()),
                    Argument::Object(tertiary_id),
                ],
            ),
            None,
            None,
        )
        .unwrap();
});

expand_test!(new_id_null_and_non_null, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::<()>::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let server_data = Arc::new(ServerData(AtomicBool::new(false)));

    // Prepare a global
    server.handle().create_global(&interfaces::TEST_GLOBAL_INTERFACE, 5, server_data.clone());

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
                    Argument::Uint(5),
                    Argument::NewId(client_backend::ObjectId::null()),
                ],
            ),
            Some(Arc::new(DoNothingData)),
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 3)),
        )
        .unwrap();
    // create the an object
    let tertiary_id = client
        .send_request(
            message!(
                test_global_id.clone(),
                2,
                [Argument::NewId(client_backend::ObjectId::null())]
            ),
            Some(Arc::new(DoNothingData)),
            None,
        )
        .unwrap();

    // link it, first is null but the second is not, this should work fine
    let _quad_id = client
        .send_request(
            message!(
                test_global_id,
                6, // newid_and_allow_null
                [
                    Argument::NewId(client_backend::ObjectId::null()),
                    Argument::Object(client_backend::ObjectId::null()),
                    Argument::Object(tertiary_id),
                ],
            ),
            Some(Arc::new(DoNothingData)),
            None,
        )
        .unwrap();

    client.flush().unwrap();
    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();
    client.prepare_read().unwrap().read().unwrap();

    assert!(server_data.0.load(Ordering::SeqCst));
});
