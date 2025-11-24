//Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

// Definitions for secret service interactions

// DBus Name
pub const SS_DBUS_NAME: &str = "org.freedesktop.secrets";

// Item Properties
pub const SS_ITEM_LABEL: &str = "org.freedesktop.Secret.Item.Label";
pub const SS_ITEM_ATTRIBUTES: &str = "org.freedesktop.Secret.Item.Attributes";

// Algorithm Names
pub const ALGORITHM_PLAIN: &str = "plain";
pub const ALGORITHM_DH: &str = "dh-ietf1024-sha256-aes128-cbc-pkcs7";

// Collection properties
pub const SS_COLLECTION_LABEL: &str = "org.freedesktop.Secret.Collection.Label";
