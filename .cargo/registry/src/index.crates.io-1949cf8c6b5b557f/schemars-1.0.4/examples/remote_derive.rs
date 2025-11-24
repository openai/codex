// Pretend that this is somebody else's crate, not a module.
mod other_crate {
    // Neither Schemars nor the other crate provides a JsonSchema impl
    // for this struct.
    pub struct Duration {
        pub secs: i64,
        pub nanos: i32,
    }
}

////////////////////////////////////////////////////////////////////////////////

use other_crate::Duration;
use schemars::{schema_for, JsonSchema};

// This is just a copy of the remote data structure that Schemars can use to
// create a suitable JsonSchema impl.
#[derive(JsonSchema)]
#[serde(remote = "Duration")]
pub struct DurationDef {
    pub secs: i64,
    pub nanos: i32,
}

// Now the remote type can be used almost like it had its own JsonSchema impl
// all along. The `with` attribute gives the path to the definition for the
// remote type. Note that the real type of the field is the remote type, not
// the definition type.
#[derive(JsonSchema)]
pub struct Process {
    pub command_line: String,
    #[serde(with = "DurationDef")]
    pub wall_time: Duration,
    // Generic types must be explicitly specified with turbofix `::<>` syntax.
    #[serde(with = "Vec::<DurationDef>")]
    pub durations: Vec<Duration>,
}

fn main() {
    let schema = schema_for!(Process);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
