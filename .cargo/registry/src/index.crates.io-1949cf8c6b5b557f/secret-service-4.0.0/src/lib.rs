//Copyright 2022  secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! # Secret Service libary
//!
//! This library implements a rust interface to the Secret Service API which is implemented
//! in Linux.
//!
//! ## About Secret Service API
//! <https://standards.freedesktop.org/secret-service/>
//!
//! Secret Service provides a secure place to store secrets.
//! Gnome keyring and KWallet implement the Secret Service API.
//!
//! ## Basic Usage
//! ```
//! use secret_service::SecretService;
//! use secret_service::EncryptionType;
//! use std::collections::HashMap;
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!    // initialize secret service (dbus connection and encryption session)
//!    let ss = SecretService::connect(EncryptionType::Dh).await.unwrap();
//!
//!    // get default collection
//!    let collection = ss.get_default_collection().await.unwrap();
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
//!    ).await.unwrap();
//!
//!    // search items by properties
//!    let search_items = ss.search_items(
//!        HashMap::from([("test", "test_value")])
//!    ).await.unwrap();
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
//!            locked_item.unlock().await.unwrap();
//!            locked_item
//!        }
//!    };
//!
//!    // retrieve secret from item
//!    let secret = item.get_secret().await.unwrap();
//!    assert_eq!(secret, b"test_secret");
//!
//!    // delete item (deletes the dbus object, not the struct instance)
//!    item.delete().await.unwrap()
//! }
//! ```
//!
//! ## Overview of this library:
//! ### Entry point
//! The entry point for this library is the `SecretService` struct. A new instance of
//! `SecretService` will initialize the dbus connection and negotiate an encryption session.
//!
//! ```
//! # use secret_service::SecretService;
//! # use secret_service::EncryptionType;
//! # async fn call() {
//! SecretService::connect(EncryptionType::Plain).await.unwrap();
//! # }
//! ```
//!
//! or
//!
//! ```
//! # use secret_service::SecretService;
//! # use secret_service::EncryptionType;
//! # async fn call() {
//! SecretService::connect(EncryptionType::Dh).await.unwrap();
//! # }
//! ```
//!
//! Once the SecretService struct is initialized, it can be used to navigate to a collection.
//! Items can also be directly searched for without getting a collection first.
//!
//! ### Collections and Items
//! The Secret Service API organizes secrets into collections, and holds each secret
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
//! each struct's documentation.
//!
//! In addition, `set` and `get` actions are available for secrets contained in an `Item`.
//!
//! ### Crypto
//! Specifics in SecretService API Draft Proposal:
//! <https://standards.freedesktop.org/secret-service/>
//!
//! ### Async
//!
//! This crate, following `zbus`, is async by default. If you want a synchronous interface
//! that blocks, see the [blocking] module instead.
//
// Util currently has interfaces (dbus method namespace) to make it easier to call methods.
// Util contains function to execute prompts (used in many collection and item methods, like
// delete)

pub mod blocking;
mod error;
mod proxy;
mod session;
mod ss;
mod util;

mod collection;
pub use collection::Collection;

pub use error::Error;

mod item;
pub use item::Item;

pub use session::EncryptionType;

use crate::proxy::service::ServiceProxy;
use crate::session::Session;
use crate::ss::SS_COLLECTION_LABEL;
use crate::util::exec_prompt;
use futures_util::TryFutureExt;
use std::collections::HashMap;
use zbus::zvariant::{ObjectPath, Value};

/// Secret Service Struct.
///
/// This the main entry point for usage of the library.
///
/// Creating a new [SecretService] will also initialize dbus
/// and negotiate a new cryptographic session
/// ([EncryptionType::Plain] or [EncryptionType::Dh])
pub struct SecretService<'a> {
    conn: zbus::Connection,
    session: Session,
    service_proxy: ServiceProxy<'a>,
}

/// Used to indicate locked and unlocked items in the
/// return value of [SecretService::search_items]
/// and [blocking::SecretService::search_items].
pub struct SearchItemsResult<T> {
    pub unlocked: Vec<T>,
    pub locked: Vec<T>,
}

impl<'a> SecretService<'a> {
    /// Create a new `SecretService` instance.
    pub async fn connect(encryption: EncryptionType) -> Result<SecretService<'a>, Error> {
        let conn = zbus::Connection::session()
            .await
            .map_err(util::handle_conn_error)?;

        let service_proxy = ServiceProxy::new(&conn)
            .await
            .map_err(util::handle_conn_error)?;

        let session = Session::new(&service_proxy, encryption).await?;

