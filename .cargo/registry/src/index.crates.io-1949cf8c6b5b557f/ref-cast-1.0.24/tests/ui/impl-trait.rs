use ref_cast::{ref_cast_custom, RefCastCustom};

#[derive(RefCastCustom)]
#[repr(transparent)]
pub struct Thing(str);

impl Thing {
    #[ref_cast_custom]
    pub fn ref_cast(s: impl AsRef<str>) -> &Self;

    #[ref_cast_custom]
    pub fn ref_cast2(s: &impl AsRef<str>) -> &Self;
}

fn main() {}
