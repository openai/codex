//! The object server API.

use event_listener::{Event, EventListener};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Write,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tracing::{debug, instrument, trace, trace_span, Instrument};

use static_assertions::assert_impl_all;
use zbus_names::InterfaceName;
use zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Signature, Type, Value};

use crate::{
    async_lock::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    connection::WeakConnection,
    fdo,
    fdo::{Introspectable, ManagedObjects, ObjectManager, Peer, Properties},
    message::{Header, Message},
    Connection, Error, Result,
};

mod interface;
pub(crate) use interface::ArcInterface;
pub use interface::{DispatchResult, Interface};

mod signal_context;
pub use signal_context::SignalContext;

/// Opaque structure that derefs to an `Interface` type.
pub struct InterfaceDeref<'d, I> {
    iface: RwLockReadGuard<'d, dyn Interface>,
    phantom: PhantomData<I>,
}

impl<I> Deref for InterfaceDeref<'_, I>
where
    I: Interface,
{
    type Target = I;

    fn deref(&self) -> &I {
        self.iface.downcast_ref::<I>().unwrap()
    }
}

/// Opaque structure that mutably derefs to an `Interface` type.
pub struct InterfaceDerefMut<'d, I> {
    iface: RwLockWriteGuard<'d, dyn Interface>,
    phantom: PhantomData<I>,
}

impl<I> Deref for InterfaceDerefMut<'_, I>
where
    I: Interface,
{
    type Target = I;

    fn deref(&self) -> &I {
        self.iface.downcast_ref::<I>().unwrap()
    }
}

impl<I> DerefMut for InterfaceDerefMut<'_, I>
where
    I: Interface,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.iface.downcast_mut::<I>().unwrap()
    }
}

/// Wrapper over an interface, along with its corresponding `SignalContext`
/// instance. A reference to the underlying interface may be obtained via
/// [`InterfaceRef::get`] and [`InterfaceRef::get_mut`].
pub struct InterfaceRef<I> {
    ctxt: SignalContext<'static>,
    lock: Arc<RwLock<dyn Interface>>,
    phantom: PhantomData<I>,
}

impl<I> InterfaceRef<I>
where
    I: 'static,
{
    /// Get a reference to the underlying interface.
    ///
    /// **WARNING:** If methods (e.g property setters) in `ObjectServer` require `&mut self`
    /// `ObjectServer` will not be able to access the interface in question until all references
    /// of this method are dropped, it is highly recommended that the scope of the interface
    /// returned is restricted.
    pub async fn get(&self) -> InterfaceDeref<'_, I> {
        let iface = self.lock.read().await;

        iface
            .downcast_ref::<I>()
            .expect("Unexpected interface type");

        InterfaceDeref {
            iface,
            phantom: PhantomData,
        }
    }

    /// Get a reference to the underlying interface.
    ///
    /// **WARNINGS:** Since the `ObjectServer` will not be able to access the interface in question
    /// until the return value of this method is dropped, it is highly recommended that the scope
    /// of the interface returned is restricted.
    ///
    /// # Errors
    ///
    /// If the interface at this instance's path is not valid, `Error::InterfaceNotFound` error is
    /// returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # use async_io::block_on;
    /// # use zbus::{Connection, interface};
    ///
    /// struct MyIface(u32);
    ///
    /// #[interface(name = "org.myiface.MyIface")]
    /// impl MyIface {
    ///    #[zbus(property)]
    ///    async fn count(&self) -> u32 {
    ///        self.0
    ///    }
    /// }
    ///
    /// # block_on(async {
    /// // Setup connection and object_server etc here and then in another part of the code:
    /// # let connection = Connection::session().await?;
    /// #
    /// # let path = "/org/zbus/path";
    /// # connection.object_server().at(path, MyIface(22)).await?;
    /// let object_server = connection.object_server();
    /// let iface_ref = object_server.interface::<_, MyIface>(path).await?;
    /// let mut iface = iface_ref.get_mut().await;
    /// iface.0 = 42;
    /// iface.count_changed(iface_ref.signal_context()).await?;
    /// # Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// # })?;
    /// #
    /// # Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// ```
    pub async fn get_mut(&self) -> InterfaceDerefMut<'_, I> {
        let mut iface = self.lock.write().await;

        iface
            .downcast_ref::<I>()
            .expect("Unexpected interface type");
        iface
            .downcast_mut::<I>()
            .expect("Unexpected interface type");

        InterfaceDerefMut {
            iface,
            phantom: PhantomData,
        }
    }

    pub fn signal_context(&self) -> &SignalContext<'static> {
        &self.ctxt
    }
}

