// Copyright 2016-2024 dbus-secret-service Contributors
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#![allow(clippy::needless_doctest_main)]

//! # Dbus-Based access to the Secret Service
//!
//! This library implements a rust wrapper that uses libdbus to access the Secret Service.
//! Its use requires that a session `DBus` is available on the target machine.
//!
//! ## About the Secret Service
//!
//! <https://standards.freedesktop.org/secret-service/>
//!
//! The Secret Service provides a secure mechanism for persistent storage of data.
//! Both the Gnome keyring and the KWallet implement the Secret Service API.
//!
//! ## Basic Usage
//!
//! ```
//! use dbus_secret_service::SecretService;
//! use dbus_secret_service::EncryptionType;
//! use std::collections::HashMap;
//!
//! fn main() {
//!    // initialize secret service (dbus connection and encryption session)
//!    let ss = SecretService::connect(EncryptionType::Plain).unwrap();
//!
//!    // get default collection
//!    let collection = ss.get_default_collection().unwrap();
//!
//!    let mut properties = HashMap::new();
//!    properties.insert("test", "test_value");
//!
//!    //create new item
//!    collection.create_item(
//!        "test_label", // label
//!        properties,
//!        b"test_secret", //secret
//!        false, // replace item with same attributes
//!        "text/plain" // secret content type
//!    ).unwrap();
//!
//!    // search items by properties
//!    let search_items = ss.search_items(
//!        HashMap::from([("test", "test_value")])
//!    ).unwrap();
//!
//!    // retrieve one item, first by checking the unlocked items
//!    let item = match search_items.unlocked.first() {
//!        Some(item) => item,
//!        None => {
//!            // if there aren't any, check the locked items and unlock the first one
//!            let locked_item = search_items
//!                .locked
//!                .first()
//!                .expect("Search didn't return any items!");
//!            locked_item.unlock().unwrap();
//!            locked_item
//!        }
//!    };
//!
//!    // retrieve secret from the item
//!    let secret = item.get_secret().unwrap();
//!    assert_eq!(secret, b"test_secret");
//!
//!    // delete item (deletes the dbus object, not the struct instance)
//!    item.delete().unwrap()
//! }
//! ```
//!
//! ## Overview of this library:
//!
//! ### Entry point
//! The entry point for this library is the [`SecretService`] struct. Creating an instance
//! of this structure will initialize the dbus connection and create a session with the
//! Secret Service.
//!
//! ```
//! # use dbus_secret_service::SecretService;
//! # use dbus_secret_service::EncryptionType;
//! # fn call() {
//! SecretService::connect(EncryptionType::Plain).unwrap();
//! # }
//! ```
//! A session started with `EncryptionType::Plain` does not obscure the content
//! of secrets in memory when sending them to and from the Secret Service.  These
//! secrets _are_ encrypted by the Secret Service when put into its secure store.
//!
//! If you have specified a crypto feature (`crypto-rust` or `crypto-openssl`),
//! then you can use `EncryptionType:Dh` to force Diffie-Hellman shared key encryption
//! of secrets in memory when they are being sent to and received from the Secret Service.
//!
//! Once you have created a `SecretService` struct, you can use it to search for items,
//! connect to the default collection of items, and to create new collections. The lifetimes
//! of all the collection and item objects you retrieve from the service are tied to
//! the service, so they cannot outlive the service instance. This restriction will
//! be enforced by the Rust compiler.
//!
//! ### Collections and Items
//! The Secret Service API organizes secrets into collections and holds each secret
//! in an item.
//!
//! Items consist of a label, attributes, and the secret. The most common way to find
//! an item is a search by attributes.
//!
//! While it's possible to create new collections, most users will simply create items
//! within the default collection.
//!
//! ### Actions overview
//! The most common supported actions are `create`, `get`, `search`, and `delete` for
//! `Collections` and `Items`. For more specifics and exact method names, please see
//! each structure's documentation.
//!
//! In addition, `set` and `get` actions are available for secrets contained in an `Item`.
//!
//! ## Headless usage
//!
//! If you must use the secret-service on a headless linux box,
//! be aware that there are known issues with getting
//! dbus and secret-service and the gnome keyring
//! to work properly in headless environments.
//! For a quick workaround, look at how this project's
//! [CI workflow](https://github.com/hwchen/keyring-rs/blob/master/.github/workflows/ci.yaml)
//! starts the Gnome keyring unlocked with a known password;
//! a similar solution is also documented in the
//! [Python Keyring docs](https://pypi.org/project/keyring/)
//! (search for "Using Keyring on headless Linux systems").
//! The following `bash` function may be helpful:
//!
//! ```shell
//! function unlock-keyring ()
//! {
//! read -rsp "Password: " pass
//! echo -n "$pass" | gnome-keyring-daemon --unlock
//! unset pass
//! }
//! ```
//!
//! For an excellent treatment of all the headless dbus issues, see
//! [this answer on ServerFault](https://serverfault.com/a/906224/79617).
//!
use std::collections::HashMap;

