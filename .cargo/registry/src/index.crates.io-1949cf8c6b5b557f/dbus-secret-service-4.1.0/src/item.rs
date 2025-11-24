// Copyright 2016-2024 dbus-secret-service Contributors
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::collections::HashMap;

use dbus::{
    blocking::{Connection, Proxy},
    strings::Path,
};

use crate::{
    error::Error,
    proxy::{item::Item as ProxyItem, new_proxy},
    session::EncryptedSecret,
    LockAction, SecretService,
};

/// Represents a Secret Service item that has key/value attributes and a secret.
///
/// Item lifetimes are tied to the [`SecretService`] instance they were retrieved
/// from or created by (whether directly or via a [`crate::Collection`] object), and they
/// cannot outlive that instance.
pub struct Item<'a> {
    service: &'a SecretService,
    pub path: Path<'static>,
}

impl<'a> Item<'a> {
    pub fn new(service: &'a SecretService, path: Path<'static>) -> Item<'a> {
        Item { service, path }
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

    pub fn get_attributes(&self) -> Result<HashMap<String, String>, Error> {
        Ok(self.proxy().attributes()?)
    }

    pub fn set_attributes(&self, attributes: HashMap<&str, &str>) -> Result<(), Error> {
        let attributes = attributes
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Ok(self.proxy().set_attributes(attributes)?)
    }

    pub fn get_label(&self) -> Result<String, Error> {
        Ok(self.proxy().label()?)
    }

    pub fn set_label(&self, new_label: &str) -> Result<(), Error> {
        Ok(self.proxy().set_label(new_label.to_string())?)
    }

    /// Delete the underlying dbus item
    pub fn delete(&self) -> Result<(), Error> {
        let p_path = self.proxy().delete()?;
        if p_path != Path::new("/")? {
            self.service.prompt_for_lock_unlock_delete(&p_path)
        } else {
            Ok(())
        }
    }

    pub fn get_secret(&self) -> Result<Vec<u8>, Error> {
        let tuple = self.proxy().get_secret(self.service.session.path.clone())?;
        let encrypted = EncryptedSecret::from_dbus(tuple);
        let decrypted = self.service.session.decrypt_secret(encrypted)?;
        Ok(decrypted)
    }

    pub fn get_secret_content_type(&self) -> Result<String, Error> {
        let tuple = self.proxy().get_secret(self.service.session.path.clone())?;
        let encrypted = EncryptedSecret::from_dbus(tuple);
        let mime = encrypted.mime.clone();
        let _ = self.service.session.decrypt_secret(encrypted)?;
        Ok(mime)
    }

    pub fn set_secret(&self, secret: &[u8], content_type: &str) -> Result<(), Error> {
        let encrypted = self.service.session.encrypt_secret(secret, content_type);
        Ok(self.proxy().set_secret(encrypted.to_dbus())?)
    }

    pub fn get_created(&self) -> Result<u64, Error> {
        Ok(self.proxy().created()?)
    }

    pub fn get_modified(&self) -> Result<u64, Error> {
        Ok(self.proxy().modified()?)
    }

    /// Compare items to see if they refer to the same secret service object.
    pub fn equal_to(&self, other: &Item<'_>) -> Result<bool, Error> {
        Ok(self.path == other.path)
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    fn create_test_default_item<'a>(collection: &'a Collection<'_>) -> Item<'a> {
        collection
            .create_item("Test", HashMap::new(), b"test", false, "text/plain")
            .unwrap()
    }

    #[test]
    fn should_create_and_delete_item() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        item.delete().unwrap();
        // Random operation to prove that item no longer exists
        if item.get_label().is_ok() {
            panic!("item still existed");
        }
    }

