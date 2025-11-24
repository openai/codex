use crate::sys::Selector;
use crate::Token;
use std::io;

#[derive(Debug)]
pub struct Waker {}

impl Waker {
    pub fn new(_: &Selector, _: Token) -> io::Result<Waker> {
        os_required!();
    }

    pub fn wake(&self) -> io::Result<()> {
        os_required!();
    }
}
