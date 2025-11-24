use std::os::fd::OwnedFd;
use std::sync::Arc;
use wayland_client::{
    backend::{self, Backend},
    protocol::{wl_display, wl_registry},
    Connection, Proxy,
};

// This struct represents the data associated with our registry.
struct RegistryData(Arc<Connection>);

// Instead of implementing Dispatch on some global state, we will implement
// ObjectData for our registry. This is required to receive events
// (specifically, the wl_registry.global events) after our wl_registry.get_registry request.
impl backend::ObjectData for RegistryData {
    fn event(
        self: Arc<Self>,
        _: &Backend,
        msg: backend::protocol::Message<backend::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn backend::ObjectData>> {
        // Here, we parse the wire message into an event using Proxy::parse_event.
        let (_registry, event) = wl_registry::WlRegistry::parse_event(&self.0, msg).unwrap();

        // Similar to the dispatch example, we only care about the global event and
        // will print out the received globals.
        if let wl_registry::Event::Global { name, interface, version } = event {
            println!("[{name}] {interface} (v{version})");
        }
        None
    }

    // This method is called whenever the object is destroyed. In the case of our registry,
    // however, there is no way to destroy it, so we will mark it as unreachable.
    fn destroyed(&self, _: wayland_backend::client::ObjectId) {
        unreachable!();
    }
}

fn main() {
    // Create our connection like the Dispatch example, except we store it in an Arc
    // to share with our registry object data.
    let conn = Arc::new(Connection::connect_to_env().unwrap());
    let display = conn.display();

    let registry_data = Arc::new(RegistryData(conn.clone()));

    // Send the `wl_display.get_registry` request, which returns a `wl_registry` to us.
    // Since this request creates a new object, we will use the `Proxy::send_constructor` method
    // to send it. If it didn't, we would use `Proxy::send_request`.
    let _registry: wl_registry::WlRegistry = display
        .send_constructor(wl_display::Request::GetRegistry {}, registry_data.clone())
        .unwrap();

    println!("Advertised globals:");

    // Invoke our roundtrip to receive the events. This essentially is the same as the
    // `EventQueue::roundtrip` method, except it does not have a state to dispatch methods on.
    conn.roundtrip().unwrap();
}
