//Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A dbus proxy for speaking with secret service's `Service` Interface.

use super::SecretStruct;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Type, Value};

/// A dbus proxy for speaking with secret service's `Service` Interface.
///
/// This will derive ServiceProxy
///
/// Note that `Value` in the method signatures corresponds to `VARIANT` dbus type.
#[zbus::proxy(
    interface = "org.freedesktop.Secret.Service",
    default_service = "org.freedesktop.secrets",
    default_path = "/org/freedesktop/secrets"
)]
trait Service {
    fn open_session(&self, algorithm: &str, input: Value<'_>) -> zbus::Result<OpenSessionResult>;

    fn create_collection(
        &self,
        properties: HashMap<&str, Value<'_>>,
        alias: &str,
    ) -> zbus::Result<CreateCollectionResult>;

    fn search_items(&self, attributes: HashMap<&str, &str>) -> zbus::Result<SearchItemsResult>;

    fn unlock(&self, objects: Vec<&ObjectPath<'_>>) -> zbus::Result<LockActionResult>;

    fn lock(&self, objects: Vec<&ObjectPath<'_>>) -> zbus::Result<LockActionResult>;

    fn get_secrets(
        &self,
        objects: Vec<ObjectPath<'_>>,
    ) -> zbus::Result<HashMap<OwnedObjectPath, SecretStruct>>;

    fn read_alias(&self, name: &str) -> zbus::Result<OwnedObjectPath>;

    fn set_alias(&self, name: &str, collection: ObjectPath<'_>) -> zbus::Result<()>;

    #[zbus(property)]
    fn collections(&self) -> zbus::fdo::Result<Vec<ObjectPath<'_>>>;
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct OpenSessionResult {
    pub(crate) output: OwnedValue,
    pub(crate) result: OwnedObjectPath,
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct CreateCollectionResult {
    pub(crate) collection: OwnedObjectPath,
    pub(crate) prompt: OwnedObjectPath,
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct SearchItemsResult {
    pub(crate) unlocked: Vec<OwnedObjectPath>,
    pub(crate) locked: Vec<OwnedObjectPath>,
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct LockActionResult {
    pub(crate) object_paths: Vec<OwnedObjectPath>,
    pub(crate) prompt: OwnedObjectPath,
}
