//! Thread-local data specific to LR(1) processing.

use crate::grammar::repr::TerminalSet;
use std::cell::RefCell;
use std::mem;
use std::sync::Arc;

thread_local! {
    static TERMINALS: RefCell<Option<Arc<TerminalSet>>> = RefCell::new(None)
}

pub struct Lr1Tls {
    old_value: Option<Arc<TerminalSet>>,
}

impl Lr1Tls {
    pub fn install(terminals: TerminalSet) -> Lr1Tls {
        let old_value = TERMINALS.with(|s| {
            let mut s = s.borrow_mut();
            mem::replace(&mut *s, Some(Arc::new(terminals)))
        });

        Lr1Tls { old_value }
    }

    pub fn with<OP, RET>(op: OP) -> RET
    where
        OP: FnOnce(&TerminalSet) -> RET,
    {
        TERMINALS.with(|s| op(s.borrow().as_ref().expect("LR1 TLS not installed")))
    }
}

impl Drop for Lr1Tls {
    fn drop(&mut self) {
        TERMINALS.with(|s| *s.borrow_mut() = self.old_value.take());
    }
}
