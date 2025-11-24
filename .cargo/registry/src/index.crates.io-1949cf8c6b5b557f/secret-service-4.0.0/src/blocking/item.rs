//Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::error::Error;
use crate::proxy::item::ItemProxyBlocking;
use crate::proxy::service::ServiceProxyBlocking;
use crate::session::decrypt;
use crate::session::Session;
use crate::ss::SS_DBUS_NAME;
use crate::util::{exec_prompt_blocking, format_secret, lock_or_unlock_blocking, LockAction};

use std::collections::HashMap;
use zbus::{zvariant::OwnedObjectPath, CacheProperties};

pub struct Item<'a> {
    conn: zbus::blocking::Connection,
    session: &'a Session,
    pub item_path: OwnedObjectPath,
    item_proxy: ItemProxyBlocking<'a>,
    service_proxy: &'a ServiceProxyBlocking<'a>,
}

impl<'a> Item<'a> {
    pub(crate) fn new(
        conn: zbus::blocking::Connection,
        session: &'a Session,
        service_proxy: &'a ServiceProxyBlocking<'a>,
        item_path: OwnedObjectPath,
    ) -> Result<Self, Error> {
        let item_proxy = ItemProxyBlocking::builder(&conn)
            .destination(SS_DBUS_NAME)?
            .path(item_path.clone())?
            .cache_properties(CacheProperties::No)
            .build()?;
        Ok(Item {
            conn,
            session,
            item_path,
            item_proxy,
            service_proxy,
        })
    }

    pub fn is_locked(&self) -> Result<bool, Error> {
        Ok(self.item_proxy.locked()?)
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
            &self.item_path,
            LockAction::Unlock,
        )
    }

    pub fn lock(&self) -> Result<(), Error> {
        lock_or_unlock_blocking(
            self.conn.clone(),
            self.service_proxy,
            &self.item_path,
            LockAction::Lock,
        )
    }

    pub fn get_attributes(&self) -> Result<HashMap<String, String>, Error> {
        Ok(self.item_proxy.attributes()?)
    }

    pub fn set_attributes(&self, attributes: HashMap<&str, &str>) -> Result<(), Error> {
        Ok(self.item_proxy.set_attributes(attributes)?)
    }

    pub fn get_label(&self) -> Result<String, Error> {
        Ok(self.item_proxy.label()?)
    }

    pub fn set_label(&self, new_label: &str) -> Result<(), Error> {
        Ok(self.item_proxy.set_label(new_label)?)
    }

    /// Deletes dbus object, but struct instance still exists (current implementation)
    pub fn delete(&self) -> Result<(), Error> {
        // ensure_unlocked handles prompt for unlocking if necessary
        self.ensure_unlocked()?;
        let prompt_path = self.item_proxy.delete()?;

        // "/" means no prompt necessary
        if prompt_path.as_str() != "/" {
            exec_prompt_blocking(self.conn.clone(), &prompt_path)?;
        }

        Ok(())
    }

    pub fn get_secret(&self) -> Result<Vec<u8>, Error> {
        let secret_struct = self.item_proxy.get_secret(&self.session.object_path)?;
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

    pub fn get_secret_content_type(&self) -> Result<String, Error> {
        let secret_struct = self.item_proxy.get_secret(&self.session.object_path)?;
        let content_type = secret_struct.content_type;

        Ok(content_type)
    }

    pub fn set_secret(&self, secret: &[u8], content_type: &str) -> Result<(), Error> {
        let secret_struct = format_secret(self.session, secret, content_type)?;
        Ok(self.item_proxy.set_secret(secret_struct)?)
    }

    pub fn get_created(&self) -> Result<u64, Error> {
        Ok(self.item_proxy.created()?)
    }

    pub fn get_modified(&self) -> Result<u64, Error> {
        Ok(self.item_proxy.modified()?)
    }
}

impl<'a> Eq for Item<'a> {}
impl<'a> PartialEq for Item<'a> {
    fn eq(&self, other: &Item) -> bool {
        self.item_path == other.item_path
            && self.get_attributes().unwrap() == other.get_attributes().unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::blocking::*;

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
        // Random operation to prove that path no longer exists
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
    #[ignore]
    fn should_lock_and_unlock() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = create_test_default_item(&collection);

        let locked = item.is_locked().unwrap();
        if locked {
            item.unlock().unwrap();
            item.ensure_unlocked().unwrap();
            assert!(!item.is_locked().unwrap());
            item.lock().unwrap();
            assert!(item.is_locked().unwrap());
        } else {
            item.lock().unwrap();
            assert!(item.is_locked().unwrap());
            item.unlock().unwrap();
            item.ensure_unlocked().unwrap();
            assert!(!item.is_locked().unwrap());
        }
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
        // at any time. Instead, we only check that the ones we provided are returned back.
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
        // at any time. Instead, we only check that the ones we provided are returned back.
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

    #[test]
    fn should_create_encrypted_item_from_empty_secret() {
        let ss = SecretService::connect(EncryptionType::Dh).unwrap();
        let collection = ss.get_default_collection().unwrap();
        let item = collection
            .create_item("Test", HashMap::new(), b"", false, "text/plain")
            .expect("Error on item creation");
        let secret = item.get_secret().unwrap();
        item.delete().unwrap();
        assert_eq!(secret, b"");
    }

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