impl<I> Clone for InterfaceRef<I> {
    fn clone(&self) -> Self {
        Self {
            ctxt: self.ctxt.clone(),
            lock: self.lock.clone(),
            phantom: PhantomData,
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct Node {
    path: OwnedObjectPath,
    children: HashMap<String, Node>,
    interfaces: HashMap<InterfaceName<'static>, ArcInterface>,
}

impl Node {
    pub(crate) fn new(path: OwnedObjectPath) -> Self {
        let mut node = Self {
            path,
            ..Default::default()
        };
        assert!(node.add_interface(Peer));
        assert!(node.add_interface(Introspectable));
        assert!(node.add_interface(Properties));

        node
    }

    // Get the child Node at path.
    pub(crate) fn get_child(&self, path: &ObjectPath<'_>) -> Option<&Node> {
        let mut node = self;

        for i in path.split('/').skip(1) {
            if i.is_empty() {
                continue;
            }
            match node.children.get(i) {
                Some(n) => node = n,
                None => return None,
            }
        }

        Some(node)
    }

    // Get the child Node at path. Optionally create one if it doesn't exist.
    // It also returns the path of parent node that implements ObjectManager (if any). If multiple
    // parents implement it (they shouldn't), then the closest one is returned.
    fn get_child_mut(
        &mut self,
        path: &ObjectPath<'_>,
        create: bool,
    ) -> (Option<&mut Node>, Option<ObjectPath<'_>>) {
        let mut node = self;
        let mut node_path = String::new();
        let mut obj_manager_path = None;

        for i in path.split('/').skip(1) {
            if i.is_empty() {
                continue;
            }

            if node.interfaces.contains_key(&ObjectManager::name()) {
                obj_manager_path = Some((*node.path).clone());
            }

            write!(&mut node_path, "/{i}").unwrap();
            match node.children.entry(i.into()) {
                Entry::Vacant(e) => {
                    if create {
                        let path = node_path.as_str().try_into().expect("Invalid Object Path");
                        node = e.insert(Node::new(path));
                    } else {
                        return (None, obj_manager_path);
                    }
                }
                Entry::Occupied(e) => node = e.into_mut(),
            }
        }

        (Some(node), obj_manager_path)
    }

    pub(crate) fn interface_lock(&self, interface_name: InterfaceName<'_>) -> Option<ArcInterface> {
        self.interfaces.get(&interface_name).cloned()
    }

    fn remove_interface(&mut self, interface_name: InterfaceName<'static>) -> bool {
        self.interfaces.remove(&interface_name).is_some()
    }

    fn is_empty(&self) -> bool {
        !self.interfaces.keys().any(|k| {
            *k != Peer::name()
                && *k != Introspectable::name()
                && *k != Properties::name()
                && *k != ObjectManager::name()
        })
    }

    fn remove_node(&mut self, node: &str) -> bool {
        self.children.remove(node).is_some()
    }

    fn add_arc_interface(&mut self, name: InterfaceName<'static>, arc_iface: ArcInterface) -> bool {
        match self.interfaces.entry(name) {
            Entry::Vacant(e) => {
                e.insert(arc_iface);
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    fn add_interface<I>(&mut self, iface: I) -> bool
    where
        I: Interface,
    {
        self.add_arc_interface(I::name(), ArcInterface::new(iface))
    }

    async fn introspect_to_writer<W: Write + Send>(&self, writer: &mut W) {
        enum Fragment<'a> {
            /// Represent an unclosed node tree, could be further splitted into sub-`Fragment`s
            Node {
                name: &'a str,
                node: &'a Node,
                level: usize,
            },
            /// Represent a closing `</node>`
            End { level: usize },
        }

        let mut stack = Vec::new();
        stack.push(Fragment::Node {
            name: "",
            node: self,
            level: 0,
        });

        // This can be seen as traversing the fragment tree in pre-order DFS with formatted XML
        // fragment, splitted `Fragment::Node`s and `Fragment::End` being current node, left
        // subtree and right leaf respectively.
        while let Some(fragment) = stack.pop() {
            match fragment {
                Fragment::Node { name, node, level } => {
                    stack.push(Fragment::End { level });

                    for (name, node) in &node.children {
                        stack.push(Fragment::Node {
                            name,
                            node,
                            level: level + 2,
                        })
                    }

                    if level == 0 {
                        writeln!(
                            writer,
                            r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>"#
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            writer,
                            "{:indent$}<node name=\"{}\">",
                            "",
                            name,
                            indent = level
                        )
                        .unwrap();
                    }

                    for iface in node.interfaces.values() {
                        iface
                            .instance
                            .read()
                            .await
                            .introspect_to_writer(writer, level + 2);
                    }
                }
                Fragment::End { level } => {
                    writeln!(writer, "{:indent$}</node>", "", indent = level).unwrap();
                }
            }
        }
    }

    pub(crate) async fn introspect(&self) -> String {
        let mut xml = String::with_capacity(1024);

        self.introspect_to_writer(&mut xml).await;

        xml
    }

    pub(crate) async fn get_managed_objects(&self) -> fdo::Result<ManagedObjects> {
        let mut managed_objects = ManagedObjects::new();

        // Recursively get all properties of all interfaces of descendants.
        let mut node_list: Vec<_> = self.children.values().collect();
        while let Some(node) = node_list.pop() {
            let mut interfaces = HashMap::new();
            for iface_name in node.interfaces.keys().filter(|n| {
                // Filter standard interfaces.
                *n != &Peer::name()
                    && *n != &Introspectable::name()
                    && *n != &Properties::name()
                    && *n != &ObjectManager::name()
            }) {
                let props = node.get_properties(iface_name.clone()).await?;
                interfaces.insert(iface_name.clone().into(), props);
            }
            managed_objects.insert(node.path.clone(), interfaces);
            node_list.extend(node.children.values());
        }

        Ok(managed_objects)
    }

    async fn get_properties(
        &self,
        interface_name: InterfaceName<'_>,
    ) -> fdo::Result<HashMap<String, OwnedValue>> {
        self.interface_lock(interface_name)
            .expect("Interface was added but not found")
            .instance
            .read()
            .await
            .get_all()
            .await
    }
}

/// An object server, holding server-side D-Bus objects & interfaces.
///
/// Object servers hold interfaces on various object paths, and expose them over D-Bus.
///
/// All object paths will have the standard interfaces implemented on your behalf, such as
/// `org.freedesktop.DBus.Introspectable` or `org.freedesktop.DBus.Properties`.
///
/// # Example
///
/// This example exposes the `org.myiface.Example.Quit` method on the `/org/zbus/path`
/// path.
///
/// ```no_run
/// # use std::error::Error;
/// use zbus::{Connection, interface};
/// use event_listener::Event;
/// # use async_io::block_on;
///
/// struct Example {
///     // Interfaces are owned by the ObjectServer. They can have
///     // `&mut self` methods.
///     quit_event: Event,
/// }
///
/// impl Example {
///     fn new(quit_event: Event) -> Self {
///         Self { quit_event }
///     }
/// }
///
/// #[interface(name = "org.myiface.Example")]
/// impl Example {
///     // This will be the "Quit" D-Bus method.
///     async fn quit(&mut self) {
///         self.quit_event.notify(1);
///     }
///
///     // See `interface` documentation to learn
///     // how to expose properties & signals as well.
/// }
///
/// # block_on(async {
/// let connection = Connection::session().await?;
///
/// let quit_event = Event::new();
/// let quit_listener = quit_event.listen();
/// let interface = Example::new(quit_event);
/// connection
///     .object_server()
///     .at("/org/zbus/path", interface)
///     .await?;
///
/// quit_listener.await;
/// # Ok::<_, Box<dyn Error + Send + Sync>>(())
/// # })?;
/// # Ok::<_, Box<dyn Error + Send + Sync>>(())
/// ```
#[derive(Debug)]
pub struct ObjectServer {
    conn: WeakConnection,
    root: RwLock<Node>,
}

assert_impl_all!(ObjectServer: Send, Sync, Unpin);

impl ObjectServer {
    /// Creates a new D-Bus `ObjectServer`.
    pub(crate) fn new(conn: &Connection) -> Self {
        Self {
            conn: conn.into(),
            root: RwLock::new(Node::new("/".try_into().expect("zvariant bug"))),
        }
    }

    pub(crate) fn root(&self) -> &RwLock<Node> {
        &self.root
    }

    /// Register a D-Bus [`Interface`] at a given path. (see the example above)
    ///
    /// Typically you'd want your interfaces to be registered immediately after the associated
    /// connection is established and therefore use [`zbus::connection::Builder::serve_at`] instead.
    /// However, there are situations where you'd need to register interfaces dynamically and that's
    /// where this method becomes useful.
    ///
    /// If the interface already exists at this path, returns false.
    pub async fn at<'p, P, I>(&self, path: P, iface: I) -> Result<bool>
    where
        I: Interface,
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        self.add_arc_interface(path, I::name(), ArcInterface::new(iface))
            .await
    }

    pub(crate) async fn add_arc_interface<'p, P>(
        &self,
        path: P,
        name: InterfaceName<'static>,
        arc_iface: ArcInterface,
    ) -> Result<bool>
    where
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        let path = path.try_into().map_err(Into::into)?;
        let mut root = self.root().write().await;
        let (node, manager_path) = root.get_child_mut(&path, true);
        let node = node.unwrap();
        let added = node.add_arc_interface(name.clone(), arc_iface);
        if added {
            if name == ObjectManager::name() {
                // Just added an object manager. Need to signal all managed objects under it.
                let ctxt = SignalContext::new(&self.connection(), path)?;
                let objects = node.get_managed_objects().await?;
                for (path, owned_interfaces) in objects {
                    let interfaces = owned_interfaces
                        .iter()
                        .map(|(i, props)| {
                            let props = props
                                .iter()
                                .map(|(k, v)| Ok((k.as_str(), Value::try_from(v)?)))
                                .collect::<Result<_>>();
                            Ok((i.into(), props?))
                        })
                        .collect::<Result<_>>()?;
                    ObjectManager::interfaces_added(&ctxt, &path, &interfaces).await?;
                }
            } else if let Some(manager_path) = manager_path {
                let ctxt = SignalContext::new(&self.connection(), manager_path.clone())?;
                let mut interfaces = HashMap::new();
                let owned_props = node.get_properties(name.clone()).await?;
                let props = owned_props
                    .iter()
                    .map(|(k, v)| Ok((k.as_str(), Value::try_from(v)?)))
                    .collect::<Result<_>>()?;
                interfaces.insert(name, props);

                ObjectManager::interfaces_added(&ctxt, &path, &interfaces).await?;
            }
        }

        Ok(added)
    }

    /// Unregister a D-Bus [`Interface`] at a given path.
    ///
    /// If there are no more interfaces left at that path, destroys the object as well.
    /// Returns whether the object was destroyed.
    pub async fn remove<'p, I, P>(&self, path: P) -> Result<bool>
    where
        I: Interface,
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        let path = path.try_into().map_err(Into::into)?;
        let mut root = self.root.write().await;
        let (node, manager_path) = root.get_child_mut(&path, false);
        let node = node.ok_or(Error::InterfaceNotFound)?;
        if !node.remove_interface(I::name()) {
            return Err(Error::InterfaceNotFound);
        }
        if let Some(manager_path) = manager_path {
            let ctxt = SignalContext::new(&self.connection(), manager_path.clone())?;
            ObjectManager::interfaces_removed(&ctxt, &path, &[I::name()]).await?;
        }
        if node.is_empty() {
            let mut path_parts = path.rsplit('/').filter(|i| !i.is_empty());
            let last_part = path_parts.next().unwrap();
            let ppath = ObjectPath::from_string_unchecked(
                path_parts.fold(String::new(), |a, p| format!("/{p}{a}")),
            );
            root.get_child_mut(&ppath, false)
                .0
                .unwrap()
                .remove_node(last_part);
            return Ok(true);
        }
        Ok(false)
    }

    /// Get the interface at the given path.
    ///
    /// # Errors
    ///
    /// If the interface is not registered at the given path, `Error::InterfaceNotFound` error is
    /// returned.
    ///
    /// # Examples
    ///
    /// The typical use of this is property changes outside of a dispatched handler:
    ///
    /// ```no_run
    /// # use std::error::Error;
    /// # use zbus::{Connection, interface};
    /// # use async_io::block_on;
    /// #
    /// struct MyIface(u32);
    ///
    /// #[interface(name = "org.myiface.MyIface")]
    /// impl MyIface {
    ///      #[zbus(property)]
    ///      async fn count(&self) -> u32 {
    ///          self.0
    ///      }
    /// }
    ///
    /// # block_on(async {
    /// # let connection = Connection::session().await?;
    /// #
    /// # let path = "/org/zbus/path";
    /// # connection.object_server().at(path, MyIface(0)).await?;
    /// let iface_ref = connection
    ///     .object_server()
    ///     .interface::<_, MyIface>(path).await?;
    /// let mut iface = iface_ref.get_mut().await;
    /// iface.0 = 42;
    /// iface.count_changed(iface_ref.signal_context()).await?;
    /// # Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// # })?;
    /// #
    /// # Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// ```
    pub async fn interface<'p, P, I>(&self, path: P) -> Result<InterfaceRef<I>>
    where
        I: Interface,
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        let path = path.try_into().map_err(Into::into)?;
        let root = self.root().read().await;
        let node = root.get_child(&path).ok_or(Error::InterfaceNotFound)?;

        let lock = node
            .interface_lock(I::name())
            .ok_or(Error::InterfaceNotFound)?
            .instance
            .clone();

        // Ensure what we return can later be dowcasted safely.
        lock.read()
            .await
            .downcast_ref::<I>()
            .ok_or(Error::InterfaceNotFound)?;

        let conn = self.connection();
        // SAFETY: We know that there is a valid path on the node as we already converted w/o error.
        let ctxt = SignalContext::new(&conn, path).unwrap().into_owned();

        Ok(InterfaceRef {
            ctxt,
            lock,
            phantom: PhantomData,
        })
    }

