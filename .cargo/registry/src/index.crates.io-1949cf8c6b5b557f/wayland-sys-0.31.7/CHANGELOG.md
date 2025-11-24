# CHANGELOG: wayland-sys

## Unreleased

## 0.31.1 -- 2023-07-13

#### Bugfixes

- When using dlopen, try the soname of the library first.

## 0.31.0 -- 2023-06-05

#### Breaking changes

- `WAYLAND_*_OPTION` and `WAYLAND_*_HANDLE` are now functions with the same name:
  `wayland_*_option()` and `wayland_*_handle()`.

## 0.30.1

#### Bugfixes

- Fix UB in `rust_listener_create`

## 0.30.0

## 0.30.0-beta.10

#### Bugfixes

- Server-side, fix the prototype of `wl_resource_add_destroy_listener`

## 0.30.0-alpha10

#### Changes

- The `ffi_dispatch!` macro no longer requires a trailing comma when invoking functions without
  any argument.

## 0.30.0-alpha1

#### Changes

- Errors when dynamiclaly loading the system libraries are now logged to `log` rather than
  printed using `eprintln!`.
