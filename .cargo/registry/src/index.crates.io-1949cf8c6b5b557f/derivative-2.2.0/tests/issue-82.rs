#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Copy, Clone, Derivative)]
#[derivative(Debug)]
#[repr(C, packed)]
struct Test {
    a: u8,
    b: u32,
}