    async fn dispatch_call_to_iface(
        &self,
        iface: Arc<RwLock<dyn Interface>>,
        connection: &Connection,
        msg: &Message,
        hdr: &Header<'_>,
    ) -> fdo::Result<()> {
        let member = hdr
            .member()
            .ok_or_else(|| fdo::Error::Failed("Missing member".into()))?;
        let iface_name = hdr
            .interface()
            .ok_or_else(|| fdo::Error::Failed("Missing interface".into()))?;

        trace!("acquiring read lock on interface `{}`", iface_name);
        let read_lock = iface.read().await;
        trace!("acquired read lock on interface `{}`", iface_name);
        match read_lock.call(self, connection, msg, member.as_ref()) {
            DispatchResult::NotFound => {
                return Err(fdo::Error::UnknownMethod(format!(
                    "Unknown method '{member}'"
                )));
            }
            DispatchResult::Async(f) => {
                return f.await.map_err(|e| match e {
                    Error::FDO(e) => *e,
                    e => fdo::Error::Failed(format!("{e}")),
                });
            }
            DispatchResult::RequiresMut => {}
        }
        drop(read_lock);
        trace!("acquiring write lock on interface `{}`", iface_name);
        let mut write_lock = iface.write().await;
        trace!("acquired write lock on interface `{}`", iface_name);
        match write_lock.call_mut(self, connection, msg, member.as_ref()) {
            DispatchResult::NotFound => {}
            DispatchResult::RequiresMut => {}
            DispatchResult::Async(f) => {
                return f.await.map_err(|e| match e {
                    Error::FDO(e) => *e,
                    e => fdo::Error::Failed(format!("{e}")),
                });
            }
        }
        drop(write_lock);
        Err(fdo::Error::UnknownMethod(format!(
            "Unknown method '{member}'"
        )))
    }

