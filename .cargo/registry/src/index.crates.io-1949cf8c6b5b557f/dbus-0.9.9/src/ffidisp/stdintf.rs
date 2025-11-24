//! This module contains some standard interfaces and an easy way to call them.
//!
//! See the [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces) for more information about these standard interfaces.
//! 
//! The code here was originally created by dbus-codegen.
//!
//! # Example
//! ```
//! use dbus::ffidisp::{Connection, BusType};
//! use dbus::ffidisp::stdintf::org_freedesktop_dbus::Introspectable;
//! let c = Connection::get_private(BusType::Session).unwrap();
//! let p = c.with_path("org.freedesktop.DBus", "/", 10000);
//! println!("Introspection XML: {}", p.introspect().unwrap());
//! ```
//!

#![allow(missing_docs)]

pub use self::org_freedesktop_dbus::Peer as OrgFreedesktopDBusPeer;

pub use self::org_freedesktop_dbus::Introspectable as OrgFreedesktopDBusIntrospectable;

pub use self::org_freedesktop_dbus::Properties as OrgFreedesktopDBusProperties;

pub use self::org_freedesktop_dbus::ObjectManager as OrgFreedesktopDBusObjectManager;

pub mod org_freedesktop_dbus {

use crate::{arg, message, ffidisp};

/// Method of the [org.freedesktop.DBus.Introspectable](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-introspectable) interface.
pub trait Introspectable {
    type Err;
    fn introspect(&self) -> Result<String, Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=ffidisp::Connection>> Introspectable for ffidisp::ConnPath<'a, C> {
    type Err = crate::Error;

    fn introspect(&self) -> Result<String, Self::Err> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Introspectable".into(), &"Introspect".into(), |_| {
        })?;
        m.as_result()?;
        let mut i = m.iter_init();
        let xml: String = i.read()?;
        Ok(xml)
    }
}

/// Methods of the [org.freedesktop.DBus.Properties](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-properties) interface.
pub trait Properties {
    type Err;
    fn get<R0: for<'b> arg::Get<'b>>(&self, interface_name: &str, property_name: &str) -> Result<R0, Self::Err>;
    fn get_all(&self, interface_name: &str) -> Result<::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>>, Self::Err>;
    fn set<I2: arg::Arg + arg::Append>(&self, interface_name: &str, property_name: &str, value: I2) -> Result<(), Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=ffidisp::Connection>> Properties for ffidisp::ConnPath<'a, C> {
    type Err = crate::Error;

    fn get<R0: for<'b> arg::Get<'b>>(&self, interface_name: &str, property_name: &str) -> Result<R0, Self::Err> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"Get".into(), |msg| {
            let mut i = arg::IterAppend::new(msg);
            i.append(interface_name);
            i.append(property_name);
        })?;
        m.as_result()?;
        let mut i = m.iter_init();
        let value: arg::Variant<R0> = i.read()?;
        Ok(value.0)
    }

    fn get_all(&self, interface_name: &str) -> Result<::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>>, Self::Err> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"GetAll".into(), |msg| {
            let mut i = arg::IterAppend::new(msg);
            i.append(interface_name);
        })?;
        m.as_result()?;
        let mut i = m.iter_init();
        let properties: ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>> = i.read()?;
        Ok(properties)
    }

    fn set<I2: arg::Arg + arg::Append>(&self, interface_name: &str, property_name: &str, value: I2) -> Result<(), Self::Err> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Properties".into(), &"Set".into(), |msg| {
            let mut i = arg::IterAppend::new(msg);
            i.append(interface_name);
            i.append(property_name);
            i.append(arg::Variant(value));
        })?;
        m.as_result()?;
        Ok(())
    }
}

#[derive(Debug, Default)]
/// Struct to send/receive the PropertiesChanged signal of the
/// [org.freedesktop.DBus.Properties](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-properties) interface.
pub struct PropertiesPropertiesChanged {
    pub interface_name: String,
    pub changed_properties: ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>>,
    pub invalidated_properties: Vec<String>,
}

impl arg::AppendAll for PropertiesPropertiesChanged {
    fn append(&self, i: &mut arg::IterAppend) {
        arg::RefArg::append(&self.interface_name, i);
        arg::RefArg::append(&self.changed_properties, i);
        arg::RefArg::append(&self.invalidated_properties ,i);
    }
}

