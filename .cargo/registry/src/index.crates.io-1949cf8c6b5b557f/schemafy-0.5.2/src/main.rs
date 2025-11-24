#[cfg(feature = "internal-regenerate")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "internal-regenerate")]
schemafy::regenerate!(
    root: Schema
    "schemafy_lib/src/schema.json"
);

fn main() {}