    async fn dispatch_method_call_try(
        &self,
        connection: &Connection,
        msg: &Message,
        hdr: &Header<'_>,
    ) -> fdo::Result<()> {
        let path = hdr
            .path()
            .ok_or_else(|| fdo::Error::Failed("Missing object path".into()))?;
        let iface_name = hdr
            .interface()
            // TODO: In the absence of an INTERFACE field, if two or more interfaces on the same
            // object have a method with the same name, it is undefined which of those
            // methods will be invoked. Implementations may choose to either return an
            // error, or deliver the message as though it had an arbitrary one of those
            // interfaces.
            .ok_or_else(|| fdo::Error::Failed("Missing interface".into()))?;
        // Check that the message has a member before spawning.
        // Note that an unknown member will still spawn a task. We should instead gather
        // all the details for the call before spawning.
        // See also https://github.com/dbus2/zbus/issues/674 for future of Interface.
        let _ = hdr
            .member()
            .ok_or_else(|| fdo::Error::Failed("Missing member".into()))?;

        // Ensure the root lock isn't held while dispatching the message. That
        // way, the object server can be mutated during that time.
        let (iface, with_spawn) = {
            let root = self.root.read().await;
            let node = root
                .get_child(path)
                .ok_or_else(|| fdo::Error::UnknownObject(format!("Unknown object '{path}'")))?;

            let iface = node.interface_lock(iface_name.as_ref()).ok_or_else(|| {
                fdo::Error::UnknownInterface(format!("Unknown interface '{iface_name}'"))
            })?;
            (iface.instance, iface.spawn_tasks_for_methods)
        };

        if with_spawn {
            let executor = connection.executor().clone();
            let task_name = format!("`{msg}` method dispatcher");
            let connection = connection.clone();
            let msg = msg.clone();
            executor
                .spawn(
                    async move {
                        let server = connection.object_server();
                        let hdr = msg.header();
                        if let Err(e) = server
                            .dispatch_call_to_iface(iface, &connection, &msg, &hdr)
                            .await
                        {
                            // When not spawning a task, this error is handled by the caller.
                            debug!("Returning error: {}", e);
                            if let Err(e) = connection.reply_dbus_error(&hdr, e).await {
                                debug!(
                                    "Error dispatching message. Message: {:?}, error: {:?}",
                                    msg, e
                                );
                            }
                        }
                    }
                    .instrument(trace_span!("{}", task_name)),
                    &task_name,
                )
                .detach();
            Ok(())
        } else {
            self.dispatch_call_to_iface(iface, connection, msg, hdr)
                .await
        }
    }

