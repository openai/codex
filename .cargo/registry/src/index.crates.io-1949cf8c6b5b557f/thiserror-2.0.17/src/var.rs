use core::fmt::{self, Pointer};

pub struct Var<'a, T: ?Sized>(pub &'a T);

impl<'a, T: Pointer + ?Sized> Pointer for Var<'a, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Pointer::fmt(self.0, formatter)
    }
}
