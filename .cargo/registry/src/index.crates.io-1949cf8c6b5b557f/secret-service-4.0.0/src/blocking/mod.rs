// Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A blocking secret service API.
//!
//! This `SecretService` will block the current thread when making requests to the
//! secret service server instead of returning futures.
//!
//! It is important to not call this these functions in an async context or otherwise the runtime
//! may stall. See [zbus's blocking documentation] for more details. If you are in an async context,
//! you should use the [async `SecretService`] instead.
//!
//! [zbus's blocking documentation]: https://docs.rs/zbus/latest/zbus/blocking/index.html
//! [async `SecretService`]: crate::SecretService

use crate::session::Session;
use crate::ss::SS_COLLECTION_LABEL;
use crate::util;
use crate::{proxy::service::ServiceProxyBlocking, util::exec_prompt_blocking};
use crate::{EncryptionType, Error, SearchItemsResult};
use std::collections::HashMap;
use zbus::zvariant::{ObjectPath, Value};

mod collection;
pub use collection::Collection;
mod item;
pub use item::Item;

/// Secret Service Struct.
///
/// This the main entry point for usage of the library.
///
/// Creating a new [SecretService] will also initialize dbus
/// and negotiate a new cryptographic session
/// ([EncryptionType::Plain] or [EncryptionType::Dh])
pub struct SecretService<'a> {
    conn: zbus::blocking::Connection,
    session: Session,
    service_proxy: ServiceProxyBlocking<'a>,
}

impl<'a> SecretService<'a> {
    /// Create a new `SecretService` instance
    pub fn connect(encryption: EncryptionType) -> Result<Self, Error> {
        let conn = zbus::blocking::Connection::session().map_err(util::handle_conn_error)?;
        let service_proxy = ServiceProxyBlocking::new(&conn).map_err(util::handle_conn_error)?;

        let session = Session::new_blocking(&service_proxy, encryption)?;

        Ok(SecretService {
            conn,
            session,
            service_proxy,
        })
    }

    /// Get all collections
    pub fn get_all_collections(&self) -> Result<Vec<Collection>, Error> {
        let collections = self.service_proxy.collections()?;
        collections
            .into_iter()
            .map(|object_path| {
                Collection::new(
                    self.conn.clone(),
                    &self.session,
                    &self.service_proxy,
                    object_path.into(),
                )
            })
            .collect()
    }

    /// Get collection by alias.
    ///
    /// Most common would be the `default` alias, but there
    /// is also a specific method for getting the collection
    /// by default alias.
    pub fn get_collection_by_alias(&self, alias: &str) -> Result<Collection, Error> {
        let object_path = self.service_proxy.read_alias(alias)?;

        if object_path.as_str() == "/" {
            Err(Error::NoResult)
        } else {
            Ok(Collection::new(
                self.conn.clone(),
                &self.session,
                &self.service_proxy,
                object_path,
            )?)
        }
    }

    /// Get default collection.
    /// (The collection whos alias is `default`)
    pub fn get_default_collection(&self) -> Result<Collection, Error> {
        self.get_collection_by_alias("default")
    }

    /// Get any collection.
    /// First tries `default` collection, then `session`
    /// collection, then the first collection when it
    /// gets all collections.
    pub fn get_any_collection(&self) -> Result<Collection, Error> {
        // default first, then session, then first

        self.get_default_collection()
            .or_else(|_| self.get_collection_by_alias("session"))
            .or_else(|_| {
                let mut collections = self.get_all_collections()?;
                if collections.is_empty() {
                    Err(Error::NoResult)
                } else {
                    Ok(collections.swap_remove(0))
                }
            })
    }

