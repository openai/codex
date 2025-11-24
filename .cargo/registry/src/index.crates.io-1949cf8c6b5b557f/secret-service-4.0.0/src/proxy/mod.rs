//Copyright 2022 secret-service-rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

pub mod collection;
pub mod item;
pub mod prompt;
pub mod service;

use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedObjectPath, Type};

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct SecretStruct {
    pub(crate) session: OwnedObjectPath,
    pub(crate) parameters: Vec<u8>,
    pub(crate) value: Vec<u8>,
    pub(crate) content_type: String,
}