    /// Dispatch an incoming message to a registered interface.
    ///
    /// The object server will handle the message by:
    ///
    /// - looking up the called object path & interface,
    ///
    /// - calling the associated method if one exists,
    ///
    /// - returning a message (responding to the caller with either a return or error message) to
    ///   the caller through the associated server connection.
    ///
    /// Returns an error if the message is malformed.
    #[instrument(skip(self))]
    pub(crate) async fn dispatch_call(&self, msg: &Message, hdr: &Header<'_>) -> Result<()> {
        let conn = self.connection();

        if let Err(e) = self.dispatch_method_call_try(&conn, msg, hdr).await {
            debug!("Returning error: {}", e);
            conn.reply_dbus_error(hdr, e).await?;
        }
        trace!("Handled: {}", msg);

        Ok(())
    }

    pub(crate) fn connection(&self) -> Connection {
        self.conn
            .upgrade()
            .expect("ObjectServer can't exist w/o an associated Connection")
    }
}

impl From<crate::blocking::ObjectServer> for ObjectServer {
    fn from(server: crate::blocking::ObjectServer) -> Self {
        server.into_inner()
    }
}

/// A response wrapper that notifies after response has been sent.
///
/// Sometimes in [`interface`] method implemenations we need to do some other work after the
/// response has been sent off. This wrapper type allows us to do that. Instead of returning your
/// intended response type directly, wrap it in this type and return it from your method. The
/// returned `EventListener` from `new` method will be notified when the response has been sent.
///
/// A typical use case is sending off signals after the response has been sent. The easiest way to
/// do that is to spawn a task from the method that sends the signal but only after being notified
/// of the response dispatch.
///
/// # Caveats
///
/// The notification indicates that the response has been sent off, not that destination peer has
/// received it. That can only be guaranteed for a peer-to-peer connection.
///
/// [`interface`]: crate::interface
#[derive(Debug)]
pub struct ResponseDispatchNotifier<R> {
    response: R,
    event: Option<Event>,
}

impl<R> ResponseDispatchNotifier<R> {
    /// Create a new `NotifyResponse`.
    pub fn new(response: R) -> (Self, EventListener) {
        let event = Event::new();
        let listener = event.listen();
        (
            Self {
                response,
                event: Some(event),
            },
            listener,
        )
    }

    /// Get the response.
    pub fn response(&self) -> &R {
        &self.response
    }
}

impl<R> Serialize for ResponseDispatchNotifier<R>
where
    R: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.response.serialize(serializer)
    }
}

impl<'de, R> Deserialize<'de> for ResponseDispatchNotifier<R>
where
    R: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            response: R::deserialize(deserializer)?,
            event: None,
        })
    }
}

impl<R> Type for ResponseDispatchNotifier<R>
where
    R: Type,
{
    fn signature() -> Signature<'static> {
        R::signature()
    }
}

impl<T> Drop for ResponseDispatchNotifier<T> {
    fn drop(&mut self) {
        if let Some(event) = self.event.take() {
            event.notify(usize::MAX);
        }
    }
}
