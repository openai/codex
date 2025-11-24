#[macro_use]
extern crate downcast_rs;

#[test]
fn test() {
    use downcast_rs::Downcast;
    trait Trait: Downcast {}
    impl_downcast!(Trait);
}
