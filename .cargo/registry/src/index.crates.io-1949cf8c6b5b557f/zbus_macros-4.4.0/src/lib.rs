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

use proc_macro::TokenStream;
use syn::{
    parse_macro_input, punctuated::Punctuated, DeriveInput, ItemImpl, ItemTrait, Meta, Token,
};

mod error;
mod iface;
mod proxy;
mod utils;

/// Attribute macro for defining D-Bus proxies (using [`zbus::Proxy`] and
/// [`zbus::blocking::Proxy`]).
///
/// The macro must be applied on a `trait T`. Two matching `impl T` will provide an asynchronous
/// Proxy implementation, named `TraitNameProxy` and a blocking one, named `TraitNameProxyBlocking`.
/// The proxy instances can be created with the associated `new()` or `builder()` methods. The
/// former doesn't take any argument and uses the default service name and path. The later allows
/// you to specify non-default proxy arguments.
///
/// The following attributes are supported:
///
/// * `interface` - the name of the D-Bus interface this proxy is for.
///
/// * `default_service` - the default service this proxy should connect to.
///
/// * `default_path` - The default object path the method calls will be sent on and signals will be
///   sent for by the target service.
///
/// * `gen_async` - Whether or not to generate the asynchronous Proxy type.
///
/// * `gen_blocking` - Whether or not to generate the blocking Proxy type. If set to `false`, the
///   asynchronous proxy type will take the name `TraitNameProxy` (i-e no `Async` prefix).
///
/// * `async_name` - Specify the exact name of the asynchronous proxy type.
///
/// * `blocking_name` - Specify the exact name of the blocking proxy type.
///
/// * `assume_defaults` - whether to auto-generate values for `default_path` and `default_service`
///   if none are specified (default: `false`). `proxy` generates a warning if neither this
///   attribute nor one of the default values are specified. Please make sure to explicitly set
///   either this attribute or the default values, according to your needs.
///
/// Each trait method will be expanded to call to the associated D-Bus remote interface.
///
/// Trait methods accept `proxy` attributes:
///
/// * `name` - override the D-Bus name (pascal case form by default)
///
/// * `property` - expose the method as a property. If the method takes an argument, it must be a
///   setter, with a `set_` prefix. Otherwise, it's a getter. Additional sub-attributes exists to
///   control specific property behaviors:
///   * `emits_changed_signal` - specifies how property changes are signaled. Valid values are those
///     documented in [DBus specifications][dbus_emits_changed_signal]:
///     * `"true"` - (default) change signal is always emitted with the value included. This uses
///       the default caching behavior of the proxy, and generates a listener method for the change
///       signal.
///     * `"invalidates"` - change signal is emitted, but the value is not included in the signal.
///       This has the same behavior as `"true"`.
///     * `"const"` - property never changes, thus no signal is ever emitted for it. This uses the
///       default caching behavior of the proxy, but does not generate a listener method for the
///       change signal.
///     * `"false"` - change signal is not (guaranteed to be) emitted if the property changes. This
///       disables property value caching, and does not generate a listener method for the change
///       signal.
///
/// * `signal` - declare a signal just like a D-Bus method. Read the [Signals](#signals) section
///   below for details.
///
/// * `no_reply` - declare a method call that does not wait for a reply.
///
/// * `no_autostart` - declare a method call that will not trigger the bus to automatically launch
///   the destination service if it is not already running.
///
/// * `allow_interactive_auth` - declare a method call that is allowed to trigger an interactive
///   prompt for authorization or confirmation from the receiver.
///
/// * `object` - methods that returns an [`ObjectPath`] can be annotated with the `object` attribute
///   to specify the proxy object to be constructed from the returned [`ObjectPath`].
///
/// * `async_object` - if the assumptions made by `object` attribute about naming of the
///   asynchronous proxy type, don't fit your bill, you can use this to specify its exact name.
///
/// * `blocking_object` - if the assumptions made by `object` attribute about naming of the blocking
///   proxy type, don't fit your bill, you can use this to specify its exact name.
///
///   NB: Any doc comments provided shall be appended to the ones added by the macro.
///
/// # Signals
///
/// For each signal method declared, this macro will provide a method, named `receive_<method_name>`
/// to create a [`zbus::SignalStream`] ([`zbus::blocking::SignalIterator`] for the blocking proxy)
/// wrapper, named `<SignalName>Stream` (`<SignalName>Iterator` for the blocking proxy) that yield
/// a [`zbus::message::Message`] wrapper, named `<SignalName>`. This wrapper provides type safe
/// access to the signal arguments. It also implements `Deref<Target = Message>` to allow easy
/// access to the underlying [`zbus::message::Message`].
///
/// # Example
///
/// ```no_run
/// # use std::error::Error;
/// use zbus_macros::proxy;
/// use zbus::{blocking::Connection, Result, fdo, zvariant::Value};
/// use futures_util::stream::StreamExt;
/// use async_io::block_on;
///
/// #[proxy(
///     interface = "org.test.SomeIface",
///     default_service = "org.test.SomeService",
///     default_path = "/org/test/SomeObject"
/// )]
/// trait SomeIface {
///     fn do_this(&self, with: &str, some: u32, arg: &Value<'_>) -> Result<bool>;
///
///     #[zbus(property)]
///     fn a_property(&self) -> fdo::Result<String>;
///
///     #[zbus(property)]
///     fn set_a_property(&self, a_property: &str) -> fdo::Result<()>;
///
///     #[zbus(signal)]
///     fn some_signal(&self, arg1: &str, arg2: u32) -> fdo::Result<()>;
///
///     #[zbus(object = "SomeOtherIface", blocking_object = "SomeOtherInterfaceBlock")]
///     // The method will return a `SomeOtherIfaceProxy` or `SomeOtherIfaceProxyBlock`, depending
///     // on whether it is called on `SomeIfaceProxy` or `SomeIfaceProxyBlocking`, respectively.
///     //
///     // NB: We explicitly specified the exact name of the blocking proxy type. If we hadn't,
///     // `SomeOtherIfaceProxyBlock` would have been assumed and expected. We could also specify
///     // the specific name of the asynchronous proxy types, using the `async_object` attribute.
///     fn some_method(&self, arg1: &str);
/// }
///
/// #[proxy(
///     interface = "org.test.SomeOtherIface",
///     default_service = "org.test.SomeOtherService",
///     blocking_name = "SomeOtherInterfaceBlock",
/// )]
/// trait SomeOtherIface {}
///
/// let connection = Connection::session()?;
/// // Use `builder` to override the default arguments, `new` otherwise.
/// let proxy = SomeIfaceProxyBlocking::builder(&connection)
///                .destination("org.another.Service")?
///                .cache_properties(zbus::CacheProperties::No)
///                .build()?;
/// let _ = proxy.do_this("foo", 32, &Value::new(true));
/// let _ = proxy.set_a_property("val");
///
/// let signal = proxy.receive_some_signal()?.next().unwrap();
/// let args = signal.args()?;
/// println!("arg1: {}, arg2: {}", args.arg1(), args.arg2());
///
/// // Now the same again, but asynchronous.
/// block_on(async move {
///     let proxy = SomeIfaceProxy::builder(&connection.into())
///                    .cache_properties(zbus::CacheProperties::No)
///                    .build()
///                    .await
///                    .unwrap();
///     let _ = proxy.do_this("foo", 32, &Value::new(true)).await;
///     let _ = proxy.set_a_property("val").await;
///
///     let signal = proxy.receive_some_signal().await?.next().await.unwrap();
///     let args = signal.args()?;
///     println!("arg1: {}, arg2: {}", args.arg1(), args.arg2());
///
///     Ok::<(), zbus::Error>(())
/// })?;
///
/// # Ok::<_, Box<dyn Error + Send + Sync>>(())
/// ```
///
/// [`zbus_polkit`] is a good example of how to bind a real D-Bus API.
///
/// [`zbus_polkit`]: https://docs.rs/zbus_polkit/1.0.0/zbus_polkit/policykit1/index.html
/// [`zbus::Proxy`]: https://docs.rs/zbus/latest/zbus/proxy/struct.Proxy.html
/// [`zbus::message::Message`]: https://docs.rs/zbus/latest/zbus/message/struct.Message.html
/// [`zbus::blocking::Proxy`]: https://docs.rs/zbus/latest/zbus/blocking/proxy/struct.Proxy.html
/// [`zbus::SignalStream`]: https://docs.rs/zbus/latest/zbus/proxy/struct.SignalStream.html
/// [`zbus::blocking::SignalIterator`]: https://docs.rs/zbus/latest/zbus/blocking/proxy/struct.SignalIterator.html
/// [`ObjectPath`]: https://docs.rs/zvariant/latest/zvariant/struct.ObjectPath.html
/// [dbus_emits_changed_signal]: https://dbus.freedesktop.org/doc/dbus-specification.html#introspection-format
#[proc_macro_attribute]
pub fn proxy(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated<Meta, Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as ItemTrait);
    proxy::expand::<proxy::TraitAttributes, proxy::MethodAttributes>(args, input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[deprecated = "Use `#[proxy(...)]` proc macro with `#[zbus(...)]` item attributes instead."]
#[proc_macro_attribute]
pub fn dbus_proxy(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated<Meta, Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as ItemTrait);
    proxy::expand::<proxy::old::TraitAttributes, proxy::old::MethodAttributes>(args, input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Attribute macro for implementing a D-Bus interface.
///
/// The macro must be applied on an `impl T`. All methods will be exported, either as methods,
/// properties or signal depending on the item attributes. It will implement the [`Interface`] trait
/// `for T` on your behalf, to handle the message dispatching and introspection support.
///
/// The trait accepts the `interface` attributes:
///
/// * `name` - the D-Bus interface name
///
/// * `spawn` - Controls the spawning of tasks for method calls. By default, `true`, allowing zbus
///   to spawn a separate task for each method call. This default behavior can lead to methods being
///   handled out of their received order, which might not always align with expected or desired
///   behavior.
///
///   - **When True (Default):** Suitable for interfaces where method calls are independent of each
///   other or can be processed asynchronously without strict ordering. In scenarios where a client
///   must wait for a reply before making further dependent calls, this default behavior is
///   appropriate.
///
///   - **When False:** Use this setting to ensure methods are handled in the order they are
///   received, which is crucial for interfaces requiring sequential processing of method calls.
///   However, care must be taken to avoid making D-Bus method calls from within your interface
///   methods when this setting is false, as it may lead to deadlocks under certain conditions.
///
/// * `proxy` - If specified, a proxy type will also be generated for the interface. This attribute
///   supports all the [`macro@proxy`]-specific sub-attributes (e.g `gen_async`). The common
///   sub-attributes (e.g `name`) are automatically forworded to the [`macro@proxy`] macro.
///
/// The methods accepts the `interface` attributes:
///
/// * `name` - override the D-Bus name (pascal case form of the method by default)
///
/// * `property` - expose the method as a property. If the method takes an argument, it must be a
///   setter, with a `set_` prefix. Otherwise, it's a getter. If it may fail, a property method must
///   return `zbus::fdo::Result`. An additional sub-attribute exists to control the emission of
///   signals on changes to the property:
///   * `emits_changed_signal` - specifies how property changes are signaled. Valid values are those
///     documented in [DBus specifications][dbus_emits_changed_signal]:
///     * `"true"` - (default) the change signal is always emitted when the property's setter is
///     called. The value of the property is included in the signal.
///     * `"invalidates"` - the change signal is emitted, but the value is not included in the
///     signal.
///     * `"const"` - the property never changes, thus no signal is ever emitted for it.
///     * `"false"` - the change signal is not emitted if the property changes.

/// * `signal` - the method is a "signal". It must be a method declaration (without body). Its code
///   block will be expanded to emit the signal from the object path associated with the interface
///   instance.
///
///   You can call a signal method from a an interface method, or from an [`ObjectServer::with`]
///   function.
///
/// * `out_args` - When returning multiple values from a method, naming the out arguments become
///   important. You can use `out_args` to specify their names.
///
/// * `proxy` - Use this to specify the [`macro@proxy`]-specific method sub-attributes (e.g
///   `object`). The common sub-attributes (e.g `name`) are automatically forworded to the
///   [`macro@proxy`] macro.
///
///   In such case, your method must return a tuple containing
///   your out arguments, in the same order as passed to `out_args`.
///
/// The `struct_return` attribute (from zbus 1.x) is no longer supported. If you want to return a
/// single structure from a method, declare it to return a tuple containing either a named structure
/// or a nested tuple.
///
/// Note: a `<property_name_in_snake_case>_changed` method is generated for each property: this
/// method emits the "PropertiesChanged" signal for the associated property. The setter (if it
/// exists) will automatically call this method. For instance, a property setter named `set_foo`
/// will be called to set the property "Foo", and will emit the "PropertiesChanged" signal with the
/// new value for "Foo". Other changes to the "Foo" property can be signaled manually with the
/// generated `foo_changed` method. In addition, a `<property_name_in_snake_case>_invalidated`
/// method is also generated that much like `_changed` method, emits a "PropertyChanged" signal
/// but does not send over the new value of the property along with it. It is usually best to avoid
/// using this since it will force all interested peers to fetch the new value and hence result in
/// excess traffic on the bus.
///
/// The method arguments support the following `zbus` attributes:
///
/// * `object_server` - This marks the method argument to receive a reference to the
///   [`ObjectServer`] this method was called by.
/// * `connection` - This marks the method argument to receive a reference to the [`Connection`] on
///   which the method call was received.
/// * `header` - This marks the method argument to receive the message header associated with the
///   D-Bus method call being handled.
/// * `signal_context` - This marks the method argument to receive a [`SignalContext`] instance,
///   which is needed for emitting signals the easy way.
///
/// # Example
///
/// ```
/// # use std::error::Error;
/// use zbus_macros::interface;
/// use zbus::{ObjectServer, object_server::SignalContext, message::Header};
///
/// struct Example {
///     _some_data: String,
/// }
///
/// #[interface(name = "org.myservice.Example")]
/// impl Example {
///     // "Quit" method. A method may throw errors.
///     async fn quit(
///         &self,
///         #[zbus(header)]
///         hdr: Header<'_>,
///         #[zbus(signal_context)]
///         ctxt: SignalContext<'_>,
///         #[zbus(object_server)]
///         _server: &ObjectServer,
///     ) -> zbus::fdo::Result<()> {
///         let path = hdr.path().unwrap();
///         let msg = format!("You are leaving me on the {} path?", path);
///         Example::bye(&ctxt, &msg).await?;
///
///         // Do some asynchronous tasks before quitting..
///
///         Ok(())
///     }
///
///     // "TheAnswer" property (note: the "name" attribute), with its associated getter.
///     // A `the_answer_changed` method has also been generated to emit the
///     // "PropertiesChanged" signal for this property.
///     #[zbus(property, name = "TheAnswer")]
///     fn answer(&self) -> u32 {
///         2 * 3 * 7
///     }
///
///     // "IFail" property with its associated getter.
///     // An `i_fail_changed` method has also been generated to emit the
///     // "PropertiesChanged" signal for this property.
///     #[zbus(property)]
///     fn i_fail(&self) -> zbus::fdo::Result<i32> {
///         Err(zbus::fdo::Error::UnknownProperty("IFail".into()))
///     }
///
///     // "Bye" signal (note: no implementation body).
///     #[zbus(signal)]
///     async fn bye(signal_ctxt: &SignalContext<'_>, message: &str) -> zbus::Result<()>;
///
///     #[zbus(out_args("answer", "question"))]
///     fn meaning_of_life(&self) -> zbus::fdo::Result<(i32, String)> {
///         Ok((42, String::from("Meaning of life")))
///     }
/// }
///
/// # Ok::<_, Box<dyn Error + Send + Sync>>(())
/// ```
///
/// See also [`ObjectServer`] documentation to learn how to export an interface over a `Connection`.
///
/// [`ObjectServer`]: https://docs.rs/zbus/latest/zbus/object_server/struct.ObjectServer.html
/// [`ObjectServer::with`]: https://docs.rs/zbus/latest/zbus/object_server/struct.ObjectServer.html#method.with
/// [`Connection`]: https://docs.rs/zbus/latest/zbus/connection/struct.Connection.html
/// [`Connection::emit_signal()`]: https://docs.rs/zbus/latest/zbus/connection/struct.Connection.html#method.emit_signal
/// [`SignalContext`]: https://docs.rs/zbus/latest/zbus/object_server/struct.SignalContext.html
/// [`Interface`]: https://docs.rs/zbus/latest/zbus/object_server/trait.Interface.html
/// [dbus_emits_changed_signal]: https://dbus.freedesktop.org/doc/dbus-specification.html#introspection-format
#[proc_macro_attribute]
pub fn interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated<Meta, Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as ItemImpl);
    iface::expand::<iface::ImplAttributes, iface::MethodAttributes>(args, input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[deprecated = "Use `#[interface(...)]` proc macro with `#[zbus(...)]` item attributes instead."]
#[proc_macro_attribute]
pub fn dbus_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated<Meta, Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as ItemImpl);
    iface::expand::<iface::old::ImplAttributes, iface::old::MethodAttributes>(args, input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive macro for implementing [`zbus::DBusError`] trait.
///
/// This macro makes it easy to implement the [`zbus::DBusError`] trait for your custom error type
/// (currently only enums are supported).
///
/// If a special variant marked with the `zbus` attribute is present, `From<zbus::Error>` is
/// also implemented for your type. This variant can only have a single unnamed field of type
/// [`zbus::Error`]. This implementation makes it possible for you to declare proxy methods to
/// directly return this type, rather than [`zbus::Error`].
///
/// Each variant (except for the special `zbus` one) can optionally have a (named or unnamed)
/// `String` field (which is used as the human-readable error description).
///
/// # Example
///
/// ```
/// use zbus_macros::DBusError;
///
/// #[derive(DBusError, Debug)]
/// #[zbus(prefix = "org.myservice.App")]
/// enum Error {
///     #[zbus(error)]
///     ZBus(zbus::Error),
///     FileNotFound(String),
///     OutOfMemory,
/// }
/// ```
///
/// [`zbus::DBusError`]: https://docs.rs/zbus/latest/zbus/trait.DBusError.html
/// [`zbus::Error`]: https://docs.rs/zbus/latest/zbus/enum.Error.html
/// [`zvariant::Type`]: https://docs.rs/zvariant/latest/zvariant/trait.Type.html
/// [`serde::Serialize`]: https://docs.rs/serde/1.0.132/serde/trait.Serialize.html
#[proc_macro_derive(DBusError, attributes(zbus))]
pub fn derive_dbus_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    error::expand_derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
