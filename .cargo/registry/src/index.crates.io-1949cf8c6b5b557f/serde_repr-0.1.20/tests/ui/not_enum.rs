use serde_repr::Serialize_repr;

#[derive(Serialize_repr)]
struct SmallPrime {
    two: u8,
    three: u8,
    five: u8,
    seven: u8,
}

fn main() {}
