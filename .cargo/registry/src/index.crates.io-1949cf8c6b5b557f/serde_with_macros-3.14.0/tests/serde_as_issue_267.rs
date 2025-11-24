#![allow(missing_docs)]

use serde::Serialize;
use serde_with_macros::serde_as;

// The field has no serde_as annotation and should not trigger any error
#[serde_as(crate = "serde_with_macros")]
#[derive(Serialize)]
pub struct Thing {
    pub id: u8,
}
