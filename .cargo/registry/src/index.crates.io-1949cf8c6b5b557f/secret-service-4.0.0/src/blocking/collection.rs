// Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use super::item::Item;
use crate::error::Error;
use crate::proxy::collection::CollectionProxyBlocking;
use crate::proxy::service::ServiceProxyBlocking;
use crate::session::Session;
use crate::ss::{SS_DBUS_NAME, SS_ITEM_ATTRIBUTES, SS_ITEM_LABEL};
use crate::util::{exec_prompt_blocking, format_secret, lock_or_unlock_blocking, LockAction};

use std::collections::HashMap;
use zbus::{
    zvariant::{Dict, ObjectPath, OwnedObjectPath, Value},
    CacheProperties,
};

// Collection struct.
// Should always be created from the SecretService entry point,
// whether through a new collection or a collection search
pub struct Collection<'a> {
    conn: zbus::blocking::Connection,
    session: &'a Session,
    pub collection_path: OwnedObjectPath,
    collection_proxy: CollectionProxyBlocking<'a>,
    service_proxy: &'a ServiceProxyBlocking<'a>,
}

impl<'a> Collection<'a> {
    pub(crate) fn new(
        conn: zbus::blocking::Connection,
        session: &'a Session,
        service_proxy: &'a ServiceProxyBlocking,
        collection_path: OwnedObjectPath,
    ) -> Result<Self, Error> {
        let collection_proxy = CollectionProxyBlocking::builder(&conn)
            .destination(SS_DBUS_NAME)?
            .path(collection_path.clone())?
            .cache_properties(CacheProperties::No)
            .build()?;
        Ok(Collection {
            conn,
            session,
            collection_path,
            collection_proxy,
            service_proxy,
        })
    }

    pub fn is_locked(&self) -> Result<bool, Error> {
        Ok(self.collection_proxy.locked()?)
    }

    pub fn ensure_unlocked(&self) -> Result<(), Error> {
        if self.is_locked()? {
            Err(Error::Locked)
        } else {
            Ok(())
        }
    }

    pub fn unlock(&self) -> Result<(), Error> {
        lock_or_unlock_blocking(
            self.conn.clone(),
            self.service_proxy,
            &self.collection_path,
            LockAction::Unlock,
        )
    }

    pub fn lock(&self) -> Result<(), Error> {
        lock_or_unlock_blocking(
            self.conn.clone(),
            self.service_proxy,
            &self.collection_path,
            LockAction::Lock,
        )
    }

    /// Deletes dbus object, but struct instance still exists (current implementation)
    pub fn delete(&self) -> Result<(), Error> {
        // ensure_unlocked handles prompt for unlocking if necessary
        self.ensure_unlocked()?;
        let prompt_path = self.collection_proxy.delete()?;

        // "/" means no prompt necessary
        if prompt_path.as_str() != "/" {
            exec_prompt_blocking(self.conn.clone(), &prompt_path)?;
        }

        Ok(())
    }

    pub fn get_all_items(&self) -> Result<Vec<Item>, Error> {
        let items = self.collection_proxy.items()?;

        // map array of item paths to Item
        let res = items
            .into_iter()
            .map(|item_path| {
                Item::new(
                    self.conn.clone(),
                    self.session,
                    self.service_proxy,
                    item_path.into(),
                )
            })
            .collect::<Result<_, _>>()?;

        Ok(res)
    }

    pub fn search_items(&self, attributes: HashMap<&str, &str>) -> Result<Vec<Item>, Error> {
        let items = self.collection_proxy.search_items(attributes)?;

        // map array of item paths to Item
        let res = items
            .into_iter()
            .map(|item_path| {
                Item::new(
                    self.conn.clone(),
                    self.session,
                    self.service_proxy,
                    item_path,
                )
            })
            .collect::<Result<_, _>>()?;

        Ok(res)
    }

    pub fn get_label(&self) -> Result<String, Error> {
        Ok(self.collection_proxy.label()?)
    }

    pub fn set_label(&self, new_label: &str) -> Result<(), Error> {
        Ok(self.collection_proxy.set_label(new_label)?)
    }