    /// Creates a new collection with a label and an alias.
    pub fn create_collection(&self, label: &str, alias: &str) -> Result<Collection, Error> {
        let mut properties: HashMap<&str, Value> = HashMap::new();
        properties.insert(SS_COLLECTION_LABEL, label.into());

        let created_collection = self.service_proxy.create_collection(properties, alias)?;

        // This prompt handling is practically identical to create_collection
        let collection_path: ObjectPath = {
            // Get path of created object
            let created_path = created_collection.collection;

            // Check if that path is "/", if so should execute a prompt
            if created_path.as_str() == "/" {
                let prompt_path = created_collection.prompt;

                // Exec prompt and parse result
                let prompt_res = util::exec_prompt_blocking(self.conn.clone(), &prompt_path)?;
                prompt_res.try_into()?
            } else {
                // if not, just return created path
                created_path.into()
            }
        };

        Collection::new(
            self.conn.clone(),
            &self.session,
            &self.service_proxy,
            collection_path.into(),
        )
    }

    /// Searches all items by attributes
    pub fn search_items(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> Result<SearchItemsResult<Item>, Error> {
        let items = self.service_proxy.search_items(attributes)?;

        let object_paths_to_items = |items: Vec<_>| {
            items
                .into_iter()
                .map(|item_path| {
                    Item::new(
                        self.conn.clone(),
                        &self.session,
                        &self.service_proxy,
                        item_path,
                    )
                })
                .collect::<Result<_, _>>()
        };

        Ok(SearchItemsResult {
            unlocked: object_paths_to_items(items.unlocked)?,
            locked: object_paths_to_items(items.locked)?,
        })
    }

    /// Unlock all items in a batch
    pub fn unlock_all(&self, items: &[&Item<'_>]) -> Result<(), Error> {
        let objects = items.iter().map(|i| &*i.item_path).collect();
        let lock_action_res = self.service_proxy.unlock(objects)?;

        if lock_action_res.object_paths.is_empty() {
            exec_prompt_blocking(self.conn.clone(), &lock_action_res.prompt)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::convert::TryFrom;
    use zbus::zvariant::ObjectPath;

    #[test]
    fn should_create_secret_service() {
        SecretService::connect(EncryptionType::Plain).unwrap();
    }

    #[test]
    fn should_get_all_collections() {
        // Assumes that there will always be a default
        // collection
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collections = ss.get_all_collections().unwrap();
        assert!(!collections.is_empty(), "no collections found");
    }

    #[test]
    fn should_get_collection_by_alias() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        ss.get_collection_by_alias("session").unwrap();
    }

    #[test]
    fn should_return_error_if_collection_doesnt_exist() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();

        match ss.get_collection_by_alias("definitely_defintely_does_not_exist") {
            Err(Error::NoResult) => {}
            _ => panic!(),
        };
    }

    #[test]
    fn should_get_default_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        ss.get_default_collection().unwrap();
    }

    #[test]
    fn should_get_any_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let _ = ss.get_any_collection().unwrap();
    }

    #[test_with::no_env(GITHUB_ACTIONS)]
    #[test]
    fn should_create_and_delete_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let test_collection = ss.create_collection("Test", "").unwrap();
        assert_eq!(
            ObjectPath::from(test_collection.collection_path.clone()),
            ObjectPath::try_from("/org/freedesktop/secrets/collection/Test").unwrap()
        );
        test_collection.delete().unwrap();
    }

    #[test]
    fn should_search_items() {
        let ss = SecretService::connect(EncryptionType::Dh).unwrap();
        let collection = ss.get_default_collection().unwrap();

        // Create an item
        let item = collection
            .create_item(
                "test",
                HashMap::from([("test_attribute_in_ss", "test_value")]),
                b"test_secret",
                false,
                "text/plain",
            )
            .unwrap();

        // handle empty vec search
        ss.search_items(HashMap::new()).unwrap();

        // handle no result
        let bad_search = ss.search_items(HashMap::from([("test", "test")])).unwrap();
        assert_eq!(bad_search.unlocked.len(), 0);
        assert_eq!(bad_search.locked.len(), 0);

        // handle correct search for item and compare
        let search_item = ss
            .search_items(HashMap::from([("test_attribute_in_ss", "test_value")]))
            .unwrap();

        assert_eq!(item.item_path, search_item.unlocked[0].item_path);
        assert_eq!(search_item.locked.len(), 0);
        item.delete().unwrap();
    }
}
