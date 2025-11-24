extern crate downcast_rs;

#[test]
fn test() {
    use downcast_rs::Downcast;
    trait Trait: Downcast {}
    downcast_rs::impl_downcast!(Trait);
}
