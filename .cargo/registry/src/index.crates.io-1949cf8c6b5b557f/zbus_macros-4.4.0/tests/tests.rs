use futures_util::{
    future::{select, Either},
    stream::StreamExt,
};
use std::future::ready;
use zbus::{block_on, fdo, object_server::SignalContext, proxy::CacheProperties};
use zbus_macros::{interface, proxy, DBusError};

mod param {
    #[zbus_macros::proxy(
        interface = "org.freedesktop.zbus_macros.ProxyParam",
        default_service = "org.freedesktop.zbus_macros",
        default_path = "/org/freedesktop/zbus_macros/test"
    )]
    trait ProxyParam {
        #[zbus(object = "super::test::Test")]
        fn some_method<T>(&self, test: &T);
    }
}

mod test {
    use zbus::{
        fdo,
        zvariant::{OwnedStructure, Structure},
    };

    #[zbus_macros::proxy(
        assume_defaults = false,
        interface = "org.freedesktop.zbus_macros.Test",
        default_service = "org.freedesktop.zbus_macros"
    )]
    trait Test {
        /// comment for a_test()
        fn a_test(&self, val: &str) -> zbus::Result<u32>;

        /// The generated proxies implement both `zvariant::Type` and `serde::ser::Serialize`
        /// which is useful to pass in a proxy as a param. It serializes it as an `ObjectPath`.
        fn some_method<T>(&self, object_path: &T) -> zbus::Result<()>;

        /// A call accepting an argument that only implements DynamicType and Serialize.
        fn test_dyn_type(&self, arg: Structure<'_>, arg2: u32) -> zbus::Result<()>;

        /// A call returning an type that only implements DynamicDeserialize
        fn test_dyn_ret(&self) -> zbus::Result<OwnedStructure>;

        #[zbus(name = "CheckRENAMING")]
        fn check_renaming(&self) -> zbus::Result<Vec<u8>>;

        #[zbus(property)]
        fn property(&self) -> fdo::Result<Vec<String>>;

        #[zbus(property(emits_changed_signal = "const"))]
        fn a_const_property(&self) -> fdo::Result<Vec<String>>;

        #[zbus(property(emits_changed_signal = "false"))]
        fn a_live_property(&self) -> fdo::Result<Vec<String>>;

        #[zbus(property)]
        fn set_property(&self, val: u16) -> fdo::Result<()>;

        #[zbus(signal)]
        fn a_signal<T>(&self, arg: u8, other: T) -> fdo::Result<()>
        where
            T: AsRef<str>;
    }
}

#[test]
fn test_proxy() {
    block_on(async move {
        let connection = zbus::Connection::session().await.unwrap();
        let proxy = test::TestProxy::builder(&connection)
            .path("/org/freedesktop/zbus_macros/test")
            .unwrap()
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .unwrap();
        fdo::DBusProxy::builder(&connection)
            .build()
            .await
            .unwrap()
            .request_name(
                "org.freedesktop.zbus_macros".try_into().unwrap(),
                fdo::RequestNameFlags::DoNotQueue.into(),
            )
            .await
            .unwrap();
        let mut stream = proxy.receive_a_signal().await.unwrap();

        let left_future = async move {
            // These calls will never happen so just testing the build mostly.
            let signal = stream.next().await.unwrap();
            let args = signal.args::<&str>().unwrap();
            assert_eq!(*args.arg(), 0u8);
            assert_eq!(*args.other(), "whatever");
        };
        futures_util::pin_mut!(left_future);
        let right_future = async {
            ready(()).await;
        };
        futures_util::pin_mut!(right_future);

        if let Either::Left((_, _)) = select(left_future, right_future).await {
            panic!("Shouldn't be receiving our dummy signal: `ASignal`");
        }
    });
}

