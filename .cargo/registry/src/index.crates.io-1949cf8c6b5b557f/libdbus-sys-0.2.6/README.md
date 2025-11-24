Raw FFI bindings to libdbus
---------------------------

Libdbus is licensed under GPL-2.0+/AFL (Academic free license), whereas the bindings are licensed under MIT/Apache-2.0.

By default, libdbus is dynamically linked, meaning that `libdbus-1.so` must be installed on the target system (which it is, by default, in all common Linux distributions).

As an option, libdbus can be built from source and included in the final executable. For this, enable the `vendored` feature. The crates.io package contains source code from libdbus; but it is only included in the build if the `vendored` feature is enabled.

The `vendored` feature is the current recommended way to cross compile dbus-rs, although some other methods are mentioned [here](https://github.com/diwic/dbus-rs/blob/master/libdbus-sys/cross_compile.md).

