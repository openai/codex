#![allow(clippy::disallowed_names)]
use std::collections::HashMap;
#[cfg(all(unix, not(feature = "tokio"), feature = "p2p"))]
use std::os::unix::net::UnixStream;
#[cfg(all(unix, feature = "tokio", feature = "p2p"))]
use tokio::net::UnixStream;

use event_listener::Event;
use futures_util::{StreamExt, TryStreamExt};
use ntest::timeout;
use serde::{Deserialize, Serialize};
use test_log::test;
use tokio::sync::mpsc::{channel, Sender};
use tracing::{debug, instrument};
use zbus::{
    block_on,
    fdo::{ObjectManager, ObjectManagerProxy},
    message,
    object_server::ResponseDispatchNotifier,
    DBusError, Error, Message, MessageStream,
};
use zvariant::{DeserializeDict, Optional, OwnedValue, SerializeDict, Str, Type, Value};

use zbus::{
    connection, interface,
    message::Header,
    object_server::{InterfaceRef, SignalContext},
    proxy::CacheProperties,
    Connection, ObjectServer,
};

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct ArgStructTest {
    foo: i32,
    bar: String,
}

// Mimic a NetworkManager interface property that's a dict. This tests ability to use a custom
// dict type using the `Type` And `*Dict` macros (issue #241).
#[derive(DeserializeDict, SerializeDict, Type, Debug, Value, OwnedValue, PartialEq, Eq)]
#[zvariant(signature = "dict")]
pub struct IP4Adress {
    prefix: u32,
    address: String,
}

// To test property setter for types with lifetimes.
#[derive(Serialize, Deserialize, Type, Debug, Value, OwnedValue, PartialEq, Eq)]
pub struct RefType<'a> {
    #[serde(borrow)]
    field1: Str<'a>,
}

#[derive(Debug, Clone)]
enum NextAction {
    Quit,
    CreateObj(String),
    DestroyObj(String),
}

#[derive(Debug)]
struct MyIface {
    next_tx: Sender<NextAction>,
    count: u32,
    emits_changed_default: u32,
    emits_changed_true: u32,
    emits_changed_invalidates: u32,
    emits_changed_const: u32,
    emits_changed_false: u32,
}

impl MyIface {
    fn new(next_tx: Sender<NextAction>) -> Self {
        Self {
            next_tx,
            count: 0,
            emits_changed_default: 0,
            emits_changed_true: 0,
            emits_changed_invalidates: 0,
            emits_changed_const: 0,
            emits_changed_false: 0,
        }
    }
}

/// Custom D-Bus error type.
#[derive(Debug, DBusError, PartialEq)]
#[zbus(prefix = "org.freedesktop.MyIface.Error")]
enum MyIfaceError {
    SomethingWentWrong(String),
    #[zbus(error)]
    ZBus(zbus::Error),
}

#[interface(
    interface = "org.freedesktop.MyIface",
    proxy(gen_blocking = true, assume_defaults = true)
)]
impl MyIface {
    #[instrument]
    async fn ping(&mut self, #[zbus(signal_context)] ctxt: SignalContext<'_>) -> u32 {
        self.count += 1;
        if self.count % 3 == 0 {
            MyIface::alert_count(&ctxt, self.count)
                .await
                .expect("Failed to emit signal");
            debug!("emitted `AlertCount` signal.");
        } else {
            debug!("Didn't emit `AlertCount` signal.");
        }
        self.count
    }

    #[instrument]
    async fn quit(&self) {
        debug!("Client asked to quit.");
        self.next_tx.send(NextAction::Quit).await.unwrap();
    }

    #[instrument]
    fn test_header(&self, #[zbus(header)] header: Header<'_>) {
        debug!("`TestHeader` called.");
        assert_eq!(header.message_type(), message::Type::MethodCall);
        assert_eq!(header.member().unwrap(), "TestHeader");
    }

    #[instrument]
    fn test_error(&self) -> zbus::fdo::Result<()> {
        debug!("`TestError` called.");
        Err(zbus::fdo::Error::Failed("error raised".to_string()))
    }

