#![deny(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/dbus2/zbus/9f7a90d2b594ddc48b7a5f39fda5e00cd56a7dfb/logo.png"
)]
#![doc = include_str!("../README.md")]
#![doc(test(attr(
    warn(unused),
    deny(warnings),
    allow(dead_code),
    // W/o this, we seem to get some bogus warning about `extern crate zbus`.
    allow(unused_extern_crates),
)))]

#[cfg(doctest)]
mod doctests {
    // Repo README.
    doc_comment::doctest!("../../README.md");
    // Book markdown checks
    doc_comment::doctest!("../../book/src/client.md");
    doc_comment::doctest!("../../book/src/concepts.md");
    // The connection chapter contains a p2p example.
    #[cfg(feature = "p2p")]
    doc_comment::doctest!("../../book/src/connection.md");
    doc_comment::doctest!("../../book/src/contributors.md");
    doc_comment::doctest!("../../book/src/introduction.md");
    doc_comment::doctest!("../../book/src/service.md");
    doc_comment::doctest!("../../book/src/blocking.md");
    doc_comment::doctest!("../../book/src/faq.md");
}

#[cfg(all(not(feature = "async-io"), not(feature = "tokio")))]
mod error_message {
    #[cfg(windows)]
    compile_error!("Either \"async-io\" (default) or \"tokio\" must be enabled. On Windows \"async-io\" is (currently) required for UNIX socket support");

    #[cfg(not(windows))]
    compile_error!("Either \"async-io\" (default) or \"tokio\" must be enabled.");
}

#[cfg(windows)]
mod win32;

mod dbus_error;
pub use dbus_error::*;

mod error;
pub use error::*;

pub mod address;
pub use address::Address;

mod guid;
pub use guid::*;

pub mod message;
pub use message::Message;

#[deprecated(since = "4.0.0", note = "Use `message::Builder` instead")]
#[doc(hidden)]
pub use message::Builder as MessageBuilder;
#[deprecated(since = "4.0.0", note = "Use `message::EndianSig` instead")]
#[doc(hidden)]
pub use message::EndianSig;
#[doc(hidden)]
pub use message::Flags as MessageFlags;
#[deprecated(since = "4.0.0", note = "Use `message::Header` instead")]
#[doc(hidden)]
pub use message::Header as MessageHeader;
#[deprecated(since = "4.0.0", note = "Use `message::PrimaryHeader` instead")]
#[doc(hidden)]
pub use message::PrimaryHeader as MessagePrimaryHeader;
#[deprecated(since = "4.0.0", note = "Use `message::Sequence` instead")]
#[doc(hidden)]
pub use message::Sequence as MessageSequence;
#[deprecated(since = "4.0.0", note = "Use `message::Type` instead")]
#[doc(hidden)]
pub use message::Type as MessageType;
#[deprecated(since = "4.0.0", note = "Use `message::NATIVE_ENDIAN_SIG` instead")]
#[doc(hidden)]
pub use message::NATIVE_ENDIAN_SIG;

pub mod connection;
/// Alias for `connection` module, for convenience.
pub use connection as conn;
pub use connection::{handshake::AuthMechanism, Connection};

#[deprecated(since = "4.0.0", note = "Use `connection::Builder` instead")]
#[doc(hidden)]
pub use connection::Builder as ConnectionBuilder;

mod message_stream;
pub use message_stream::*;
mod abstractions;
pub use abstractions::*;

pub mod match_rule;
pub use match_rule::{MatchRule, OwnedMatchRule};

#[deprecated(since = "4.0.0", note = "Use `match_rule::Builder` instead")]
#[doc(hidden)]
pub use match_rule::Builder as MatchRuleBuilder;
#[deprecated(since = "4.0.0", note = "Use `match_rule::PathSpec` instead")]
#[doc(hidden)]
pub use match_rule::PathSpec as MatchRulePathSpec;

pub mod proxy;
pub use proxy::Proxy;

