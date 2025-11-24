# Refactor I/O driver

Describes changes to the I/O driver for the Tokio 0.3 release.

## Goals

* Support `async fn` on I/O types with `&self`.
* Refine the `Registration` API.

### Non-goals

* Implement `AsyncRead` / `AsyncWrite` for `&TcpStream` or other reference type.

## Overview

Currently, I/O types require `&mut self` for `async` functions. The reason for
this is the task's waker is stored in the I/O resource's internal state
(`ScheduledIo`) instead of in the future returned by the `async` function.
Because of this limitation, I/O types limit the number of wakers to one per
direction (a direction is either read-related events or write-related events).

Moving the waker from the internal I/O resource's state to the operation's
future enables multiple wakers to be registered per operation. The "intrusive
wake list" strategy used by `Notify` applies to this case, though there are some
concerns unique to the I/O driver.

## Reworking the `Registration` type

While `Registration` is made private (per #2728), it remains in Tokio as an
implementation detail backing I/O resources such as `TcpStream`. The API of
`Registration` is updated to support waiting for an arbitrary interest set with
`&self`. This supports concurrent waiters with a different readiness interest.

```rust
struct Registration { ... }

// TODO: naming
struct ReadyEvent {
    tick: u32,
    ready: mio::Ready,
}

impl Registration {
    /// `interest` must be a super set of **all** interest sets specified in
    /// the other methods. This is the interest set passed to `mio`.
    pub fn new<T>(io: &T, interest: mio::Ready) -> io::Result<Registration>
        where T: mio::Evented;

    /// Awaits for any readiness event included in `interest`. Returns a
    /// `ReadyEvent` representing the received readiness event.
    async fn readiness(&self, interest: mio::Ready) -> io::Result<ReadyEvent>;

    /// Clears resource level readiness represented by the specified `ReadyEvent`
    async fn clear_readiness(&self, ready_event: ReadyEvent);
```

A new registration is created for a `T: mio::Evented` and a `interest`. This
creates a `ScheduledIo` entry with the I/O driver and registers the resource
with `mio`.

Because Tokio uses **edge-triggered** notifications, the I/O driver only
receives readiness from the OS once the ready state **changes**. The I/O driver
must track each resource's known readiness state. This helps prevent syscalls
when the process knows the syscall should return with `EWOULDBLOCK`.

A call to `readiness()` checks if the currently known resource readiness
overlaps with `interest`. If it does, then the `readiness()` immediately
returns. If it does not, then the task waits until the I/O driver receives a
readiness event.

The pseudocode to perform a TCP read is as follows.

```rust
async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        // Await readiness
        let event = self.readiness(interest).await?;

        match self.mio_socket.read(buf) {
            Ok(v) => return Ok(v),
            Err(ref e) if e.kind() == WouldBlock => {
                self.clear_readiness(event);
            }
            Err(e) => return Err(e),
        }
    }
}
```

## Reworking the `ScheduledIo` type

The `ScheduledIo` type is switched to use an intrusive waker linked list. Each
entry in the linked list includes the `interest` set passed to `readiness()`.

```rust
#[derive(Debug)]
pub(crate) struct ScheduledIo {
    /// Resource's known state packed with other state that must be
    /// atomically updated.
    readiness: AtomicUsize,

    /// Tracks tasks waiting on the resource
    waiters: Mutex<Waiters>,
}

#[derive(Debug)]
struct Waiters {
    // List of intrusive waiters.
    list: LinkedList<Waiter>,

    /// Waiter used by `AsyncRead` implementations.
    reader: Option<Waker>,

    /// Waiter used by `AsyncWrite` implementations.
    writer: Option<Waker>,
}

// This struct is contained by the **future** returned by `readiness()`.
#[derive(Debug)]
struct Waiter {
    /// Intrusive linked-list pointers
    pointers: linked_list::Pointers<Waiter>,

    /// Waker for task waiting on I/O resource
    waiter: Option<Waker>,

    /// Readiness events being waited on. This is
    /// the value passed to `readiness()`
    interest: mio::Ready,

    /// Should not be `Unpin`.
    _p: PhantomPinned,
}
```

When an I/O event is received from `mio`, the associated resources' readiness is
updated and the waiter list is iterated. All waiters with `interest` that
overlap the received readiness event are notified. Any waiter with an `interest`
that does not overlap the readiness event remains in the list.

## Cancel interest on drop

The future returned by `readiness()` uses an intrusive linked list to store the
waker with `ScheduledIo`. Because `readiness()` can be called concurrently, many
wakers may be stored simultaneously in the list. If the `readiness()` future is
dropped early, it is essential that the waker is removed from the list. This
prevents leaking memory.

## Race condition

Consider how many tasks may concurrently attempt I/O operations. This, combined
with how Tokio uses edge-triggered events, can result in a race condition. Let's
revisit the TCP read function:

```rust
async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        // Await readiness
        let event = self.readiness(interest).await?;

        match self.mio_socket.read(buf) {
            Ok(v) => return Ok(v),
            Err(ref e) if e.kind() == WouldBlock => {
                self.clear_readiness(event);
            }
            Err(e) => return Err(e),
        }
    }
}
```

If care is not taken, if between `mio_socket.read(buf)` returning and
`clear_readiness(event)` is called, a readiness event arrives, the `read()`
function could deadlock. This happens because the readiness event is received,
`clear_readiness()` unsets the readiness event, and on the next iteration,
`readiness().await` will block forever as a new readiness event is not received.

The current I/O driver handles this condition by always registering the task's
waker before performing the operation. This is not ideal as it will result in
unnecessary task notification.

Instead, we will use a strategy to prevent clearing readiness if an "unseen"
readiness event has been received. The I/O driver will maintain a "tick" value.
Every time the `mio` `poll()` function is called, the tick is incremented. Each
readiness event has an associated tick. When the I/O driver sets the resource's
readiness, the driver's tick is packed into the atomic `usize`.

The `ScheduledIo` readiness `AtomicUsize` is structured as:

```
| shutdown | generation |  driver tick | readiness |
|----------+------------+--------------+-----------|
|   1 bit  |   7 bits   +    8 bits    +  16 bits  |
```

The `shutdown` and `generation` components exist today.

The `readiness()` function returns a `ReadyEvent` value. This value includes the
`tick` component read with the resource's readiness value. When
`clear_readiness()` is called, the `ReadyEvent` is provided. Readiness is only
cleared if the current `tick` matches the `tick` included in the `ReadyEvent`.
If the tick values do not match, the call to `readiness()` on the next iteration
will not block and the new `tick` is included in the new `ReadyToken.`

TODO

## Implementing `AsyncRead` / `AsyncWrite`

The `AsyncRead` and `AsyncWrite` traits use a "poll" based API. This means that
it is not possible to use an intrusive linked list to track the waker.
Additionally, there is no future associated with the operation which means it is
not possible to cancel interest in the readiness events.

To implement `AsyncRead` and `AsyncWrite`, `ScheduledIo` includes dedicated
waker values for the read direction and the write direction. These values are
used to store the waker. Specific `interest` is not tracked for `AsyncRead` and
`AsyncWrite` implementations. It is assumed that only events of interest are:

* Read ready
* Read closed
* Write ready
* Write closed

Note that "read closed" and "write closed" are only available with Mio 0.7. With
Mio 0.6, things were a bit messy.

It is only possible to implement `AsyncRead` and `AsyncWrite` for resource types
themselves and not for `&Resource`. Implementing the traits for `&Resource`
would permit concurrent operations to the resource. Because only a single waker
is stored per direction, any concurrent usage would result in deadlocks. An
alternate implementation would call for a `Vec<Waker>` but this would result in
memory leaks.

## Enabling reads and writes for `&TcpStream`

Instead of implementing `AsyncRead` and `AsyncWrite` for `&TcpStream`, a new
function is added to `TcpStream`.

```rust
impl TcpStream {
    /// Naming TBD
    fn by_ref(&self) -> TcpStreamRef<'_>;
}

struct TcpStreamRef<'a> {
    stream: &'a TcpStream,

    // `Waiter` is the node in the intrusive waiter linked-list
    read_waiter: Waiter,
    write_waiter: Waiter,
}
```

Now, `AsyncRead` and `AsyncWrite` can be implemented on `TcpStreamRef<'a>`. When
the `TcpStreamRef` is dropped, all associated waker resources are cleaned up.

### Removing all the `split()` functions

With `TcpStream::by_ref()`, `TcpStream::split()` is no longer needed. Instead,
it is possible to do something as follows.

```rust
let rd = my_stream.by_ref();
let wr = my_stream.by_ref();

select! {
    // use `rd` and `wr` in separate branches.
}
```

It is also possible to store a `TcpStream` in an `Arc`.

```rust
let arc_stream = Arc::new(my_tcp_stream);
let n = arc_stream.by_ref().read(buf).await?;
```
