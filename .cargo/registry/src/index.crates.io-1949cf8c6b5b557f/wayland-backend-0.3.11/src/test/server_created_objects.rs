use std::{
    ffi::CString,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::protocol::Message;

use super::*;

struct ServerData;

macro_rules! impl_globalhandler {
    ($server_backend:tt) => {
        impl $server_backend::GlobalHandler<()> for ServerData {
            fn bind(
                self: Arc<Self>,
                handle: &$server_backend::Handle,
                _: &mut (),
                client: $server_backend::ClientId,
                _: $server_backend::GlobalId,
                object_id: $server_backend::ObjectId,
            ) -> Arc<dyn $server_backend::ObjectData<()>> {
                // send the first event with a newid & a null object
                let obj_1 = handle
                    .create_object::<()>(
                        client.clone(),
                        &interfaces::QUAD_INTERFACE,
                        3,
                        Arc::new(DoNothingData),
                    )
                    .unwrap();
                let null_id = $server_backend::ObjectId::null();
                handle
                    .send_event(message!(
                        object_id.clone(),
                        2,
                        [Argument::NewId(obj_1.clone()), Argument::Object(null_id)],
                    ))
                    .unwrap();
                // send the second
                let obj_2 = handle
                    .create_object::<()>(
                        client,
                        &interfaces::QUAD_INTERFACE,
                        3,
                        Arc::new(DoNothingData),
                    )
                    .unwrap();
                handle
                    .send_event(message!(
                        object_id.clone(),
                        2,
                        [Argument::NewId(obj_2), Argument::Object(obj_1)]
                    ))
                    .unwrap();
                Arc::new(DoNothingData)
            }
        }
    };
}

impl_globalhandler!(server_rs);
impl_globalhandler!(server_sys);

struct ClientData(AtomicU32);

macro_rules! impl_client_objectdata {
    ($client_backend:tt) => {
        impl $client_backend::ObjectData for ClientData {
            fn event(
                self: Arc<Self>,
                handle: &$client_backend::Backend,
                msg: Message<$client_backend::ObjectId, OwnedFd>,
            ) -> Option<Arc<dyn $client_backend::ObjectData>> {
                assert_eq!(msg.opcode, 2);
                if self.0.load(Ordering::SeqCst) == 0 {
                    if let [Argument::NewId(obj_1), Argument::Object(null_id)] = &msg.args[..] {
                        let info = handle.info(obj_1.clone()).unwrap();
                        assert_eq!(info.id, 0xFF00_0000);
                        assert_eq!(info.interface.name, "quad");
                        assert!(null_id.is_null());
                    } else {
                        panic!("Bad argument list!");
                    }
                    self.0.store(1, Ordering::SeqCst);
                } else {
                    if let [Argument::NewId(obj_2), Argument::Object(obj_1)] = &msg.args[..] {
                        // check obj1
                        let info = handle.info(obj_1.clone()).unwrap();
                        assert_eq!(info.id, 0xFF00_0000);
                        assert_eq!(info.interface.name, "quad");
                        // check obj2
                        let info = handle.info(obj_2.clone()).unwrap();
                        assert_eq!(info.id, 0xFF00_0001);
                        assert_eq!(info.interface.name, "quad");
                    } else {
                        panic!("Bad argument list!");
                    }
                    self.0.store(2, Ordering::SeqCst);
                }
                Some(self)
            }
            fn destroyed(&self, _object_id: $client_backend::ObjectId) {}
        }
    };
}

impl_client_objectdata!(client_rs);
impl_client_objectdata!(client_sys);

expand_test!(server_created_object, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let client_data = Arc::new(ClientData(AtomicU32::new(0)));

    // Prepare a global
    server.handle().create_global(&interfaces::TEST_GLOBAL_INTERFACE, 3, Arc::new(ServerData));

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
    let _test_global_id = client
        .send_request(
            message!(
                registry_id,
                0,
                [
                    Argument::Uint(1),
                    Argument::Str(Some(Box::new(
                        CString::new(interfaces::TEST_GLOBAL_INTERFACE.name.as_bytes()).unwrap(),
                    ))),
                    Argument::Uint(1),
                    Argument::NewId(client_backend::ObjectId::null()),
                ],
            ),
            Some(client_data.clone()),
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 3)),
        )
        .unwrap();

    client.flush().unwrap();
    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();
    client.prepare_read().unwrap().read().unwrap();

    assert_eq!(client_data.0.load(Ordering::SeqCst), 2);
});
