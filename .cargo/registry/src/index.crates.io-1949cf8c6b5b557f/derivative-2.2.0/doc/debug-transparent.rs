# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(Debug="transparent")]
pub struct Wrapping<T>(pub T);