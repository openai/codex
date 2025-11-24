D-Bus bindings for Rust
=======================

[![crates.io](https://img.shields.io/crates/v/dbus.svg)](https://crates.io/crates/dbus)
[![API documentation](https://docs.rs/dbus/badge.svg)](https://docs.rs/dbus)
[![license](https://img.shields.io/crates/l/dbus.svg)](https://crates.io/crates/dbus)
[![Github CI](https://github.com/diwic/dbus-rs/actions/workflows/dbus-rs-github-ci.yml/badge.svg)](https://github.com/diwic/dbus-rs/actions/workflows/dbus-rs-github-ci.yml/badge.svg)

 * Use `blocking::Connection` to connect to the session or system bus. (Or `SyncConnection` / `LocalConnection`)
 * Use `Message` to send and receive messages. Get and append arguments of all types, see the [argument guide](dbus/examples/argument_guide.md) for details.
 * Build method dispatching servers using the `dbus-crossroads` or `dbus-tree` crates.
   Standard D-Bus interfaces (introspection, properties, object manager) are supported.

Breaking changes
----------------

The main dbus crate is fairly mature and the features you need should be all there. Breaking changes can still happen, but not often.

 * In 0.9, the `dbus::tree` module moved to the `dbus-tree` crate (but consider migrating to `dbus-crossroads` instead).
 * If you're currently using 0.6.x of dbus and want to upgrade to later versions, you can read [changes in dbus-rs 0.7](dbus/changes-in-0.7.md).


Additional crates
-----------------

 * [dbus-crossroads](http://crates.io/crates/dbus-crossroads/) for easy building of method
    dispatching servers. [![API documentation](https://docs.rs/dbus-crossroads/badge.svg)](https://docs.rs/dbus-crossroads)
 * [dbus-tokio](http://crates.io/crates/dbus-tokio/) integrates D-Bus with [Tokio](http://tokio.rs). [![API documentation](https://docs.rs/dbus-tokio/badge.svg)](https://docs.rs/dbus-tokio)
 * [dbus-codegen](http://crates.io/crates/dbus-codegen/) installs a binary tool which generates Rust code from D-Bus XML introspection data. The [readme](https://github.com/diwic/dbus-rs/tree/master/dbus-codegen) contains an introduction to how to use it.
 * [libdbus-sys](http://crates.io/crates/libdbus-sys/) contains the raw FFI bindings to libdbus.
 * [dbus-tree](http://crates.io/crates/dbus-tree/) facilitates easy building of method
    dispatching servers (legacy design). [![API documentation](https://docs.rs/dbus-tree/badge.svg)](https://docs.rs/dbus-tree)

Invitation
----------

You are hereby invited to participate in the development of these crates:

 * If you have discovered what you believe is a bug, [file an issue](https://github.com/diwic/dbus-rs/issues).
 * If you have questions or comments that the documentation cannot answer in an easy way, [start a discussion](https://github.com/diwic/dbus-rs/discussions).
 * If you have smaller improvements to code, documentation, examples etc, go ahead and [submit a pull request](https://github.com/diwic/dbus-rs/pulls).
   Larger pieces of work are better off discussed first.

The code is Apache 2.0 / MIT dual licensed. Any code submitted in Pull Requests, discussions or issues is assumed to have this license,
unless explicitly stated otherwise.


Examples
========

Client
------

This example opens a connection to the session bus and asks for a list of all names currently present.

```rust
use dbus::blocking::Connection;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // First open up a connection to the session bus.
    let conn = Connection::new_session()?;

    // Second, create a wrapper struct around the connection that makes it easy
    // to send method calls to a specific destination and path.
    let proxy = conn.with_proxy("org.freedesktop.DBus", "/", Duration::from_millis(5000));

    // Now make the method call. The ListNames method call takes zero input parameters and
    // one output parameter which is an array of strings.
    // Therefore the input is a zero tuple "()", and the output is a single tuple "(names,)".
    let (names,): (Vec<String>,) = proxy.method_call("org.freedesktop.DBus", "ListNames", ())?;

    // Let's print all the names to stdout.
    for name in names { println!("{}", name); }

    Ok(())
}
```

Examples of client code in the examples directory:

 * [client.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/client.rs) (same as the above)
 * [properties.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/properties.rs)
 * [match_signal.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/match_signal.rs)
 * [rtkit.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/rtkit.rs)
 * [monitor.rs](https://github.com/diwic/dbus-rs/tree/master/dbus/examples/monitor.rs)

Server
------

This example grabs the `com.example.dbustest` bus name, adds the `/hello` path
which implements the `com.example.dbustest` interface, and specifies that this
interface has a `Hello` method.
It then listens for incoming D-Bus method calls on this path and handles them accordingly.

**dbus-crossroads**:

```rust
let c = Connection::new_session()?;
c.request_name("com.example.dbustest", false, true, false)?;
let mut cr = Crossroads::new();
let token = cr.register("com.example.dbustest", |b| {
    b.method("Hello", ("name",), ("reply",), |_, _, (name,): (String,)| {
        Ok((format!("Hello {}!", name),))
    });
});
cr.insert("/hello", &[token], ());
cr.serve(&c)?;
```

Examples of server code using `dbus-crossroads` in the examples directory:

 * [server_cr.rs](https://github.com/diwic/dbus-rs/blob/master/dbus-crossroads/examples/server_cr.rs)
 * [tokio_server_cr.rs](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio_server_cr.rs)
 * [tokio_adv_server_cr.rs](https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio_adv_server_cr.rs)

**dbus-tree**:

```rust
let c = Connection::new_session()?;
c.request_name("com.example.dbustest", false, true, false)?;
let f = Factory::new_fn::<()>();
let tree = f.tree(())
    .add(f.object_path("/hello", ()).introspectable()
        .add(f.interface("com.example.dbustest", ())
            .add_m(f.method("Hello", (), |m| {
                let n: &str = m.msg.read1()?;
                let s = format!("Hello {}!", n);
                Ok(vec!(m.msg.method_return().append1(s)))
            }).inarg::<&str,_>("name")
              .outarg::<&str,_>("reply")
        )
    ).add(f.object_path("/", ()).introspectable());
tree.start_receive(&c);
loop { c.process(Duration::from_millis(1000))?; }
```

Examples of server code using `dbus-tree` in the examples directory:

 * [server.rs](https://github.com/diwic/dbus-rs/tree/master/dbus-tree/examples/server.rs)
 * [adv_server.rs](https://github.com/diwic/dbus-rs/tree/master/dbus-tree/examples/adv_server.rs)

Features
========

The `futures` feature makes `dbus` depend on the `futures` crate. This enables the `nonblock` module (used by the `dbus-tokio` crate).

The `vendored` feature links libdbus statically into the final executable.

The `stdfd` feature uses std's `OwnedFd` instead of dbus own. (This will be the default in the next major release.)

The `no-string-validation` feature skips an extra check that a specific string (e g a `Path`, `ErrorName` etc) conforms to the D-Bus specification, which might also make things a tiny bit faster. But - if you do so, and then actually send invalid strings to the D-Bus library, you might get a panic instead of a proper error.

Requirements
============

Default
-------
[Libdbus](https://dbus.freedesktop.org/releases/dbus/) 1.6 or higher, and latest stable release of [Rust](https://www.rust-lang.org/). If you run Ubuntu (any maintained version should be okay), this means having the `libdbus-1-dev` and `pkg-config` packages installed while building, and the `libdbus-1-3` package installed while running.

Vendored
--------
If the `vendored` feature is enabled, none of the default requirements.

The `vendored` feature is the current recommended way to cross compile dbus-rs. More information and some other methods are mentioned [here](https://github.com/diwic/dbus-rs/blob/master/libdbus-sys/cross_compile.md).

Alternatives
============

[zbus](https://github.com/dbus2/zbus) and [rustbus](https://github.com/KillingSpark/rustbus) (stalled?) are D-Bus crates
written completely in Rust (i e, no bindings to C libraries).
Some more alternatives are listed [here](https://github.com/KillingSpark/rust-dbus-comparisons), but I'm not sure how usable they are.
