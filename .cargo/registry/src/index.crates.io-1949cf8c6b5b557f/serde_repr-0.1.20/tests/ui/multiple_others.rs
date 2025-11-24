use serde_repr::Deserialize_repr;

#[derive(Deserialize_repr)]
#[repr(u8)]
enum MultipleOthers {
    #[serde(other)]
    A,
    #[serde(other)]
    B,
}

fn main() {}
