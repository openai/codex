extern crate ctor;
extern crate libc_print;

use ctor::*;
use libc_print::*;
use std::collections::HashMap;

#[ctor]
/// This is an immutable static, evaluated at init time
static STATIC_CTOR: HashMap<u32, &'static str> = {
    let mut m = HashMap::new();
    m.insert(0, "foo");
    m.insert(1, "bar");
    m.insert(2, "baz");
    libc_eprintln!("STATIC_CTOR");
    m
};

#[ctor]
fn ctor() {
    libc_eprintln!("ctor");
}

#[ctor]
unsafe fn ctor_unsafe() {
    libc_eprintln!("ctor_unsafe");
}

#[dtor]
fn dtor() {
    libc_eprintln!("dtor");
}

#[dtor]
unsafe fn dtor_unsafe() {
    libc_eprintln!("dtor_unsafe");
}

mod module {
    use ctor::*;
    use libc_print::*;

    #[ctor]
    pub static STATIC_CTOR: u8 = {
        libc_eprintln!("module::STATIC_CTOR");
        42
    };
}

pub fn main() {
    libc_eprintln!("main!");
    libc_eprintln!("STATIC_CTOR = {:?}", *STATIC_CTOR);
    libc_eprintln!("module::STATIC_CTOR = {:?}", *module::STATIC_CTOR);
}
