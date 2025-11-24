D-Bus crate 0.7 overview
========================

In 0.7 the `Connection` struct has been rewritten (but the old one is still there). The new struct(s) have the following advantages:

 * It makes it possible to make a connection `Send`/`Sync`, see `blocking::SyncConnection`.
 * The call to `dbus_connection_dispatch` has been removed and what it did has been rewritten in Rust. This means better performance and panic safety.
 * The two-layer design (with `Channel` as a low-level connection) makes it easier to write a custom `Connection`, should that be necessary.
 * Preparation for a first class async/non-blocking `Connection`. A `nonblock` module is in the works.
 * Some preparations for interfacing the `dbus` crate with a native backend instead of relying on `libdbus`, should someone want to write such a backend. There is a lot more to do before that becomes a reality though.

Things that have moved
======================

If you want the quickest upgrade experience, then you can just update your imports:

 * The old `Connection` is now at `ffidisp::Connection`, likewise for `ConnPath`, `BusType` and many others. Have a look at the `ffidisp` module to see if your struct has moved there.
 * The `stdintf` module is at `ffidisp::stdintf`.
 * The old `MessageItem`, should you still need it, is now under `arg::messageitem`. But do use the generic functions instead of `MessageItem` whenever possible. `MessageItem::DictEntry` has changed to `MessageItem::Dict` (which contains the entire dict, not just one entry) to make it less error prone.

Migrating / upgrading
=====================

On a long term, consider migrating / upgrading to `blocking::Connection` or `blocking::SyncConnection`. You would need to make the following adjustments:

 * Create and connect your connection easily with just `Connection::new_session()` or `Connection::new_system()`.
 * Instead of `ConnPath`, use a `Proxy`. It works approximately the same way.
 * `blocking::stdintf` can be helpful to make standard method calls, such as getting and setting properties on a remote peer.
 * `Connection::register_name` has been renamed to `request_name`. (This was just a misnaming.)
 * Instead of `incoming()` to process incoming messages, use `process()` (which takes a std `Duration`). This will not hand out any messages though, instead register callbacks using (in order from most convenient to most generic): `Proxy::match_signal`, `Proxy::match_start` or `channel::MatchingReceiver::start_receive`.
 * For `tree`s, you must now make sure the root path (`/`) is always part of your tree. 
 * For `tree`s, call `Tree::start_receive()` to attach the tree and the connection, then call `Connection::process()` to process incoming messages.

Have a look at the `client`, `server` and `match_signal` examples to get started.

ReadAll / AppendAll
===================

The `ReadAll` and `AppendAll` traits were present in later 0.6.x versions as well, but are worthy a mention because they can make code more ergonomic in many cases. They are implemented for tuples: the empty tuple `()`, the single-tuple `(TYPE,)`, and usual tuples `(TYPE1, TYPE2)` up to 11. So if you have a method call that takes `STRING arg1, INT32 arg2` and returns a `BOOLEAN`, you can call it like this:

```
let (r,): (bool,) = myProxy.method_call(interface, name, (arg1, arg2))?;
```

...where `arg1` is a `&str` and `arg2` is a `i32`. 
