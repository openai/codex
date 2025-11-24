// Copyright 2016-2024 dbus-secret-service Contributors
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::collections::HashMap;

use dbus::{
    arg::{PropMap, RefArg, Variant},
    blocking::{Connection, Proxy},
    strings::Path,
};

use crate::{
    proxy::collection::Collection as ProxyCollection,
    proxy::new_proxy,
    ss::{SS_ITEM_ATTRIBUTES, SS_ITEM_LABEL},
    Error, Item, LockAction, SecretService,
};

/// Represents a Secret Service collection of items.
///
/// Collections are retrieved from and created by a
/// [`SecretService`] instance and cannot outlive it.
pub struct Collection<'a> {
    service: &'a SecretService,
    pub path: Path<'static>,
}

impl<'a> Collection<'a> {
    pub fn new(service: &'a SecretService, path: Path<'static>) -> Collection<'a> {
        Collection { service, path }
    }

    fn proxy(&'_ self) -> Proxy<'_, &'_ Connection> {
        new_proxy(&self.service.connection, &self.path)
    }

    pub fn is_locked(&self) -> Result<bool, Error> {
        Ok(self.proxy().locked()?)
    }

    pub fn ensure_unlocked(&self) -> Result<(), Error> {
        if self.is_locked()? {
            self.unlock()
        } else {
            Ok(())
        }
    }

    pub fn unlock(&self) -> Result<(), Error> {
        let paths = vec![self.path.clone()];
        self.service.lock_unlock_all(LockAction::Unlock, paths)
    }

    pub fn lock(&self) -> Result<(), Error> {
        let paths = vec![self.path.clone()];
        self.service.lock_unlock_all(LockAction::Lock, paths)
    }

    /// Delete the underlying dbus collection
    pub fn delete(&self) -> Result<(), Error> {
        let p_path = self.proxy().delete()?;
        if p_path != Path::new("/").unwrap() {
            self.service.prompt_for_lock_unlock_delete(&p_path)
        } else {
            Ok(())
        }
    }

    pub fn get_all_items(&self) -> Result<Vec<Item<'_>>, Error> {
        let paths = self.proxy().items()?;
        let result = paths
            .into_iter()
            .map(|path| Item::new(self.service, path))
            .collect();
        Ok(result)
    }

    pub fn search_items(&self, attributes: HashMap<&str, &str>) -> Result<Vec<Item<'_>>, Error> {
        let paths = self.proxy().search_items(attributes)?;
        let result = paths
            .into_iter()
            .map(|path| Item::new(self.service, path))
            .collect();
        Ok(result)
    }

    pub fn get_label(&self) -> Result<String, Error> {
        Ok(self.proxy().label()?)
    }

    pub fn set_label(&self, new_label: &str) -> Result<(), Error> {
        Ok(self.proxy().set_label(new_label.to_string())?)
    }

    pub fn create_item(
        &self,
        label: &str,
        attributes: HashMap<&str, &str>,
        secret: &[u8],
        replace: bool,
        content_type: &str,
    ) -> Result<Item<'_>, Error> {
        let encrypted = self.service.session.encrypt_secret(secret, content_type);
        let attributes: HashMap<String, String> = attributes
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let attributes = Box::new(attributes) as Box<dyn RefArg>;
        let label = Box::new(label.to_string()) as Box<dyn RefArg>;
        let mut properties: PropMap = PropMap::new();
        properties.insert(SS_ITEM_LABEL.to_string(), Variant(label));
        properties.insert(SS_ITEM_ATTRIBUTES.to_string(), Variant(attributes));
        let (c_path, p_path) =
            self.proxy()
                .create_item(properties, encrypted.to_dbus(), replace)?;
        let created = {
            if c_path == Path::new("/")? {
                // no creation path, so prompt
                self.service.prompt_for_create(&p_path)?
            } else {
                c_path
            }
        };
        Ok(Item::new(self.service, created))
    }
}

#[cfg(test)]
mod test {
    use crate::*;

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
    #[ignore] // can't run headless - prompts
    fn should_lock_and_unlock() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss
            .create_collection("TestCollectionLockUnlock", "")
            .unwrap();
        collection.ensure_unlocked().unwrap();
        collection.lock().unwrap();
        assert!(collection.is_locked().unwrap());
        collection.delete().unwrap();
    }

    #[test]
    #[ignore] // can't run headless - prompts
    fn should_delete_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        ss.create_collection("TestDelete", "").unwrap();
        let collections = ss.get_all_collections().unwrap();
        let count_before = collections.len();
        for collection in collections {
            let collection_path = &collection.path;
            if collection_path.contains("/Test") {
                collection.delete().unwrap();
            }
        }
        //double check after
        let collections = ss.get_all_collections().unwrap();
        assert!(
            collections.len() < count_before,
            "collections before delete {count_before} after delete {}",
            collections.len()
        );
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

        assert_eq!(item.path, search_item[0].path);
        item.delete().unwrap();
    }

    #[test]
    #[ignore] // can't run headless - prompts
    fn should_get_and_set_collection_label() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.create_collection("TestGetSetLabel", "").unwrap();
        let label = collection.get_label().unwrap();
        assert_eq!(label, "TestGetSetLabel");

        // Set label to test and check
        collection.ensure_unlocked().unwrap();
        collection.set_label("DoubleTest").unwrap();
        let label = collection.get_label().unwrap();
        assert_eq!(label, "DoubleTest");

        // Reset label to original and test
        collection.ensure_unlocked().unwrap();
        collection.set_label("Test").unwrap();
        let label = collection.get_label().unwrap();
        assert_eq!(label, "Test");

        collection.delete().unwrap();
    }
}
