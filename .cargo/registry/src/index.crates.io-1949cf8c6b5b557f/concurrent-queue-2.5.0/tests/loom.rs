#![cfg(loom)]

use concurrent_queue::{ConcurrentQueue, ForcePushError, PopError, PushError};
use loom::sync::atomic::{AtomicUsize, Ordering};
use loom::sync::{Arc, Condvar, Mutex};
use loom::thread;

#[cfg(target_family = "wasm")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// A basic MPMC channel based on a ConcurrentQueue and loom primitives.
struct Channel<T> {
    /// The queue used to contain items.
    queue: ConcurrentQueue<T>,

    /// The number of senders.
    senders: AtomicUsize,

    /// The number of receivers.
    receivers: AtomicUsize,

    /// The event that is signaled when a new item is pushed.
    push_event: Event,

    /// The event that is signaled when a new item is popped.
    pop_event: Event,
}

/// The sending side of a channel.
struct Sender<T> {
    /// The channel.
    channel: Arc<Channel<T>>,
}

/// The receiving side of a channel.
struct Receiver<T> {
    /// The channel.
    channel: Arc<Channel<T>>,
}

/// Create a new pair of senders/receivers based on a queue.
fn pair<T>(queue: ConcurrentQueue<T>) -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(Channel {
        queue,
        senders: AtomicUsize::new(1),
        receivers: AtomicUsize::new(1),
        push_event: Event::new(),
        pop_event: Event::new(),
    });

    (
        Sender {
            channel: channel.clone(),
        },
        Receiver { channel },
    )
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.channel.senders.fetch_add(1, Ordering::SeqCst);
        Sender {
            channel: self.channel.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if self.channel.senders.fetch_sub(1, Ordering::SeqCst) == 1 {
            // Close the channel and notify the receivers.
            self.channel.queue.close();
            self.channel.push_event.signal_all();
        }
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.channel.receivers.fetch_add(1, Ordering::SeqCst);
        Receiver {
            channel: self.channel.clone(),
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        if self.channel.receivers.fetch_sub(1, Ordering::SeqCst) == 1 {
            // Close the channel and notify the senders.
            self.channel.queue.close();
            self.channel.pop_event.signal_all();
        }
    }
}

impl<T> Sender<T> {
    /// Send a value.
    ///
    /// Returns an error with the value if the channel is closed.
    fn send(&self, mut value: T) -> Result<(), T> {
        loop {
            match self.channel.queue.push(value) {
                Ok(()) => {
                    // Notify a single receiver.
                    self.channel.push_event.signal();
                    return Ok(());
                }
                Err(PushError::Closed(val)) => return Err(val),
                Err(PushError::Full(val)) => {
                    // Wait for a receiver to pop an item.
                    value = val;
                    self.channel.pop_event.wait();
                }
            }
        }
    }

    /// Send a value forcefully.
    fn force_send(&self, value: T) -> Result<Option<T>, T> {
        match self.channel.queue.force_push(value) {
            Ok(bumped) => {
                self.channel.push_event.signal();
                Ok(bumped)
            }

            Err(ForcePushError(val)) => Err(val),
        }
    }
}

impl<T> Receiver<T> {
    /// Channel capacity.
    fn capacity(&self) -> Option<usize> {
        self.channel.queue.capacity()
    }

    /// Receive a value.
    ///
    /// Returns an error if the channel is closed.
    fn recv(&self) -> Result<T, ()> {
        loop {
            match self.channel.queue.pop() {
                Ok(value) => {
                    // Notify a single sender.
                    self.channel.pop_event.signal();
                    return Ok(value);
                }
                Err(PopError::Closed) => return Err(()),
                Err(PopError::Empty) => {
                    // Wait for a sender to push an item.
                    self.channel.push_event.wait();
                }
            }
        }
    }
}

/// An event that can be waited on and then signaled.
struct Event {
    /// The condition variable used to wait on the event.
    condvar: Condvar,

    /// The mutex used to protect the event.
    ///
    /// Inside is the event's state. The first bit is used to indicate if the
    /// notify_one method was called. The second bit is used to indicate if the
    /// notify_all method was called.
    mutex: Mutex<usize>,
}

impl Event {
    /// Create a new event.
    fn new() -> Self {
        Self {
            condvar: Condvar::new(),
            mutex: Mutex::new(0),
        }
    }

    /// Wait for the event to be signaled.
    fn wait(&self) {
        let mut state = self.mutex.lock().unwrap();

        loop {
            if *state & 0b11 != 0 {
                // The event was signaled.
                *state &= !0b01;
                return;
            }

            // Wait for the event to be signaled.
            state = self.condvar.wait(state).unwrap();
        }
    }

    /// Signal the event.
    fn signal(&self) {
        let mut state = self.mutex.lock().unwrap();
        *state |= 1;
        drop(state);

        self.condvar.notify_one();
    }

    /// Signal the event, but notify all waiters.
    fn signal_all(&self) {
        let mut state = self.mutex.lock().unwrap();
        *state |= 3;
        drop(state);

        self.condvar.notify_all();
    }
}

/// Wrapper to run tests on all three queues.
fn run_test<F: Fn(ConcurrentQueue<usize>, usize) + Send + Sync + Clone + 'static>(f: F) {
    // The length of a loom test seems to increase exponentially the higher this number is.
    const LIMIT: usize = 4;

    let fc = f.clone();
    loom::model(move || {
        fc(ConcurrentQueue::bounded(1), LIMIT);
    });

    let fc = f.clone();
    loom::model(move || {
        fc(ConcurrentQueue::bounded(LIMIT / 2), LIMIT);
    });

    loom::model(move || {
        f(ConcurrentQueue::unbounded(), LIMIT);
    });
}

#[test]
fn spsc() {
    run_test(|q, limit| {
        // Create a new pair of senders/receivers.
        let (tx, rx) = pair(q);

        // Push each onto a thread and run them.
        let handle = thread::spawn(move || {
            for i in 0..limit {
                if tx.send(i).is_err() {
                    break;
                }
            }
        });

        let mut recv_values = vec![];

        loop {
            match rx.recv() {
                Ok(value) => recv_values.push(value),
                Err(()) => break,
            }
        }

        // Values may not be in order.
        recv_values.sort_unstable();
        assert_eq!(recv_values, (0..limit).collect::<Vec<_>>());

        // Join the handle before we exit.
        handle.join().unwrap();
    });
}

#[test]
fn spsc_force() {
    run_test(|q, limit| {
        // Create a new pair of senders/receivers.
        let (tx, rx) = pair(q);

        // Push each onto a thread and run them.
        let handle = thread::spawn(move || {
            for i in 0..limit {
                if tx.force_send(i).is_err() {
                    break;
                }
            }
        });

        let mut recv_values = vec![];

        loop {
            match rx.recv() {
                Ok(value) => recv_values.push(value),
                Err(()) => break,
            }
        }

        // Values may not be in order.
        recv_values.sort_unstable();
        let cap = rx.capacity().unwrap_or(usize::MAX);
        for (left, right) in (0..limit)
            .rev()
            .take(cap)
            .zip(recv_values.into_iter().rev())
        {
            assert_eq!(left, right);
        }

        // Join the handle before we exit.
        handle.join().unwrap();
    });
}