        Ok(SecretService {
            conn,
            session,
            service_proxy,
        })
    }

    /// Get all collections
    pub async fn get_all_collections(&self) -> Result<Vec<Collection<'_>>, Error> {
        let collections = self.service_proxy.collections().await?;

        futures_util::future::join_all(collections.into_iter().map(|object_path| {
            Collection::new(
                self.conn.clone(),
                &self.session,
                &self.service_proxy,
                object_path.into(),
            )
        }))
        .await
        .into_iter()
        .collect::<Result<_, _>>()
    }

    /// Get collection by alias.
    ///
    /// Most common would be the `default` alias, but there
    /// is also a specific method for getting the collection
    /// by default alias.
    pub async fn get_collection_by_alias(&self, alias: &str) -> Result<Collection<'_>, Error> {
        let object_path = self.service_proxy.read_alias(alias).await?;

        if object_path.as_str() == "/" {
            Err(Error::NoResult)
        } else {
            Collection::new(
                self.conn.clone(),
                &self.session,
                &self.service_proxy,
                object_path,
            )
            .await
        }
    }

    /// Get default collection.
    /// (The collection whos alias is `default`)
    pub async fn get_default_collection(&self) -> Result<Collection<'_>, Error> {
        self.get_collection_by_alias("default").await
    }

    /// Get any collection.
    /// First tries `default` collection, then `session`
    /// collection, then the first collection when it
    /// gets all collections.
    pub async fn get_any_collection(&self) -> Result<Collection<'_>, Error> {
        // default first, then session, then first

        self.get_default_collection()
            .or_else(|_| self.get_collection_by_alias("session"))
            .or_else(|_| async {
                let mut collections = self.get_all_collections().await?;
                if collections.is_empty() {
                    Err(Error::NoResult)
                } else {
                    Ok(collections.swap_remove(0))
                }
            })
            .await
    }

    /// Creates a new collection with a label and an alias.
    pub async fn create_collection(
        &self,
        label: &str,
        alias: &str,
    ) -> Result<Collection<'_>, Error> {
        let mut properties: HashMap<&str, Value> = HashMap::new();
        properties.insert(SS_COLLECTION_LABEL, label.into());

        let created_collection = self
            .service_proxy
            .create_collection(properties, alias)
            .await?;

        // This prompt handling is practically identical to create_collection
        let collection_path: ObjectPath = {
            // Get path of created object
            let created_path = created_collection.collection;

            // Check if that path is "/", if so should execute a prompt
            if created_path.as_str() == "/" {
                let prompt_path = created_collection.prompt;

                // Exec prompt and parse result
                let prompt_res = exec_prompt(self.conn.clone(), &prompt_path).await?;
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
        .await
    }

    /// Searches all items by attributes
    pub async fn search_items(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> Result<SearchItemsResult<Item<'_>>, Error> {
        let items = self.service_proxy.search_items(attributes).await?;

        let object_paths_to_items = |items: Vec<_>| {
            futures_util::future::join_all(items.into_iter().map(|item_path| {
                Item::new(
                    self.conn.clone(),
                    &self.session,
                    &self.service_proxy,
                    item_path,
                )
            }))
        };

        Ok(SearchItemsResult {
            unlocked: object_paths_to_items(items.unlocked)
                .await
                .into_iter()
                .collect::<Result<_, _>>()?,
            locked: object_paths_to_items(items.locked)
                .await
                .into_iter()
                .collect::<Result<_, _>>()?,
        })
    }

    /// Unlock all items in a batch
    pub async fn unlock_all(&self, items: &[&Item<'_>]) -> Result<(), Error> {
        let objects = items.iter().map(|i| &*i.item_path).collect();
        let lock_action_res = self.service_proxy.unlock(objects).await?;

        if lock_action_res.object_paths.is_empty() {
            exec_prompt(self.conn.clone(), &lock_action_res.prompt).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::convert::TryFrom;
    use zbus::zvariant::ObjectPath;

    #[tokio::test]
    async fn should_create_secret_service() {
        SecretService::connect(EncryptionType::Plain).await.unwrap();
    }

    #[tokio::test]
    async fn should_get_all_collections() {
        // Assumes that there will always be a default collection
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collections = ss.get_all_collections().await.unwrap();
        assert!(!collections.is_empty(), "no collections found");
    }

    #[tokio::test]
    async fn should_get_collection_by_alias() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        ss.get_collection_by_alias("session").await.unwrap();
    }

    #[tokio::test]
    async fn should_return_error_if_collection_doesnt_exist() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();

        match ss
            .get_collection_by_alias("definitely_defintely_does_not_exist")
            .await
        {
            Err(Error::NoResult) => {}
            _ => panic!(),
        };
    }

    #[tokio::test]
    async fn should_get_default_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        ss.get_default_collection().await.unwrap();
    }

    #[tokio::test]
    async fn should_get_any_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let _ = ss.get_any_collection().await.unwrap();
    }

    #[test_with::no_env(GITHUB_ACTIONS)]
    #[tokio::test]
    async fn should_create_and_delete_collection() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let test_collection = ss.create_collection("Test", "").await.unwrap();
        assert_eq!(
            ObjectPath::from(test_collection.collection_path.clone()),
            ObjectPath::try_from("/org/freedesktop/secrets/collection/Test").unwrap()
        );
        test_collection.delete().await.unwrap();
    }

    #[tokio::test]
    async fn should_search_items() {
        let ss = SecretService::connect(EncryptionType::Plain).await.unwrap();
        let collection = ss.get_default_collection().await.unwrap();

        // Create an item
        let item = collection
            .create_item(
                "test",
                HashMap::from([("test_attribute_in_ss", "test_value")]),
                b"test_secret",
                false,
                "text/plain",
            )
            .await
            .unwrap();

        // handle empty vec search
        ss.search_items(HashMap::new()).await.unwrap();

        // handle no result
        let bad_search = ss
            .search_items(HashMap::from([("test", "test")]))
            .await
            .unwrap();
        assert_eq!(bad_search.unlocked.len(), 0);
        assert_eq!(bad_search.locked.len(), 0);

        // handle correct search for item and compare
        let search_item = ss
            .search_items(HashMap::from([("test_attribute_in_ss", "test_value")]))
            .await
            .unwrap();

        assert_eq!(item.item_path, search_item.unlocked[0].item_path);
        assert_eq!(search_item.locked.len(), 0);
        item.delete().await.unwrap();
    }
}
