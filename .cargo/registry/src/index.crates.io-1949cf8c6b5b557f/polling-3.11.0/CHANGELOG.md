# Version 3.11.0

- Bump MSRV to 1.71. (#251)
- Update to `windows-sys` v0.61. (#251)

# Version 3.10.0

- Add `wait_deadline` function. (#226)

# Version 3.9.0

- Add a new optional `tracing` feature. When enabled, this feature adds logging
  to the implementation. By default it is disabled. (#238)
- Update to `windows-sys` v0.60. (#239)

# Version 3.8.0

- Implement `AsRawFd` and `AsFd` for `Poller` on Redox OS. (#235)
- Update `hermit-abi` to v0.5.0. (#229)
- Update `rustix` to 1.0 (#230). This also fixed a bug in `wait` which (contradicting docs) cleared the events vector instead of appending to it.
- Add support for QNX. (#201)

# Version 3.7.4

- Add support for visionOS. (#217)
- Fix typos in documentation. (#216)

# Version 3.7.3

- Update to `windows-sys` v0.59. (#214)

# Version 3.7.2

- Update `hermit-abi` to v0.4.0. (#209)

# Version 3.7.1

- Fix a typo in `Event::is_err()`. (#204)

# Version 3.7.0

- Add support for the PS Vita as a platform. (#160)

# Version 3.6.0

- Add an `is_err` method to `Event` to tell when an error has occurred. (#189)
- Deprecate the `is_connect_failed` function. (#189)
- Add support for HermitOS to `polling`. (#194)

# Version 3.5.0

- Use the `epoll` backend when RedoxOS is enabled. (#190)

# Version 3.4.0

- Add the ability to identify whether socket connection has failed. (#185)
- On BSD, add the ability to wait on a process by its PID. Previously, it was
  only possible to wait on a process by a `Child` object. (#180)
- On ESP-IDF, annotate `eventfd` initialization failures with a message
  indicating the source of those failures. (#186)

# Version 3.3.2

- When AFD fails to initialize, the resulting error now references
  the underlying system error. (#174)

# Version 3.3.1

- Bump `windows-sys` to v0.52.0. (#169)

# Version 3.3.0

- Automatically restarts polling when `ErrorKind::Interrupted` is returned, rather than relying on the user to handle it. (#164)
- Fix bad link in documentation for `Poller::wait()`. (#163)

# Version 3.2.0

- The `kqueue` backend previously allowed the following operations that other backends forbid. Now these operations result in an error: (#153)
  - Inserting a source that was already inserted.
  - Modifying/deleting a source that was not already inserted.
- Add support for Haiku OS. (#154)

# Version 3.1.0

- Add an `Event::new()` constructor to simplify creating `Event`s. (#149)

# Version 3.0.0

- Replace `libc` in all backends with the `rustix` crate (#108).
- Use `tracing` instead of `log` for logging (#119).
- **Breaking:** Rework the API to use I/O safety. Note that this makes several previously safe functions unsafe. (#123)
- Add support for the ESP-IDF platform. (#128)
- **Breaking:** Make `Event` partially opaque, and create a new `Events` struct for holding events. (#133)
- Add support for running `polling` in Linux containers without `eventfd` available. (#134)
- Specify the behavior when registered in multiple `Poller`s. (#136)
- **Breaking:** Use `c_int` from the standard library in `polling::os::kqueue` instead of defining our own. (#143)
- **Breaking:** Remove the useless `std` feature. (#147)

# Version 2.8.0

- Add functionality for posting events to the IOCP. (#101)

# Version 2.7.0

- Add edge/oneshot combination mode. (#96)
- Update windows-sys requirement from 0.45 to 0.48. (#103)

# Version 2.6.0

- Add level and edge triggered modes to the poller (#59)
- Support tvOS and watchOS (#60)
- Prevent large timeouts from causing panics on certain backends (#71)
- For certain BSDs, use `EVFILT_USER` to wake up the poller instead of a pipe (#73)
- For Solaris/illumos, use `port_send` to wake up the poller instead of a pipe (#74)
- Update `windows_sys` from 0.42 to 0.45 (#80)
- Expose other `kqueue` filter types (#83)
- Replace the Windows backend with a hand-written version, rather than bringing in a C dependency (#88)

# Version 2.5.2

- Update use of `libc::timespec` to prepare for future libc version (#55)
- Update use of `libc::kevent` to prepare for future libc version (#56)
- Add error message for Wepoll (#54)

# Version 2.5.1

- Fix the build error with MSRV on Windows

# Version 2.5.0

- Switch from `winapi` to `windows-sys` (#47)

# Version 2.4.0

- Fix the build error on illumos and Solaris (#43)
- Bump MSRV to 1.47 (#40)
- Optimize `Poller` internal representation (#40)

# Version 2.3.0

- Implement `AsRawFd` for `Poller` on most Unix systems (#39)
- Implement `AsRawHandle` for `Poller` on Windows (#39)
- Implement I/O safety traits on Rust 1.63+ (#39)

# Version 2.2.0

- Support VxWorks, Fuchsia and other Unix systems by using poll. (#26)

# Version 2.1.0

- Switch from `wepoll-sys` to `wepoll-ffi`.

# Version 2.0.3

- Update `cfg-if` dependency to 1.

# Version 2.0.2

- Replace manual pointer conversion with `as_ptr()` and `as_mut_ptr()`.

# Version 2.0.1

- Minor docs improvements.

# Version 2.0.0

- Add `Event` argument to `Poller::insert()`.
- Don't put fd/socket in non-blocking mode upon insertion.
- Rename `insert()`/`interest()`/`remove()` to `add()`/`modify()`/`delete()`.
- Replace `wepoll-sys-stjepang` with an `wepoll-sys`.

# Version 1.1.0

- Add "std" cargo feature.

# Version 1.0.3

- Remove `libc` dependency on Windows.

# Version 1.0.2

- Bump MSRV to 1.40.0
- Replace the `epoll_create1` hack with a cleaner solution.
- Pass timeout to `epoll_wait` to support systems without `timerfd`.

# Version 1.0.1

- Fix a typo in the readme.

# Version 1.0.0

- Stabilize.

# Version 0.1.9

- Fix compilation on x86_64-unknown-linux-gnux32

# Version 0.1.8

- Replace `log::debug!` with `log::trace!`.

# Version 0.1.7

- Specify oneshot mode in epoll/wepoll at insert.

# Version 0.1.6

- Add logging.

# Version 0.1.5

- Fix a bug where epoll would block when the timeout is set to zero.
- More tests.

# Version 0.1.4

- Optimize notifications.
- Fix a bug in timeouts on Windows where it would trigger too early.
- Support sub-nanosecond precision on Linux/Android.

# Version 0.1.3

- Improve error handling around event ports fcntl

# Version 0.1.2

- Add support for event ports (illumos and Solaris)

# Version 0.1.1

- Improve documentation
- Fix a bug in `Event::none()`.

# Version 0.1.0

- Initial version
