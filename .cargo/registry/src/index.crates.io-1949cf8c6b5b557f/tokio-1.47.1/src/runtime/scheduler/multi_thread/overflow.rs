use crate::runtime::task;

#[cfg(test)]
use std::cell::RefCell;

pub(crate) trait Overflow<T: 'static> {
    fn push(&self, task: task::Notified<T>);

    fn push_batch<I>(&self, iter: I)
    where
        I: Iterator<Item = task::Notified<T>>;
}

#[cfg(test)]
impl<T: 'static> Overflow<T> for RefCell<Vec<task::Notified<T>>> {
    fn push(&self, task: task::Notified<T>) {
        self.borrow_mut().push(task);
    }

    fn push_batch<I>(&self, iter: I)
    where
        I: Iterator<Item = task::Notified<T>>,
    {
        self.borrow_mut().extend(iter);
    }
}
