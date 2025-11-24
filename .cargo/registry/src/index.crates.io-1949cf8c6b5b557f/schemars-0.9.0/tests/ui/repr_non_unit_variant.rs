use schemars::JsonSchema_repr;

#[derive(JsonSchema_repr)]
#[repr(u8)]
pub enum Enum {
    Unit,
    EmptyTuple(),
}

fn main() {}
