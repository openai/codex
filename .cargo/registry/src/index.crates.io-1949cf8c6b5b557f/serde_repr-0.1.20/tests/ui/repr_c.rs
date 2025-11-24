use serde_repr::Serialize_repr;

#[derive(Serialize_repr)]
#[repr(C)]
enum SmallPrime {
    Two = 2,
    Three = 3,
    Five = 5,
    Seven = 7,
}

fn main() {}