    #[instrument]
    fn test_custom_error(&self) -> Result<(), MyIfaceError> {
        debug!("`TestCustomError` called.");
        Err(MyIfaceError::SomethingWentWrong("oops".to_string()))
    }

    #[instrument]
    fn test_single_struct_arg(
        &self,
        arg: ArgStructTest,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        debug!("`TestSingleStructArg` called.");
        assert_eq!(header.signature().unwrap(), "(is)");
        assert_eq!(arg.foo, 1);
        assert_eq!(arg.bar, "TestString");

        Ok(())
    }

    #[instrument]
    fn test_single_struct_ret(&self) -> zbus::fdo::Result<ArgStructTest> {
        debug!("`TestSingleStructRet` called.");
        Ok(ArgStructTest {
            foo: 42,
            bar: String::from("Meaning of life"),
        })
    }

    #[instrument]
    #[zbus(out_args("foo", "bar"))]
    fn test_multi_ret(&self) -> zbus::fdo::Result<(i32, String)> {
        debug!("`TestMultiRet` called.");
        Ok((42, String::from("Meaning of life")))
    }

    #[instrument]
    fn test_response_notify(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> zbus::fdo::Result<ResponseDispatchNotifier<String>> {
        debug!("`TestResponseNotify` called.");
        let (response, listener) = ResponseDispatchNotifier::new(String::from("Meaning of life"));
        let ctxt = ctxt.to_owned();
        conn.executor()
            .spawn(
                async move {
                    listener.await;

                    Self::test_response_notified(ctxt).await.unwrap();
                },
                "TestResponseNotify",
            )
            .detach();

        Ok(response)
    }

    #[zbus(signal)]
    async fn test_response_notified(ctxt: SignalContext<'_>) -> zbus::Result<()>;

    #[instrument]
    async fn test_hashmap_return(&self) -> zbus::fdo::Result<HashMap<String, String>> {
        debug!("`TestHashmapReturn` called.");
        let mut map = HashMap::new();
        map.insert("hi".into(), "hello".into());
        map.insert("bye".into(), "now".into());

        Ok(map)
    }

    #[instrument]
    async fn create_obj(&self, key: &str) {
        debug!("`CreateObj` called.");
        self.next_tx
            .send(NextAction::CreateObj(key.into()))
            .await
            .unwrap();
    }

    #[instrument]
    async fn create_obj_inside(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        key: String,
    ) {
        debug!("`CreateObjInside` called.");
        object_server
            .at(
                format!("/zbus/test/{key}"),
                MyIface::new(self.next_tx.clone()),
            )
            .await
            .unwrap();
    }

    #[instrument]
    async fn destroy_obj(&self, key: &str) {
        debug!("`DestroyObj` called.");
        self.next_tx
            .send(NextAction::DestroyObj(key.into()))
            .await
            .unwrap();
    }

    #[cfg(feature = "option-as-array")]
    #[instrument]
    async fn optional_args(&self, arg: Option<&str>) -> zbus::fdo::Result<Option<String>> {
        debug!("`OptionalArgs` called.");
        Ok(arg.map(|s| format!("Hello {}", s)))
    }

    #[instrument]
    #[zbus(property)]
    fn set_count(&mut self, val: u32) -> zbus::fdo::Result<()> {
        debug!("`Count` setter called.");
        if val == 42 {
            return Err(zbus::fdo::Error::InvalidArgs("Tsss tsss!".to_string()));
        }
        self.count = val;
        Ok(())
    }

    #[instrument]
    #[zbus(property)]
    fn count(&self) -> u32 {
        debug!("`Count` getter called.");
        self.count
    }

    #[instrument]
    #[zbus(property)]
    async fn hash_map(&self) -> HashMap<String, String> {
        debug!("`HashMap` getter called.");
        self.test_hashmap_return().await.unwrap()
    }

    #[instrument]
    #[zbus(property)]
    async fn fail_property(&self) -> zbus::fdo::Result<u32> {
        Err(zbus::fdo::Error::UnknownProperty(
            "FailProperty".to_string(),
        ))
    }

    #[instrument]
    #[zbus(property)]
    fn optional_property(&self) -> Optional<u32> {
        debug!("`OptionalAsProp` getter called.");
        Some(42).into()
    }

    #[instrument]
    #[zbus(property)]
    fn address_data(&self) -> IP4Adress {
        debug!("`AddressData` getter called.");
        IP4Adress {
            address: "127.0.0.1".to_string(),
            prefix: 1234,
        }
    }

    #[instrument]
    #[zbus(property)]
    fn set_address_data(&self, addr: IP4Adress) {
        debug!("`AddressData` setter called with {:?}", addr);
    }

    // On the bus, this should return the same value as address_data above. We want to test if
    // this works both ways.
    #[instrument]
    #[zbus(property)]
    fn address_data2(&self) -> HashMap<String, OwnedValue> {
        debug!("`AddressData2` getter called.");
        let mut map = HashMap::new();
        map.insert(
            "address".into(),
            Value::from("127.0.0.1").try_into().unwrap(),
        );
        map.insert("prefix".into(), 1234u32.into());

        map
    }

    #[instrument]
    #[zbus(property)]
    fn str_prop(&self) -> String {
        "Hello".to_string()
    }

    #[instrument]
    #[zbus(property)]
    fn set_str_prop(&self, str_prop: &str) {
        debug!("`SetStrRef` called with {:?}", str_prop);
    }

    #[instrument]
    #[zbus(property)]
    fn ref_prop(&self) -> RefType<'_> {
        RefType {
            field1: "Hello".into(),
        }
    }

    #[instrument]
    #[zbus(property)]
    fn set_ref_prop(&self, ref_type: RefType<'_>) {
        debug!("`SetRefType` called with {:?}", ref_type);
    }

    #[instrument]
    #[zbus(proxy(no_reply))]
    fn test_no_reply(&self, #[zbus(header)] header: Header<'_>) {
        debug!("`TestNoReply` called");
        assert_eq!(header.message_type(), zbus::message::Type::MethodCall);
        assert!(header
            .primary()
            .flags()
            .contains(zbus::message::Flags::NoReplyExpected));
    }

    #[instrument]
    #[zbus(proxy(no_autostart))]
    fn test_no_autostart(&self, #[zbus(header)] header: Header<'_>) {
        debug!("`TestNoAutostart` called");
        assert_eq!(header.message_type(), zbus::message::Type::MethodCall);
        assert!(header
            .primary()
            .flags()
            .contains(zbus::message::Flags::NoAutoStart));
    }

    #[instrument]
    #[zbus(proxy(allow_interactive_auth))]
    fn test_interactive_auth(&self, #[zbus(header)] header: Header<'_>) {
        debug!("`TestInteractiveAuth` called");
        assert_eq!(header.message_type(), zbus::message::Type::MethodCall);
        assert!(header
            .primary()
            .flags()
            .contains(zbus::message::Flags::AllowInteractiveAuth));
    }

    #[zbus(signal)]
    async fn alert_count(ctxt: &SignalContext<'_>, val: u32) -> zbus::Result<()>;

    #[instrument]
    #[zbus(property)]
    fn emits_changed_default(&self) -> u32 {
        debug!("`EmitsChangedDefault` getter called.");
        self.emits_changed_default
    }

    #[instrument]
    #[zbus(property)]
    fn set_emits_changed_default(&mut self, val: u32) -> zbus::fdo::Result<()> {
        debug!("`EmitsChangedDefault` setter called.");
        self.emits_changed_default = val;
        Ok(())
    }

    #[instrument]
    #[zbus(property(emits_changed_signal = "true"))]
    fn emits_changed_true(&self) -> u32 {
        debug!("`EmitsChangedTrue` getter called.");
        self.emits_changed_true
    }

    #[instrument]
    #[zbus(property)]
    fn set_emits_changed_true(&mut self, val: u32) -> zbus::fdo::Result<()> {
        debug!("`EmitsChangedTrue` setter called.");
        self.emits_changed_true = val;
        Ok(())
    }

    #[instrument]
    #[zbus(property(emits_changed_signal = "invalidates"))]
    fn emits_changed_invalidates(&self) -> u32 {
        debug!("`EmitsChangedInvalidates` getter called.");
        self.emits_changed_invalidates
    }

    #[instrument]
    #[zbus(property)]
    fn set_emits_changed_invalidates(&mut self, val: u32) -> zbus::fdo::Result<()> {
        debug!("`EmitsChangedInvalidates` setter called.");
        self.emits_changed_invalidates = val;
        Ok(())
    }

    #[instrument]
    #[zbus(property(emits_changed_signal = "const"))]
    fn emits_changed_const(&self) -> u32 {
        debug!("`EmitsChangedConst` getter called.");
        self.emits_changed_const
    }

    #[instrument]
    #[zbus(property)]
    fn set_emits_changed_const(&mut self, val: u32) -> zbus::fdo::Result<()> {
        debug!("`EmitsChangedConst` setter called.");
        self.emits_changed_const = val;
        Ok(())
    }

    #[instrument]
    #[zbus(property(emits_changed_signal = "false"))]
    fn emits_changed_false(&self) -> u32 {
        debug!("`EmitsChangedFalse` getter called.");
        self.emits_changed_false
    }

    #[instrument]
    #[zbus(property)]
    fn set_emits_changed_false(&mut self, val: u32) -> zbus::fdo::Result<()> {
        debug!("`EmitsChangedFalse` setter called.");
        self.emits_changed_false = val;
        Ok(())
    }
}

fn check_hash_map(map: HashMap<String, String>) {
    assert_eq!(map["hi"], "hello");
    assert_eq!(map["bye"], "now");
}

fn check_ipv4_address(address: IP4Adress) {
    assert_eq!(
        address,
        IP4Adress {
            address: "127.0.0.1".to_string(),
            prefix: 1234,
        }
    );
}

fn check_ipv4_address_hashmap(address: HashMap<String, OwnedValue>) {
    assert_eq!(**address.get("address").unwrap(), Value::from("127.0.0.1"));
    assert_eq!(**address.get("prefix").unwrap(), Value::from(1234u32));
}

#[instrument]
async fn my_iface_test(conn: Connection, event: Event) -> zbus::Result<u32> {
    debug!("client side starting..");
    // Use low-level API for `TestResponseNotify` because we need to ensure that the signal is
    // always received after the response.
    let mut stream = MessageStream::from(&conn);
    let method = Message::method("/org/freedesktop/MyService", "TestResponseNotify")?
        .interface("org.freedesktop.MyIface")?
        .destination("org.freedesktop.MyService")?
        .build(&())?;
    let serial = method.primary_header().serial_num();
    conn.send(&method).await?;
    let mut method_returned = false;
    let mut signal_received = false;
    while !method_returned && !signal_received {
        let msg = stream.try_next().await?.unwrap();

        let hdr = msg.header();
        if hdr.message_type() == message::Type::MethodReturn && hdr.reply_serial() == Some(serial) {
            assert!(!signal_received);
            method_returned = true;
        } else if hdr.message_type() == message::Type::Signal
            && hdr.interface().unwrap() == "org.freedesktop.MyService"
            && hdr.member().unwrap() == "TestResponseNotified"
        {
            assert!(method_returned);
            signal_received = true;
        }
    }
    drop(stream);

    let root_introspect_proxy = zbus::fdo::IntrospectableProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/")?
        .build()
        .await?;
    debug!("Created: {:?}", root_introspect_proxy);

    let root_xml = root_introspect_proxy.introspect().await?;
    let root_node = zbus_xml::Node::from_reader(root_xml.as_bytes())
        .map_err(|e| Error::Failure(e.to_string()))?;
    let mut node = &root_node;
    for name in ["org", "freedesktop", "MyService"] {
        node = node
            .nodes()
            .iter()
            .find(|&n| n.name().is_some_and(|n| n == name))
            .expect("Child node not exist");
    }
    assert!(node
        .interfaces()
        .iter()
        .any(|i| i.name() == "org.freedesktop.MyIface"));

    let proxy = MyIfaceProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/org/freedesktop/MyService")?
        // the server isn't yet running
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    debug!("Created: {:?}", proxy);

    // First let's call a non-existent method. It should immediately fail.
    // There was a regression where object server would just not reply in this case:
    // https://github.com/dbus2/zbus/issues/905
    assert!(proxy
        .inner()
        .call::<_, _, ()>("NonExistantMethod", &())
        .await
        .is_err());

    let props_proxy = zbus::fdo::PropertiesProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/org/freedesktop/MyService")?
        .build()
        .await?;
    debug!("Created: {:?}", props_proxy);

    let mut props_changed_stream = props_proxy.receive_properties_changed().await?;
    debug!("Created: {:?}", props_changed_stream);
    event.notify(1);
    debug!("Notified service that client is ready");

    match props_changed_stream.next().await {
        Some(changed) => {
            assert_eq!(
                *changed.args()?.changed_properties().keys().next().unwrap(),
                "Count"
            );
        }
        None => panic!(""),
    };
    drop(props_changed_stream);

    proxy.ping().await?;
    assert_eq!(proxy.count().await?, 1);
    assert_eq!(proxy.cached_count()?, None);

    proxy.test_header().await?;
    proxy
        .test_single_struct_arg(ArgStructTest {
            foo: 1,
            bar: "TestString".into(),
        })
        .await?;

    proxy.test_error().await.unwrap_err();
    assert_eq!(
        proxy.test_custom_error().await.unwrap_err(),
        MyIfaceError::SomethingWentWrong("oops".to_string())
    );

    check_hash_map(proxy.test_hashmap_return().await?);
    check_hash_map(proxy.hash_map().await?);
    proxy
        .set_address_data(IP4Adress {
            address: "localhost".to_string(),
            prefix: 1234,
        })
        .await?;
    proxy.set_str_prop("This is an str ref").await?;
    check_ipv4_address(proxy.address_data().await?);
    check_ipv4_address_hashmap(proxy.address_data2().await?);

    proxy.test_no_reply().await?;
    proxy.test_no_autostart().await?;
    proxy.test_interactive_auth().await?;

    let err = proxy.fail_property().await;
    assert_eq!(
        err.unwrap_err(),
        zbus::Error::FDO(Box::new(zbus::fdo::Error::UnknownProperty(
            "FailProperty".into()
        )))
    );

    assert_eq!(proxy.optional_property().await?, Some(42).into());

    let xml = proxy.inner().introspect().await?;
    debug!("Introspection: {}", xml);
    let node =
        zbus_xml::Node::from_reader(xml.as_bytes()).map_err(|e| Error::Failure(e.to_string()))?;
    let ifaces = node.interfaces();
    let iface = ifaces
        .iter()
        .find(|i| i.name() == "org.freedesktop.MyIface")
        .unwrap();
    let methods = iface.methods();
    for method in methods {
        if method.name() != "TestSingleStructRet" && method.name() != "TestMultiRet" {
            continue;
        }
        let args = method.args();
        let mut out_args = args
            .iter()
            .filter(|a| a.direction().unwrap() == zbus_xml::ArgDirection::Out);

        if method.name() == "TestSingleStructRet" {
            assert_eq!(args.len(), 1);
            assert_eq!(out_args.next().unwrap().ty().signature(), "(is)");
            assert!(out_args.next().is_none());
        } else {
            assert_eq!(args.len(), 2);
            let foo = out_args.find(|a| a.name() == Some("foo")).unwrap();
            assert_eq!(foo.ty().signature(), "i");
            let bar = out_args.find(|a| a.name() == Some("bar")).unwrap();
            assert_eq!(bar.ty().signature(), "s");
        }
    }
    // build-time check to see if macro is doing the right thing.
    let _ = proxy.test_single_struct_ret().await?.foo;
    let _ = proxy.test_multi_ret().await?.1;

    let val = proxy.ping().await?;

    let obj_manager_proxy = ObjectManagerProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/zbus/test")?
        .build()
        .await?;
    debug!("Created: {:?}", obj_manager_proxy);
    let mut ifaces_added_stream = obj_manager_proxy.receive_interfaces_added().await?;
    debug!("Created: {:?}", ifaces_added_stream);

    // Must process in parallel, so the stream listener does not block receiving
    // the method return message.
    let (ifaces_added, _) = futures_util::future::join(
        async {
            let ret = ifaces_added_stream.next().await.unwrap();
            drop(ifaces_added_stream);
            ret
        },
        async {
            proxy.create_obj("MyObj").await.unwrap();
        },
    )
    .await;

    #[cfg(feature = "option-as-array")]
    {
        assert!(proxy.optional_args(None).await.unwrap().is_none());
        assert_eq!(
            proxy.optional_args(Some("🚌")).await.unwrap().unwrap(),
            "Hello 🚌",
        );
    }

    assert_eq!(ifaces_added.args()?.object_path(), "/zbus/test/MyObj");
    let args = ifaces_added.args()?;
    let ifaces = args.interfaces_and_properties();
    let _ = ifaces.get("org.freedesktop.MyIface").unwrap();
    // TODO: Check if the properties are correct.

    // issue#207: interface panics on incorrect number of args.
    assert!(proxy.inner().call_method("CreateObj", &()).await.is_err());

    let my_obj_proxy = MyIfaceProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/zbus/test/MyObj")?
        .build()
        .await?;
    debug!("Created: {:?}", my_obj_proxy);
    my_obj_proxy.receive_count_changed().await;
    // Calling this after creating the stream was panicking if the property doesn't get cached
    // before the call (MR !460).
    my_obj_proxy.cached_count()?;
    assert_eq!(my_obj_proxy.count().await?, 0);
    assert_eq!(my_obj_proxy.cached_count()?, Some(0));
    assert_eq!(
        my_obj_proxy.inner().cached_property_raw("Count").as_deref(),
        Some(&Value::from(0u32))
    );
    my_obj_proxy.ping().await?;

    let mut ifaces_removed_stream = obj_manager_proxy.receive_interfaces_removed().await?;
    debug!("Created: {:?}", ifaces_removed_stream);
    // Must process in parallel, so the stream listener does not block receiving
    // the method return message.
    let (ifaces_removed, _) = futures_util::future::join(
        async {
            let ret = ifaces_removed_stream.next().await.unwrap();
            drop(ifaces_removed_stream);
            ret
        },
        async {
            proxy.destroy_obj("MyObj").await.unwrap();
        },
    )
    .await;

    let args = ifaces_removed.args()?;
    assert_eq!(args.object_path(), "/zbus/test/MyObj");
    assert_eq!(args.interfaces(), &["org.freedesktop.MyIface"]);

    assert!(my_obj_proxy.inner().introspect().await.is_err());
    assert!(my_obj_proxy.ping().await.is_err());

    // Make sure methods modifying the ObjectServer can be called without
    // deadlocks.
    proxy
        .inner()
        .call_method("CreateObjInside", &("CreatedInside"))
        .await?;
    let created_inside_proxy = MyIfaceProxy::builder(&conn)
        .destination("org.freedesktop.MyService")?
        .path("/zbus/test/CreatedInside")?
        .build()
        .await?;
    created_inside_proxy.ping().await?;
    proxy.destroy_obj("CreatedInside").await?;

    // Test that interfaces emit signals when properties change
    // according to their emits_changed_signal flags.
    let mut props_changed = props_proxy.receive_properties_changed().await?;
    let expected_property_value = 4;

    proxy
        .set_emits_changed_default(expected_property_value)
        .await?;
    let changed = props_changed.next().await.unwrap();
    let expected_property_key = "EmitsChangedDefault";
    let args = changed.args()?;
    assert!(args.invalidated_properties().is_empty());

    let changed_prop_key = *args.changed_properties().keys().next().unwrap();
    assert_eq!(changed_prop_key, expected_property_key);

    let changed_value = &args.changed_properties()[expected_property_key];
    assert_eq!(
        <&Value as TryInto<u32>>::try_into(changed_value).unwrap(),
        expected_property_value
    );
    assert!(args.invalidated_properties().is_empty());

    proxy
        .set_emits_changed_true(expected_property_value)
        .await?;
    let changed = props_changed.next().await.unwrap();
    let expected_property_key = "EmitsChangedTrue";
    let args = changed.args()?;
    assert!(args.invalidated_properties().is_empty());

    let changed_prop_key = *args.changed_properties().keys().next().unwrap();
    assert_eq!(changed_prop_key, expected_property_key);

    let changed_value = &args.changed_properties()[expected_property_key];
    assert_eq!(
        <&Value as TryInto<u32>>::try_into(changed_value).unwrap(),
        expected_property_value
    );
    assert!(args.invalidated_properties().is_empty());

    proxy
        .set_emits_changed_invalidates(expected_property_value)
        .await?;
    let changed = props_changed.next().await.unwrap();
    let expected_property_key = "EmitsChangedInvalidates";
    let args = changed.args()?;
    assert_eq!(
        args.invalidated_properties(),
        &vec![expected_property_key.to_string()]
    );
    assert!(args.changed_properties().is_empty());

    // First set a property for which we don't expect a signal
    // then set a property for which we do (and we checked above
    // that we receive it. The next item in the iter should correspond
    // to the second property we set.
    proxy
        .set_emits_changed_const(expected_property_value)
        .await?;
    proxy
        .set_emits_changed_true(expected_property_value)
        .await?;
    let changed = props_changed.next().await.unwrap();
    let unexpected_property_key = "EmitsChangedConst";
    let args = changed.args()?;
    assert_ne!(
        args.invalidated_properties(),
        &vec![unexpected_property_key.to_string()]
    );
    assert!(!args.changed_properties().is_empty());
    assert!(args.invalidated_properties().is_empty());

    proxy
        .set_emits_changed_false(expected_property_value)
        .await?;
    proxy
        .set_emits_changed_true(expected_property_value)
        .await?;
    let changed = props_changed.next().await.unwrap();
    let unexpected_property_key = "EmitsChangedFalse";
    let args = changed.args()?;
    assert_ne!(
        args.invalidated_properties(),
        &vec![unexpected_property_key.to_string()]
    );
    assert!(!args.changed_properties().is_empty());
    assert!(args.invalidated_properties().is_empty());

    proxy.quit().await?;
    Ok(val)
}

#[test]
#[timeout(15000)]
fn iface_and_proxy() {
    block_on(iface_and_proxy_(false));
}

#[cfg(feature = "p2p")]
#[cfg(unix)]
#[test]
#[timeout(15000)]
fn iface_and_proxy_unix_p2p() {
    block_on(iface_and_proxy_(true));
}

#[instrument]
async fn iface_and_proxy_(#[allow(unused)] p2p: bool) {
    let event = event_listener::Event::new();
    #[cfg(feature = "p2p")]
    let guid = zbus::Guid::generate();

    let session_conns_build = || {
        let service_conn_builder = connection::Builder::session()
            .unwrap()
            .name("org.freedesktop.MyService")
            .unwrap()
            .name("org.freedesktop.MyService.foo")
            .unwrap()
            .name("org.freedesktop.MyService.bar")
            .unwrap()
            .name("org.freedesktop.MyEmitsChangedSignalIface")
            .unwrap();
        let client_conn_builder = connection::Builder::session().unwrap();

        (service_conn_builder, client_conn_builder)
    };
    #[cfg(feature = "p2p")]
    let (service_conn_builder, client_conn_builder) = if p2p {
        #[cfg(unix)]
        {
            let (p0, p1) = UnixStream::pair().unwrap();

            (
                connection::Builder::unix_stream(p0)
                    .server(guid)
                    .unwrap()
                    .p2p(),
                connection::Builder::unix_stream(p1).p2p(),
            )
        }

        #[cfg(windows)]
        {
            #[cfg(not(feature = "tokio"))]
            {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let addr = listener.local_addr().unwrap();
                let p1 = std::net::TcpStream::connect(addr).unwrap();
                let p0 = listener.incoming().next().unwrap().unwrap();

                (
                    connection::Builder::tcp_stream(p0)
                        .server(guid)
                        .unwrap()
                        .p2p(),
                    connection::Builder::tcp_stream(p1).p2p(),
                )
            }

            #[cfg(feature = "tokio")]
            {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                let p1 = tokio::net::TcpStream::connect(addr).await.unwrap();
                let p0 = listener.accept().await.unwrap().0;

                (
                    connection::Builder::tcp_stream(p0)
                        .server(guid)
                        .unwrap()
                        .p2p(),
                    connection::Builder::tcp_stream(p1).p2p(),
                )
            }
        }
    } else {
        session_conns_build()
    };
    #[cfg(not(feature = "p2p"))]
    let (service_conn_builder, client_conn_builder) = session_conns_build();

    debug!(
        "Client connection builder created: {:?}",
        client_conn_builder
    );
    debug!(
        "Service connection builder created: {:?}",
        service_conn_builder
    );
    let (next_tx, mut next_rx) = channel(64);
    let iface = MyIface::new(next_tx.clone());
    let service_conn_builder = service_conn_builder
        .serve_at("/org/freedesktop/MyService", iface)
        .unwrap()
        .serve_at("/zbus/test", ObjectManager)
        .unwrap();
    debug!("ObjectServer set-up.");

    let (service_conn, client_conn) =
        futures_util::try_join!(service_conn_builder.build(), client_conn_builder.build(),)
            .unwrap();
    debug!("Client connection created: {:?}", client_conn);
    debug!("Service connection created: {:?}", service_conn);

    let listen = event.listen();
    let child = client_conn
        .executor()
        .spawn(my_iface_test(client_conn.clone(), event), "client_task");
    debug!("Child task spawned.");
    // Wait for the listener to be ready
    listen.await;
    debug!("Child task signaled it's ready.");

    let iface: InterfaceRef<MyIface> = service_conn
        .object_server()
        .interface("/org/freedesktop/MyService")
        .await
        .unwrap();
    iface
        .get()
        .await
        .count_changed(iface.signal_context())
        .await
        .unwrap();
    debug!("`PropertiesChanged` emitted for `Count` property.");

    loop {
        MyIface::alert_count(iface.signal_context(), 51)
            .await
            .unwrap();
        debug!("`AlertCount` signal emitted.");

        match next_rx.recv().await.unwrap() {
            NextAction::Quit => break,
            NextAction::CreateObj(key) => {
                let path = format!("/zbus/test/{key}");
                service_conn
                    .object_server()
                    .at(path.clone(), MyIface::new(next_tx.clone()))
                    .await
                    .unwrap();
                debug!("Object `{path}` added.");
            }
            NextAction::DestroyObj(key) => {
                let path = format!("/zbus/test/{key}");
                service_conn
                    .object_server()
                    .remove::<MyIface, _>(path.clone())
                    .await
                    .unwrap();
                debug!("Object `{path}` removed.");
            }
        }
    }
    debug!("Server done.");

    // don't close the connection before we end the loop
    drop(client_conn);
    debug!("Connection closed.");

    let val = child.await.unwrap();
    debug!("Client task done.");
    assert_eq!(val, 2);

    if p2p {
        debug!("p2p connection, no need to release names..");
        return;
    }

    // Release primary name explicitly and let others be released implicitly.
    assert_eq!(
        service_conn.release_name("org.freedesktop.MyService").await,
        Ok(true)
    );
    debug!("Bus name `org.freedesktop.MyService` released.");
    assert_eq!(
        service_conn
            .release_name("org.freedesktop.MyService.foo")
            .await,
        Ok(true)
    );
    debug!("Bus name `org.freedesktop.MyService.foo` released.");
    assert_eq!(
        service_conn
            .release_name("org.freedesktop.MyService.bar")
            .await,
        Ok(true)
    );
    debug!("Bus name `org.freedesktop.MyService.bar` released.");

    // Let's ensure all names were released.
    let proxy = zbus::fdo::DBusProxy::new(&service_conn).await.unwrap();
    debug!("DBusProxy created to ensure all names were released.");
    assert_eq!(
        proxy
            .name_has_owner("org.freedesktop.MyService".try_into().unwrap())
            .await,
        Ok(false)
    );
    assert_eq!(
        proxy
            .name_has_owner("org.freedesktop.MyService.foo".try_into().unwrap())
            .await,
        Ok(false)
    );
    assert_eq!(
        proxy
            .name_has_owner("org.freedesktop.MyService.bar".try_into().unwrap())
            .await,
        Ok(false)
    );
    debug!("Bus confirmed that all names were definitely released.");
}
