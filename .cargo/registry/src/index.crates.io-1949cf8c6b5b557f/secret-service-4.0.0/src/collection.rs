// Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::proxy::collection::CollectionProxy;
use crate::proxy::service::ServiceProxy;
use crate::session::Session;
use crate::ss::{SS_DBUS_NAME, SS_ITEM_ATTRIBUTES, SS_ITEM_LABEL};
use crate::util::{exec_prompt, format_secret, lock_or_unlock, LockAction};
use crate::Error;
use crate::Item;

use std::collections::HashMap;
use zbus::{
    zvariant::{Dict, ObjectPath, OwnedObjectPath, Value},
    CacheProperties,
};

// Collection struct.
// Should always be created from the SecretService entry point,
// whether through a new collection or a collection search
pub struct Collection<'a> {
    conn: zbus::Connection,
    session: &'a Session,
    pub collection_path: OwnedObjectPath,
    collection_proxy: CollectionProxy<'a>,
    service_proxy: &'a ServiceProxy<'a>,
}

impl<'a> Collection<'a> {
    pub(crate) async fn new(
        conn: zbus::Connection,
        session: &'a Session,
        service_proxy: &'a ServiceProxy<'_>,
        collection_path: OwnedObjectPath,
    ) -> Result<Collection<'a>, Error> {
        let collection_proxy = CollectionProxy::builder(&conn)
            .destination(SS_DBUS_NAME)?
            .path(collection_path.clone())?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;

