use std::{
    ffi::{CStr, CString},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::protocol::Message;

use super::*;

struct ServerData(AtomicBool);

macro_rules! serverdata_impls {
    ($server_backend:tt) => {
        impl $server_backend::ObjectData<()> for ServerData {
            fn request(
                self: Arc<Self>,
                _: &$server_backend::Handle,
                _: &mut (),
                _: $server_backend::ClientId,
                msg: Message<$server_backend::ObjectId, OwnedFd>
            )
                -> Option<Arc<dyn $server_backend::ObjectData<()>>>
            {
                assert_eq!(msg.opcode, 0);
                if let [Argument::Uint(u), Argument::Int(i), Argument::Fixed(f), Argument::Array(ref a), Argument::Str(Some(ref s)), Argument::Fd(fd)] =
                    &msg.args[..]
                {
                    assert_eq!(*u, 42);
                    assert_eq!(*i, -13);
                    assert_eq!(*f, 4589);
                    assert_eq!(&**a, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
                    assert_eq!(&***s, CStr::from_bytes_with_nul(b"I like trains\0").unwrap());
                    // compare the fd to stdin
                    let stat1 = rustix::fs::fstat(&fd).unwrap();
                    let stat2 = rustix::fs::fstat(std::io::stdin()).unwrap();
                    assert_eq!(stat1.st_dev, stat2.st_dev);
                    assert_eq!(stat1.st_ino, stat2.st_ino);
                } else {
                    panic!("Bad argument list !")
                }
                self.0.store(true, Ordering::SeqCst);
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
                handle: &$server_backend::Handle,
                _: &mut (),
                _: $server_backend::ClientId,
                _: $server_backend::GlobalId,
                object_id: $server_backend::ObjectId,
            ) -> Arc<dyn $server_backend::ObjectData<()>> {
                handle
                    .send_event(message!(
                        object_id,
                        0,
                        [
                            Argument::Uint(1337),
                            Argument::Int(-53),
                            Argument::Fixed(9823),
                            Argument::Array(Box::new(vec![10, 20, 30, 40, 50, 60, 70, 80, 90])),
                            Argument::Str(Some(Box::new(CString::new("I want cake".as_bytes()).unwrap()))),
                            Argument::Fd(1), // stdout
                        ],
                    ))
                    .unwrap();
                self
            }
        }
    }
}

serverdata_impls!(server_rs);
serverdata_impls!(server_sys);

struct ClientData(AtomicBool);

macro_rules! clientdata_impls {
    ($client_backend:tt) => {
        impl $client_backend::ObjectData for ClientData {
            fn event(
                self: Arc<Self>,
                _handle: & $client_backend::Backend,
                msg: Message<$client_backend::ObjectId, OwnedFd>
            ) -> Option<Arc<dyn $client_backend::ObjectData>> {
                assert_eq!(msg.opcode, 0);
                if let [Argument::Uint(u), Argument::Int(i), Argument::Fixed(f), Argument::Array(ref a), Argument::Str(Some(ref s)), Argument::Fd(fd)] =
                    &msg.args[..]
                {
                    assert_eq!(*u, 1337);
                    assert_eq!(*i, -53);
                    assert_eq!(*f, 9823);
                    assert_eq!(&**a, &[10, 20, 30, 40, 50, 60, 70, 80, 90]);
                    assert_eq!(&***s, CStr::from_bytes_with_nul(b"I want cake\0").unwrap());
                    // compare the fd to stdout
                    let stat1 = rustix::fs::fstat(&fd).unwrap();
                    let stat2 = rustix::fs::fstat(std::io::stdout()).unwrap();
                    assert_eq!(stat1.st_dev, stat2.st_dev);
                    assert_eq!(stat1.st_ino, stat2.st_ino);
                } else {
                    panic!("Bad argument list !")
                }
                self.0.store(true, Ordering::SeqCst);
                None
            }
            fn destroyed(&self, _object_id: $client_backend::ObjectId) {}
        }
    }
}

clientdata_impls!(client_rs);
clientdata_impls!(client_sys);

// create a global and send the many_args method
expand_test!(many_args, {
    let (tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = server_backend::Backend::new().unwrap();
    let _client_id = server.handle().insert_client(rx, Arc::new(())).unwrap();
    let client = client_backend::Backend::connect(tx).unwrap();

    let server_data = Arc::new(ServerData(AtomicBool::new(false)));
    let client_data = Arc::new(ClientData(AtomicBool::new(false)));

    // Prepare a global
    server.handle().create_global(&interfaces::TEST_GLOBAL_INTERFACE, 1, server_data.clone());

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
                    Argument::Uint(1),
                    Argument::NewId(client_backend::ObjectId::null()),
                ],
            ),
            Some(client_data.clone()),
            Some((&interfaces::TEST_GLOBAL_INTERFACE, 1)),
        )
        .unwrap();

    client.flush().unwrap();
    server.dispatch_all_clients(&mut ()).unwrap();
    server.flush(None).unwrap();
    client.prepare_read().unwrap().read().unwrap();
    assert!(client_data.0.load(Ordering::SeqCst));

    // send the many_args request
    client
        .send_request(
            message!(
                test_global_id,
                0,
                [
                    Argument::Uint(42),
                    Argument::Int(-13),
                    Argument::Fixed(4589),
                    Argument::Array(Box::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9])),
                    Argument::Str(Some(Box::new(
                        CString::new("I like trains".as_bytes()).unwrap()
                    ))),
                    Argument::Fd(0), // stdin
                ],
            ),
            None,
            None,
        )
        .unwrap();
    client.flush().unwrap();

    server.dispatch_all_clients(&mut ()).unwrap();

    assert!(server_data.0.load(Ordering::SeqCst));
});
