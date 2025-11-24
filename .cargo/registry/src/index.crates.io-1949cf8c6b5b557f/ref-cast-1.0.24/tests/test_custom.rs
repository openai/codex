#[forbid(unsafe_code)]
mod forbid_unsafe {
    use ref_cast::{ref_cast_custom, RefCastCustom};

    #[derive(RefCastCustom)]
    #[repr(transparent)]
    pub struct Custom(#[allow(dead_code)] str);

    impl Custom {
        #[ref_cast_custom]
        pub fn new(s: &str) -> &Custom;
    }
}

#[test]
fn test_forbid_unsafe() {
    forbid_unsafe::Custom::new("...");
}