#[deprecated(since = "4.0.0", note = "Use `proxy::Builder` instead")]
#[doc(hidden)]
pub use proxy::Builder as ProxyBuilder;
#[deprecated(since = "4.0.0", note = "Use `proxy::CacheProperties` instead")]
#[doc(hidden)]
pub use proxy::CacheProperties;
#[deprecated(since = "4.0.0", note = "Use `proxy::MethodFlags` instead")]
#[doc(hidden)]
pub use proxy::MethodFlags;
#[deprecated(since = "4.0.0", note = "Use `proxy::OwnerChangedStream` instead")]
#[doc(hidden)]
pub use proxy::OwnerChangedStream;
#[deprecated(since = "4.0.0", note = "Use `proxy::PropertyChanged` instead")]
#[doc(hidden)]
pub use proxy::PropertyChanged;
#[deprecated(since = "4.0.0", note = "Use `proxy::PropertyStream` instead")]
#[doc(hidden)]
pub use proxy::PropertyStream;
#[deprecated(since = "4.0.0", note = "Use `proxy::ProxyDefault` instead")]
#[doc(hidden)]
pub use proxy::ProxyDefault;

pub mod object_server;
pub use object_server::ObjectServer;

#[deprecated(since = "4.0.0", note = "Use `object_server::DispatchResult` instead")]
#[doc(hidden)]
pub use object_server::DispatchResult;
#[deprecated(since = "4.0.0", note = "Use `object_server::Interface` instead")]
#[doc(hidden)]
pub use object_server::Interface;
#[deprecated(since = "4.0.0", note = "Use `object_server::InterfaceDeref` instead")]
#[doc(hidden)]
pub use object_server::InterfaceDeref;
#[deprecated(
    since = "4.0.0",
    note = "Use `object_server::InterfaceDerefMut` instead"
)]
#[doc(hidden)]
pub use object_server::InterfaceDerefMut;
#[deprecated(since = "4.0.0", note = "Use `object_server::InterfaceRef` instead")]
#[doc(hidden)]
pub use object_server::InterfaceRef;
#[deprecated(
    since = "4.0.0",
    note = "Use `object_server::ResponseDispatchNotifier` instead"
)]
#[doc(hidden)]
pub use object_server::ResponseDispatchNotifier;
#[deprecated(since = "4.0.0", note = "Use `object_server::SignalContext` instead")]
#[doc(hidden)]
pub use object_server::SignalContext;

mod utils;
pub use utils::*;

#[macro_use]
pub mod fdo;

#[deprecated(since = "4.0.0", note = "Use `connection::Socket` instead")]
#[doc(hidden)]
pub use connection::Socket;

pub mod blocking;

pub use zbus_macros::{interface, proxy, DBusError};
// Old names used for backwards compatibility
pub use zbus_macros::{dbus_interface, dbus_proxy};

// Required for the macros to function within this crate.
extern crate self as zbus;

// Macro support module, not part of the public API.
#[doc(hidden)]
pub mod export {
    pub use async_trait;
    pub use futures_core;
    pub use futures_util;
    pub use ordered_stream;
    pub use serde;
    pub use static_assertions;
}

