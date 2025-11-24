use ref_cast::ref_cast_custom;

#[repr(transparent)]
pub struct Thing(String);

impl Thing {
    #[ref_cast_custom]
    pub fn ref_cast(s: &String) -> &Self;
}

fn main() {}
