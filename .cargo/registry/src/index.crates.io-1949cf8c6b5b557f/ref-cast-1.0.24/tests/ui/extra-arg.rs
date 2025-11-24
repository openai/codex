use ref_cast::{ref_cast_custom, RefCastCustom};

#[derive(RefCastCustom)]
#[repr(transparent)]
pub struct Thing(String);

impl Thing {
    #[ref_cast_custom]
    pub fn ref_cast(s: &String, wat: i32) -> &Self;
}

fn main() {}
