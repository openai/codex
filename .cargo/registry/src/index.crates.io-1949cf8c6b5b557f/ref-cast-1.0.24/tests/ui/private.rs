use ref_cast::{ref_cast_custom, RefCast, RefCastCustom};

#[derive(RefCast, RefCastCustom)]
#[repr(transparent)]
pub struct Public {
    private: Private,
}

struct Private;

impl Public {
    #[ref_cast_custom]
    fn ref_cast(private: &Private) -> &Public;

    #[ref_cast_custom]
    fn ref_cast_mut(private: &mut Private) -> &mut Public;
}

fn main() {}
