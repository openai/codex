use ref_cast::RefCast;
use std::marker::PhantomData;

#[derive(RefCast)]
#[repr(transparent)]
struct Bytes<'arena> {
    bytes: [u8],
    #[trivial]
    marker: PhantomData<&'arena ()>,
}

fn main() {}