    pub fn create_item(
        &self,
        label: &str,
        attributes: HashMap<&str, &str>,
        secret: &[u8],
        replace: bool,
        content_type: &str,
    ) -> Result<Item, Error> {
        let secret_struct = format_secret(self.session, secret, content_type)?;

        let mut properties: HashMap<&str, Value> = HashMap::new();
        let attributes: Dict = attributes.into();

        properties.insert(SS_ITEM_LABEL, label.into());
        properties.insert(SS_ITEM_ATTRIBUTES, attributes.into());

        let created_item = self
            .collection_proxy
            .create_item(properties, secret_struct, replace)?;

        // This prompt handling is practically identical to create_collection
        let item_path: ObjectPath = {
            // Get path of created object
            let created_path = created_item.item;

            // Check if that path is "/", if so should execute a prompt
            if created_path.as_str() == "/" {
                let prompt_path = created_item.prompt;

                // Exec prompt and parse result
                let prompt_res = exec_prompt_blocking(self.conn.clone(), &prompt_path)?;
                prompt_res.try_into()?
            } else {
                // if not, just return created path
                created_path.into()
            }
        };

        Item::new(
            self.conn.clone(),
            self.session,
            self.service_proxy,
            item_path.into(),
        )
    }
}

#[cfg(test)]
mod test {
    use crate::blocking::*;

    #[test]
    fn should_create_collection_struct() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let _ = ss.get_default_collection().unwrap();
        // tested under SecretService struct
    }

    #[test]
    fn should_check_if_collection_locked() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let _ = collection.is_locked().unwrap();
    }

    #[test]
    #[ignore] // should unignore this test this manually, otherwise will constantly prompt during tests.
    fn should_lock_and_unlock() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let locked = collection.is_locked().unwrap();
        if locked {
            collection.unlock().unwrap();
            collection.ensure_unlocked().unwrap();
            assert!(!collection.is_locked().unwrap());
            collection.lock().unwrap();
            assert!(collection.is_locked().unwrap());
        } else {
            collection.lock().unwrap();
            assert!(collection.is_locked().unwrap());
            collection.unlock().unwrap();
            collection.ensure_unlocked().unwrap();
            assert!(!collection.is_locked().unwrap());
        }
    }

    #[test]
    #[ignore]
    fn should_delete_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collections = ss.get_all_collections().unwrap();
        let count_before = collections.len();
        for collection in collections {
            let collection_path = &*collection.collection_path;
            if collection_path.contains("Test") {
                collection.unlock().unwrap();
                collection.delete().unwrap();
            }
        }
        //double check after
        let collections = ss.get_all_collections().unwrap();
        assert!(
            collections.len() < count_before,
            "collections before delete {count_before}",
        )
    }

    #[test]
    fn should_get_all_items() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        collection.get_all_items().unwrap();
    }

    #[test]
    fn should_search_items() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();

        // Create an item
        let item = collection
            .create_item(
                "test",
                HashMap::from([("test_attributes_in_collection", "test")]),
                b"test_secret",
                false,
                "text/plain",
            )
            .unwrap();

        // handle empty vec search
        collection.search_items(HashMap::new()).unwrap();

        // handle no result
        let bad_search = collection
            .search_items(HashMap::from([("test_bad", "test")]))
            .unwrap();
        assert_eq!(bad_search.len(), 0);

        // handle correct search for item and compare
        let search_item = collection
            .search_items(HashMap::from([("test_attributes_in_collection", "test")]))
            .unwrap();

        assert_eq!(item.item_path, search_item[0].item_path);
        item.delete().unwrap();
    }

    #[test]
    #[ignore]
    fn should_get_and_set_collection_label() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let label = collection.get_label().unwrap();
        assert_eq!(label, "Login");

        // Set label to test and check
        collection.unlock().unwrap();
        collection.set_label("Test").unwrap();
        let label = collection.get_label().unwrap();
        assert_eq!(label, "Test");

        // Reset label to original and test
        collection.unlock().unwrap();
        collection.set_label("Login").unwrap();
        let label = collection.get_label().unwrap();
        assert_eq!(label, "Login");

        collection.lock().unwrap();
    }
}
