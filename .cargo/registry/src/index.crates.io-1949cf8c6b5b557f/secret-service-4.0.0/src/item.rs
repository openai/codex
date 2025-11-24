//Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::error::Error;
use crate::proxy::item::ItemProxy;
use crate::proxy::service::ServiceProxy;
use crate::session::decrypt;
use crate::session::Session;
use crate::ss::SS_DBUS_NAME;
use crate::util::{exec_prompt, format_secret, lock_or_unlock, LockAction};

use std::collections::HashMap;
use zbus::{zvariant::OwnedObjectPath, CacheProperties};

pub struct Item<'a> {
    conn: zbus::Connection,
    session: &'a Session,
    pub item_path: OwnedObjectPath,
    item_proxy: ItemProxy<'a>,
    service_proxy: &'a ServiceProxy<'a>,
}

impl<'a> Item<'a> {
    pub(crate) async fn new(
        conn: zbus::Connection,
        session: &'a Session,
        service_proxy: &'a ServiceProxy<'a>,
        item_path: OwnedObjectPath,
    ) -> Result<Item<'a>, Error> {
        let item_proxy = ItemProxy::builder(&conn)
            .destination(SS_DBUS_NAME)?
            .path(item_path.clone())?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;

        Ok(Item {
            conn,
            session,
            item_path,
            item_proxy,
            service_proxy,
        })
    }

    pub async fn is_locked(&self) -> Result<bool, Error> {
        Ok(self.item_proxy.locked().await?)
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
            &self.item_path,
            LockAction::Unlock,
        )
        .await
    }

    pub async fn lock(&self) -> Result<(), Error> {
        lock_or_unlock(
            self.conn.clone(),
            self.service_proxy,
            &self.item_path,
            LockAction::Lock,
        )
        .await
    }

    pub async fn get_attributes(&self) -> Result<HashMap<String, String>, Error> {
        Ok(self.item_proxy.attributes().await?)
    }

    pub async fn set_attributes(&self, attributes: HashMap<&str, &str>) -> Result<(), Error> {
        Ok(self.item_proxy.set_attributes(attributes).await?)
    }

    pub async fn get_label(&self) -> Result<String, Error> {
        Ok(self.item_proxy.label().await?)
    }

    pub async fn set_label(&self, new_label: &str) -> Result<(), Error> {
        Ok(self.item_proxy.set_label(new_label).await?)
    }

    /// Deletes dbus object, but struct instance still exists (current implementation)
    pub async fn delete(&self) -> Result<(), Error> {
        // ensure_unlocked handles prompt for unlocking if necessary
        self.ensure_unlocked().await?;
        let prompt_path = self.item_proxy.delete().await?;

        // "/" means no prompt necessary
        if prompt_path.as_str() != "/" {
            exec_prompt(self.conn.clone(), &prompt_path).await?;
        }

        Ok(())
    }

    pub async fn get_secret(&self) -> Result<Vec<u8>, Error> {
        let secret_struct = self
            .item_proxy
            .get_secret(&self.session.object_path)
            .await?;
        let secret = secret_struct.value;

        if let Some(session_key) = self.session.get_aes_key() {
            // get "param" (aes_iv) field out of secret struct
            let aes_iv = secret_struct.parameters;

            // decrypt
            let decrypted_secret = decrypt(&secret, session_key, &aes_iv)?;

            Ok(decrypted_secret)
        } else {
            Ok(secret)
        }
    }

    pub async fn get_secret_content_type(&self) -> Result<String, Error> {
        let secret_struct = self
            .item_proxy
            .get_secret(&self.session.object_path)
            .await?;
        let content_type = secret_struct.content_type;

        Ok(content_type)
    }

    pub async fn set_secret(&self, secret: &[u8], content_type: &str) -> Result<(), Error> {
        let secret_struct = format_secret(self.session, secret, content_type)?;
        Ok(self.item_proxy.set_secret(secret_struct).await?)
    }

    pub async fn get_created(&self) -> Result<u64, Error> {
        Ok(self.item_proxy.created().await?)
    }

    pub async fn get_modified(&self) -> Result<u64, Error> {
        Ok(self.item_proxy.modified().await?)
    }

    /// Returns if an item is equal to `other`.
    ///
    /// This is the equivalent of the `PartialEq` trait, but `async`.
    pub async fn equal_to(&self, other: &Item<'_>) -> Result<bool, Error> {
        let this_attrs = self.get_attributes().await?;
        let other_attrs = other.get_attributes().await?;

        Ok(self.item_path == other.item_path && this_attrs == other_attrs)
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    async fn create_test_default_item<'a>(collection: &'a Collection<'_>) -> Item<'a> {
        collection
            .create_item("Test", HashMap::new(), b"test", false, "text/plain")
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn should_create_and_delete_item() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        item.delete().await.unwrap();
        // Random operation to prove that path no longer exists
        if item.get_label().await.is_ok() {
            panic!("item still existed");
        }
    }

    #[tokio::test]
    async fn should_check_if_item_locked() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        item.is_locked().await.unwrap();
        item.delete().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn should_lock_and_unlock() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        let locked = item.is_locked().await.unwrap();
        if locked {
            item.unlock().await.unwrap();
            item.ensure_unlocked().await.unwrap();
            assert!(!item.is_locked().await.unwrap());
            item.lock().await.unwrap();
            assert!(item.is_locked().await.unwrap());
        } else {
            item.lock().await.unwrap();
            assert!(item.is_locked().await.unwrap());
            item.unlock().await.unwrap();
            item.ensure_unlocked().await.unwrap();
            assert!(!item.is_locked().await.unwrap());
        }
        item.delete().await.unwrap();
    }

    #[tokio::test]
    async fn should_get_and_set_item_label() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        // Set label to test and check
        item.set_label("Tester").await.unwrap();
        let label = item.get_label().await.unwrap();
        assert_eq!(label, "Tester");
        item.delete().await.unwrap();
    }

    #[tokio::test]
    async fn should_create_with_item_attributes() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = collection
            .create_item(
                "Test",
                HashMap::from([("test_attributes_in_item", "test")]),
                b"test",
                false,
                "text/plain",
            )
            .await
            .unwrap();

        let attributes = item.get_attributes().await.unwrap();

        // We do not compare exact attributes, since the secret service provider could add its own
        // at any time. Instead, we only check that the ones we provided are returned back.
        assert_eq!(
            attributes
                .get("test_attributes_in_item")
                .map(String::as_str),
            Some("test")
        );

        item.delete().await.unwrap();
    }

    #[tokio::test]
    async fn should_get_and_set_item_attributes() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        // Also test empty array handling
        item.set_attributes(HashMap::new()).await.unwrap();
        item.set_attributes(HashMap::from([("test_attributes_in_item_get", "test")]))
            .await
            .unwrap();

        let attributes = item.get_attributes().await.unwrap();

        // We do not compare exact attributes, since the secret service provider could add its own
        // at any time. Instead, we only check that the ones we provided are returned back.
        assert_eq!(
            attributes
                .get("test_attributes_in_item_get")
                .map(String::as_str),
            Some("test")
        );

        item.delete().await.unwrap();
    }

    #[tokio::test]
    async fn should_get_modified_created_props() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        item.set_label("Tester").await.unwrap();
        let _created = item.get_created().await.unwrap();
        let _modified = item.get_modified().await.unwrap();
        item.delete().await.unwrap();
    }

    #[tokio::test]
    async fn should_create_and_get_secret() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        let secret = item.get_secret().await.unwrap();
        item.delete().await.unwrap();
        assert_eq!(secret, b"test");
    }

    #[tokio::test]
    async fn should_create_and_get_secret_encrypted() {
        let ss = SecretService::connect(EncryptionType::Dh).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        let secret = item.get_secret().await.unwrap();
        item.delete().await.unwrap();
        assert_eq!(secret, b"test");
    }

    #[tokio::test]
    async fn should_get_secret_content_type() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        let content_type = item.get_secret_content_type().await.unwrap();
        item.delete().await.unwrap();
        assert_eq!(content_type, "text/plain".to_owned());
    }

    #[tokio::test]
    async fn should_set_secret() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = create_test_default_item(&collection).await;

        item.set_secret(b"new_test", "text/plain").await.unwrap();
        let secret = item.get_secret().await.unwrap();
        item.delete().await.unwrap();
        assert_eq!(secret, b"new_test");
    }

    #[tokio::test]
    async fn should_create_encrypted_item() {
        let ss = SecretService::connect(EncryptionType::Dh).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = collection
            .create_item(
                "Test",
                HashMap::new(),
                b"test_encrypted",
                false,
                "text/plain",
            )
            .await
            .expect("Error on item creation");
        let secret = item.get_secret().await.unwrap();
        item.delete().await.unwrap();
        assert_eq!(secret, b"test_encrypted");
    }

    #[tokio::test]
    async fn should_create_encrypted_item_from_empty_secret() {
        //empty string
        let ss = SecretService::connect(EncryptionType::Dh).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();
        let item = collection
            .create_item("Test", HashMap::new(), b"", false, "text/plain")
            .await
            .expect("Error on item creation");
        let secret = item.get_secret().await.unwrap();
        item.delete().await.unwrap();
        assert_eq!(secret, b"");
    }

    #[tokio::test]
    async fn should_get_encrypted_secret_across_dbus_connections() {
        {
            let ss = SecretService::connect(EncryptionType::Dh).await.unwrap();
            let collection = ss.get_default_collection().await.unwrap();
            let item = collection
                .create_item(
                    "Test",
                    HashMap::from([("test_attributes_in_item_encrypt", "test")]),
                    b"test_encrypted",
                    false,
                    "text/plain",
                )
                .await
                .expect("Error on item creation");
            let secret = item.get_secret().await.unwrap();
            assert_eq!(secret, b"test_encrypted");
        }
        {
            let ss = SecretService::connect(EncryptionType::Dh).await.unwrap();
            let collection = ss.get_default_collection().await.unwrap();
            let search_item = collection
                .search_items(HashMap::from([("test_attributes_in_item_encrypt", "test")]))
                .await
                .unwrap();
            let item = search_item.first().unwrap();
            assert_eq!(item.get_secret().await.unwrap(), b"test_encrypted");
            item.delete().await.unwrap();
        }
    }
}
