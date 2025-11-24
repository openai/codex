use std::cell::RefCell;
use std::task::Waker;

pub(crate) struct Defer {
    deferred: RefCell<Vec<Waker>>,
}

impl Defer {
    pub(crate) fn new() -> Defer {
        Defer {
            deferred: RefCell::default(),
        }
    }

    pub(crate) fn defer(&self, waker: &Waker) {
        let mut deferred = self.deferred.borrow_mut();

        // If the same task adds itself a bunch of times, then only add it once.
        if let Some(last) = deferred.last() {
            if last.will_wake(waker) {
                return;
            }
        }

        deferred.push(waker.clone());
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.deferred.borrow().is_empty()
    }

    pub(crate) fn wake(&self) {
        while let Some(waker) = self.deferred.borrow_mut().pop() {
            waker.wake();
        }
    }

    #[cfg(tokio_taskdump)]
    pub(crate) fn take_deferred(&self) -> Vec<Waker> {
        let mut deferred = self.deferred.borrow_mut();
        std::mem::take(&mut *deferred)
    }
}
