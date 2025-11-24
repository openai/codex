# CHANGELOG: wayland-backend

## Unreleased

## 0.3.10 -- 2025-04-29

#### Bugfixes

- Freeze when dispatching within the queue dispatch on the same thread with system lib

## 0.3.9 -- 2025-04-28

#### Bugfixes

- backend/sys: Prevent send_request being called in parallel with dispatch_pending

#### Additions

- backend/client_sys: `ReadEventsGuard::read_without_dispatch` for just reading the events without dispatch

## 0.3.8 -- 2025-01-31

### Bugfixes

- backend/rs: Prevent a potential deadlock during client cleanup

## 0.3.7 -- 2024-09-04

### Bugfixes

- backend/sys: Fix importing external objects with `Backend::manage_object` by
  associating the proxy with the correct event queue

## 0.3.6 -- 2024-07-16

### Bugfixes

- backend/rs: server: Fixed potential deadlock on object destruction

## 0.3.5 -- 2024-07-03

#### Additions

- `Backend::manage_object` for handling foreign proxies with the sys backend

#### Bugfixes

- backend/rs: server: Fixed potential deadlock caused by dead clients
- backend/sys: client dispatching now always uses distinct event queue

## 0.3.4 -- 2024-05-30

#### Additions

- Add `rwh_06` feature for `raw-window-handle` 0.6

#### Bugfixes
- backend/rs: `WAYLAND_DEBUG` now displays `fixed` as decimal, not integer.
  * Matches libwayland

## 0.3.3 -- 2024-01-29

#### Additions
- client: Implement `Eq` for `Backend`

#### Bugfixes
- backend/sys: Fix error/segfault if object argument is no longer alive
- backend/rs: Retry send/recv on `EINTR`
  * Matches the behavior of libwayland

## 0.3.2 -- 2023-09-25

#### Bugfixes

- server/sys: Fix an issue where the backend could deadlock in some cases when sending an event that destroys
  an object.

## 0.3.1 -- 2023-09-19

#### Additions

- client: Add `Backend::poll_fd` to return fd for polling

## 0.3.0 -- 2023-09-02

#### Breaking change

- MSRV bumped to 1.65
- `io-lifetimes` is no longer a (public) dependency
- The `Backend::prepare_read()` method now returns `None` if the inner queue of the backend
  needs to be dispatched using `Backend::dispatch_inner_queue()`, instead of trying to dispatch
  it by itself. This can only happen when using the `sys` backend, and allows the crate to
  behave properly when multiple threads try to read the socket using the libwayland API.
- server: `ObjectData::destroyed` function signature has changed to pass the `Handle` and `self` as `Arc<Self>`.

#### Additions

- Add `flush` method to server `Handle`.

#### Bugfixes

- Setting `WAYLAND_DEBUG` server-side now properly prints incoming requests

## 0.2.0 -- 2023-07-13

#### Breaking changes

- Update wayland-sys to 0.31

## 0.1.2 -- 19/04/2023

#### Bugfixes

- In the rust server backend, don't send `delete_id` messages for server-created objects.
- In the system server backend, wakeup the event loop if there are pending destructors waiting
  to be precessed.

## 0.1.1 -- 16/02/2023

#### Bugfixes

- In sys backend, fix global data not being cleaned up on display drop.

## 0.1.0 -- 27/12/2022

## 0.1.0-beta.14

#### Bugfixes

- In rust backend, retry read if message is incomplete, instead of `Malformed` error.

## 0.1.0-beta.10

#### Breaking changes

- `Message` is now also generic on the `Fd` type, and `io_lifetimes::OwnedFd` is used instead of `RawFd` when
  appropriate.

#### Bugfixes

- The rust backend no longer ever does a blocking flush.
- Server-side sys backend is now able to track liveness of external objects.

## 0.1.0-beta.8

#### Breaking changes

- all backends: creating null `ObjectId` is now done through the `ObjectId::null()` method, and the
  `null_id()` methods on the backends are removed.
- `Argument::Str` now contains an `Option`, which is correctly mapped to nullable strings. This fixes
  segfaults that previously occurred dereferencing the null pointer in the system backend.
- server: `disable_global` and `remove_global` now require the state type parameter, like `create_global`.
  This is required for compatibility with libwayland 1.21.
- `ArgumentType::Array` and `ArgumentType::NewId` no longer take a `AllowNull` and are never nullable.

#### Additions

- client: introduce `Backend::dispatch_inner_queue()` meant for ensuring a system backend in guest mode can
  still process events event it does not control reading the socket.
- introduce the `log` cargo feature to control logging behavior
- A dummy implementation of ClientData is now provided through `()` and all trait methods are optional

## 0.1.0-beta.7

#### Bugfixes

- backend/sys: the inner lock is no longer held when destructors are invoked
- backend/sys: sys backend does not abort process when `Backend::disable_global` is invoked more than once

## 0.1.0-beta.6

#### Additions

- client: `ObjectId` now implements the `Hash` trait
- server: `ObjectId`, `ClientId` and `GlobalId` now implement the `Hash` trait

#### Bugfixes

- backend/sys: the inner lock is no longer held when destructors are invoked

## 0.1.0-beta.5

#### Additions

- client/sys: introduce `Backend::from_foreign_display`

#### Bugfixes

- The server backend now correctly associates interfaces with its object arguments when parsing
  messages with nullable object arguments.

## 0.1.0-beta.4

#### Bugfixes

- server-rs: the inner lock is no longer help when destructors are invoked

## 0.1.0-beta.3

#### Bugfixes

- server-sys: Skip unmanaged globals in the global filter (caused a segfault)

## 0.1.0-beta.2

#### Breaking changes

- server-sys: move `display_ptr()` from `Backend` to `Handle`.

## 0.1.0-beta.1

#### Breaking changes

- Both client and server APIs have been profoundly reworked. The backend now has internal locking
  mechanism allowing handles to it to be cloned and shared accross the application.

#### Bugfixes

- Fix a crash when exactly filling the internal buffers.

## 0.1.0-alpha7

#### Bugfixes

- Client-side with the rust backend, `wl_display` events are now properly printed with other events
  when `WAYLAND_DEBUG=1` is set.

## 0.1.0-alpha6

#### Changes

- Server-side, request callbacks are now allowed to omit providing an `ObjectData` for newly
  created objects if they triggered a protocol error

#### Bugfixes

- Fix display leaks on system server backend.
- Fix a panic when trying to send a message with a null object argument followed by a
  non-null object argument
- Fix various memory leaks

## 0.1.0-alpha5

- Expose `wl_display` pointer on system server backend

## 0.1.0-alpha4

#### Breaking changes

- Server-side `ObjectData::destroyed()` now has access to the `&mut D` server-wide data.

## 0.1.0-alpha3

#### Additions

- Introduce `WEnum::into_result` as a convenience method.

## 0.1.0-alpha2

## 0.1.0-alpha1

Initial pre-release of the crate.