use dbus::arg::RefArg;
pub use dbus::strings::Path;
use dbus::{
    arg::{PropMap, Variant},
    blocking::{Connection, Proxy},
};

pub use collection::Collection;
pub use error::Error;
pub use item::Item;
use proxy::{new_proxy, service::Service};
pub use session::EncryptionType;
use session::Session;
use ss::{SS_COLLECTION_LABEL, SS_DBUS_PATH};

mod collection;
mod error;
mod item;
mod prompt;
mod proxy;
mod session;
mod ss;

/// Encapsulates a session connected to the Secret Service.
pub struct SecretService {
    connection: Connection,
    session: Session,
    timeout: Option<u64>,
}

/// Represents the results of doing a service-wide search.
///
/// The returned items are organized in two vectors: one
/// holds unlocked items and the other holds locked items.
/// (Reading or writing the secret of a locked item requires
/// prompting the user interactively for permission.  This
/// prompting is done by the Secret Service itself.)
pub struct SearchItemsResult<T> {
    pub unlocked: Vec<T>,
    pub locked: Vec<T>,
}

pub(crate) enum LockAction {
    Lock,
    Unlock,
}

impl SecretService {
    /// Connect to the DBus and return a new [SecretService] instance.
    ///
    /// If this service instance needs to prompt a user for permission to
    /// access a locked item or collection, it will block indefinitely waiting for
    /// the user's response  See [connect_with_timeout] if you want
    /// different behavior.
    pub fn connect(encryption: EncryptionType) -> Result<Self, Error> {
        let connection = Connection::new_session()?;
        let session = Session::new(new_proxy(&connection, SS_DBUS_PATH), encryption)?;
        Ok(SecretService {
            connection,
            session,
            timeout: None,
        })
    }

    /// Connect to the DBus and return a new [SecretService] instance.
    ///
    /// If this service instance needs to prompt a user for permission to
    /// access a locked item or collection,
    /// it will only block for the given number of seconds,
    /// after which it will dismiss the prompt and cancel the operation.
    /// (Specifying 0 for the number of seconds will prevent the prompt
    /// from appearing at all: the operation will immediately be canceled.)
    pub fn connect_with_max_prompt_timeout(
        encryption: EncryptionType,
        seconds: u64,
    ) -> Result<Self, Error> {
        let mut service = Self::connect(encryption)?;
        service.timeout = Some(seconds);
        Ok(service)
    }

