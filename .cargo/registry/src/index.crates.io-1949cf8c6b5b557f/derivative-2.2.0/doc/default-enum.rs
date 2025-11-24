# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(Default(bound=""))]
pub enum Option<T> {
    #[derivative(Default)]
    /// No value
    None,
    /// Some value `T`
    Some(T),
}