pub use zbus_names as names;
pub use zvariant;

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{mpsc::channel, Arc, Condvar, Mutex},
    };

    use crate::utils::block_on;
    use enumflags2::BitFlags;
    use event_listener::Event;
    use ntest::timeout;
    use test_log::test;
    use tracing::{debug, instrument, trace};

    use zbus_names::UniqueName;
    use zvariant::{OwnedObjectPath, OwnedValue, Type};

    use crate::{
        blocking::{self, MessageIterator},
        fdo::{RequestNameFlags, RequestNameReply},
        message::Message,
        object_server::SignalContext,
        Connection, Result,
    };

    #[test]
    fn msg() {
        let m = Message::method("/org/freedesktop/DBus", "GetMachineId")
            .unwrap()
            .destination("org.freedesktop.DBus")
            .unwrap()
            .interface("org.freedesktop.DBus.Peer")
            .unwrap()
            .build(&())
            .unwrap();
        let hdr = m.header();
        assert_eq!(hdr.path().unwrap(), "/org/freedesktop/DBus");
        assert_eq!(hdr.interface().unwrap(), "org.freedesktop.DBus.Peer");
        assert_eq!(hdr.member().unwrap(), "GetMachineId");
    }

    #[test]
    #[timeout(15000)]
    #[instrument]
    fn basic_connection() {
        let connection = blocking::Connection::session()
            .map_err(|e| {
                debug!("error: {}", e);

                e
            })
            .unwrap();
        // Hello method is already called during connection creation so subsequent calls are
        // expected to fail but only with a D-Bus error.
        match connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "Hello",
            &(),
        ) {
            Err(crate::Error::MethodError(_, _, _)) => (),
            Err(e) => panic!("{}", e),

            _ => panic!(),
        };
    }

    #[test]
    #[timeout(15000)]
    fn basic_connection_async() {
        block_on(test_basic_connection()).unwrap();
    }

    async fn test_basic_connection() -> Result<()> {
        let connection = Connection::session().await?;

        match connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "Hello",
                &(),
            )
            .await
        {
            Err(crate::Error::MethodError(_, _, _)) => (),
            Err(e) => panic!("{}", e),

            _ => panic!(),
        };

        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    #[timeout(15000)]
    fn fdpass_systemd() {
        use std::{fs::File, os::unix::io::AsRawFd};
        use zvariant::OwnedFd;

        let connection = blocking::Connection::system().unwrap();

        let reply = connection
            .call_method(
                Some("org.freedesktop.systemd1"),
                "/org/freedesktop/systemd1",
                Some("org.freedesktop.systemd1.Manager"),
                "DumpByFileDescriptor",
                &(),
            )
            .unwrap();

        let fd: OwnedFd = reply.body().deserialize().unwrap();
        assert!(fd.as_raw_fd() >= 0);
        let f = File::from(std::os::fd::OwnedFd::from(fd));
        f.metadata().unwrap();
    }

    #[test]
    #[instrument]
    #[timeout(15000)]
    fn freedesktop_api() {
        let connection = blocking::Connection::session()
            .map_err(|e| {
                debug!("error: {}", e);

                e
            })
            .unwrap();

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "RequestName",
                &(
                    "org.freedesktop.zbus.sync",
                    BitFlags::from(RequestNameFlags::ReplaceExisting),
                ),
            )
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == "u").unwrap());
        let reply: RequestNameReply = body.deserialize().unwrap();
        assert_eq!(reply, RequestNameReply::PrimaryOwner);

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetId",
                &(),
            )
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == <&str>::signature()).unwrap());
        let id: &str = body.deserialize().unwrap();
        debug!("Unique ID of the bus: {}", id);

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &"org.freedesktop.zbus.sync",
            )
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == bool::signature()).unwrap());
        assert!(body.deserialize::<bool>().unwrap());

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetNameOwner",
                &"org.freedesktop.zbus.sync",
            )
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == <&str>::signature()).unwrap());
        assert_eq!(
            body.deserialize::<UniqueName<'_>>().unwrap(),
            *connection.unique_name().unwrap(),
        );

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetConnectionCredentials",
                &"org.freedesktop.DBus",
            )
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == "a{sv}").unwrap());
        let hashmap: HashMap<&str, OwnedValue> = body.deserialize().unwrap();

        let pid: u32 = (&hashmap["ProcessID"]).try_into().unwrap();
        debug!("DBus bus PID: {}", pid);

        #[cfg(unix)]
        {
            let uid: u32 = (&hashmap["UnixUserID"]).try_into().unwrap();
            debug!("DBus bus UID: {}", uid);
        }
    }

    #[test]
    #[timeout(15000)]
    fn freedesktop_api_async() {
        block_on(test_freedesktop_api()).unwrap();
    }

    #[instrument]
    async fn test_freedesktop_api() -> Result<()> {
        let connection = Connection::session().await?;

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "RequestName",
                &(
                    "org.freedesktop.zbus.async",
                    BitFlags::from(RequestNameFlags::ReplaceExisting),
                ),
            )
            .await
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == "u").unwrap());
        let reply: RequestNameReply = body.deserialize().unwrap();
        assert_eq!(reply, RequestNameReply::PrimaryOwner);

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetId",
                &(),
            )
            .await
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == <&str>::signature()).unwrap());
        let id: &str = body.deserialize().unwrap();
        debug!("Unique ID of the bus: {}", id);

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &"org.freedesktop.zbus.async",
            )
            .await
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == bool::signature()).unwrap());
        assert!(body.deserialize::<bool>().unwrap());

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetNameOwner",
                &"org.freedesktop.zbus.async",
            )
            .await
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == <&str>::signature()).unwrap());
        assert_eq!(
            body.deserialize::<UniqueName<'_>>().unwrap(),
            *connection.unique_name().unwrap(),
        );

        let reply = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetConnectionCredentials",
                &"org.freedesktop.DBus",
            )
            .await
            .unwrap();

        let body = reply.body();
        assert!(body.signature().map(|s| s == "a{sv}").unwrap());
        let hashmap: HashMap<&str, OwnedValue> = body.deserialize().unwrap();

        let pid: u32 = (&hashmap["ProcessID"]).try_into().unwrap();
        debug!("DBus bus PID: {}", pid);

        #[cfg(unix)]
        {
            let uid: u32 = (&hashmap["UnixUserID"]).try_into().unwrap();
            debug!("DBus bus UID: {}", uid);
        }

        Ok(())
    }

    #[test]
    #[timeout(15000)]
    fn issue_68() {
        // Tests the fix for https://github.com/dbus2/zbus/issues/68
        //
        // While this is not an exact reproduction of the issue 68, the underlying problem it
        // produces is exactly the same: `Connection::call_method` dropping all incoming messages
        // while waiting for the reply to the method call.
        let conn = blocking::Connection::session().unwrap();
        let stream = MessageIterator::from(&conn);

        // Send a message as client before service starts to process messages
        let client_conn = blocking::Connection::session().unwrap();
        let destination = conn.unique_name().map(UniqueName::<'_>::from).unwrap();
        let msg = Message::method("/org/freedesktop/Issue68", "Ping")
            .unwrap()
            .destination(destination)
            .unwrap()
            .interface("org.freedesktop.Issue68")
            .unwrap()
            .build(&())
            .unwrap();
        let serial = msg.primary_header().serial_num();
        client_conn.send(&msg).unwrap();

        crate::blocking::fdo::DBusProxy::new(&conn)
            .unwrap()
            .get_id()
            .unwrap();

        for m in stream {
            let msg = m.unwrap();

            if msg.primary_header().serial_num() == serial {
                break;
            }
        }
    }

    #[test]
    #[timeout(15000)]
    fn issue104() {
        // Tests the fix for https://github.com/dbus2/zbus/issues/104
        //
        // The issue is caused by `proxy` macro adding `()` around the return value of methods
        // with multiple out arguments, ending up with double parenthesis around the signature of
        // the return type and zbus only removing the outer `()` only and then it not matching the
        // signature we receive on the reply message.
        use zvariant::{ObjectPath, Value};

        struct Secret;
        #[super::interface(name = "org.freedesktop.Secret.Service")]
        impl Secret {
            fn open_session(
                &self,
                _algorithm: &str,
                input: Value<'_>,
            ) -> zbus::fdo::Result<(OwnedValue, OwnedObjectPath)> {
                Ok((
                    OwnedValue::try_from(input).unwrap(),
                    ObjectPath::try_from("/org/freedesktop/secrets/Blah")
                        .unwrap()
                        .into(),
                ))
            }
        }

        let secret = Secret;
        let conn = blocking::connection::Builder::session()
            .unwrap()
            .serve_at("/org/freedesktop/secrets", secret)
            .unwrap()
            .build()
            .unwrap();
        let service_name = conn.unique_name().unwrap().clone();

        {
            let conn = blocking::Connection::session().unwrap();
            #[super::proxy(
                interface = "org.freedesktop.Secret.Service",
                assume_defaults = true,
                gen_async = false
            )]
            trait Secret {
                fn open_session(
                    &self,
                    algorithm: &str,
                    input: &zvariant::Value<'_>,
                ) -> zbus::Result<(OwnedValue, OwnedObjectPath)>;
            }

            let proxy = SecretProxy::builder(&conn)
                .destination(UniqueName::from(service_name))
                .unwrap()
                .path("/org/freedesktop/secrets")
                .unwrap()
                .build()
                .unwrap();

            trace!("Calling open_session");
            proxy.open_session("plain", &Value::from("")).unwrap();
            trace!("Called open_session");
        };
    }

    // This one we just want to see if it builds, no need to run it. For details see:
    //
    // https://github.com/dbus2/zbus/issues/121
    #[test]
    #[ignore]
    fn issue_121() {
        use crate::proxy;

        #[proxy(interface = "org.freedesktop.IBus", assume_defaults = true)]
        trait IBus {
            /// CurrentInputContext property
            #[zbus(property)]
            fn current_input_context(&self) -> zbus::Result<OwnedObjectPath>;

            /// Engines property
            #[zbus(property)]
            fn engines(&self) -> zbus::Result<Vec<zvariant::OwnedValue>>;
        }
    }

    #[test]
    #[timeout(15000)]
    fn issue_122() {
        let conn = blocking::Connection::session().unwrap();
        let stream = MessageIterator::from(&conn);

        #[allow(clippy::mutex_atomic)]
        let pair = Arc::new((Mutex::new(false), Condvar::new()));
        let pair2 = Arc::clone(&pair);

        let child = std::thread::spawn(move || {
            {
                let (lock, cvar) = &*pair2;
                let mut started = lock.lock().unwrap();
                *started = true;
                cvar.notify_one();
            }

            for m in stream {
                let msg = m.unwrap();
                let hdr = msg.header();

                if hdr.member().map(|m| m.as_str()) == Some("ZBusIssue122") {
                    break;
                }
            }
        });

        // Wait for the receiving thread to start up.
        let (lock, cvar) = &*pair;
        let mut started = lock.lock().unwrap();
        while !*started {
            started = cvar.wait(started).unwrap();
        }
        // Still give it some milliseconds to ensure it's already blocking on receive_message call
        // when we send a message.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let destination = conn.unique_name().map(UniqueName::<'_>::from).unwrap();
        let msg = Message::method("/does/not/matter", "ZBusIssue122")
            .unwrap()
            .destination(destination)
            .unwrap()
            .build(&())
            .unwrap();
        conn.send(&msg).unwrap();

        child.join().unwrap();
    }

    #[test]
    #[ignore]
    fn issue_81() {
        use zbus::proxy;
        use zvariant::{OwnedValue, Type};

        #[derive(
            Debug, PartialEq, Eq, Clone, Type, OwnedValue, serde::Serialize, serde::Deserialize,
        )]
        pub struct DbusPath {
            id: String,
            path: OwnedObjectPath,
        }

        #[proxy(assume_defaults = true)]
        trait Session {
            #[zbus(property)]
            fn sessions_tuple(&self) -> zbus::Result<(String, String)>;

            #[zbus(property)]
            fn sessions_struct(&self) -> zbus::Result<DbusPath>;
        }
    }

    #[test]
    #[timeout(15000)]
    fn issue173() {
        // Tests the fix for https://github.com/dbus2/zbus/issues/173
        //
        // The issue is caused by proxy not keeping track of its destination's owner changes
        // (service restart) and failing to receive signals as a result.
        let (tx, rx) = channel();
        let child = std::thread::spawn(move || {
            let conn = blocking::Connection::session().unwrap();
            #[super::proxy(
                interface = "org.freedesktop.zbus.ComeAndGo",
                default_service = "org.freedesktop.zbus.ComeAndGo",
                default_path = "/org/freedesktop/zbus/ComeAndGo"
            )]
            trait ComeAndGo {
                #[zbus(signal)]
                fn the_signal(&self) -> zbus::Result<()>;
            }

            let proxy = ComeAndGoProxyBlocking::new(&conn).unwrap();
            let signals = proxy.receive_the_signal().unwrap();
            tx.send(()).unwrap();

            // We receive two signals, each time from different unique names. W/o the fix for
            // issue#173, the second iteration hangs.
            for _ in signals.take(2) {
                tx.send(()).unwrap();
            }
        });

        struct ComeAndGo;
        #[super::interface(name = "org.freedesktop.zbus.ComeAndGo")]
        impl ComeAndGo {
            #[zbus(signal)]
            async fn the_signal(signal_ctxt: &SignalContext<'_>) -> zbus::Result<()>;
        }

        rx.recv().unwrap();
        for _ in 0..2 {
            let conn = blocking::connection::Builder::session()
                .unwrap()
                .serve_at("/org/freedesktop/zbus/ComeAndGo", ComeAndGo)
                .unwrap()
                .name("org.freedesktop.zbus.ComeAndGo")
                .unwrap()
                .build()
                .unwrap();

            let iface_ref = conn
                .object_server()
                .interface::<_, ComeAndGo>("/org/freedesktop/zbus/ComeAndGo")
                .unwrap();
            block_on(ComeAndGo::the_signal(iface_ref.signal_context())).unwrap();

            rx.recv().unwrap();

            // Now we release the name ownership to use a different connection (i-e new unique
            // name).
            conn.release_name("org.freedesktop.zbus.ComeAndGo").unwrap();
        }

        child.join().unwrap();
    }

    #[test]
    #[timeout(15000)]
    fn uncached_property() {
        block_on(test_uncached_property()).unwrap();
    }

    async fn test_uncached_property() -> Result<()> {
        // A dummy boolean test service. It starts as `false` and can be
        // flipped to `true`. Two properties can access the inner value, with
        // and without caching.
        #[derive(Default)]
        struct ServiceUncachedPropertyTest(bool);
        #[crate::interface(name = "org.freedesktop.zbus.UncachedPropertyTest")]
        impl ServiceUncachedPropertyTest {
            #[zbus(property)]
            fn cached_prop(&self) -> bool {
                self.0
            }
            #[zbus(property)]
            fn uncached_prop(&self) -> bool {
                self.0
            }
            async fn set_inner_to_true(&mut self) -> zbus::fdo::Result<()> {
                self.0 = true;
                Ok(())
            }
        }

        #[crate::proxy(
            interface = "org.freedesktop.zbus.UncachedPropertyTest",
            default_service = "org.freedesktop.zbus.UncachedPropertyTest",
            default_path = "/org/freedesktop/zbus/UncachedPropertyTest"
        )]
        trait UncachedPropertyTest {
            #[zbus(property)]
            fn cached_prop(&self) -> zbus::Result<bool>;

            #[zbus(property(emits_changed_signal = "false"))]
            fn uncached_prop(&self) -> zbus::Result<bool>;

            fn set_inner_to_true(&self) -> zbus::Result<()>;
        }

        let service = crate::connection::Builder::session()
            .unwrap()
            .serve_at(
                "/org/freedesktop/zbus/UncachedPropertyTest",
                ServiceUncachedPropertyTest(false),
            )
            .unwrap()
            .build()
            .await
            .unwrap();

        let dest = service.unique_name().unwrap();

        let client_conn = crate::Connection::session().await.unwrap();
        let client = UncachedPropertyTestProxy::builder(&client_conn)
            .destination(dest)
            .unwrap()
            .build()
            .await
            .unwrap();

        // Query properties; this populates the cache too.
        assert!(!client.cached_prop().await.unwrap());
        assert!(!client.uncached_prop().await.unwrap());

        // Flip the inner value so we can observe the different semantics of
        // the two properties.
        client.set_inner_to_true().await.unwrap();

        // Query properties again; the first one should incur a stale read from
        // cache, while the second one should be able to read the live/updated
        // value.
        assert!(!client.cached_prop().await.unwrap());
        assert!(client.uncached_prop().await.unwrap());

        Ok(())
    }

    #[test]
    #[timeout(15000)]
    fn issue_260() {
        // Low-level server example in the book doesn't work. The reason was that
        // `Connection::request_name` implicitly created the associated `ObjectServer` to avoid
        // #68. This meant that the `ObjectServer` ended up replying to the incoming method call
        // with an error, before the service code could do so.
        block_on(async {
            let connection = Connection::session().await?;

            connection.request_name("org.zbus.Issue260").await?;

            futures_util::try_join!(
                issue_260_service(&connection),
                issue_260_client(&connection),
            )?;

            Ok::<(), zbus::Error>(())
        })
        .unwrap();
    }

    async fn issue_260_service(connection: &Connection) -> Result<()> {
        use futures_util::stream::TryStreamExt;

        let mut stream = zbus::MessageStream::from(connection);
        while let Some(msg) = stream.try_next().await? {
            let msg_header = msg.header();

            match msg_header.message_type() {
                zbus::message::Type::MethodCall => {
                    connection.reply(&msg, &()).await?;

                    break;
                }
                _ => continue,
            }
        }

        Ok(())
    }

    async fn issue_260_client(connection: &Connection) -> Result<()> {
        zbus::Proxy::new(
            connection,
            "org.zbus.Issue260",
            "/org/zbus/Issue260",
            "org.zbus.Issue260",
        )
        .await?
        .call("Whatever", &())
        .await?;
        Ok(())
    }

    #[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
    // Issue specific to tokio runtime.
    #[cfg(all(unix, feature = "tokio", feature = "p2p"))]
    #[instrument]
    async fn issue_279() {
        // On failure to read from the socket, we were closing the error channel from the sender
        // side and since the underlying tokio API doesn't provide a `close` method on the sender,
        // the async-channel abstraction was achieving this through calling `close` on receiver,
        // which is behind an async mutex and we end up with a deadlock.
        use crate::{connection::Builder, MessageStream};
        use futures_util::{stream::TryStreamExt, try_join};
        use tokio::net::UnixStream;

        let guid = crate::Guid::generate();
        let (p0, p1) = UnixStream::pair().unwrap();

        let server = Builder::unix_stream(p0).server(guid).unwrap().p2p().build();
        let client = Builder::unix_stream(p1).p2p().build();
        let (client, server) = try_join!(client, server).unwrap();
        let mut stream = MessageStream::from(client);
        let next_msg_fut = stream.try_next();

        drop(server);

        assert!(matches!(next_msg_fut.await, Err(_)));
    }

    #[test(tokio::test(flavor = "multi_thread"))]
    // Issue specific to tokio runtime.
    #[cfg(all(unix, feature = "tokio"))]
    #[instrument]
    async fn issue_310() {
        // The issue was we were deadlocking on fetching the new property value after invalidation.
        // This turned out to be caused by us trying to grab a read lock on resource while holding
        // a write lock. Thanks to connman for being weird and invalidating the property just before
        // updating it, so this issue could be exposed.
        use futures_util::StreamExt;
        use zbus::connection::Builder;

        struct Station(u64);

        #[zbus::interface(name = "net.connman.iwd.Station")]
        impl Station {
            #[zbus(property)]
            fn connected_network(&self) -> OwnedObjectPath {
                format!("/net/connman/iwd/0/33/Network/{}", self.0)
                    .try_into()
                    .unwrap()
            }
        }

        #[zbus::proxy(
            interface = "net.connman.iwd.Station",
            default_service = "net.connman.iwd"
        )]
        trait Station {
            #[zbus(property)]
            fn connected_network(&self) -> zbus::Result<OwnedObjectPath>;
        }
        let connection = Builder::session()
            .unwrap()
            .serve_at("/net/connman/iwd/0/33", Station(0))
            .unwrap()
            .name("net.connman.iwd")
            .unwrap()
            .build()
            .await
            .unwrap();
        let event = Arc::new(event_listener::Event::new());
        let conn_clone = connection.clone();
        let event_clone = event.clone();
        tokio::spawn(async move {
            for _ in 0..10 {
                let listener = event_clone.listen();
                let iface_ref = conn_clone
                    .object_server()
                    .interface::<_, Station>("/net/connman/iwd/0/33")
                    .await
                    .unwrap();

                {
                    let iface = iface_ref.get().await;
                    iface
                        .connected_network_invalidate(iface_ref.signal_context())
                        .await
                        .unwrap();
                    iface
                        .connected_network_changed(iface_ref.signal_context())
                        .await
                        .unwrap();
                }
                listener.await;
                iface_ref.get_mut().await.0 += 1;
            }
        });

        let station = StationProxy::builder(&connection)
            .path("/net/connman/iwd/0/33")
            .unwrap()
            .build()
            .await
            .unwrap();

        let mut changes = station.receive_connected_network_changed().await;

        let mut last_received = 0;
        while last_received < 9 {
            let change = changes.next().await.unwrap();
            let path = change.get().await.unwrap();
            let received: u64 = path
                .split('/')
                .last()
                .unwrap()
                .parse()
                .expect("invalid path");
            assert!(received >= last_received);
            last_received = received;
            event.notify(1);
        }
    }

    #[test]
    #[ignore]
    fn issue_466() {
        #[crate::proxy(interface = "org.Some.Thing1", assume_defaults = true)]
        trait MyGreeter {
            fn foo(
                &self,
                arg: &(u32, zbus::zvariant::Value<'_>),
            ) -> zbus::Result<(u32, zbus::zvariant::OwnedValue)>;

            #[zbus(property)]
            fn bar(&self) -> zbus::Result<(u32, zbus::zvariant::OwnedValue)>;
        }
    }

    #[instrument]
    #[test]
    fn concurrent_interface_methods() {
        // This is  test case for ensuring the regression of #799 doesn't come back.
        block_on(async {
            struct Iface(Event);

            #[zbus::interface(name = "org.zbus.test.issue799")]
            impl Iface {
                async fn method1(&self) {
                    self.0.notify(1);
                    // Never return
                    std::future::pending::<()>().await;
                }

                async fn method2(&self) {}
            }

            let event = Event::new();
            let listener = event.listen();
            let iface = Iface(event);
            let conn = zbus::connection::Builder::session()
                .unwrap()
                .name("org.zbus.test.issue799")
                .unwrap()
                .serve_at("/org/zbus/test/issue799", iface)
                .unwrap()
                .build()
                .await
                .unwrap();

            #[zbus::proxy(
                default_service = "org.zbus.test.issue799",
                default_path = "/org/zbus/test/issue799",
                interface = "org.zbus.test.issue799"
            )]
            trait Iface {
                async fn method1(&self) -> Result<()>;
                async fn method2(&self) -> Result<()>;
            }

            let proxy = IfaceProxy::new(&conn).await.unwrap();
            let proxy_clone = proxy.clone();
            conn.executor()
                .spawn(
                    async move {
                        proxy_clone.method1().await.unwrap();
                    },
                    "method1",
                )
                .detach();
            // Wait till the `method1`` is called.
            listener.await;

            // Now while the `method1` is in progress, a call to `method2` should just work.
            proxy.method2().await.unwrap();
        })
    }

    #[cfg(all(unix, feature = "p2p"))]
    #[instrument]
    #[test]
    #[timeout(15000)]
    fn issue_813() {
        // Our server-side handshake code was unable to handle FDs being sent in the first messages
        // if the client sent them too quickly after sending `BEGIN` command.
        //
        // We test this by manually sending out the auth commands together with 2 method calls with
        // 1 FD each. Before a fix for this issue, the server handshake would fail with an
        // `Unexpected FDs during handshake` error.
        use crate::{conn::socket::WriteHalf, connection::Builder};
        use futures_util::try_join;
        use nix::unistd::Uid;
        #[cfg(not(feature = "tokio"))]
        use std::os::unix::net::UnixStream;
        use std::{os::fd::AsFd, vec};
        #[cfg(feature = "tokio")]
        use tokio::net::UnixStream;
        use zvariant::Fd;

        #[derive(Debug)]
        struct Issue813Iface {
            event: event_listener::Event,
            call_count: u8,
        }
        #[crate::interface(interface = "org.zbus.Issue813")]
        impl Issue813Iface {
            #[instrument]
            fn pass_fd(&mut self, fd: Fd<'_>) {
                self.call_count += 1;
                debug!("`PassFd` called with {} {} times", fd, self.call_count);
                if self.call_count == 2 {
                    self.event.notify(1);
                }
            }
        }
        #[crate::proxy(
            gen_blocking = false,
            default_path = "/org/zbus/Issue813",
            interface = "org.zbus.Issue813"
        )]
        trait Issue813 {
            fn pass_fd(&self, fd: Fd<'_>) -> zbus::Result<()>;
        }

        block_on(async move {
            let guid = crate::Guid::generate();
            let (p0, p1) = UnixStream::pair().unwrap();

            let client_event = event_listener::Event::new();
            let client_listener = client_event.listen();
            let server_event = event_listener::Event::new();
            let server_listener = server_event.listen();
            let server = async move {
                let _conn = Builder::unix_stream(p0)
                    .server(guid)?
                    .p2p()
                    .serve_at(
                        "/org/zbus/Issue813",
                        Issue813Iface {
                            event: server_event,
                            call_count: 0,
                        },
                    )?
                    .name("org.zbus.Issue813")?
                    .build()
                    .await?;
                client_listener.await;

                Result::<()>::Ok(())
            };
            let client = async move {
                let commands = format!(
                    "\0AUTH EXTERNAL {}\r\nNEGOTIATE_UNIX_FD\r\nBEGIN\r\n",
                    hex::encode(Uid::effective().to_string())
                );
                let mut bytes: Vec<u8> = commands.bytes().collect();
                let fd = std::io::stdin();
                let msg = crate::message::Message::method("/org/zbus/Issue813", "PassFd")?
                    .destination("org.zbus.Issue813")?
                    .interface("org.zbus.Issue813")?
                    .build(&(Fd::from(fd.as_fd())))?;
                let msg_data = msg.data();
                let mut fds = vec![];
                for _ in 0..2 {
                    bytes.extend_from_slice(&*msg_data);
                    fds.push(fd.as_fd());
                }

                #[cfg(feature = "tokio")]
                let mut split = crate::conn::Socket::split(p1);
                #[cfg(not(feature = "tokio"))]
                let mut split = crate::conn::Socket::split(async_io::Async::new(p1)?);
                split.write_mut().sendmsg(&bytes, &fds).await?;

                server_listener.await;
                client_event.notify(1);

                Ok(())
            };
            let (_, _) = try_join!(client, server)?;

            Result::<()>::Ok(())
        })
        .unwrap();
    }
}