    /// Get the service proxy (internal)
    fn proxy(&self) -> Proxy<'_, &Connection> {
        new_proxy(&self.connection, SS_DBUS_PATH)
    }

    /// Get all collections
    pub fn get_all_collections(&'_ self) -> Result<Vec<Collection<'_>>, Error> {
        let paths = self.proxy().collections()?;
        let collections = paths
            .into_iter()
            .map(|path| Collection::new(self, path))
            .collect();
        Ok(collections)
    }

    /// Get a collection by alias.
    ///
    /// The most common would be the `default` alias, but there
    /// is also a specific method for getting the collection
    /// by default alias.
    pub fn get_collection_by_alias(&'_ self, alias: &str) -> Result<Collection<'_>, Error> {
        let path = self.proxy().read_alias(alias)?;
        if path == Path::new("/")? {
            Err(Error::NoResult)
        } else {
            Ok(Collection::new(self, path))
        }
    }

    /// Get the default collection.
    /// (The collection whose alias is `default`)
    pub fn get_default_collection(&self) -> Result<Collection<'_>, Error> {
        self.get_collection_by_alias("default")
    }

    /// Get any collection.
    /// First tries `default` collection, then `session`
    /// collection, then the first collection when it
    /// gets all collections.
    pub fn get_any_collection(&self) -> Result<Collection<'_>, Error> {
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
    pub fn create_collection(&self, label: &str, alias: &str) -> Result<Collection<'_>, Error> {
        let mut properties: PropMap = HashMap::new();
        properties.insert(
            SS_COLLECTION_LABEL.to_string(),
            Variant(Box::new(label.to_string()) as Box<dyn RefArg>),
        );
        // create a collection returning the collection path and prompt path
        let (c_path, p_path) = self.proxy().create_collection(properties, alias)?;
        let created = {
            if c_path == Path::new("/")? {
                // no creation path, so prompt
                self.prompt_for_create(&p_path)?
            } else {
                c_path
            }
        };
        Ok(Collection::new(self, created))
    }

    /// Searches all items by attributes
    pub fn search_items(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> Result<SearchItemsResult<Item<'_>>, Error> {
        let (unlocked, locked) = self.proxy().search_items(attributes)?;
        let result = SearchItemsResult {
            unlocked: unlocked.into_iter().map(|p| Item::new(self, p)).collect(),
            locked: locked.into_iter().map(|p| Item::new(self, p)).collect(),
        };
        Ok(result)
    }

    /// Unlock all items in a batch
    pub fn unlock_all(&self, items: &[&Item<'_>]) -> Result<(), Error> {
        let paths = items.iter().map(|i| i.path.clone()).collect();
        self.lock_unlock_all(LockAction::Unlock, paths)
    }

    pub(crate) fn lock_unlock_all(
        &self,
        action: LockAction,
        paths: Vec<Path>,
    ) -> Result<(), Error> {
        let (_, p_path) = match action {
            LockAction::Lock => self.proxy().lock(paths)?,
            LockAction::Unlock => self.proxy().unlock(paths)?,
        };
        if p_path == Path::new("/")? {
            Ok(())
        } else {
            self.prompt_for_lock_unlock_delete(&p_path)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn should_create_secret_service() {
        SecretService::connect(EncryptionType::Plain).unwrap();
    }

    #[test]
    fn should_get_all_collections() {
        // Assumes that there will always be a default collection
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

        match ss.get_collection_by_alias("definitely_definitely_does_not_exist") {
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

    #[test]
    #[ignore] // can't run headless - prompts
    fn should_create_and_delete_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let test_collection = ss.create_collection("TestCreateDelete", "").unwrap();
        assert!(test_collection
            .path
            .starts_with("/org/freedesktop/secrets/collection/Test"));
        test_collection.delete().unwrap();
    }

    #[test]
    fn should_search_items() {
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
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

        assert_eq!(item.path, search_item.unlocked[0].path);
        assert_eq!(search_item.locked.len(), 0);
        item.delete().unwrap();
    }

    #[test]
    #[ignore] // can't run headless - prompts
    fn should_lock_and_unlock() {
        // Assumes that there will always be at least one collection
        let ss = SecretService::connect(EncryptionType::Plain).unwrap();
        let collections = ss.get_all_collections().unwrap();
        assert!(!collections.is_empty(), "no collections found");
        let paths: Vec<Path> = collections.iter().map(|c| c.path.clone()).collect();
        ss.lock_unlock_all(LockAction::Lock, paths.clone()).unwrap();
        ss.lock_unlock_all(LockAction::Unlock, paths).unwrap();
    }
}
