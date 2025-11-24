// Copyright 2016-2024 dbus-secret-service Contributors
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use dbus_secret_service::{EncryptionType, SecretService};
use std::{collections::HashMap, str};

fn main() {
    // Initialize secret service
    let ss = SecretService::connect(EncryptionType::Plain).unwrap();

    // navigate to default collection
    let collection = ss.get_default_collection().unwrap();

    let mut properties = HashMap::new();
    properties.insert("test", "test_value");

    //create new item
    collection
        .create_item(
            "test_label", // label
            properties,
            b"test_secret", //secret
            false,          // replace item with same attributes
            "text/plain",   // secret content type
        )
        .unwrap();

    //println!("New Item: {:?}", new_item);

    // search items by properties
    let mut search_properties = HashMap::new();
    search_properties.insert("test", "test_value");

    let search_items = ss.search_items(search_properties).unwrap();

    //println!("Searched Item: {:?}", search_items);

    // retrieve one item, first by checking the unlocked items
    let item = match search_items.unlocked.first() {
        Some(item) => item,
        None => {
            // if there aren't any, check the locked items and unlock the first one
            let locked_item = search_items
                .locked
                .first()
                .expect("Search didn't return any items!");
            locked_item.unlock().unwrap();
            locked_item
        }
    };

    // retrieve secret from item
    let secret = item.get_secret().unwrap();
    println!("Retrieved secret: {:?}", str::from_utf8(&secret).unwrap());
    assert_eq!(secret, b"test_secret");
    item.delete().unwrap();
}