    #[test]
    fn should_check_if_item_locked() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        item.is_locked().unwrap();
        item.delete().unwrap();
    }

    #[test]
    #[ignore] // can't run headless - prompts
    fn should_lock_and_unlock() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.create_collection("TestItemLockUnlock", "").unwrap();
        let item = create_test_default_item(&collection);
        item.ensure_unlocked().unwrap();
        item.lock().unwrap();
        assert!(item.is_locked().unwrap());
        item.ensure_unlocked().unwrap();
        item.delete().unwrap();
    }

    #[test]
    fn should_get_and_set_item_label() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        // Set label to test and check
        item.set_label("Tester").unwrap();
        let label = item.get_label().unwrap();
        assert_eq!(label, "Tester");
        item.delete().unwrap();
    }

    #[test]
    fn should_create_with_item_attributes() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = collection
            .create_item(
                "Test",
                HashMap::from([("test_attributes_in_item", "test")]),
                b"test",
                false,
                "text/plain",
            )
            .unwrap();

        let attributes = item.get_attributes().unwrap();

        // We do not compare exact attributes, since the secret service provider could add its own
        // at any time. Instead, we only check that the ones we provided are returned.
        assert_eq!(
            attributes
                .get("test_attributes_in_item")
                .map(String::as_str),
            Some("test")
        );

        item.delete().unwrap();
    }

    #[test]
    fn should_get_and_set_item_attributes() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        // Also test empty array handling
        item.set_attributes(HashMap::new()).unwrap();
        item.set_attributes(HashMap::from([("test_attributes_in_item_get", "test")]))
            .unwrap();

        let attributes = item.get_attributes().unwrap();

        // We do not compare exact attributes, since the secret service provider could add its own
        // at any time. Instead, we only check that the ones we provided are returned.
        assert_eq!(
            attributes
                .get("test_attributes_in_item_get")
                .map(String::as_str),
            Some("test")
        );

        item.delete().unwrap();
    }

    #[test]
    fn should_get_modified_created_props() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        item.set_label("Tester").unwrap();
        let _created = item.get_created().unwrap();
        let _modified = item.get_modified().unwrap();
        item.delete().unwrap();
    }

    #[test]
    fn should_create_and_get_secret() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        let secret = item.get_secret().unwrap();
        item.delete().unwrap();
        assert_eq!(secret, b"test");
    }

    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
    #[test]
    fn should_create_and_get_secret_encrypted() {
        let ss = SecretService::connect(EncryptionType::Dh).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        let secret = item.get_secret().unwrap();
        item.delete().unwrap();
        assert_eq!(secret, b"test");
    }

    #[test]
    fn should_get_secret_content_type() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        let content_type = item.get_secret_content_type().unwrap();
        item.delete().unwrap();
        assert_eq!(content_type, "text/plain".to_owned());
    }

    #[test]
    fn should_set_secret() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        item.set_secret(b"new_test", "text/plain").unwrap();
        let secret = item.get_secret().unwrap();
        item.delete().unwrap();
        assert_eq!(secret, b"new_test");
    }

    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
    #[test]
    fn should_create_encrypted_item() {
        let ss = SecretService::connect(EncryptionType::Dh).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = collection
            .create_item(
                "Test",
                HashMap::new(),
                b"test_encrypted",
                false,
                "text/plain",
            )
            .expect("Error on item creation");
        let secret = item.get_secret().unwrap();
        item.delete().unwrap();
        assert_eq!(secret, b"test_encrypted");
    }

    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
    #[test]
    fn should_create_encrypted_item_from_empty_secret() {
        //empty string
        let ss = SecretService::connect(EncryptionType::Dh).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = collection
            .create_item("Test", HashMap::new(), b"", false, "text/plain")
            .expect("Error on item creation");
        let secret = item.get_secret().unwrap();
        item.delete().unwrap();
        assert_eq!(secret, b"");
    }

    #[cfg(any(feature = "crypto-rust", feature = "crypto-openssl"))]
    #[test]
    fn should_get_encrypted_secret_across_dbus_connections() {
        {
            let ss = SecretService::connect(EncryptionType::Dh).unwrap();
            let collection = ss.get_default_collection().unwrap();
            let item = collection
                .create_item(
                    "Test",
                    HashMap::from([("test_attributes_in_item_encrypt", "test")]),
                    b"test_encrypted",
                    false,
                    "text/plain",
                )
                .expect("Error on item creation");
            let secret = item.get_secret().unwrap();
            assert_eq!(secret, b"test_encrypted");
        }
        {
            let ss = SecretService::connect(EncryptionType::Dh).unwrap();
            let collection = ss.get_default_collection().unwrap();
            let search_item = collection
                .search_items(HashMap::from([("test_attributes_in_item_encrypt", "test")]))
                .unwrap();
            let item = search_item.first().unwrap();
            assert_eq!(item.get_secret().unwrap(), b"test_encrypted");
            item.delete().unwrap();
        }
    }
}