impl arg::ReadAll for PropertiesPropertiesChanged {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(PropertiesPropertiesChanged {
            interface_name: i.read()?,
            changed_properties: i.read()?,
            invalidated_properties: i.read()?,
        })
    }
}

impl message::SignalArgs for PropertiesPropertiesChanged {
    const NAME: &'static str = "PropertiesChanged";
    const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
}

/// Method of the [org.freedesktop.DBus.ObjectManager](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-objectmanager) interface.
pub trait ObjectManager {
    type Err;
    fn get_managed_objects(&self) -> Result<::std::collections::HashMap<crate::Path<'static>, ::std::collections::HashMap<String, ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>>>>, Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=ffidisp::Connection>> ObjectManager for ffidisp::ConnPath<'a, C> {
    type Err = crate::Error;

    fn get_managed_objects(&self) -> Result<::std::collections::HashMap<crate::Path<'static>, ::std::collections::HashMap<String, ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>>>>, Self::Err> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.ObjectManager".into(), &"GetManagedObjects".into(), |_| {
        })?;
        m.as_result()?;
        let mut i = m.iter_init();
        let objects: ::std::collections::HashMap<crate::Path<'static>, ::std::collections::HashMap<String, ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>>>> = i.read()?;
        Ok(objects)
    }
}

#[derive(Debug, Default)]
/// Struct to send/receive the InterfacesAdded signal of the
/// [org.freedesktop.DBus.ObjectManager](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-objectmanager) interface.
pub struct ObjectManagerInterfacesAdded {
    pub object: crate::Path<'static>,
    pub interfaces: ::std::collections::HashMap<String, ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg>>>>,
}

impl arg::AppendAll for ObjectManagerInterfacesAdded {
    fn append(&self, i: &mut arg::IterAppend) {
        arg::RefArg::append(&self.object, i);
        arg::RefArg::append(&self.interfaces, i);
    }
}

impl arg::ReadAll for ObjectManagerInterfacesAdded {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ObjectManagerInterfacesAdded {
            object: i.read()?,
            interfaces: i.read()?,
        })
    }
}

impl message::SignalArgs for ObjectManagerInterfacesAdded {
    const NAME: &'static str = "InterfacesAdded";
    const INTERFACE: &'static str = "org.freedesktop.DBus.ObjectManager";
}

#[derive(Debug, Default)]
/// Struct to send/receive the InterfacesRemoved signal of the
/// [org.freedesktop.DBus.ObjectManager](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-objectmanager) interface.
pub struct ObjectManagerInterfacesRemoved {
    pub object: crate::Path<'static>,
    pub interfaces: Vec<String>,
}

impl arg::AppendAll for ObjectManagerInterfacesRemoved {
    fn append(&self, i: &mut arg::IterAppend) {
        arg::RefArg::append(&self.object, i);
        arg::RefArg::append(&self.interfaces, i);
    }
}

impl arg::ReadAll for ObjectManagerInterfacesRemoved {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(ObjectManagerInterfacesRemoved {
            object: i.read()?,
            interfaces: i.read()?,
        })
    }
}

impl message::SignalArgs for ObjectManagerInterfacesRemoved {
    const NAME: &'static str = "InterfacesRemoved";
    const INTERFACE: &'static str = "org.freedesktop.DBus.ObjectManager";
}

/// Methods of the [org.freedesktop.DBus.Peer](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces-peer) interface.
pub trait Peer {
    type Err;
    fn ping(&self) -> Result<(), Self::Err>;
    fn get_machine_id(&self) -> Result<String, Self::Err>;
}

impl<'a, C: ::std::ops::Deref<Target=ffidisp::Connection>> Peer for ffidisp::ConnPath<'a, C> {
    type Err = crate::Error;

    fn ping(&self) -> Result<(), Self::Err> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Peer".into(), &"Ping".into(), |_| {
        })?;
        m.as_result()?;
        Ok(())
    }

    fn get_machine_id(&self) -> Result<String, Self::Err> {
        let mut m = self.method_call_with_args(&"org.freedesktop.DBus.Peer".into(), &"GetMachineId".into(), |_| {
        })?;
        m.as_result()?;
        let mut i = m.iter_init();
        let machine_uuid: String = i.read()?;
        Ok(machine_uuid)
    }
}


}
