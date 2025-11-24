use crate::sys::windows::Event;
use crate::sys::windows::Selector;
use crate::Token;

use super::iocp::CompletionPort;
use std::io;
use std::sync::Arc;

#[derive(Debug)]
pub struct Waker {
    token: Token,
    port: Arc<CompletionPort>,
}

impl Waker {
    pub fn new(selector: &Selector, token: Token) -> io::Result<Waker> {
        Ok(Waker {
            token,
            port: selector.clone_port(),
        })
    }

    pub fn wake(&self) -> io::Result<()> {
        let mut ev = Event::new(self.token);
        ev.set_readable();

        self.port.post(ev.to_completion_status())
    }
}