#[test]
fn test_derive_error() {
    #[derive(Debug, DBusError)]
    #[zbus(prefix = "org.freedesktop.zbus")]
    enum Test {
        #[zbus(error)]
        ZBus(zbus::Error),
        SomeExcuse,
        #[zbus(name = "I.Am.Sorry.Dave")]
        IAmSorryDave(String),
        LetItBe {
            desc: String,
        },
    }
}

#[test]
fn test_interface() {
    use serde::{Deserialize, Serialize};
    use zbus::{
        object_server::Interface,
        zvariant::{Type, Value},
    };

    struct Test<T> {
        something: String,
        generic: T,
    }

    #[derive(Serialize, Deserialize, Type, Value)]
    struct MyCustomPropertyType(u32);

    #[interface(name = "org.freedesktop.zbus.Test", spawn = false)]
    impl<T: 'static> Test<T>
    where
        T: serde::ser::Serialize + zbus::zvariant::Type + Send + Sync,
    {
        /// Testing `no_arg` documentation is reflected in XML.
        fn no_arg(&self) {
            unimplemented!()
        }

        // Also tests that mut argument bindings work for regular methods
        #[allow(unused_assignments)]
        fn str_u32(&self, mut val: &str) -> zbus::fdo::Result<u32> {
            let res = val
                .parse()
                .map_err(|e| zbus::fdo::Error::Failed(format!("Invalid val: {e}")));
            val = "test mut";
            res
        }

        // TODO: naming output arguments after "RFC: Structural Records #2584"
        fn many_output(&self) -> zbus::fdo::Result<(&T, String)> {
            Ok((&self.generic, self.something.clone()))
        }

        fn pair_output(&self) -> zbus::fdo::Result<((u32, String),)> {
            unimplemented!()
        }

        #[zbus(property)]
        fn my_custom_property(&self) -> MyCustomPropertyType {
            unimplemented!()
        }

        // Also tests that mut argument bindings work for properties
        #[zbus(property)]
        fn set_my_custom_property(&self, mut _value: MyCustomPropertyType) {
            _value = MyCustomPropertyType(42);
        }

        // Test that the emits_changed_signal property results in the correct annotation
        #[zbus(property(emits_changed_signal = "false"))]
        fn my_custom_property_emits_false(&self) -> MyCustomPropertyType {
            unimplemented!()
        }

        #[zbus(property(emits_changed_signal = "invalidates"))]
        fn my_custom_property_emits_invalidates(&self) -> MyCustomPropertyType {
            unimplemented!()
        }

        #[zbus(property(emits_changed_signal = "const"))]
        fn my_custom_property_emits_const(&self) -> MyCustomPropertyType {
            unimplemented!()
        }

        #[zbus(name = "CheckVEC")]
        fn check_vec(&self) -> Vec<u8> {
            unimplemented!()
        }

        /// Testing my_prop documentation is reflected in XML.
        ///
        /// And that too.
        #[zbus(property)]
        fn my_prop(&self) -> u16 {
            unimplemented!()
        }

        #[zbus(property)]
        fn set_my_prop(&mut self, _val: u16) {
            unimplemented!()
        }

        /// Emit a signal.
        #[zbus(signal)]
        async fn signal(ctxt: &SignalContext<'_>, arg: u8, other: &str) -> zbus::Result<()>;
    }

    const EXPECTED_XML: &str = r#"<interface name="org.freedesktop.zbus.Test">
  <!--
   Testing `no_arg` documentation is reflected in XML.
   -->
  <method name="NoArg">
  </method>
  <method name="StrU32">
    <arg name="val" type="s" direction="in"/>
    <arg type="u" direction="out"/>
  </method>
  <method name="ManyOutput">
    <arg type="u" direction="out"/>
    <arg type="s" direction="out"/>
  </method>
  <method name="PairOutput">
    <arg type="(us)" direction="out"/>
  </method>
  <method name="CheckVEC">
    <arg type="ay" direction="out"/>
  </method>
  <!--
   Emit a signal.
   -->
  <signal name="Signal">
    <arg name="arg" type="y"/>
    <arg name="other" type="s"/>
  </signal>
  <property name="MyCustomProperty" type="u" access="readwrite"/>
  <property name="MyCustomPropertyEmitsConst" type="u" access="read">
    <annotation name="org.freedesktop.DBus.Property.EmitsChangedSignal" value="const"/>
  </property>
  <property name="MyCustomPropertyEmitsFalse" type="u" access="read">
    <annotation name="org.freedesktop.DBus.Property.EmitsChangedSignal" value="false"/>
  </property>
  <property name="MyCustomPropertyEmitsInvalidates" type="u" access="read">
    <annotation name="org.freedesktop.DBus.Property.EmitsChangedSignal" value="invalidates"/>
  </property>
  <!--
   Testing my_prop documentation is reflected in XML.

   And that too.
   -->
  <property name="MyProp" type="q" access="readwrite"/>
</interface>
"#;
    let t = Test {
        something: String::from("somewhere"),
        generic: 42u32,
    };
    let mut xml = String::new();
    t.introspect_to_writer(&mut xml, 0);
    assert_eq!(xml, EXPECTED_XML);

    assert_eq!(Test::<u32>::name(), "org.freedesktop.zbus.Test");

    if false {
        block_on(async {
            // check compilation
            let c = zbus::Connection::session().await.unwrap();
            let s = c.object_server();
            let m = zbus::message::Message::method("/", "StrU32")
                .unwrap()
                .build(&(42,))
                .unwrap();
            let _ = t.call(&s, &c, &m, "StrU32".try_into().unwrap());
            let ctxt = SignalContext::new(&c, "/does/not/matter").unwrap();
            block_on(Test::<u32>::signal(&ctxt, 23, "ergo sum")).unwrap();
        });
    }
}

mod signal_from_message {
    use super::*;
    use zbus::message::Message;

    #[proxy(
        interface = "org.freedesktop.zbus_macros.Test",
        default_service = "org.freedesktop.zbus_macros",
        default_path = "/org/freedesktop/zbus_macros/test"
    )]
    trait Test {
        #[zbus(signal)]
        fn signal_u8(&self, arg: u8) -> fdo::Result<()>;

        #[zbus(signal)]
        fn signal_string(&self, arg: String) -> fdo::Result<()>;
    }

    #[test]
    fn signal_u8() {
        let message = Message::signal(
            "/org/freedesktop/zbus_macros/test",
            "org.freedesktop.zbus_macros.Test",
            "SignalU8",
        )
        .expect("Failed to create signal message builder")
        .build(&(1u8,))
        .expect("Failed to build signal message");

        assert!(
            SignalU8::from_message(message.clone()).is_some(),
            "Message is a SignalU8"
        );
        assert!(
            SignalString::from_message(message).is_none(),
            "Message is not a SignalString"
        );
    }

    #[test]
    fn signal_string() {
        let message = Message::signal(
            "/org/freedesktop/zbus_macros/test",
            "org.freedesktop.zbus_macros.Test",
            "SignalString",
        )
        .expect("Failed to create signal message builder")
        .build(&(String::from("test"),))
        .expect("Failed to build signal message");

        assert!(
            SignalString::from_message(message.clone()).is_some(),
            "Message is a SignalString"
        );
        assert!(
            SignalU8::from_message(message).is_none(),
            "Message is not a SignalU8"
        );
    }

    #[test]
    fn wrong_data() {
        let message = Message::signal(
            "/org/freedesktop/zbus_macros/test",
            "org.freedesktop.zbus_macros.Test",
            "SignalU8",
        )
        .expect("Failed to create signal message builder")
        .build(&(String::from("test"),))
        .expect("Failed to build signal message");

        let signal = SignalU8::from_message(message).expect("Message is a SignalU8");
        signal
            .args()
            .expect_err("Message does not have correct data");
    }
}
