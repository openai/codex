use super::{Shared, Synced};

use crate::runtime::scheduler::Lock;
use crate::runtime::task;

use std::sync::atomic::Ordering::Release;

impl<'a> Lock<Synced> for &'a mut Synced {
    type Handle = &'a mut Synced;

    fn lock(self) -> Self::Handle {
        self
    }
}

impl AsMut<Synced> for Synced {
    fn as_mut(&mut self) -> &mut Synced {
        self
    }
}

impl<T: 'static> Shared<T> {
    /// Pushes several values into the queue.
    ///
    /// # Safety
    ///
    /// Must be called with the same `Synced` instance returned by `Inject::new`
    #[inline]
    pub(crate) unsafe fn push_batch<L, I>(&self, shared: L, mut iter: I)
    where
        L: Lock<Synced>,
        I: Iterator<Item = task::Notified<T>>,
    {
        let first = match iter.next() {
            Some(first) => first.into_raw(),
            None => return,
        };

        // Link up all the tasks.
        let mut prev = first;
        let mut counter = 1;

        // We are going to be called with an `std::iter::Chain`, and that
        // iterator overrides `for_each` to something that is easier for the
        // compiler to optimize than a loop.
        iter.for_each(|next| {
            let next = next.into_raw();

            // safety: Holding the Notified for a task guarantees exclusive
            // access to the `queue_next` field.
            unsafe { prev.set_queue_next(Some(next)) };
            prev = next;
            counter += 1;
        });

        // Now that the tasks are linked together, insert them into the
        // linked list.
        self.push_batch_inner(shared, first, prev, counter);
    }

    /// Inserts several tasks that have been linked together into the queue.
    ///
    /// The provided head and tail may be the same task. In this case, a
    /// single task is inserted.
    #[inline]
    unsafe fn push_batch_inner<L>(
        &self,
        shared: L,
        batch_head: task::RawTask,
        batch_tail: task::RawTask,
        num: usize,
    ) where
        L: Lock<Synced>,
    {
        debug_assert!(unsafe { batch_tail.get_queue_next().is_none() });

        let mut synced = shared.lock();

        if synced.as_mut().is_closed {
            drop(synced);

            let mut curr = Some(batch_head);

            while let Some(task) = curr {
                curr = task.get_queue_next();

                let _ = unsafe { task::Notified::<T>::from_raw(task) };
            }

            return;
        }

        let synced = synced.as_mut();

        if let Some(tail) = synced.tail {
            unsafe {
                tail.set_queue_next(Some(batch_head));
            }
        } else {
            synced.head = Some(batch_head);
        }

        synced.tail = Some(batch_tail);

        // Increment the count.
        //
        // safety: All updates to the len atomic are guarded by the mutex. As
        // such, a non-atomic load followed by a store is safe.
        let len = self.len.unsync_load();

        self.len.store(len + num, Release);
    }
}
