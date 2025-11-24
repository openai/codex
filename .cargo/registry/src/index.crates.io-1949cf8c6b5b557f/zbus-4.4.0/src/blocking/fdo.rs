//! D-Bus standard interfaces.
//!
//! Provides blocking versions of the proxy types in [`zbus::fdo`] module.

use enumflags2::BitFlags;
use static_assertions::assert_impl_all;
use std::collections::HashMap;
use zbus_names::{
    BusName, InterfaceName, OwnedBusName, OwnedInterfaceName, OwnedUniqueName, UniqueName,
    WellKnownName,
};
use zvariant::{ObjectPath, Optional, OwnedValue, Value};

use crate::{
    fdo::{
        ConnectionCredentials, ManagedObjects, ReleaseNameReply, RequestNameFlags,
        RequestNameReply, Result,
    },
    proxy, OwnedGuid,
};

gen_introspectable_proxy!(false, true);
assert_impl_all!(IntrospectableProxy<'_>: Send, Sync, Unpin);

gen_properties_proxy!(false, true);
assert_impl_all!(PropertiesProxy<'_>: Send, Sync, Unpin);

gen_object_manager_proxy!(false, true);
assert_impl_all!(ObjectManagerProxy<'_>: Send, Sync, Unpin);

gen_peer_proxy!(false, true);
assert_impl_all!(PeerProxy<'_>: Send, Sync, Unpin);

gen_monitoring_proxy!(false, true);
assert_impl_all!(MonitoringProxy<'_>: Send, Sync, Unpin);

gen_stats_proxy!(false, true);
assert_impl_all!(StatsProxy<'_>: Send, Sync, Unpin);

gen_dbus_proxy!(false, true);
assert_impl_all!(DBusProxy<'_>: Send, Sync, Unpin);