        Ok(Collection {
            conn,
            session,
            collection_path,
            collection_proxy,
            service_proxy,
        })
    }

    pub async fn is_locked(&self) -> Result<bool, Error> {
        Ok(self.collection_proxy.locked().await?)
    }

    pub async fn ensure_unlocked(&self) -> Result<(), Error> {
        if self.is_locked().await? {
            Err(Error::Locked)
        } else {
            Ok(())
        }
    }

    pub async fn unlock(&self) -> Result<(), Error> {
        lock_or_unlock(
            self.conn.clone(),
            self.service_proxy,
            &self.collection_path,
            LockAction::Unlock,
        )
        .await
    }

    pub async fn lock(&self) -> Result<(), Error> {
        lock_or_unlock(
            self.conn.clone(),
            self.service_proxy,
            &self.collection_path,
            LockAction::Lock,
        )
        .await
    }

    /// Deletes dbus object, but struct instance still exists (current implementation)
    pub async fn delete(&self) -> Result<(), Error> {
        // ensure_unlocked handles prompt for unlocking if necessary
        self.ensure_unlocked().await?;
        let prompt_path = self.collection_proxy.delete().await?;

        // "/" means no prompt necessary
        if prompt_path.as_str() != "/" {
            exec_prompt(self.conn.clone(), &prompt_path).await?;
        }

        Ok(())
    }

    pub async fn get_all_items(&self) -> Result<Vec<Item<'_>>, Error> {
        let items = self.collection_proxy.items().await?;

        // map array of item paths to Item
        futures_util::future::join_all(items.into_iter().map(|item_path| {
            Item::new(
                self.conn.clone(),
                self.session,
                self.service_proxy,
                item_path.into(),
            )
        }))
        .await
        .into_iter()
        .collect::<Result<_, _>>()
    }

    pub async fn search_items(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> Result<Vec<Item<'_>>, Error> {
        let items = self.collection_proxy.search_items(attributes).await?;

        // map array of item paths to Item
        futures_util::future::join_all(items.into_iter().map(|item_path| {
            Item::new(
                self.conn.clone(),
                self.session,
                self.service_proxy,
                item_path,
            )
        }))
        .await
        .into_iter()
        .collect::<Result<_, _>>()
    }

    pub async fn get_label(&self) -> Result<String, Error> {
        Ok(self.collection_proxy.label().await?)
    }

    pub async fn set_label(&self, new_label: &str) -> Result<(), Error> {
        Ok(self.collection_proxy.set_label(new_label).await?)
    }

    pub async fn create_item(
        &self,
        label: &str,
        attributes: HashMap<&str, &str>,
        secret: &[u8],
        replace: bool,
        content_type: &str,
    ) -> Result<Item<'_>, Error> {
        let secret_struct = format_secret(self.session, secret, content_type)?;

        let mut properties: HashMap<&str, Value> = HashMap::new();
        let attributes: Dict = attributes.into();

        properties.insert(SS_ITEM_LABEL, label.into());
        properties.insert(SS_ITEM_ATTRIBUTES, attributes.into());

        let created_item = self
            .collection_proxy
            .create_item(properties, secret_struct, replace)
            .await?;

        // This prompt handling is practically identical to create_collection
        let item_path: ObjectPath = {
            // Get path of created object
            let created_path = created_item.item;

            // Check if that path is "/", if so should execute a prompt
            if created_path.as_str() == "/" {
                let prompt_path = created_item.prompt;

                // Exec prompt and parse result
                let prompt_res = exec_prompt(self.conn.clone(), &prompt_path).await?;
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
        .await
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[tokio::test]
    async fn should_create_collection_struct() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let _ = ss.get_default_collection().await.unwrap();
        // tested under SecretService struct
    }

    #[tokio::test]
    async fn should_check_if_collection_locked() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let _ = collection.is_locked().await.unwrap();
    }

    #[tokio::test]
    #[ignore] // should unignore this test this manually, otherwise will constantly prompt during tests.
    async fn should_lock_and_unlock() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let locked = collection.is_locked().await.unwrap();
        if locked {
            collection.unlock().await.unwrap();
            collection.ensure_unlocked().await.unwrap();
            assert!(!collection.is_locked().await.unwrap());
            collection.lock().await.unwrap();
            assert!(collection.is_locked().await.unwrap());
        } else {
            collection.lock().await.unwrap();
            assert!(collection.is_locked().await.unwrap());
            collection.unlock().await.unwrap();
            collection.ensure_unlocked().await.unwrap();
            assert!(!collection.is_locked().await.unwrap());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn should_delete_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collections = ss.get_all_collections().await.unwrap();
        let count_before = collections.len();
        for collection in collections {
            let collection_path = &*collection.collection_path;
            if collection_path.contains("Test") {
                collection.unlock().await.unwrap();
                collection.delete().await.unwrap();
            }
        }
        //double check after
        let collections = ss.get_all_collections().await.unwrap();
        assert!(
            collections.len() < count_before,
            "collections before delete {count_before}"
        );
    }

    #[tokio::test]
    async fn should_get_all_items() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        collection.get_all_items().await.unwrap();
    }

    #[tokio::test]
    async fn should_search_items() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();

        // Create an item
        let item = collection
            .create_item(
                "test",
                HashMap::from([("test_attributes_in_collection", "test")]),
                b"test_secret",
                false,
                "text/plain",
            )
            .await
            .unwrap();

        // handle empty vec search
        collection.search_items(HashMap::new()).await.unwrap();

        // handle no result
        let bad_search = collection
            .search_items(HashMap::from([("test_bad", "test")]))
            .await
            .unwrap();
        assert_eq!(bad_search.len(), 0);

        // handle correct search for item and compare
        let search_item = collection
            .search_items(HashMap::from([("test_attributes_in_collection", "test")]))
            .await
            .unwrap();

        assert_eq!(item.item_path, search_item[0].item_path);
        item.delete().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn should_get_and_set_collection_label() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let label = collection.get_label().await.unwrap();
        assert_eq!(label, "Login");

        // Set label to test and check
        collection.unlock().await.unwrap();
        collection.set_label("Test").await.unwrap();
        let label = collection.get_label().await.unwrap();
        assert_eq!(label, "Test");

        // Reset label to original and test
        collection.unlock().await.unwrap();
        collection.set_label("Login").await.unwrap();
        let label = collection.get_label().await.unwrap();
        assert_eq!(label, "Login");

        collection.lock().await.unwrap();
    }
}
