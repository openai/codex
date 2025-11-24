# CHANGELOG: wayland-client

## Unreleased

## 0.31.11 -- 2025-07-28

- Updated Wayland core protocol to 1.24

## 0.31.7 -- 2024-10-23

- Updated Wayland core protocol to 1.23

## 0.31.2 -- 2024-01-29

#### Additions

- Implement `Eq` for `Connection`

#### Bugfixes

- Fix a possible deadlock in `EventQueue::blocking_dispatch()`

## 0.31.1 -- 2023-09-19

#### Additions

- Implement `AsFd` for `Connection` and `EventQueue` so they can easily be used in a
  `calloop` source.

## 0.31.0 -- 2023-09-02

#### Breaking changes

- Bump bitflags to 2.0
- Updated wayland-backend to 0.3
- Calloop integration is now removed, to avoid tying wayland-client to it you can use the
  `calloop-wayland-source` crate instead
- Use `BorrowedFd<'_>` arguments instead of `RawFd`

## 0.30.2 -- 30/05/2023

- Updated Wayland core protocol to 1.22

## 0.30.1 -- 04/02/2023

#### Bugfixes

- Fix compilation without the `log` feature.

## 0.30.0 -- 27/12/2022

## 0.30.0-beta.14

#### Additions

- Introduce `WaylandSource`, an adapter to insert an `EventQueue` into a
  calloop `EventLoop`, hidden under the new `calloop` cargo feature

## 0.30.0-beta.11

#### Bugfixes

- `Weak::upgrade` now checks if the object has been destroyed

## 0.30.0-beta.10

#### Additions

- Support absolute paths in `WAYLAND_DISPLAY`
- Introduce `Weak`, a helper type to store proxies without risking reference cycles
- Introduce `Proxy::is_alive()` method checking if the protocol object referenced by a proxy is still
  alive in the protocol state.

#### Bugfixes

- Fix `EventQueue::blocking_dispatch()` not flushing the connection as it should
- Ensure that `XDG_RUNTIME_DIR` is an absolute path before trying to use it

## 0.30.0-beta.9

#### Breaking changes

- Requests that create new objects now produce inert proxies when called on
  objects with invalid IDs instead of failing with `InvalidId`.  This matches
  the behavior of non-object-creating requests (which also ignore the error).

- `Connection::blocking_dispatch` has been removed; use `EventQueue::blocking_dispatch`.

#### Additions

- `QueueFreezeGuard` for avoiding race conditions while constructing objects.

## 0.30.0-beta.8

#### Breaking changes

- `Connection::null_id()` has been removed, instead use `ObjectId::null()`.
- `EventQueue::sync_roundtrip()` has been renamed to `EventQueue::roundtrip()`.
- Module `globals` has been removed as the abstractions it provide are not deemed useful.
- The trait `DelegateDispatch` as been removed, its functionnality being fused into a more generic
  version of the `Dispatch` trait.

#### Additions

- Introduce the `log` cargo feature to control logging behavior

## 0.30.0-beta.6

- Introduce `EventQueue::poll_dispatch_pending` for running dispatch using an async runtime.

## 0.30.0-beta.1

#### Breaking changes

- Large rework of the API as a consequence of the rework of the backend.

## 0.30.0-alpha10

- Introduce conversion methods between `wayland_backend::Handle` and `ConnectionHandle`

## 0.30.0-alpha2

#### Breaking changes

- The `DelegateDispatch` mechanism is changed around an explicit trait-base extraction of module
  state from the main app state.

## 0.30.0-alpha1

Full rework of the crate, which is now organized around a trait-based `Dispatch` metchanism.

This can effectively be considered a new crate altogether.
